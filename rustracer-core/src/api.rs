use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use failure::{err_msg, Error};
use indicatif::HumanDuration;

use {Point3f, Transform, Vector3f};
use bvh::BVH;
use camera::{Camera, PerspectiveCamera};
use display::NoopDisplayUpdater;
use filter::{BoxFilter, Filter, GaussianFilter, MitchellNetravali, TriangleFilter};
use film::Film;
use geometry::Matrix4x4;
use light::{AreaLight, DiffuseAreaLight, DistantLight, InfiniteAreaLight, Light, PointLight};
use integrator::{DirectLightingIntegrator, Normal, PathIntegrator, SamplerIntegrator, Whitted};
use material::{DisneyMaterial, GlassMaterial, Material, MatteMaterial, Metal, MirrorMaterial,
               Plastic, SubstrateMaterial, TranslucentMaterial, UberMaterial};
use paramset::{ParamSet, TextureParams};
use primitive::{GeometricPrimitive, TransformedPrimitive, Primitive};
use renderer;
use sampler::Sampler;
use sampler::zerotwosequence::ZeroTwoSequence;
use scene::Scene;
use shapes::{Cylinder, Disk, Shape, Sphere, TriangleMesh};
use shapes::plymesh;
use spectrum::Spectrum;
use stats;
use texture::{CheckerboardTexture, ConstantTexture, ImageTexture, ScaleTexture, Texture,
              UVTexture, FbmTexture};


stat_counter!("Scene/Materials created", n_materials_created);
stat_counter!("Scene/Object instances created", n_object_instances_created);
stat_counter!("Scene/Object instances used", n_object_instances_used);
pub fn init_stats() {
    n_materials_created::init();
    n_object_instances_created::init();
    n_object_instances_used::init();
}

#[derive(Debug, Copy, Clone)]
pub enum ApiState {
    Uninitialized,
    OptionsBlock,
    WorldBlock,
}

impl ApiState {
    pub fn verify_uninitialized(&self) -> Result<(), Error> {
        match self {
            ApiState::Uninitialized => Ok(()),
            _ => Err(err_msg("Api::init() has already been called!")),
        }
    }

    pub fn verify_initialized(&self) -> Result<(), Error> {
        match self {
            ApiState::Uninitialized => Err(err_msg("Api::init() has not been called!")),
            _ => Ok(()),
        }
    }

    pub fn verify_options(&self) -> Result<(), Error> {
        self.verify_initialized()?;
        match self {
            ApiState::WorldBlock => Err(err_msg("Options cannot be set inside world block.")),
            _ => Ok(()),
        }
    }

    pub fn verify_world(&self) -> Result<(), Error> {
        self.verify_initialized()?;
        match self {
            ApiState::OptionsBlock => Err(err_msg("Scene description must be inside world block.")),
            _ => Ok(()),
        }
    }
}

