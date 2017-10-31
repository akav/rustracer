use std::ops::{AddAssign, Mul};
use std::cmp;
use std::f32;
use std::fmt::Debug;

use num::{zero, Zero};

use {Point2f, Point2i};
use {clamp, lerp, is_power_of_2, round_up_pow_2};
use blockedarray::BlockedArray;
use spectrum::Spectrum;

#[derive(Debug)]
pub enum WrapMode {
    Repeat,
    Black,
    Clamp,
}

#[derive(Debug)]
pub struct MIPMap<T> {
    do_trilinear: bool,
    max_anisotropy: f32,
    wrap_mode: WrapMode,
    resolution: Point2i,
    pyramid: Vec<BlockedArray<T>>,
    black: T,
}

impl<T> MIPMap<T>
    where T: Zero,
          T: Clone,
          T: Copy,
          T: Clampable,
          T: Debug,
          T: AddAssign<T>,
          T: Mul<f32, Output = T>
{
    pub fn new(res: &Point2i,
               img: &[T],
               do_trilinear: bool,
               max_anisotropy: f32,
               wrap_mode: WrapMode)
               -> MIPMap<T> {
        info!("Creating MIPMap for texture");
        let mut resolution = *res;
        let mut resampled_image = Vec::new();
        if !is_power_of_2(res.x) || !is_power_of_2(res.y) {
            // resample image to power of two resolution
            let res_pow2 = Point2i::new(round_up_pow_2(res.x), round_up_pow_2(res.y));
            info!("Texture dimensions are not powers of 2: re-sampling MIPMap from {} to {}.",
                  res,
                  res_pow2);
            // resample image in s direction
            resampled_image.resize((res_pow2.x * res_pow2.y) as usize, zero());
            let s_weights = MIPMap::<T>::resample_weights(res.x as usize, res_pow2.x as usize);
            // apply s_weights to zoom in s direction
            for t in 0..res.y as usize {
                for s in 0..res_pow2.x as usize {
                    // Compute texel (s,t) in s-zoomed image
                    resampled_image[t * res_pow2.x as usize + s] = zero();
                    for j in 0..4usize {
                        let mut orig_s = s_weights[s].first_texel as isize + j as isize;
                        orig_s = match wrap_mode {
                            WrapMode::Repeat => orig_s % res.x as isize,
                            WrapMode::Clamp => clamp(orig_s, 0, res.x as isize - 1),
                            WrapMode::Black => orig_s,
                        };
                        if orig_s >= 0 && orig_s < res.x as isize {
                            resampled_image[t * res_pow2.x as usize + s] += img[(t * res.x as usize + orig_s as usize) as
                            usize] *
                                                                            s_weights[s].weights[j];
                        }
                    }
                }
            }
            // TODO use rayon to parallelize this loop?
            // resample image in t direction
            let t_weights = MIPMap::<T>::resample_weights(res.y as usize, res_pow2.y as usize);
            // apply t_weights to zoom in t direction
            for s in 0..res_pow2.x as usize {
                let mut work_data: Vec<T> = vec![zero(); res_pow2.y as usize];
                for t in 0..res_pow2.y as usize {
                    // work_data[t] = zero();
                    // Compute texel (s,t) in t-zoomed image
                    for j in 0..4 {
                        let mut offset = t_weights[t].first_texel as isize + j as isize;
                        offset = match wrap_mode {
                            WrapMode::Repeat => offset % res.y as isize,
                            WrapMode::Clamp => clamp(offset, 0, res.y as isize - 1),
                            WrapMode::Black => offset,
                        };
                        if offset >= 0 && offset < res.y as isize {
                            work_data[t] += resampled_image[(offset * res_pow2.x as isize + s as isize) as
                            usize] *
                                            t_weights[t].weights[j];
                        }
                    }
                }
                for t in 0..res_pow2.y as usize {
                    resampled_image[t * res_pow2.x as usize + s] = work_data[t].clamp(0.0, 1.0);
                }
            }
            resolution = res_pow2;
        }

        let mut mipmap = MIPMap {
            do_trilinear: do_trilinear,
            max_anisotropy: max_anisotropy,
            wrap_mode: wrap_mode,
            resolution: resolution,
            pyramid: Vec::new(),
            black: zero(),
        };

        // initialize levels of MIPMap for image
        let n_levels = 1 + (cmp::max(resolution.x, resolution.y) as f32).log2() as usize;
        info!("mipmap will have {} levels", n_levels);
        // Initialize most detailed level of the pyramid
        let img_data = if resampled_image.is_empty() {
            img
        } else {
            &resampled_image[..]
        };
        // level 0
        mipmap
            .pyramid
            .push(BlockedArray::new_from(resolution.x as usize, resolution.y as usize, img_data));
        for i in 1..n_levels {
            // initialize ith level of the pyramid
            let s_res = cmp::max(1, mipmap.pyramid[i - 1].u_size() / 2);
            let t_res = cmp::max(1, mipmap.pyramid[i - 1].v_size() / 2);
            let mut ba = BlockedArray::new(s_res, t_res);
            // Filter 4 texels from finer level of pyramid
            for t in 0..t_res {
                for s in 0..s_res {
                    let (si, ti) = (s as isize, t as isize);
                    ba[(s, t)] = (*mipmap.texel(i - 1, 2 * si, 2 * ti) +
                                  *mipmap.texel(i - 1, 2 * si + 1, 2 * ti) +
                                  *mipmap.texel(i - 1, 2 * si, 2 * ti + 1) +
                                  *mipmap.texel(i - 1, 2 * si + 1, 2 * ti + 1)) *
                                 0.25;
                    debug!("l={}, ba[({}, {})]={:?}", i, s, t, ba[(s, t)]);
                }
            }
            mipmap.pyramid.push(ba);
        }
        // TODO initialize EWA filter weights if needed

        mipmap
    }

    pub fn width(&self) -> usize {
        self.resolution.x as usize
    }

    pub fn height(&self) -> usize {
        self.resolution.y as usize
    }

    pub fn levels(&self) -> usize {
        self.pyramid.len()
    }

    pub fn texel(&self, level: usize, s: isize, t: isize) -> &T {
        let l = &self.pyramid[level];
        let (u_size, v_size) = (l.u_size() as isize, l.v_size() as isize);
        let (ss, tt): (usize, usize) = match self.wrap_mode {
            WrapMode::Repeat => (modulo(s, u_size), modulo(t, v_size)),
            WrapMode::Clamp => (clamp(s, 0, u_size - 1) as usize, clamp(t, 0, v_size - 1) as usize),
            WrapMode::Black => {
                if s < 0 || s >= u_size || t < 0 || t >= v_size {
                    return &self.black;
                }
                (s as usize, t as usize)
            }
        };
        &l[(ss, tt)]
    }

    pub fn lookup(&self, st: &Point2f, width: f32) -> T {
        // Compute MIPMap-level for trilinear filtering
        let level = self.levels() as f32 - 1.0 + width.max(1e-8).log2();
        // Perform trilinear interpolation at appropriate MIPMap level
        if level < 0.0 {
            self.triangle(0, st)
        } else if level >= self.levels() as f32 - 1.0 {
            *self.texel(self.levels() - 1, 0, 0)
        } else {
            let i_level = level.floor();
            let delta = level - i_level;
            lerp(delta,
                 self.triangle(i_level as usize, st),
                 self.triangle(i_level as usize + 1, st))
        }
    }

    pub fn triangle(&self, level: usize, st: &Point2f) -> T {
        let level = clamp(level, 0, self.levels() - 1);
        let s = st.x * self.pyramid[level].u_size() as f32 - 0.5;
        let t = st.y * self.pyramid[level].v_size() as f32 - 0.5;
        let s0 = s.floor() as isize;
        let t0 = t.floor() as isize;
        let ds = s - s0 as f32;
        let dt = t - t0 as f32;
        trace!("st={:?}, s={}, t={}, s0={}, t0={}, ds={}, dt={}",
               st,
               s,
               t,
               s0,
               t0,
               ds,
               dt);

        *self.texel(level, s0, t0) * (1.0 - ds) * (1.0 - dt) +
        *self.texel(level, s0, t0 + 1) * (1.0 - ds) * dt +
        *self.texel(level, s0 + 1, t0) * ds * (1.0 - dt) +
        *self.texel(level, s0 + 1, t0 + 1) * ds * dt
    }

    fn resample_weights(old_res: usize, new_res: usize) -> Vec<ResampleWeight> {
        assert!(new_res >= old_res);
        let mut wt = Vec::with_capacity(new_res);
        let filter_width = 2.0;
        let mut w = [0.0; 4];
        for i in 0..new_res {
            // compute image resampling weights for ith texel
            let center = (i as f32 + 0.5) * (old_res as f32 / new_res as f32);
            let first_texel = ((center - filter_width) + 0.5).floor();
            for j in 0..4 {
                let pos = first_texel + j as f32 + 0.5;
                w[j] = Self::lanczos((pos - center) / filter_width);
            }
            // Normalize filter weights for texel resampling
            let inv_sum_weights = 1.0 / (w[0] + w[1] + w[2] + w[3]);
            for j in 0..4 {
                w[j] *= inv_sum_weights;
                assert!(w[j] <= 1.0,
                        "w[j]={}, inv_sum_weights={}",
                        w[j],
                        inv_sum_weights);
            }
            wt.push(ResampleWeight {
                        first_texel: first_texel as i32,
                        weights: w,
                    });
        }

        wt
    }

    fn lanczos(f: f32) -> f32 {
        let tau = 2.0;
        let mut x = f.abs();
        if x < 1e-5 {
            return 1.0;
        };
        if x > 1.0 {
            return 0.0;
        };
        x *= f32::consts::PI;
        let s = (x * tau).sin() / (x * tau);
        let lanczos = x.sin() / x;
        s * lanczos
    }
}

struct ResampleWeight {
    pub first_texel: i32,
    pub weights: [f32; 4],
}

pub trait Clampable {
    fn clamp(self, min: f32, max: f32) -> Self;
}

impl Clampable for f32 {
    fn clamp(self, min: f32, max: f32) -> f32 {
        clamp(self, min, max)
    }
}

impl Clampable for Spectrum {
    fn clamp(self, min: f32, max: f32) -> Spectrum {
        Spectrum::rgb(self.r.clamp(min, max),
                      self.g.clamp(min, max),
                      self.b.clamp(min, max))
    }
}

fn modulo(a: isize, b: isize) -> usize {
    let result = a % b;
    if result < 0 {
        (result + b) as usize
    } else {
        result as usize
    }
}
