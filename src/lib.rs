pub mod air;
pub mod baddie;
pub mod bird;
pub mod body;
pub mod play;
pub mod rng;
pub mod seg;
pub mod sprite;

pub fn dist(a: (f32, f32), b: (f32, f32)) -> f32 {
    (a.0 - b.0).hypot(a.1 - b.1)
}
