use crate::air::Air;
use crate::dist;
use crate::seg::Mask;

pub const CATCH: f32 = 5.0;

pub struct Baddie {
    pub x: f32,
    pub y: f32,
    pub facing_left: bool,
}

fn cell(x: f32, y: f32) -> (i32, i32) {
    (x.floor() as i32, y.floor() as i32)
}

impl Baddie {
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            x,
            y,
            facing_left: false,
        }
    }

    pub fn cell(&self) -> (i32, i32) {
        cell(self.x, self.y)
    }

    pub fn hunt(&mut self, mask: &Mask, air: &Air, prey: (f32, f32), speed: f32, dt: f32) -> bool {
        let free = |x: f32, y: f32| {
            let (cx, cy) = cell(x, y);
            !mask.solid(cx, cy)
        };
        let mut left = speed * dt;
        while left > 1e-4 {
            let (cx, cy) = self.cell();
            let target = air
                .downhill(cx, cy, prey)
                .map_or(prey, |(x, y)| (x as f32 + 0.5, y as f32 + 0.5));
            let (dx, dy) = (target.0 - self.x, target.1 - self.y);
            let len = dx.hypot(dy);
            if len < 1e-4 {
                break;
            }
            let s = left.min(len).min(1.0);
            left -= s;
            let (sx, sy) = (dx / len * s, dy / len * s);
            if dx.abs() > 0.01 {
                self.facing_left = dx < 0.0;
            }
            if free(self.x + sx, self.y + sy) {
                self.x += sx;
                self.y += sy;
            } else if sx.abs() > 1e-3 && free(self.x + sx, self.y) {
                self.x += sx;
            } else if sy.abs() > 1e-3 && free(self.x, self.y + sy) {
                self.y += sy;
            } else {
                break;
            }
        }
        dist((self.x, self.y), prey) < CATCH
    }
}
