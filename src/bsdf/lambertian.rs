use std::f32::consts;

use bsdf::{BxDF, BxDFType, BSDF_DIFFUSE, BSDF_REFLECTION};
use ::Vector3f;
use spectrum::Spectrum;

pub struct LambertianReflection {
    r: Spectrum,
}

impl LambertianReflection {
    pub fn new(r: Spectrum) -> LambertianReflection {
        LambertianReflection { r: r }
    }
}

impl BxDF for LambertianReflection {
    fn f(&self, _wo: &Vector3f, _wi: &Vector3f) -> Spectrum {
        self.r * consts::FRAC_1_PI
    }

    fn get_type(&self) -> BxDFType {
        BSDF_DIFFUSE | BSDF_REFLECTION
    }
}
