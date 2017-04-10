use std::mem;

use {Vector3f, Point2f};
use bsdf::{BxDF, BxDFType, BSDF_SPECULAR, BSDF_REFLECTION, BSDF_TRANSMISSION};
use geometry::*;
use spectrum::Spectrum;
use na::clamp;

/// Compute the reflection direction
pub fn reflect(wo: &Vector3f, n: &Vector3f) -> Vector3f {
    (-(*wo) + *n * 2.0 * wo.dot(n)).normalize()
}

/// Compute the refraction direction
pub fn refract(i: &Vector3f, n: &Vector3f, eta: f32) -> Option<Vector3f> {
    let cos_theta_i = n.dot(i);
    let sin2theta_i = (1.0 - cos_theta_i * cos_theta_i).max(0.0);
    let sin2theta_t = eta * eta * sin2theta_i;

    if sin2theta_t >= 1.0 {
        None
    } else {
        let cos_theta_t = (1.0 - sin2theta_t).sqrt();
        Some(eta * -*i + (eta * cos_theta_i - cos_theta_t) * *n)
    }
}

/// Compute the Fresnel coefficient for dielectric materials
pub fn fr_dielectric(cos_theta_i: f32, eta_i: f32, eta_t: f32) -> f32 {
    let mut cos_theta_i = clamp(cos_theta_i, -1.0, 1.0);
    let (mut eta_i, mut eta_t) = (eta_i, eta_t);
    if cos_theta_i <= 0.0 {
        // If leaving the surface, swap the indices of refraction
        mem::swap(&mut eta_i, &mut eta_t);
        cos_theta_i = cos_theta_i.abs();
    }

    let sin_theta_i = (1.0 - cos_theta_i * cos_theta_i).max(0.0).sqrt();
    let sin_theta_t = eta_i / eta_t * sin_theta_i;
    if sin_theta_t >= 1.0 {
        // Total internal reflection
        1.0
    } else {
        let cos_theta_t = (1.0 - sin_theta_t * sin_theta_t).max(0.0).sqrt();
        // Reflectance for parallel polarized light
        let r_parl = ((eta_t * cos_theta_i) - (eta_i * cos_theta_t)) /
                     ((eta_t * cos_theta_i) + (eta_i * cos_theta_t));
        // Reflectance for perpendicular polarized light
        let r_perp = ((eta_i * cos_theta_i) - (eta_t * cos_theta_t)) /
                     ((eta_i * cos_theta_i) + (eta_t * cos_theta_t));
        // Total reflectance for unpolarized light
        0.5 * (r_parl * r_parl + r_perp * r_perp)
    }
}

fn fr_conductor(cos_theta_i: f32, eta_i: &Spectrum, eta_t: &Spectrum, k: &Spectrum) -> Spectrum {
    let cos_theta_i = clamp(cos_theta_i, -1.0, 1.0);
    let eta = *eta_t / *eta_i;
    let eta_k = *k / *eta_i;

    let cos2_theta_i = cos_theta_i * cos_theta_i;
    let sin2_theta_i = 1.0 - cos2_theta_i;
    let eta2 = eta * eta;
    let eta_k2 = eta_k * eta_k;

    let t0 = eta2 - eta_k2 - sin2_theta_i;
    let a2plusb2 = (t0 * t0 + 4.0 * eta2 * eta_k2).sqrt();
    let t1 = a2plusb2 + cos2_theta_i;
    let a = (0.5 * (a2plusb2 + t0)).sqrt();
    let t2 = 2.0 * cos_theta_i * a;
    let r_s = (t1 - t2) / (t1 + t2);

    let t3 = cos2_theta_i * a2plusb2 + sin2_theta_i * sin2_theta_i;
    let t4 = t2 * sin2_theta_i;
    let r_p = r_s * (t3 - t4) / (t3 + t4);

    0.5 * (r_p + r_s)
}