impl Default for ApiState {
    fn default() -> Self {
        ApiState::Uninitialized
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ParamType {
    Int,
    Bool,
    Float,
    Point2,
    Vector2,
    Point3,
    Vector3,
    Normal,
    Rgb,
    Xyz,
    Blackbody,
    Spectrum,
    String,
    Texture,
}
#[derive(Debug, PartialEq)]
pub enum Array {
    NumArray(Vec<f32>),
    StrArray(Vec<String>),
}

impl Array {
    pub fn as_num_array(&self) -> Vec<f32> {
        // TODO proper error handling
        match self {
            Array::NumArray(a) => a.clone(),
            Array::StrArray(a) => {
                warn!("Attempted to cast a string array to a num array: {:?}", a);
                vec![]
            },
        }
    }

    // TODO proper error handling
    pub fn as_str_array(&self) -> Vec<String> {
        match self {
            Array::StrArray(a) => a.clone(),
            Array::NumArray(a) => {
                warn!("Attempted to cast a num array to a string array: {:?}", a);
                vec![]
            },
        }
    }
}

#[derive(Debug)]
pub struct ParamListEntry {
    pub param_type: ParamType,
    pub param_name: String,
    pub values: Array,
}

impl ParamListEntry {
    pub fn new(t: ParamType, name: String, values: Array) -> ParamListEntry {
        ParamListEntry {
            param_type: t,
            param_name: name,
            values: values,
        }
    }
}

pub struct RenderOptions {
    _transform_start_time: f32,
    _transform_end_time: f32,
    film_name: String,
    film_params: ParamSet,
    filter_name: String,
    filter_params: ParamSet,
    sampler_name: String,
    sampler_params: ParamSet,
    accelerator_name: String,
    accelerator_params: ParamSet,
    integrator_name: String,
    integrator_params: ParamSet,
    camera_name: String,
    camera_params: ParamSet,
    camera_to_world: Transform,
    lights: Vec<Arc<Light>>,
    primitives: Vec<Arc<Primitive>>,
    instances: HashMap<String, Vec<Arc<Primitive>>>,
    current_instance: Option<String>,
}

impl RenderOptions {
    pub fn make_filter(&mut self) -> Result<Box<Filter>, Error> {
        debug!("Making filter");
        let filter = match self.filter_name.as_ref() {
            "box" => BoxFilter::create(&mut self.filter_params),
            "mitchell" => MitchellNetravali::create(&mut self.filter_params),
            "gaussian" => GaussianFilter::create(&mut self.filter_params),
            "triangle" => TriangleFilter::create(&mut self.filter_params),
            _ => bail!("Filter \"{}\" unknown.", self.filter_name),
        };

        Ok(filter)
    }

    pub fn make_film(&mut self, filter: Box<Filter>) -> Result<Box<Film>, Error> {
        debug!("Making film");
        let film = if self.film_name == "image" {
            Film::create(&mut self.film_params, filter)
        } else {
            bail!("Film \"{}\" unknown.", self.film_name);
        };

        Ok(film)
    }

    pub fn make_sampler(&mut self) -> Result<Box<Sampler>, Error> {
        let sampler = if self.sampler_name == "lowdiscrepancy" ||
                         self.sampler_name == "02sequence" {
            ZeroTwoSequence::create(&mut self.sampler_params)
        } else {
            bail!("Sampler \"{}\" unknown.", self.sampler_name);
        };

        Ok(sampler)
    }

    pub fn make_camera(&mut self) -> Result<Box<Camera>, Error> {
        debug!("Making camera");
        let filter = self.make_filter()?;
        let film = self.make_film(filter)?;

        let camera = if self.camera_name == "perspective" {
            PerspectiveCamera::create(&mut self.camera_params, &self.camera_to_world, film)
        } else {
            bail!("Camera \"{}\" unknown.", self.camera_name);
        };

        Ok(camera)
    }

    pub fn make_integrator(&mut self,
                           camera: &Camera)
                           -> Result<Box<SamplerIntegrator>, Error> {
        debug!("Making integrator");
        let integrator: Box<SamplerIntegrator> =
            if self.integrator_name == "whitted" {
                Whitted::create(&mut self.integrator_params)
                // Box::new(Normal {})
            } else if self.integrator_name == "directlighting" {
                DirectLightingIntegrator::create(&mut self.integrator_params)
            } else if self.integrator_name == "path" {
                PathIntegrator::create(&mut self.integrator_params, camera)
            } else if self.integrator_name == "normal" {
                Box::new(Normal::default())
            } else {
                bail!("Integrator \"{}\" unknown.", self.integrator_name);
            };

        Ok(integrator)
    }

    pub fn make_scene(&mut self) -> Result<Arc<Scene>, Error> {
        info!("Making scene with {} primitives and {} lights",
              self.primitives.len(),
              self.lights.len());
        let accelerator = make_accelerator(&self.accelerator_name,
                                           &self.primitives,
                                           &mut self.accelerator_params);
        Ok(Arc::new(Scene::new(accelerator, self.lights.clone())))
    }
}

pub fn make_accelerator(accelerator_name: &str,
                        prims: &[Arc<Primitive>],
                        accelerator_params: &mut ParamSet)
                        -> Arc<Primitive> {
    if accelerator_name == "kdtree" {
        unimplemented!()
    } else if accelerator_name == "bvh" {
        Arc::new(BVH::create(prims, accelerator_params))
    } else {
        warn!("Accelerator \"{}\" unknown.", accelerator_name);
        Arc::new(BVH::create(prims, accelerator_params))
    }
}

impl Default for RenderOptions {
    fn default() -> Self {
        RenderOptions {
            _transform_start_time: 0.0,
            _transform_end_time: 1.0,
            film_name: "image".to_owned(),
            film_params: ParamSet::default(),
            filter_name: "box".to_owned(),
            filter_params: ParamSet::default(),
            sampler_name: "halton".to_owned(),
            sampler_params: ParamSet::default(),
            accelerator_name: "bvh".to_owned(),
            accelerator_params: ParamSet::default(),
            integrator_name: "path".to_owned(),
            integrator_params: ParamSet::default(),
            camera_name: "perspective".to_owned(),
            camera_params: ParamSet::default(),
            camera_to_world: Transform::default(),
            lights: Vec::new(),
            primitives: Vec::new(),
            instances: HashMap::new(),
            current_instance: None,
        }
    }
}

#[derive(Clone)]
pub struct GraphicsState {
    float_textures: HashMap<String, Arc<Texture<f32>>>,
    spectrum_textures: HashMap<String, Arc<Texture<Spectrum>>>,
    material_param: ParamSet,
    material: String,
    named_material: HashMap<String, Arc<Material>>,
    current_named_material: String,
    area_light_params: ParamSet,
    area_light: String,
    reverse_orientation: bool,
}

impl GraphicsState {
    pub fn create_material(&mut self, params: &mut ParamSet) -> Arc<Material> {
        let mut mp = TextureParams::new(params,
                                        &mut self.material_param,
                                        &self.float_textures,
                                        &self.spectrum_textures);
        if !self.current_named_material.is_empty() {
            let cur_mat_name = &self.current_named_material;
            self.named_material
                .get(cur_mat_name)
                .cloned()
                .unwrap_or_else(|| {
                                    warn!("No material named \"{}\". Using matte material instead.",
                                          cur_mat_name);
                                    make_material("matte", &mut mp)
                                })
        } else {
            make_material(&self.material, &mut mp)
        }
    }
}

impl Default for GraphicsState {
    fn default() -> Self {
        GraphicsState {
            float_textures: HashMap::new(),
            spectrum_textures: HashMap::new(),
            material_param: ParamSet::default(),
            material: "matte".to_owned(),
            named_material: HashMap::new(),
            current_named_material: String::new(),
            area_light_params: ParamSet::default(),
            area_light: String::new(),
            reverse_orientation: false,
        }
    }
}

#[derive(Default)]
pub struct State {
    api_state: ApiState,
    render_options: RenderOptions,
    cur_transform: Transform,
    named_coordinate_systems: HashMap<String, Transform>,
    pushed_transforms: Vec<Transform>,
    graphics_state: GraphicsState,
    pushed_graphics_states: Vec<GraphicsState>,
}

impl State {
    pub fn save_graphics_state(&mut self) {
        let gs = self.graphics_state.clone();
        self.pushed_graphics_states.push(gs);
    }

    pub fn save_transform(&mut self) {
        let t = self.cur_transform.clone();
        self.pushed_transforms.push(t);
    }

    pub fn restore_graphics_state(&mut self) {
        self.graphics_state = self.pushed_graphics_states.pop().unwrap();
    }

    pub fn restore_transform(&mut self) {
        self.cur_transform = self.pushed_transforms.pop().unwrap();
    }
}

pub trait Api {
    fn init(&self) -> Result<(), Error>;
    // TODO cleanup
    fn identity(&self) -> Result<(), Error>;
    fn translate(&self, dx: f32, dy: f32, dz: f32) -> Result<(), Error>;
    fn rotate(&self, angle: f32, dx: f32, dy: f32, dz: f32) -> Result<(), Error>;
    fn scale(&self, sx: f32, sy: f32, sz: f32) -> Result<(), Error>;
    fn look_at(&self,
               ex: f32,
               ey: f32,
               ez: f32,
               lx: f32,
               ly: f32,
               lz: f32,
               ux: f32,
               uy: f32,
               uz: f32)
               -> Result<(), Error>;
    fn concat_transform(&self,
                 tr00: f32,
                 tr01: f32,
                 tr02: f32,
                 tr03: f32,
                 tr04: f32,
                 tr05: f32,
                 tr06: f32,
                 tr07: f32,
                 tr08: f32,
                 tr09: f32,
                 tr10: f32,
                 tr11: f32,
                 tr12: f32,
                 tr13: f32,
                 tr14: f32,
                 tr15: f32)
                 -> Result<(), Error>;
    fn transform(&self,
                 tr00: f32,
                 tr01: f32,
                 tr02: f32,
                 tr03: f32,
                 tr04: f32,
                 tr05: f32,
                 tr06: f32,
                 tr07: f32,
                 tr08: f32,
                 tr09: f32,
                 tr10: f32,
                 tr11: f32,
                 tr12: f32,
                 tr13: f32,
                 tr14: f32,
                 tr15: f32)
                 -> Result<(), Error>;
    fn coordinate_system(&self, name: String) -> Result<(), Error>;
    fn coord_sys_transform(&self, name: String) -> Result<(), Error>;
    // TODO active_transform_all
    // TODO active_transform_end_time
    // TODO active_transform_start_time
    // TODO transform_times
    fn pixel_filter(&self, name: String, params: &mut ParamSet) -> Result<(), Error>;
    fn film(&self, name: String, params: &mut ParamSet) -> Result<(), Error>;
    fn sampler(&self, name: String, params: &mut ParamSet) -> Result<(), Error>;
    fn accelerator(&self, name: String, params: &mut ParamSet) -> Result<(), Error>;
    fn integrator(&self, name: String, params: &mut ParamSet) -> Result<(), Error>;
    fn camera(&self, name: String, params: &mut ParamSet) -> Result<(), Error>;
    // TODO make_named_medium
    // TODO medium_interface
    fn world_begin(&self) -> Result<(), Error>;
    fn attribute_begin(&self) -> Result<(), Error>;
    fn attribute_end(&self) -> Result<(), Error>;
    fn transform_begin(&self) -> Result<(), Error>;
    fn transform_end(&self) -> Result<(), Error>;
    fn texture(&self,
               name: String,
               typ: String,
               texname: String,
               params: &mut ParamSet)
               -> Result<(), Error>;
    fn material(&self, name: String, params: &mut ParamSet) -> Result<(), Error>;
    fn make_named_material(&self, name: String, params: &mut ParamSet) -> Result<(), Error>;
    fn named_material(&self, name: String) -> Result<(), Error>;
    fn lightsource(&self, name: String, params: &mut ParamSet) -> Result<(), Error>;
    fn arealightsource(&self, name: String, params: &mut ParamSet) -> Result<(), Error>;
    fn shape(&self, name: String, params: &mut ParamSet) -> Result<(), Error>;
    fn reverse_orientation(&self) -> Result<(), Error>;
    fn object_begin(&self, name: String) -> Result<(), Error>;
    fn object_end(&self) -> Result<(), Error>;
    fn object_instance(&self, name: String) -> Result<(), Error>;
    fn world_end(&self) -> Result<(), Error>;
}

#[derive(Default)]
pub struct RealApi {
    state: RefCell<State>,
}

impl RealApi {
    fn make_light(&self,
                  name: &str,
                  param_set: &mut ParamSet,
                  light_2_world: &Transform)
                  -> Result<Arc<Light>, Error> {
        if name == "point" {
            let light = PointLight::create(light_2_world, param_set);
            Ok(light)
        } else if name == "distant" {
            let light = DistantLight::create(light_2_world, param_set);
            Ok(light)
        } else if name == "infinite" {
            let light = InfiniteAreaLight::create(light_2_world, param_set);
            Ok(light)
        } else {
            warn!("Light {} unknown", name);
            Err(err_msg("Unsupported light type"))
        }
    }
}

impl Api for RealApi {
    fn init(&self) -> Result<(), Error> {
        debug!("API initialized!");
        let mut state = self.state.borrow_mut();
        state.api_state.verify_uninitialized()?;

        state.api_state = ApiState::OptionsBlock;
        Ok(())
    }

    fn identity(&self) -> Result<(), Error> {
        debug!("Identity called");
        let mut state = self.state.borrow_mut();
        state.api_state.verify_initialized()?;
        state.cur_transform = Transform::default();
        Ok(())
    }

    fn translate(&self, dx: f32, dy: f32, dz: f32) -> Result<(), Error> {
        debug!("Translate called with {} {} {}", dx, dy, dz);
        let mut state = self.state.borrow_mut();
        state.api_state.verify_initialized()?;
        let t = Transform::translate(&Vector3f::new(dx, dy, dz));
        state.cur_transform = &state.cur_transform * &t;
        Ok(())
    }

    fn rotate(&self, angle: f32, dx: f32, dy: f32, dz: f32) -> Result<(), Error> {
        debug!("Rotate called with {} {} {} {}", angle, dx, dy, dz);
        let mut state = self.state.borrow_mut();
        state.api_state.verify_initialized()?;
        let t = Transform::rotate(angle, Vector3f::new(dx, dy, dz));
        state.cur_transform = &state.cur_transform * &t;
        Ok(())
    }

    fn scale(&self, sx: f32, sy: f32, sz: f32) -> Result<(), Error> {
        debug!("Scale called with {} {} {}", sx, sy, sz);
        let mut state = self.state.borrow_mut();
        state.api_state.verify_initialized()?;
        let t = Transform::scale(sx, sy, sz);
        state.cur_transform = &state.cur_transform * &t;
        Ok(())
    }

    fn concat_transform(&self,
                 tr00: f32,
                 tr01: f32,
                 tr02: f32,
                 tr03: f32,
                 tr04: f32,
                 tr05: f32,
                 tr06: f32,
                 tr07: f32,
                 tr08: f32,
                 tr09: f32,
                 tr10: f32,
                 tr11: f32,
                 tr12: f32,
                 tr13: f32,
                 tr14: f32,
                 tr15: f32)
                 -> Result<(), Error> {
        let mut state = self.state.borrow_mut();
        state.api_state.verify_initialized()?;
        let mat = Matrix4x4::from_elements(tr00,
                                           tr04,
                                           tr08,
                                           tr12,
                                           tr01,
                                           tr05,
                                           tr09,
                                           tr13,
                                           tr02,
                                           tr06,
                                           tr10,
                                           tr14,
                                           tr03,
                                           tr07,
                                           tr11,
                                           tr15);
        state.cur_transform = &state.cur_transform * &Transform {
            m: mat,
            m_inv: mat.inverse(),
        };
        Ok(())
    }

    fn transform(&self,
                 tr00: f32,
                 tr01: f32,
                 tr02: f32,
                 tr03: f32,
                 tr04: f32,
                 tr05: f32,
                 tr06: f32,
                 tr07: f32,
                 tr08: f32,
                 tr09: f32,
                 tr10: f32,
                 tr11: f32,
                 tr12: f32,
                 tr13: f32,
                 tr14: f32,
                 tr15: f32)
                 -> Result<(), Error> {
        let mut state = self.state.borrow_mut();
        state.api_state.verify_initialized()?;
        let mat = Matrix4x4::from_elements(tr00,
                                           tr04,
                                           tr08,
                                           tr12,
                                           tr01,
                                           tr05,
                                           tr09,
                                           tr13,
                                           tr02,
                                           tr06,
                                           tr10,
                                           tr14,
                                           tr03,
                                           tr07,
                                           tr11,
                                           tr15);
        state.cur_transform = Transform {
            m: mat,
            m_inv: mat.inverse(),
        };
        Ok(())
    }

    fn look_at(&self,
               ex: f32,
               ey: f32,
               ez: f32,
               lx: f32,
               ly: f32,
               lz: f32,
               ux: f32,
               uy: f32,
               uz: f32)
               -> Result<(), Error> {
        debug!("look_at called");
        let mut state = self.state.borrow_mut();
        state.api_state.verify_initialized()?;
        let look_at = Transform::look_at(&Point3f::new(ex, ey, ez),
                                         &Point3f::new(lx, ly, lz),
                                         &Vector3f::new(ux, uy, uz));
        state.cur_transform = &state.cur_transform * &look_at;
        Ok(())
    }

    fn coordinate_system(&self, name: String) -> Result<(), Error> {
        debug!("coordinate_system called");
        let mut state = self.state.borrow_mut();
        state.api_state.verify_initialized()?;
        state.named_coordinate_systems.insert(name, state.cur_transform.clone());

        Ok(())
    }

    fn coord_sys_transform(&self, name: String) -> Result<(), Error> {
        debug!("coord_sys_transform called");
        let mut state = self.state.borrow_mut();
        state.api_state.verify_initialized()?;

        if let Some(t) = state.named_coordinate_systems.get(&name).cloned() {
            state.cur_transform = t;
        } else {
            warn!("Couldn't find named coordinate system \"{}\"", name);
        }

        Ok(())
    }

    fn pixel_filter(&self, name: String, params: &mut ParamSet) -> Result<(), Error> {
        let mut state = self.state.borrow_mut();
        state.api_state.verify_options()?;
        debug!("pixel_filter called");
        state.render_options.filter_name = name;
        state.render_options.filter_params = params.clone();
        Ok(())
    }

    fn film(&self, name: String, params: &mut ParamSet) -> Result<(), Error> {
        debug!("Film called with {}", name);
        let mut state = self.state.borrow_mut();
        state.api_state.verify_options()?;
        state.render_options.film_name = name;
        state.render_options.film_params = params.clone();
        Ok(())
    }

    fn sampler(&self, name: String, params: &mut ParamSet) -> Result<(), Error> {
        let mut state = self.state.borrow_mut();
        state.api_state.verify_options()?;
        debug!("sampler called");
        state.render_options.sampler_name = name;
        state.render_options.sampler_params = params.clone();
        Ok(())
    }

    fn accelerator(&self, name: String, params: &mut ParamSet) -> Result<(), Error> {
        let mut state = self.state.borrow_mut();
        state.api_state.verify_options()?;
        debug!("accelerator called");
        state.render_options.accelerator_name = name;
        state.render_options.accelerator_params = params.clone();
        Ok(())
    }

    fn integrator(&self, name: String, params: &mut ParamSet) -> Result<(), Error> {
        debug!("Integrator called with {}", name);
        let mut state = self.state.borrow_mut();
        state.api_state.verify_options()?;
        state.render_options.integrator_name = name;
        state.render_options.integrator_params = params.clone();
        Ok(())
    }

    fn camera(&self, name: String, params: &mut ParamSet) -> Result<(), Error> {
        let mut state = self.state.borrow_mut();
        state.api_state.verify_options()?;
        debug!("Camera called with {}", name);
        state.render_options.camera_name = name;
        state.render_options.camera_params = params.clone();
        state.render_options.camera_to_world = state.cur_transform.inverse();
        let c2w = state.render_options.camera_to_world.clone();
        state
            .named_coordinate_systems
            .insert("camera".into(), c2w);
        Ok(())
    }

    fn world_begin(&self) -> Result<(), Error> {
        debug!("world_begin called");
        let mut state = self.state.borrow_mut();
        state.api_state.verify_options()?;
        state.api_state = ApiState::WorldBlock;
        let cur_transform = state.cur_transform.clone();
        state
            .named_coordinate_systems
            .insert("world".into(), cur_transform);
        state.cur_transform = Transform::default();
        Ok(())
    }

    fn attribute_begin(&self) -> Result<(), Error> {
        debug!("attribute_begin called");
        let mut state = self.state.borrow_mut();
        state.api_state.verify_world()?;
        state.save_graphics_state();
        state.save_transform();

        Ok(())
    }

    fn attribute_end(&self) -> Result<(), Error> {
        debug!("attribute_end called");
        let mut state = self.state.borrow_mut();
        state.api_state.verify_world()?;
        if state.pushed_graphics_states.is_empty() {
            error!("Unmatched AttributeEnd encountered. Ignoring it.");
            return Ok(());
        }
        state.restore_graphics_state();
        state.restore_transform();

        Ok(())
    }

    fn transform_begin(&self) -> Result<(), Error> {
        debug!("transform_begin called");
        let mut state = self.state.borrow_mut();
        state.api_state.verify_world()?;
        state.save_transform();

        Ok(())
    }

    fn transform_end(&self) -> Result<(), Error> {
        debug!("transform_end called");
        let mut state = self.state.borrow_mut();
        state.api_state.verify_world()?;
        if state.pushed_transforms.is_empty() {
            error!("Unmatched TransformEnd encountered. Ignoring it.");
            return Ok(());
        }
        state.restore_transform();

        Ok(())
    }

    fn texture(&self,
               name: String,
               typ: String,
               texname: String,
               params: &mut ParamSet)
               -> Result<(), Error> {
        debug!("texture() called with {} and {} and {}", name, typ, texname);
        let mut state = self.state.borrow_mut();
        state.api_state.verify_world()?;
        let mut empty_params = ParamSet::default();

        if typ == "float" {
            let ft = {
                let mut tp = TextureParams::new(params,
                                                &mut empty_params, // was `params`
                                                &state.graphics_state.float_textures,
                                                &state.graphics_state.spectrum_textures);
                make_float_texture(&texname, &state.cur_transform, &mut tp)
            };
            if let Ok(ft) = ft {
                if state
                       .graphics_state
                       .float_textures
                       .insert(name.clone(), ft)
                       .is_some() {
                    warn!("Texture \"{}\" being redefined.", name);
                }
            }
        } else if typ == "color" || typ == "spectrum" {
            let ft = {
                let mut tp = TextureParams::new(params,
                                                &mut empty_params, // was `params`
                                                &state.graphics_state.float_textures,
                                                &state.graphics_state.spectrum_textures);
                make_spectrum_texture(&texname, &state.cur_transform, &mut tp)
            };
            match ft {
                Ok(ft) => {
                    if state
                           .graphics_state
                           .spectrum_textures
                           .insert(name.clone(), ft)
                           .is_some() {
                        warn!("Texture \"{}\" being redefined.", name);
                    }
                }
                Err(e) => {
                    error!("Failed to create texture {}: {}", name, e);
                }
            }
        } else {
            error!("Texture type \"{}\" unknown.", typ);
        }

        Ok(())
    }

    fn make_named_material(&self, name: String, params: &mut ParamSet) -> Result<(), Error> {
        debug!("MakeNamedMaterial called with {}", name);
        let mut state = self.state.borrow_mut();

        let mtl = {
            let mut empty_params = ParamSet::default();
            let mut mp = TextureParams::new(params,
                                            &mut empty_params,
                                            &state.graphics_state.float_textures,
                                            &state.graphics_state.spectrum_textures);

            let mat_name = mp.find_string("type", "");
            if mat_name == "" {
                bail!("No parameter string \"type\" found in named_material");
            }
            make_material(&mat_name, &mut mp)
        };
        if state
               .graphics_state
               .named_material
               .insert(name.clone(), mtl)
               .is_some() {
            warn!("Named material {} redefined", name);
        }
        Ok(())
    }

    fn material(&self, name: String, params: &mut ParamSet) -> Result<(), Error> {
        debug!("Material called with {}", name);
        let mut state = self.state.borrow_mut();
        state.graphics_state.material = name;
        state.graphics_state.material_param = params.clone();
        state.graphics_state.current_named_material = String::new();
        Ok(())
    }

    fn named_material(&self, name: String) -> Result<(), Error> {
        debug!("NamedMaterial called with {}", name);
        let mut state = self.state.borrow_mut();
        state.api_state.verify_world()?;
        state.graphics_state.current_named_material = name;
        Ok(())
    }

    fn lightsource(&self, name: String, params: &mut ParamSet) -> Result<(), Error> {
        debug!("Lightsource called with {}", name);
        let mut state = self.state.borrow_mut();
        state.api_state.verify_world()?;
        let lt = self.make_light(&name, params, &state.cur_transform)?;
        state.render_options.lights.push(lt);
        Ok(())
    }

    fn arealightsource(&self, name: String, params: &mut ParamSet) -> Result<(), Error> {
        debug!("Arealightsource called with {}", name);
        let mut state = self.state.borrow_mut();
        state.graphics_state.area_light = name;
        state.graphics_state.area_light_params = params.clone();
        Ok(())
    }

    fn shape(&self, name: String, params: &mut ParamSet) -> Result<(), Error> {
        debug!("Shape called with {}", name);
        let mut state = self.state.borrow_mut();
        state.api_state.verify_world()?;

        let mut prims: Vec<Arc<Primitive>> = Vec::new();
        let mut area_lights: Vec<Arc<Light>> = Vec::new();
        let shapes = make_shapes(&name,
                                 &state.cur_transform,
                                 &state.cur_transform.inverse(),
                                 state.graphics_state.reverse_orientation,
                                 params,
                                 &state.graphics_state);
        let mat = if !shapes.is_empty() {
            Some(state.graphics_state.create_material(params))
        } else {
            None
        };
        for s in shapes {
            let area = if state.graphics_state.area_light != "" {
                let mut ps = state.graphics_state.area_light_params.clone();
                let (area_light, light) = make_area_light(&state.graphics_state.area_light,
                                                          &state.cur_transform,
                                                          &mut ps,
                                                          Arc::clone(&s))?;
                area_lights.push(light);
                Some(area_light)
            } else {
                None
            };
            let prim: Arc<Primitive> = Arc::new(GeometricPrimitive {
                                                                  shape: s,
                                                                  area_light: area,
                                                                  material: mat.clone(),
                                                              });
            prims.push(prim);
        }
        if let Some(name) = &state.render_options.current_instance {
            let mut inst = state.render_options.instances
                .get_mut(name)
                .ok_or(format_err!("Unable to find instance named {}", name))?;
            inst.append(&mut prims);
        } else {
            state.render_options.primitives.append(&mut prims);
            state.render_options.lights.append(&mut area_lights);
        }
        Ok(())
    }

    fn reverse_orientation(&self) -> Result<(), Error> {
        debug!("ReverseOrientation called");
        let mut state = self.state.borrow_mut();
        state.api_state.verify_world()?;
        state.graphics_state.reverse_orientation = !state.graphics_state.reverse_orientation;

        Ok(())
    }

    fn world_end(&self) -> Result<(), Error> {
        debug!("world_end called");
        let mut state = self.state.borrow_mut();
        state.api_state.verify_world()?;

        while !state.pushed_graphics_states.is_empty() {
            warn!("Missing AttributeEnd");
            let _ = state.pushed_graphics_states.pop();
            let _ = state.pushed_transforms.pop();
        }
        while !state.pushed_transforms.is_empty() {
            warn!("Missing TransformEnd!");
            let _ = state.pushed_transforms.pop();
        }

        let camera = state.render_options.make_camera()?;
        let mut integrator = state.render_options.make_integrator(&*camera)?;
        let mut sampler = state.render_options.make_sampler()?;
        let scene = state.render_options.make_scene()?;

        // TODO finish
        let start_time = ::std::time::Instant::now();
        renderer::render(scene,
                         &mut *integrator,
                         &*camera,
                         8,
                         &mut sampler,
                         16,
                         Box::new(NoopDisplayUpdater {}))?;
        stats::report_stats();
        let duration = start_time.elapsed();
        println!("Render time: {}", HumanDuration(duration));
        stats::print_stats();

        Ok(())
    }

    fn object_begin(&self, name: String) -> Result<(), Error> {
        debug!("object_begin called");
        self.attribute_begin()?;
        // Make sure we mutably borrow _state_ *after* we call attribute_begin(), as it
        // needs to borrow _state_ mutably as well...
        let mut state = self.state.borrow_mut();
        state.api_state.verify_world()?;

        if state.render_options.current_instance.is_some() {
            return Err(err_msg("ObjectBegin called inside of instance definition"));
        }
        state.render_options.current_instance = Some(name.to_owned());
        state.render_options.instances.insert(name, Vec::new());

        Ok(())
    }

    fn object_end(&self) -> Result<(), Error> {
        debug!("object_end called");
        {
            let mut state = self.state.borrow_mut();
            state.api_state.verify_world()?;

            if state.render_options.current_instance.is_none() {
                return Err(err_msg("ObjectEnd called outside of instance definition "));
            }
            state.render_options.current_instance = None;
        }
        self.attribute_end()?;
        n_object_instances_created::inc();

        Ok(())
    }

    fn object_instance(&self, name: String) -> Result<(), Error> {
        debug!("object_instance called");
        let mut state = self.state.borrow_mut();
        state.api_state.verify_world()?;

        if state.render_options.current_instance.is_some() {
            return Err(err_msg("ObjectInstance called inside of instance definition"));
        }
        let inst = state
            .render_options
            .instances
            .get_mut(&name)
            .ok_or(format_err!("Unable to find instance named {}", name))?;
        if inst.is_empty() {
            return Ok(());
        }
        n_object_instances_used::inc();

        if inst.len() > 1 {
            // Create aggregate for instance primitives
            let accel = make_accelerator(&state.render_options.accelerator_name,
                                         &inst,
                                         &mut state.render_options.accelerator_params);
            inst.clear();
            inst.push(accel);
        }
        let prim = Arc::new(TransformedPrimitive {
            primitive: inst.get(0).unwrap().clone(),
            primitive_to_world: state.cur_transform.clone(),
        });
        state.render_options.primitives.push(prim);

        Ok(())
    }
}

fn make_shapes(name: &str,
               object2world: &Transform,
               world2object: &Transform,
               reverse_orientation: bool,
               ps: &mut ParamSet,
               graphics_state: &GraphicsState)
               -> Vec<Arc<Shape>> {
    let mut shapes: Vec<Arc<Shape>> = Vec::new();
    if name == "sphere" {
        shapes.push(Sphere::create(object2world, reverse_orientation, ps));
    } else if name == "cylinder" {
        shapes.push(Cylinder::create(object2world, reverse_orientation, ps));
    } else if name == "disk" {
        shapes.push(Disk::create(object2world, reverse_orientation, ps));
    } else if name == "cone" {
        unimplemented!();
    } else if name == "paraboloid" {
        unimplemented!();
    } else if name == "hyperboloid" {
        unimplemented!();
    } else if name == "curve" {
        unimplemented!();
    } else if name == "trianglemesh" {
        let mut tris = TriangleMesh::create(object2world,
                                            world2object,
                                            reverse_orientation,
                                            ps,
                                            &graphics_state.float_textures);
        shapes.append(&mut tris);
    } else if name == "plymesh" {
        let mut tris = plymesh::create(object2world,
                                       world2object,
                                       reverse_orientation,
                                       ps,
                                       &graphics_state.float_textures);
        shapes.append(&mut tris);
    } else {
        warn!("Unknown shape {}", name);
    }

    shapes
}

fn make_material(name: &str, mp: &mut TextureParams) -> Arc<Material> {
    n_materials_created::inc();
    if name == "matte" {
        MatteMaterial::create(mp)
    } else if name == "plastic" {
        Plastic::create(mp)
    } else if name == "glass" {
        GlassMaterial::create(mp)
    } else if name == "mirror" {
        MirrorMaterial::create(mp)
    } else if name == "metal" {
        Metal::create(mp)
    } else if name == "substrate" {
        SubstrateMaterial::create(mp)
    } else if name == "translucent" {
        TranslucentMaterial::create(mp)
    } else if name == "uber" {
        UberMaterial::create(mp)
    } else if name == "disney" {
        DisneyMaterial::create(mp)
    } else {
        warn!("Unknown material {}. Using matte.", name);
        MatteMaterial::create(mp)
    }
}

fn make_area_light(name: &str,
                   light2world: &Transform,
                   params: &mut ParamSet,
                   shape: Arc<Shape>)
                   -> Result<(Arc<AreaLight>, Arc<Light>), Error> {
    if name == "area" || name == "diffuse" {
        let l = DiffuseAreaLight::create(light2world, params, shape);
        let light: Arc<Light> = l.clone();
        let area_light: Arc<AreaLight> = l.clone();
        Ok((area_light, light))
    } else {
        Err(format_err!("Area light {} unknown", name))
    }
}

fn make_float_texture(name: &str,
                      transform: &Transform,
                      tp: &mut TextureParams)
                      -> Result<Arc<Texture<f32>>, Error> {
    let tex: Arc<Texture<f32>> = if name == "constant" {
        Arc::new(ConstantTexture::create_float(transform, tp))
    } else if name == "scale" {
        Arc::new(ScaleTexture::<f32>::create(tp))
    } else if name == "imagemap" {
        Arc::new(ImageTexture::<f32>::create(transform, tp))
    } else if name == "fbm" {
        Arc::new(FbmTexture::create_float(transform, tp))
    } else {
        bail!("Unkown texture type {}", name);
    };

    Ok(tex)
}

fn make_spectrum_texture(name: &str,
                         transform: &Transform,
                         tp: &mut TextureParams)
                         -> Result<Arc<Texture<Spectrum>>, Error> {
    let tex: Arc<Texture<Spectrum>> = if name == "constant" {
        Arc::new(ConstantTexture::create_spectrum(transform, tp))
    } else if name == "scale" {
        Arc::new(ScaleTexture::<Spectrum>::create(tp))
    } else if name == "mix" {
        unimplemented!()
    } else if name == "bilerp" {
        unimplemented!()
    } else if name == "imagemap" {
        Arc::new(ImageTexture::<Spectrum>::create(transform, tp))
    } else if name == "uv" {
        Arc::new(UVTexture::create_spectrum(transform, tp))
    } else if name == "checkerboard" {
        Arc::new(CheckerboardTexture::create_spectrum(transform, tp))
    } else if name == "dots" {
        unimplemented!()
    } else if name == "fbm" {
        Arc::new(FbmTexture::create_spectrum(transform, tp))
    } else if name == "wrinkled" {
        unimplemented!()
    } else if name == "marble" {
        unimplemented!()
    } else if name == "windy" {
        unimplemented!()
    } else if name == "ptex" {
        unimplemented!()
    } else {
        bail!("Unkown texture type {}", name);
    };

    Ok(tex)
}
