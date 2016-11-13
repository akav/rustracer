mod bxdf;
mod fresnel;

pub use self::bxdf::*;
pub use self::fresnel::*;

use std::mem;
use na::{self, Cross, Dot, zero, Norm, clamp};

use ::{Vector, Point2f};
use colour::Colourf;
use intersection::Intersection;
use interaction::SurfaceInteraction;

bitflags! {
    pub flags BxDFType: u32 {
        const BSDF_REFLECTION   = 0b_00000001,
        const BSDF_TRANSMISSION = 0b_00000010,
        const BSDF_DIFFUSE      = 0b_00000100,
        const BSDF_GLOSSY       = 0b_00001000,
        const BSDF_SPECULAR     = 0b_00010000,
    }
}

/// Represents the Bidirectional Scattering Distribution Function.
/// It represents the properties of a material at a given point.
#[derive(Copy, Clone)]
pub struct BSDF<'a> {
    /// Index of refraction of the surface
    eta: f32,
    /// Shading normal (i.e. potentially affected by bump-mapping)
    ns: Vector,
    /// Geometry normal
    ng: Vector,
    ss: Vector,
    ts: Vector,
    bxdfs: &'a [Box<BxDF + Sync + Send>],
}

impl<'a> BSDF<'a> {
    pub fn new(isect: &SurfaceInteraction,
               eta: f32,
               bxdfs: &'a [Box<BxDF + Sync + Send>])
               -> BSDF<'a> {
        let ss = isect.dpdu.normalize();
        BSDF {
            eta: eta,
            ns: isect.shading.n,
            ng: isect.n,
            ss: ss,
            ts: isect.shading.n.cross(&ss),
            bxdfs: bxdfs,
        }
    }

    /// Evaluate the BSDF for the given incoming light direction and outgoing light direction.
    pub fn f(&self, wi_w: &Vector, wo_w: &Vector, flags: BxDFType) -> Colourf {
        let wi = self.world_to_local(wi_w);
        let wo = self.world_to_local(wo_w);
        let reflect = wi_w.dot(&self.ng) * wo_w.dot(&self.ng) > 0.0;
        self.bxdfs
            .iter()
            .filter(|b| {
                // Make sure we only evaluate reflection or transmission based on whether wi and wo
                // lie in the same hemisphere.
                b.matches(flags) &&
                ((reflect && (b.get_type().contains(BSDF_REFLECTION))) ||
                 (!reflect && (b.get_type().contains(BSDF_TRANSMISSION))))
            })
            .fold(Colourf::black(), |c, b| c + b.f(&wi, &wo))
    }

    pub fn sample_f(&self,
                    wo_w: &Vector,
                    sample: (f32, f32),
                    flags: BxDFType)
                    -> (Colourf, Vector, f32) {
        if !flags.contains(BSDF_SPECULAR) {
            unimplemented!();
        }

        if flags.contains(BSDF_REFLECTION) {
            let wo = self.world_to_local(&wo_w);
            let wi = Vector::new(-wo.x, -wo.y, wo.z);
            let cos_theta_i = wi.z;
            let kr = fr_dielectric(cos_theta_i, 1.0, self.eta);
            let colour = Colourf::rgb(1.0, 1.0, 1.0) * kr / cos_theta_i.abs();

            assert!(!colour.has_nan());
            return (colour, self.local_to_world(&wi), 1.0);
        } else if flags.contains(BSDF_TRANSMISSION) {
            let wo = self.world_to_local(&wo_w);
            let entering = wo.z > 0.0;
            let (eta_i, eta_t) = if entering {
                (1.0, self.eta)
            } else {
                (self.eta, 1.0)
            };
            let n = if wo.z < 0.0 {
                -Vector::z()
            } else {
                Vector::z()
            };
            return refract(&wo, &n, eta_i / eta_t)
                .map(|wi| {
                    let cos_theta_i = wi.z;
                    let kr = fr_dielectric(cos_theta_i, 1.0, self.eta);
                    let colour = Colourf::rgb(1.0, 1.0, 1.0) * (1.0 - kr) / cos_theta_i.abs();

                    assert!(!colour.has_nan());
                    (colour, self.local_to_world(&wi), 1.0)
                })
                .unwrap_or((Colourf::black(), zero(), 0.0));
        }

        (Colourf::black(), zero(), 0.0)
    }

    fn world_to_local(&self, v: &Vector) -> Vector {
        Vector::new(v.dot(&self.ss), v.dot(&self.ts), v.dot(&self.ns))
    }

    fn local_to_world(&self, v: &Vector) -> Vector {
        Vector::new(self.ss.x * v.x + self.ts.x * v.y + self.ns.x * v.z,
                    self.ss.y * v.x + self.ts.y * v.y + self.ns.y * v.z,
                    self.ss.z * v.z + self.ts.z * v.y + self.ns.z * v.z)
    }
}

// Common geometric functions
#[inline]
pub fn cos_theta(w: &Vector) -> f32 {
    w.z
}

#[inline]
pub fn cos2_theta(w: &Vector) -> f32 {
    w.z * w.z
}

#[inline]
pub fn abs_cos_theta(w: &Vector) -> f32 {
    w.z.abs()
}

#[inline]
pub fn sin2_theta(w: &Vector) -> f32 {
    (1.0 - cos2_theta(w)).max(0.0)
}

#[inline]
pub fn sin_theta(w: &Vector) -> f32 {
    sin2_theta(w).sqrt()
}

#[inline]
pub fn tan_theta(w: &Vector) -> f32 {
    sin_theta(w) / cos_theta(w)
}

#[inline]
pub fn tan2_theta(w: &Vector) -> f32 {
    sin2_theta(w) / cos2_theta(w)
}

#[inline]
pub fn cos_phi(w: &Vector) -> f32 {
    let sin_theta = sin_theta(w);
    if sin_theta == 0.0 {
        0.0
    } else {
        na::clamp(w.x / sin_theta, -1.0, 1.0)
    }
}

#[inline]
pub fn sin_phi(w: &Vector) -> f32 {
    let sin_theta = sin_theta(w);
    if sin_theta == 0.0 {
        0.0
    } else {
        na::clamp(w.y / sin_theta, -1.0, 1.0)
    }
}

#[inline]
pub fn cos2_phi(w: &Vector) -> f32 {
    cos_phi(w) / cos_phi(w)
}

#[inline]
pub fn sin2_phi(w: &Vector) -> f32 {
    sin_phi(w) / sin_phi(w)
}

#[inline]
pub fn cos_d_phi(wa: &Vector, wb: &Vector) -> f32 {
    na::clamp((wa.x * wb.x + wa.y * wa.y) /
              ((wa.x * wa.x + wa.y * wa.y) * (wb.x * wb.x + wb.y * wb.y)).sqrt(),
              -1.0,
              1.0)
}