/// Trait for Fresnel materials
pub trait Fresnel {
    fn evaluate(&self, cos_theta_i: f32) -> Spectrum;
}

impl Fresnel {
    pub fn conductor(eta_i: Spectrum, eta_t: Spectrum, k: Spectrum) -> FresnelConductor {
        FresnelConductor {
            eta_i: eta_i,
            eta_t: eta_t,
            k: k,
        }
    }

    pub fn dielectric(eta_i: f32, eta_t: f32) -> FresnelDielectric {
        FresnelDielectric {
            eta_i: eta_i,
            eta_t: eta_t,
        }
    }
}


/// Fresnel for conductor materials
pub struct FresnelConductor {
    eta_i: Spectrum,
    eta_t: Spectrum,
    k: Spectrum,
}

impl Fresnel for FresnelConductor {
    fn evaluate(&self, cos_theta_i: f32) -> Spectrum {
        fr_conductor(cos_theta_i.abs(), &self.eta_i, &self.eta_t, &self.k)
    }
}

/// Fresnel for dielectric materials
pub struct FresnelDielectric {
    eta_i: f32,
    eta_t: f32,
}

impl Fresnel for FresnelDielectric {
    fn evaluate(&self, cos_theta_i: f32) -> Spectrum {
        Spectrum::grey(fr_dielectric(cos_theta_i.abs(), self.eta_i, self.eta_t))
    }
}

/// BRDF for perfect specular reflection
pub struct SpecularReflection {
    r: Spectrum,
    fresnel: Box<Fresnel + Send + Sync>,
}

impl SpecularReflection {
    pub fn new(r: Spectrum, fresnel: Box<Fresnel + Send + Sync>) -> SpecularReflection {
        SpecularReflection {
            r: r,
            fresnel: fresnel,
        }
    }
}

impl BxDF for SpecularReflection {
    fn f(&self, _wo: &Vector3f, _wi: &Vector3f) -> Spectrum {
        // The probability to call f() with the exact (wo, wi) for specular reflection is 0, so we
        // return black here. Use sample_f() instead.
        Spectrum::black()
    }

    fn sample_f(&self, wo: &Vector3f, _sample: &Point2f) -> (Spectrum, Vector3f, f32, BxDFType) {
        // There's only one possible wi for a given wo, so we always return it with a pdf of 1.
        let wi = Vector3f::new(-wo.x, -wo.y, wo.z);
        let spectrum = self.fresnel.evaluate(cos_theta(&wi)) * self.r / abs_cos_theta(&wi);
        (spectrum, wi, 1.0, BxDFType::empty())
    }

    fn pdf(&self, _wo: &Vector3f, _wi: &Vector3f) -> f32 {
        0.0
    }

    fn get_type(&self) -> BxDFType {
        BSDF_SPECULAR | BSDF_REFLECTION
    }
}

pub struct SpecularTransmission {
    t: Spectrum,
    eta_a: f32,
    eta_b: f32,
    fresnel: FresnelDielectric,
}

impl SpecularTransmission {
    pub fn new(t: Spectrum, eta_a: f32, eta_b: f32) -> SpecularTransmission {
        SpecularTransmission {
            t: t,
            eta_a: eta_a,
            eta_b: eta_b,
            fresnel: Fresnel::dielectric(eta_a, eta_b),
        }
    }
}

impl BxDF for SpecularTransmission {
    fn f(&self, _wo: &Vector3f, _wi: &Vector3f) -> Spectrum {
        // The probability to call f() with the exact (wo, wi) for specular transmission is 0, so we
        // return black here. Use sample_f() instead.
        Spectrum::black()
    }

