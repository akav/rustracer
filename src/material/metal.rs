use std::sync::Arc;

use bsdf::{BSDF, BxDF, Fresnel, TrowbridgeReitzDistribution, MicrofacetReflection};
use interaction::SurfaceInteraction;
use material::{self, Material, TransportMode};
use spectrum::Spectrum;
use texture::{Texture, ConstantTexture};

pub struct Metal {
    eta: Box<Texture<Spectrum> + Send + Sync>,
    k: Box<Texture<Spectrum> + Send + Sync>,
    rough: Box<Texture<f32> + Send + Sync>,
    bump: Option<Box<Texture<f32> + Send + Sync>>,
    // urough: Box<Texture<f32>>,
    // vrough: Box<Texture<f32>>,
    remap_roughness: bool,
}

impl Metal {
    pub fn new() -> Self {
        Metal {
            eta: Box::new(ConstantTexture::new(Spectrum::from_sampled(&COPPER_WAVELENGTHS[..],
                                                                      &COPPER_N[..],
                                                                      COPPER_SAMPLES))),
            k: Box::new(ConstantTexture::new(Spectrum::from_sampled(&COPPER_WAVELENGTHS[..],
                                                                    &COPPER_K[..],
                                                                    COPPER_SAMPLES))),
            rough: Box::new(ConstantTexture::new(0.01)),
            // urough: Box::new(ConstantTexture::new(...)),
            // vrough: Box::new(ConstantTexture::new(...)),
            remap_roughness: true,
            bump: None,
        }
    }

    pub fn gold() -> Self {
        Metal {
            eta: Box::new(ConstantTexture::new(Spectrum::from_sampled(&AU_WAVELENGTHS[..],
                                                                      &AU_N[..],
                                                                      AU_SAMPLES))),
            k: Box::new(ConstantTexture::new(Spectrum::from_sampled(&AU_WAVELENGTHS[..],
                                                                    &AU_K[..],
                                                                    AU_SAMPLES))),
            rough: Box::new(ConstantTexture::new(0.01)),
            // urough: Box::new(ConstantTexture::new(...)),
            // vrough: Box::new(ConstantTexture::new(...)),
            remap_roughness: true,
            bump: None,
        }
    }
}

impl Material for Metal {
    fn compute_scattering_functions(&self,
                                    si: &mut SurfaceInteraction,
                                    mode: TransportMode,
                                    allow_multiple_lobes: bool) {
        if let Some(ref bump) = self.bump {
            material::bump(bump, si);
        }
        let mut bxdfs: Vec<Box<BxDF + Send + Sync>> = Vec::new();
        let mut rough = self.rough.evaluate(si);
        if self.remap_roughness {
            rough = TrowbridgeReitzDistribution::roughness_to_alpha(rough);
        }
        let fresnel = Box::new(Fresnel::conductor(Spectrum::white(),
                                                  self.eta.evaluate(si),
                                                  self.k.evaluate(si)));
        let distrib = Box::new(TrowbridgeReitzDistribution::new(rough, rough));
        bxdfs.push(Box::new(MicrofacetReflection::new(Spectrum::white(), distrib, fresnel)));

        let bsdf = BSDF::new(si, 1.0, bxdfs);
        si.bsdf = Some(Arc::new(bsdf));
    }
}

const COPPER_SAMPLES: usize = 56;
const COPPER_WAVELENGTHS: [f32; COPPER_SAMPLES] = [298.7570554,
                                                   302.4004341,
                                                   306.1337728,
                                                   309.960445,
                                                   313.8839949,
                                                   317.9081487,
                                                   322.036826,
                                                   326.2741526,
                                                   330.6244747,
                                                   335.092373,
                                                   339.6826795,
                                                   344.4004944,
                                                   349.2512056,
                                                   354.2405086,
                                                   359.374429,
                                                   364.6593471,
                                                   370.1020239,
                                                   375.7096303,
                                                   381.4897785,
                                                   387.4505563,
                                                   393.6005651,
                                                   399.9489613,
                                                   406.5055016,
                                                   413.2805933,
                                                   420.2853492,
                                                   427.5316483,
                                                   435.0322035,
                                                   442.8006357,
                                                   450.8515564,
                                                   459.2006593,
                                                   467.8648226,
                                                   476.8622231,
                                                   486.2124627,
                                                   495.936712,
                                                   506.0578694,
                                                   516.6007417,
                                                   527.5922468,
                                                   539.0616435,
                                                   551.0407911,
                                                   563.5644455,
                                                   576.6705953,
                                                   590.4008476,
                                                   604.8008683,
                                                   619.92089,
                                                   635.8162974,
                                                   652.5483053,
                                                   670.1847459,
                                                   688.8009889,
                                                   708.4810171,
                                                   729.3186941,
                                                   751.4192606,
                                                   774.9011125,
                                                   799.8979226,
                                                   826.5611867,
                                                   855.0632966,
                                                   885.6012714];

