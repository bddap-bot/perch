use crate::air::Air;
use crate::baddie::Baddie;
use crate::bird::{Brain, Look, Senses};
use crate::body::{Body, HEIGHT, WIDTH};
use crate::dist;
use crate::rng::Rng;
use crate::seg::Mask;

pub const BREATH_SECONDS: f32 = 6.0;
pub const MAX_BADDIES: usize = 16;
pub const MAX_POOFS: usize = 16;
pub const POOF_SECONDS: f32 = 0.45;
const REFILL_SECONDS: f32 = 1.5;
const WAVE_SECONDS: f32 = 20.0;
const GRACE: f32 = 4.0;
const LULL: f32 = 3.0;
const SETTLED: f32 = 0.5;
const MOVING: f32 = 0.25;
const CURIOUS: f32 = 6.0;
const KNOCK: i32 = 3;
const KNOCK_CELLS: usize = 3;
const SPAWN_CLEARANCE: f32 = 60.0;

#[derive(Clone, Copy)]
enum Cell {
    Open { since: f32 },
    Solid { appeared: Option<f32> },
}

pub struct Ground {
    width: usize,
    cells: Vec<Cell>,
    now: f32,
}

impl Ground {
    fn new(mask: &Mask) -> Self {
        Self {
            width: mask.width,
            cells: mask
                .cells
                .iter()
                .map(|&solid| {
                    if solid {
                        Cell::Solid { appeared: None }
                    } else {
                        Cell::Open {
                            since: f32::NEG_INFINITY,
                        }
                    }
                })
                .collect(),
            now: 0.0,
        }
    }

    fn observe(&mut self, mask: &Mask) {
        let now = self.now;
        for (cell, &solid) in self.cells.iter_mut().zip(&mask.cells) {
            *cell = match (*cell, solid) {
                (Cell::Open { since }, true) => Cell::Solid {
                    appeared: (now - since >= SETTLED).then_some(now),
                },
                (Cell::Solid { .. }, false) => Cell::Open { since: now },
                (kept, _) => kept,
            };
        }
    }

    pub fn appeared(&self, x: i32, y: i32, within: f32) -> bool {
        if x < 0 || y < 0 || x as usize >= self.width {
            return false;
        }
        matches!(
            self.cells.get(y as usize * self.width + x as usize),
            Some(Cell::Solid { appeared: Some(t) }) if self.now - t < within
        )
    }