    fn sample_f(&self, wo: &Vector3f, _sample: &Point2f) -> (Spectrum, Vector3f, f32, BxDFType) {
        // Figure out which $\eta$ is incident and which is transmitted
        let entering = cos_theta(wo) > 0.0;
        let eta_i = if entering { self.eta_a } else { self.eta_b };
        let eta_t = if entering { self.eta_b } else { self.eta_a };

        // Compute ray direction for specular transmission
        if let Some(wi) = refract(wo,
                                  &face_forward(&Vector3f::new(0.0, 0.0, 1.0), wo),
                                  eta_i / eta_t) {
            let mut ft = self.t * (Spectrum::white() - self.fresnel.evaluate(cos_theta(&wi)));

            // Account for non-symmetry with transmission to different medium TODO
            ft = ft * (eta_i * eta_i) / (eta_t * eta_t);
            debug!("wo={}. wi={}, cos_theta(wo)={}, cos_theta(wi)={}, abs_cos_theta(wi)={}, ft={}",
                   wo,
                   wi,
                   cos_theta(wo),
                   cos_theta(&wi),
                   abs_cos_theta(&wi),
                   ft);

            return (ft / abs_cos_theta(&wi), wi, 1.0, BSDF_SPECULAR | BSDF_TRANSMISSION);
        } else {
            return (Spectrum::white(), Vector3f::new(0.0, 0.0, 0.0), 0.0, BxDFType::empty());
        }
    }

    fn pdf(&self, _wo: &Vector3f, _wi: &Vector3f) -> f32 {
        0.0
    }

    fn get_type(&self) -> BxDFType {
        BSDF_SPECULAR | BSDF_TRANSMISSION
    }
}

pub struct FresnelSpecular {
    r: Spectrum,
    t: Spectrum,
    eta_a: f32,
    eta_b: f32,
}

impl FresnelSpecular {
    pub fn new() -> FresnelSpecular {
        FresnelSpecular {
            r: Spectrum::white(),
            t: Spectrum::white(),
            eta_a: 1.0,
            eta_b: 1.5,
        }
    }
}

impl BxDF for FresnelSpecular {
    fn f(&self, _wo: &Vector3f, _wi: &Vector3f) -> Spectrum {
        // The probability to call f() with the exact (wo, wi) for specular reflection is 0, so we
        // return black here. Use sample_f() instead.
        Spectrum::black()
    }

    fn sample_f(&self, wo: &Vector3f, u: &Point2f) -> (Spectrum, Vector3f, f32, BxDFType) {
        let fr = fr_dielectric(cos_theta(wo), self.eta_a, self.eta_b);
        if u[0] < fr {
            // Compute specular reflection for FresnelSpecular

            // Compute perfect specular reflection direction
            let wi = Vector3f::new(-wo.x, -wo.y, wo.z);

            return (fr * self.r * abs_cos_theta(&wi), wi, fr, BSDF_SPECULAR | BSDF_REFLECTION);
        } else {
            // Compute specular transmission for FresnelSpecular

            // Figure out which $\eta$ is incident and which is transmitted
            let entering = cos_theta(wo) > 0.0;
            let eta_i = if entering { self.eta_a } else { self.eta_b };
            let eta_t = if entering { self.eta_b } else { self.eta_a };

            // Compute ray direction for specular transmission
            if let Some(wi) = refract(wo,
                                      &face_forward(&Vector3f::new(0.0, 0.0, 1.0), wo),
                                      eta_i / eta_t) {
                let ft = self.t * (1.0 - fr);

                // Account for non-symmetry with transmission to different medium
                // TODO
                //
                return (ft / abs_cos_theta(&wi), wi, 1.0 - fr, BSDF_SPECULAR | BSDF_TRANSMISSION);
            } else {
                return (Spectrum::white(), Vector3f::new(0.0, 0.0, 0.0), 0.0, BxDFType::empty());
            }
        }
    }

    fn pdf(&self, _wo: &Vector3f, _wi: &Vector3f) -> f32 {
        0.0
    }

    fn get_type(&self) -> BxDFType {
        BSDF_SPECULAR | BSDF_REFLECTION | BSDF_TRANSMISSION
    }
}
