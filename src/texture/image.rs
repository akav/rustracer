use std::sync::Arc;
use std::path::Path;

use img;

use ::Point2i;
use interaction::SurfaceInteraction;
use mipmap::{MIPMap, WrapMode};
use spectrum::Spectrum;
use texture::{Texture, TextureMapping2D, UVMapping2D};

pub struct ImageTexture {
    mapping: Box<TextureMapping2D + Send + Sync>,
    mipmap: Arc<MIPMap<Spectrum>>,
}

impl ImageTexture {
    pub fn new(path: &Path) -> ImageTexture {
        info!("Loading texture {}", path.display());
        // TODO log warning and use constant texture if cannot open texture file
        let buf = img::open(path).unwrap();
        let rgb = buf.to_rgb();
        let res = Point2i::new(rgb.width(), rgb.height());
        let pixels: Vec<Spectrum> = rgb.pixels()
            .map(|p| Spectrum::from_srgb(&p.data))
            .collect();

        ImageTexture {
            mapping: Box::new(UVMapping2D::new(1.0, 1.0, 0.5, 0.5)),
            mipmap: Arc::new(MIPMap::new(&res, &pixels[..], false, 0.0, WrapMode::Repeat)),
        }
    }
}

impl Texture<Spectrum> for ImageTexture {
    fn evaluate(&self, si: &SurfaceInteraction) -> Spectrum {
        let st = self.mapping.map(si);
        // TODO Call correct lookup method once we have ray differentials
        self.mipmap.lookup(&st, 0.1)
    }
}

#[test]
fn load_texture() {
    ImageTexture::new(&Path::new("lines.png"));
    assert!(true);
}