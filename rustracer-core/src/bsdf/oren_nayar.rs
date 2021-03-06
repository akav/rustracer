use std::f32::consts;

use Vector3f;
use spectrum::Spectrum;
use bsdf::BxDFType;
use geometry::{abs_cos_theta, cos_phi, sin_phi, sin_theta};
use bsdf::bxdf::BxDF;

#[derive(Copy, Clone, Debug)]
pub struct OrenNayar {
    r: Spectrum,
    a: f32,
    b: f32,
}

impl OrenNayar {
    pub fn new(r: Spectrum, sigma: f32) -> OrenNayar {
        let sigma_rad = sigma.to_radians();
        let sigma2 = sigma_rad * sigma_rad;

        OrenNayar {
            r: r,
            a: 1.0 - (sigma2 / (2.0 * (sigma2 + 0.33))),
            b: 0.45 * sigma2 / (sigma2 + 0.09),
        }
    }
}

impl BxDF for OrenNayar {
    fn f(&self, wo: &Vector3f, wi: &Vector3f) -> Spectrum {
        let sin_theta_i = sin_theta(wi);
        let sin_theta_o = sin_theta(wo);

        // compute cosine term of the Oren-Nayar model
        let max_cos = if sin_theta_i > 1e-4 && sin_theta_o > 1e-4 {
            let sin_phi_i = sin_phi(wi);
            let cos_phi_i = cos_phi(wi);
            let sin_phi_o = sin_phi(wo);
            let cos_phi_o = cos_phi(wo);
            let d_cos = sin_phi_i * sin_phi_o + cos_phi_i * cos_phi_o;
            d_cos.max(0.0)
        } else {
            0.0
        };
        // compute sine and tangent terms of Oren-Nayar model
        let (sin_alpha, tan_beta) = if abs_cos_theta(wi) > abs_cos_theta(wo) {
            (sin_theta_o, sin_theta_i / abs_cos_theta(wi))
        } else {
            (sin_theta_i, sin_theta_o / abs_cos_theta(wo))
        };

        self.r * consts::FRAC_1_PI * (self.a + self.b * max_cos * sin_alpha * tan_beta)
    }

    fn get_type(&self) -> BxDFType {
        BxDFType::BSDF_REFLECTION | BxDFType::BSDF_DIFFUSE
    }
}
