use std::sync::Arc;
use ray::Ray;
use bounds::Bounds3f;
use interaction::SurfaceInteraction;
use shapes::Shape;


pub trait AreaLight {}

pub trait Material {
    fn compute_scattering_functions(&self,
                                    isect: &mut SurfaceInteraction,
                                    mode: TransportMode,
                                    allow_multiple_lobes: bool);
}

pub enum TransportMode {
}

pub trait Primitive {
    fn world_bounds(&self) -> Bounds3f;

    fn intersect(&self, ray: &mut Ray) -> Option<SurfaceInteraction>;

    fn intersect_p(&self, ray: &mut Ray) -> bool;

    fn area_light(&self) -> Option<Arc<AreaLight + Send + Sync>>;

    fn material(&self) -> Option<Arc<Material + Send + Sync>>;
    fn compute_scattering_functions(&self,
                                    isect: &mut SurfaceInteraction,
                                    mode: TransportMode,
                                    allow_multiple_lobes: bool);
}

pub struct GeometricPrimitive {
    pub shape: Arc<Shape + Send + Sync>,
    pub area_light: Option<Arc<AreaLight + Send + Sync>>,
    pub material: Option<Arc<Material + Send + Sync>>,
}

impl Primitive for GeometricPrimitive {
    fn world_bounds(&self) -> Bounds3f {
        self.shape.world_bounds()
    }

    fn intersect(&self, ray: &mut Ray) -> Option<SurfaceInteraction> {
        self.shape.intersect(ray).map(|(mut isect, t_hit)| {
            isect.primitive = Some(self);
            ray.t_max = t_hit;
            isect
        })
    }

    fn intersect_p(&self, ray: &mut Ray) -> bool {
        self.shape.intersect_p(ray)
    }

    fn area_light(&self) -> Option<Arc<AreaLight + Send + Sync>> {
        self.area_light.clone()
    }

    fn material(&self) -> Option<Arc<Material + Send + Sync>> {
        self.material.clone()
    }

    fn compute_scattering_functions(&self,
                                    isect: &mut SurfaceInteraction,
                                    mode: TransportMode,
                                    allow_multiple_lobes: bool) {
        if let Some(ref material) = self.material {
            material.compute_scattering_functions(isect, mode, allow_multiple_lobes);
        }
    }
}