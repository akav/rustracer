#![macro_use]

use std::fmt::Debug;
use std::collections::HashMap;
use std::sync::Arc;

use {Point2f, Point3f, Vector3f};
use api::{ParamListEntry, ParamType};
use spectrum::Spectrum;
use texture::Texture;
use texture::ConstantTexture;

macro_rules! find_one(
    ($x:ident, $y:ident, $t:ty) => (
        pub fn $x(&mut self, name: &str, d: $t) -> $t {
            let mut res = self.$y.iter_mut().find(|ref mut e| e.name == name);

            if let Some(e) = res.as_mut() {
                e.looked_up = true;
            }

            res.map(|e| e.values[0].clone()).unwrap_or(d)
        }
    );
);

macro_rules! find(
    ($x:ident, $y:ident, $t:ty) => (
        pub fn $x(&mut self, name: &str) -> Option<Vec<$t>> {
            let mut res = self.$y.iter_mut().find(|ref mut e| e.name == name);

            if let Some(e) = res.as_mut() {
                e.looked_up = true;
            }

            res.map(|e| e.values.clone())
        }
    );
);


#[derive(Default, Debug, Clone)]
pub struct ParamSet {
    bools: Vec<ParamSetItem<bool>>,
    ints: Vec<ParamSetItem<i32>>,
    floats: Vec<ParamSetItem<f32>>,
    strings: Vec<ParamSetItem<String>>,
    spectra: Vec<ParamSetItem<Spectrum>>,
    point2fs: Vec<ParamSetItem<Point2f>>,
    point3fs: Vec<ParamSetItem<Point3f>>,
    vector3fs: Vec<ParamSetItem<Vector3f>>,
    textures: Vec<ParamSetItem<String>>,
}

impl ParamSet {
    pub fn init(&mut self, entries: Vec<ParamListEntry>) {
        for entry in entries {
            match entry.param_type {
                ParamType::Bool => {
                    let bools = entry
                        .values
                        .as_str_array()
                        .iter()
                        .map(|x| if x == "true" { true } else { false })
                        .collect();
                    self.add_bool(entry.param_name.clone(), bools);
                }
                ParamType::Int => {
                    let ints = entry
                        .values
                        .as_num_array()
                        .iter()
                        .map(|x| *x as i32)
                        .collect::<Vec<_>>();
                    self.add_int(entry.param_name.clone(), ints);
                }
                ParamType::Float => {
                    self.add_float(entry.param_name.clone(), entry.values.as_num_array())
                }
                ParamType::String => {
                    self.add_string(entry.param_name.clone(), entry.values.as_str_array())
                }
                ParamType::Rgb => {
                    let spectra = entry
                        .values
                        .as_num_array()
                        .chunks(3)
                        .filter(|s| s.len() == 3)
                        .map(|s| Spectrum::rgb(s[0], s[1], s[2]))
                        .collect();
                    self.add_rgb_spectrum(entry.param_name.clone(), spectra);
                }
                ParamType::Point2 => {
                    let points = entry
                        .values
                        .as_num_array()
                        .chunks(2)
                        .filter(|s| s.len() == 2)
                        .map(|s| Point2f::new(s[0], s[1]))
                        .collect();
                    self.add_point2f(entry.param_name.clone(), points);
                }
                ParamType::Point3 => {
                    let points = entry
                        .values
                        .as_num_array()
                        .chunks(3)
                        .filter(|s| s.len() == 3)
                        .map(|s| Point3f::new(s[0], s[1], s[2]))
                        .collect();
                    self.add_point3f(entry.param_name.clone(), points);
                }
                ParamType::Vector3 => {
                    let vectors = entry
                        .values
                        .as_num_array()
                        .chunks(3)
                        .filter(|s| s.len() == 3)
                        .map(|s| Vector3f::new(s[0], s[1], s[2]))
                        .collect();
                    self.add_vector3f(entry.param_name.clone(), vectors);
                }
                _ => error!(
                    "Parameter type {:?} is not implemented yet!",
                    entry.param_type
                ),
            }
        }
    }

    fn add_bool(&mut self, name: String, values: Vec<bool>) {
        self.bools.push(ParamSetItem {
            name: name,
            values: values,
            looked_up: false,
        });
    }

    fn add_int(&mut self, name: String, values: Vec<i32>) {
        self.ints.push(ParamSetItem {
            name: name,
            values: values,
            looked_up: false,
        });
    }

    fn add_float(&mut self, name: String, values: Vec<f32>) {
        self.floats.push(ParamSetItem {
            name: name,
            values: values,
            looked_up: false,
        });
    }

    fn add_string(&mut self, name: String, values: Vec<String>) {
        self.strings.push(ParamSetItem {
            name: name,
            values: values,
            looked_up: false,
        });
    }

