extern crate nalgebra as na;
extern crate rand;

use na::{Vector3, Point3, Similarity3};

pub mod bvh;
pub mod camera;
pub mod colour;
pub mod filter;
pub mod geometry;
pub mod image;
pub mod instance;
pub mod integrator;
pub mod intersection;
pub mod light;
pub mod material;
mod partition;
pub mod ray;
pub mod sampling;
pub mod scene;
pub mod skydome;
pub mod stats;


pub fn mix(a: f32, b: f32, mix: f32) -> f32 {
    b * mix + a * (1.0 - mix)
}

pub type Dim = (usize, usize);

pub type Vector = Vector3<f32>;
pub type Point = Point3<f32>;
pub type Transform = Similarity3<f32>;