    pub fn novel(&self, x: i32, y: i32) -> bool {
        self.appeared(x, y, CURIOUS)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Loss {
    Caught,
    Suffocated,
}

pub struct Poof {
    pub x: f32,
    pub y: f32,
    pub age: f32,
}

pub struct Round {
    pub body: Body,
    pub look: Look,
    pub air: Air,
    pub baddies: Vec<Baddie>,
    pub poofs: Vec<Poof>,
    pub breath: f32,
    pub time: f32,
    pub wave: u32,
    pub lost: Option<Loss>,
    brain: Brain,
    ground: Ground,
    to_spawn: u32,
    spawn_in: f32,
    wave_age: f32,
    rng: Rng,
}

impl Round {
    pub fn new(mask: &Mask, seed: u32) -> Self {
        Self {
            body: Body::spawn(mask),
            look: Look::default(),
            air: Air::default(),
            baddies: Vec::new(),
            poofs: Vec::new(),
            breath: 1.0,
            time: 0.0,
            wave: 1,
            lost: None,
            brain: Brain::default(),
            ground: Ground::new(mask),
            to_spawn: 2,
            spawn_in: GRACE,
            wave_age: 0.0,
            rng: Rng::new(seed),
        }
    }

    pub fn observe(&mut self, mask: &Mask) {
        self.ground.observe(mask);
    }

    pub fn step(&mut self, mask: &Mask, eyes: Option<(f32, f32)>, dt: f32) {
        for poof in &mut self.poofs {
            poof.age += dt;
        }
        self.poofs.retain(|p| p.age < POOF_SECONDS);
        if self.lost.is_some() {
            return;
        }
        self.time += dt;
        self.ground.now = self.time;

        let (bx, by) = self.body.cell();
        self.air.fill(
            mask,
            (by..by + HEIGHT).flat_map(|y| (bx..bx + WIDTH).map(move |x| (x, y))),
        );
        self.breath = if self.air.breathes() {
            (self.breath + dt / REFILL_SECONDS).min(1.0)
        } else {
            (self.breath - dt / BREATH_SECONDS).max(0.0)
        };

        self.waves(mask, dt);

        let me = self.body.centre();
        let threat = self
            .baddies
            .iter()
            .map(|b| (b.x, b.y))
            .min_by(|a, b| dist(*a, me).total_cmp(&dist(*b, me)));
        let senses = Senses {
            mask,
            air: &self.air,
            ground: &self.ground,
            threat,
            eyes,
            breath: self.breath,
        };
        let input;
        (input, self.look) = self.brain.think(&self.body, &senses, &mut self.rng, dt);
        self.body.step(mask, input, dt);

        let prey = self.body.centre();
        let speed = (14.0 + 3.0 * self.wave as f32).min(40.0);
        let mut caught = false;
        let (ground, air, poofs) = (&self.ground, &self.air, &mut self.poofs);
        self.baddies.retain_mut(|b| {
            let (cx, cy) = b.cell();
            let swats = (-KNOCK..=KNOCK)
                .flat_map(|dy| (-KNOCK..=KNOCK).map(move |dx| (cx + dx, cy + dy)))
                .filter(|&(x, y)| ground.appeared(x, y, MOVING))
                .count();
            if swats >= KNOCK_CELLS || mask.solid(cx, cy) {
                if poofs.len() == MAX_POOFS {
                    poofs.remove(0);
                }
                poofs.push(Poof {
                    x: b.x,
                    y: b.y,
                    age: 0.0,
                });
                return false;
            }
            caught |= b.hunt(mask, air, prey, speed, dt);
            true
        });
        if caught {
            self.lost = Some(Loss::Caught);
        } else if self.breath <= 0.0 {
            self.lost = Some(Loss::Suffocated);
        }
    }

    fn waves(&mut self, mask: &Mask, dt: f32) {
        self.wave_age += dt;
        self.spawn_in -= dt;
        if self.wave_age > WAVE_SECONDS || (self.to_spawn == 0 && self.baddies.is_empty()) {
            self.wave += 1;
            self.to_spawn = self.wave + 1;
            self.spawn_in = LULL;
            self.wave_age = 0.0;
        } else if self.to_spawn > 0 && self.spawn_in <= 0.0 && self.baddies.len() < MAX_BADDIES {
            if self.spawn(mask) {
                self.to_spawn -= 1;
            }
            self.spawn_in = (3.0 - 0.25 * self.wave as f32).max(0.8) * self.rng.range(0.7, 1.3);
        }
    }

    fn spawn(&mut self, mask: &Mask) -> bool {
        let (w, h) = (mask.width as f32 - 0.5, mask.height as f32 - 0.5);
        let me = self.body.centre();
        let mut best = None;
        let mut best_score = f32::MIN;
        for _ in 0..32 {
            let t = self.rng.unit();
            let (x, y) = match self.rng.bits() % 4 {
                0 => (t * w, 0.5),
                1 => (t * w, h),
                2 => (0.5, t * h),
                _ => (w, t * h),
            };
            let (cx, cy) = (x as i32, y as i32);
            if mask.solid(cx, cy) || dist((x, y), me) < SPAWN_CLEARANCE {
                continue;
            }
            let score = if self.air.reached(cx, cy) { 10.0 } else { 0.0 } + self.rng.unit();
            if score > best_score {
                best_score = score;
                best = Some((x, y));
            }
        }
        if let Some((x, y)) = best {
            self.baddies.push(Baddie::new(x, y));
        }
        best.is_some()
    }
}