    fn add_rgb_spectrum(&mut self, name: String, values: Vec<Spectrum>) {
        self.spectra.push(ParamSetItem {
            name: name,
            values: values,
            looked_up: false,
        });
    }

    fn add_point2f(&mut self, name: String, values: Vec<Point2f>) {
        self.point2fs.push(ParamSetItem {
            name: name,
            values: values,
            looked_up: false,
        });
    }

    fn add_point3f(&mut self, name: String, values: Vec<Point3f>) {
        self.point3fs.push(ParamSetItem {
            name: name,
            values: values,
            looked_up: false,
        });
    }

    fn add_vector3f(&mut self, name: String, values: Vec<Vector3f>) {
        self.vector3fs.push(ParamSetItem {
            name: name,
            values: values,
            looked_up: false,
        });
    }

    find!(find_bool, bools, bool);
    find!(find_int, ints, i32);
    find!(find_float, floats, f32);
    find!(find_string, strings, String);
    find!(find_spectrum, spectra, Spectrum);
    find!(find_point2f, point2fs, Point2f);
    find!(find_point3f, point3fs, Point3f);
    find!(find_vector3f, vector3fs, Vector3f);
    find!(find_texture, textures, String);
    find_one!(find_one_bool, bools, bool);
    find_one!(find_one_int, ints, i32);
    find_one!(find_one_float, floats, f32);
    find_one!(find_one_string, strings, String);
    find_one!(find_one_spectrum, spectra, Spectrum);
    find_one!(find_one_point2f, point2fs, Point2f);
    find_one!(find_one_point3f, point3fs, Point3f);
    find_one!(find_one_vector3f, vector3fs, Vector3f);
    find_one!(find_one_texture, textures, String);
}

#[derive(Debug, Clone)]
struct ParamSetItem<T: Debug> {
    name: String,
    values: Vec<T>,
    looked_up: bool,
}

impl<T: Debug> Default for ParamSetItem<T> {
    fn default() -> Self {
        ParamSetItem {
            name: String::new(),
            values: Vec::new(),
            looked_up: false,
        }
    }
}

pub struct TextureParams<'a> {
    geom_params: &'a mut ParamSet,
    material_params: &'a mut ParamSet,
    float_textures: &'a HashMap<String, Arc<Texture<f32> + Send + Sync>>,
    spectrum_textures: &'a HashMap<String, Arc<Texture<Spectrum> + Send + Sync>>,
}

impl<'a> TextureParams<'a> {
    pub fn new(
        gp: &'a mut ParamSet,
        mp: &'a mut ParamSet,
        ft: &'a HashMap<String, Arc<Texture<f32> + Send + Sync>>,
        st: &'a HashMap<String, Arc<Texture<Spectrum> + Send + Sync>>,
    ) -> TextureParams<'a> {
        TextureParams {
            geom_params: gp,
            material_params: mp,
            float_textures: ft,
            spectrum_textures: st,
        }
    }

    pub fn find_string(&mut self, n: &str) -> String {
        let mat_string = self.material_params.find_one_string(n, "".to_owned());
        self.geom_params.find_one_string(n, mat_string)
    }

    pub fn find_bool(&mut self, n: &str, d: bool) -> bool {
        let d = self.material_params.find_one_bool(n, d);
        self.geom_params.find_one_bool(n, d)
    }

    pub fn get_spectrum_texture(
        &mut self,
        n: &str,
        default: &Spectrum,
    ) -> Arc<Texture<Spectrum> + Send + Sync> {
        let mut name = self.geom_params.find_one_texture(n, "".to_owned());
        if &name == "" {
            name = self.material_params.find_one_texture(n, "".to_owned());
        }
        if &name != "" {
            if let Some(tex) = self.spectrum_textures.get(&name) {
                return tex.clone();
            } else {
                error!(
                    "Couldn't find spectrum texture {} for parameter {}",
                    name,
                    n
                );
            }
        }
        // If texture wasn't found
        let val = self.material_params.find_one_spectrum(n, *default);
        let val = self.geom_params.find_one_spectrum(n, val);
        Arc::new(ConstantTexture::new(val))
    }

    pub fn get_float_texture(&mut self, n: &str, default: f32) -> Arc<Texture<f32> + Send + Sync> {
        let mut name = self.geom_params.find_one_texture(n, "".to_owned());
        if &name == "" {
            name = self.material_params.find_one_texture(n, "".to_owned());
        }
        if &name != "" {
            if let Some(tex) = self.float_textures.get(&name) {
                return tex.clone();
            } else {
                error!(
                    "Couldn't find spectrum texture {} for parameter {}",
                    name,
                    n
                );
            }
        }
        // If texture wasn't found
        let val = self.material_params.find_one_float(n, default);
        let val = self.geom_params.find_one_float(n, val);
        Arc::new(ConstantTexture::new(val))
    }
}
