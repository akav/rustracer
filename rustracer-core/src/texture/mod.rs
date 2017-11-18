use std::fmt::Debug;

use {Point2f, Point3f, Transform, Vector3f};
use interaction::SurfaceInteraction;
use paramset::TextureParams;
use spectrum::Spectrum;

mod constant;
mod checkerboard;
mod imagemap;
mod fbm;
mod scale;

pub use self::constant::ConstantTexture;
pub use self::checkerboard::CheckerboardTexture;
pub use self::imagemap::ImageTexture;
pub use self::fbm::FbmTexture;
pub use self::scale::ScaleTexture;

pub trait Texture<T>: Debug {
    fn evaluate(&self, si: &SurfaceInteraction) -> T;
}

#[derive(Debug)]
pub struct UVTexture {
    mapping: Box<TextureMapping2D + Send + Sync>,
}

impl UVTexture {
    pub fn new() -> UVTexture {
        UVTexture { mapping: Box::new(UVMapping2D::new(1.0, 1.0, 0.0, 0.0)) }
    }

    pub fn create_spectrum(_tex2world: &Transform, tp: &mut TextureParams) -> UVTexture {
        let typ = tp.find_string("mapping", "uv");
        let mapping = if typ == "uv" {
            let su = tp.find_float("uscale", 1.0);
            let sv = tp.find_float("vscale", 1.0);
            let du = tp.find_float("udelta", 0.0);
            let dv = tp.find_float("vdelta", 0.0);
            Box::new(UVMapping2D::new(su, sv, du, dv))
        } else if typ == "spherical" {
            unimplemented!()
        } else if typ == "cylindrical" {
            unimplemented!()
        } else if typ == "planar" {
            unimplemented!()
        } else {
            error!("2D texture mapping \"{}\" unknown.", typ);
            Box::new(UVMapping2D::new(1.0, 1.0, 0.0, 0.0))
        };

        UVTexture { mapping }
    }
}

impl Texture<Spectrum> for UVTexture {
    fn evaluate(&self, si: &SurfaceInteraction) -> Spectrum {
        let st = self.mapping.map(si);
        Spectrum::rgb(st[0] - st[0].floor(), st[1] - st[1].floor(), 0.0)
    }
}

// Texture mappings

pub trait TextureMapping2D: Debug {
    fn map(&self, si: &SurfaceInteraction) -> Point2f;
}

#[derive(Debug)]
pub struct UVMapping2D {
    su: f32,
    sv: f32,
    du: f32,
    dv: f32,
}

impl UVMapping2D {
    pub fn new(su: f32, sv: f32, du: f32, dv: f32) -> UVMapping2D {
        UVMapping2D {
            su: su,
            sv: sv,
            du: du,
            dv: dv,
        }
    }
}

impl TextureMapping2D for UVMapping2D {
    fn map(&self, si: &SurfaceInteraction) -> Point2f {
        Point2f::new(self.su * si.uv.x + self.du, self.sv * si.uv.y + self.dv)
    }
}

pub trait TextureMapping3D: Debug {
    fn map(&self, si: &SurfaceInteraction) -> (Point3f, Vector3f, Vector3f);
}

#[derive(Debug, Default)]
pub struct TransformMapping3D {
    world_to_texture: Transform,
}

impl TransformMapping3D {
    pub fn new() -> TransformMapping3D {
        TransformMapping3D { world_to_texture: Transform::default() }
    }
}

impl TextureMapping3D for TransformMapping3D {
    fn map(&self, si: &SurfaceInteraction) -> (Point3f, Vector3f, Vector3f) {
        let dpdx = &self.world_to_texture * &si.dpdx;
        let dpdy = &self.world_to_texture * &si.dpdy;
        let p = &self.world_to_texture * &si.hit.p;

        (p, dpdx, dpdy)
    }
}