const COPPER_N: [f32; COPPER_SAMPLES] =
    [1.400313, 1.38, 1.358438, 1.34, 1.329063, 1.325, 1.3325, 1.34, 1.334375, 1.325, 1.317812,
     1.31, 1.300313, 1.29, 1.281563, 1.27, 1.249062, 1.225, 1.2, 1.18, 1.174375, 1.175, 1.1775,
     1.18, 1.178125, 1.175, 1.172812, 1.17, 1.165312, 1.16, 1.155312, 1.15, 1.142812, 1.135,
     1.131562, 1.12, 1.092437, 1.04, 0.950375, 0.826, 0.645875, 0.468, 0.35125, 0.272, 0.230813,
     0.214, 0.20925, 0.213, 0.21625, 0.223, 0.2365, 0.25, 0.254188, 0.26, 0.28, 0.3];

const COPPER_K: [f32; COPPER_SAMPLES] =
    [1.662125, 1.687, 1.703313, 1.72, 1.744563, 1.77, 1.791625, 1.81, 1.822125, 1.834, 1.85175,
     1.872, 1.89425, 1.916, 1.931688, 1.95, 1.972438, 2.015, 2.121562, 2.21, 2.177188, 2.13,
     2.160063, 2.21, 2.249938, 2.289, 2.326, 2.362, 2.397625, 2.433, 2.469187, 2.504, 2.535875,
     2.564, 2.589625, 2.605, 2.595562, 2.583, 2.5765, 2.599, 2.678062, 2.809, 3.01075, 3.24,
     3.458187, 3.67, 3.863125, 4.05, 4.239563, 4.43, 4.619563, 4.817, 5.034125, 5.26, 5.485625,
     5.717];

const AU_SAMPLES: usize = 56;
const AU_WAVELENGTHS: [f32; AU_SAMPLES] =
    [298.757050, 302.400421, 306.133759, 309.960449, 313.884003, 317.908142, 322.036835,
     326.274139, 330.624481, 335.092377, 339.682678, 344.400482, 349.251221, 354.240509,
     359.374420, 364.659332, 370.102020, 375.709625, 381.489777, 387.450562, 393.600555,
     399.948975, 406.505493, 413.280579, 420.285339, 427.531647, 435.032196, 442.800629,
     450.851562, 459.200653, 467.864838, 476.862213, 486.212463, 495.936707, 506.057861,
     516.600769, 527.592224, 539.061646, 551.040771, 563.564453, 576.670593, 590.400818,
     604.800842, 619.920898, 635.816284, 652.548279, 670.184753, 688.800964, 708.481018,
     729.318665, 751.419250, 774.901123, 799.897949, 826.561157, 855.063293, 885.601257];

const AU_N: [f32; AU_SAMPLES] =
    [1.795000, 1.812000, 1.822625, 1.830000, 1.837125, 1.840000, 1.834250, 1.824000, 1.812000,
     1.798000, 1.782000, 1.766000, 1.752500, 1.740000, 1.727625, 1.716000, 1.705875, 1.696000,
     1.684750, 1.674000, 1.666000, 1.658000, 1.647250, 1.636000, 1.628000, 1.616000, 1.596250,
     1.562000, 1.502125, 1.426000, 1.345875, 1.242000, 1.086750, 0.916000, 0.754500, 0.608000,
     0.491750, 0.402000, 0.345500, 0.306000, 0.267625, 0.236000, 0.212375, 0.194000, 0.177750,
     0.166000, 0.161000, 0.160000, 0.160875, 0.164000, 0.169500, 0.176000, 0.181375, 0.188000,
     0.198125, 0.210000];

const AU_K: [f32; AU_SAMPLES] =
    [1.920375, 1.920000, 1.918875, 1.916000, 1.911375, 1.904000, 1.891375, 1.878000, 1.868250,
     1.860000, 1.851750, 1.846000, 1.845250, 1.848000, 1.852375, 1.862000, 1.883000, 1.906000,
     1.922500, 1.936000, 1.947750, 1.956000, 1.959375, 1.958000, 1.951375, 1.940000, 1.924500,
     1.904000, 1.875875, 1.846000, 1.814625, 1.796000, 1.797375, 1.840000, 1.956500, 2.120000,
     2.326250, 2.540000, 2.730625, 2.880000, 2.940625, 2.970000, 3.015000, 3.060000, 3.070000,
     3.150000, 3.445812, 3.800000, 4.087687, 4.357000, 4.610188, 4.860000, 5.125813, 5.390000,
     5.631250, 5.880000];
