use crate::air::Air;
use crate::body::{Body, HEIGHT, Input, STEP_UP, WIDTH, centre_at, overlaps};
use crate::dist;
use crate::play::Ground;
use crate::rng::Rng;
use crate::seg::Mask;
use crate::sprite::{Eyes, Frame, Legs, Wings};

const SCARED: f32 = 45.0;
const PANIC_BREATH: f32 = 0.5;
pub const LOW_AIR: f32 = 0.35;
const PECK_BEAT: f32 = 0.22;
const CANDIDATES: usize = 48;
const NEAR_EYES: f32 = 40.0;

pub struct Senses<'a> {
    pub mask: &'a Mask,
    pub air: &'a Air,
    pub ground: &'a Ground,
    pub threat: Option<(f32, f32)>,
    pub eyes: Option<(f32, f32)>,
    pub breath: f32,
}

struct Mood {
    scared: bool,
    choking: bool,
    panic: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Look {
    pub frame: Frame,
    pub face_left: bool,
    pub tilt: f32,
    pub squash: f32,
    pub shake: f32,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Act {
    Still,
    LookBack,
    Tilt(f32),
    Peck,
    Hop,
}

#[derive(Clone, Copy)]
struct Goal {
    x: f32,
    y: f32,
    perch: bool,
}

pub struct Brain {
    goal: Option<Goal>,
    goal_age: f32,
    rest: f32,
    act: Act,
    act_left: f32,
    flap_cd: f32,
    since_flap: f32,
    sealed_for: f32,
    joy: f32,
    blink: f32,
    squash: f32,
    fall_speed: f32,
    was_grounded: bool,
    clock: f32,
}

fn ground_within(mask: &Mask, x: i32, y: i32, depth: i32) -> bool {
    (1..=depth).any(|d| overlaps(mask, x, y + d))
}

impl Default for Brain {
    fn default() -> Self {
        Self {
            goal: None,
            goal_age: 0.0,
            rest: 0.0,
            act: Act::Still,
            act_left: 0.0,
            flap_cd: 0.0,
            since_flap: 1.0,
            sealed_for: 0.0,
            joy: 0.0,
            blink: 2.0,
            squash: 0.0,
            fall_speed: 0.0,
            was_grounded: false,
            clock: 0.0,
        }
    }
}

impl Brain {
    pub fn think(&mut self, body: &Body, s: &Senses, rng: &mut Rng, dt: f32) -> (Input, Look) {
        self.clock += dt;
        self.goal_age += dt;
        self.flap_cd -= dt;
        self.since_flap += dt;
        self.joy -= dt;
        self.blink -= dt;

        let me = body.centre();
        let scared = s.threat.is_some_and(|t| dist(me, t) < SCARED);
        let choking = !s.air.breathes();
        let mut hop = false;
        if choking {
            self.sealed_for += dt;
        } else {
            if self.sealed_for > 0.7 {
                self.joy = 1.4;
                hop = true;
            }
            self.sealed_for = 0.0;
        }
        let mood = Mood {
            scared,
            choking,
            panic: scared || (choking && s.breath < PANIC_BREATH),
        };

        let arrived = self.goal.is_some_and(|g| {
            let (dx, dy) = (g.x - body.x, g.y - body.y);
            if g.perch {
                body.grounded && dx.abs() < 2.0 && dy.abs() < 3.0
            } else {
                dx.hypot(dy) < 4.0
            }
        });
        if arrived {
            self.rest -= dt;
        }
        let lost_goal = self.goal.is_some_and(|g| {
            let c = centre_at(g.x, g.y);
            !s.air.reached(c.0 as i32, c.1 as i32)
        });
        if self.goal.is_none()
            || lost_goal
            || self.goal_age > 6.0
            || (scared && self.goal_age > 0.4)
            || (choking && self.goal_age > 1.5)
            || (arrived && self.rest <= 0.0)
        {
            self.goal = self.pick(body, s, rng, &mood);
            self.goal_age = 0.0;
            self.rest = if self.goal.is_some_and(|g| g.perch) {
                rng.range(1.2, 4.0)
            } else {
                rng.range(0.2, 0.8)
            };
            if let (Some(g), Some(e)) = (self.goal, s.eyes)
                && !scared
                && dist(centre_at(g.x, g.y), e) < NEAR_EYES
            {
                self.rest *= 2.0;
            }
            self.act = Act::Still;
            self.act_left = 0.0;
        }

        let mut input = self.steer(body, s, mood.panic, rng);
        if body.grounded && (hop || self.act == Act::Hop) {
            input.jump_pressed = true;
            self.act = Act::Still;
        }

        let perched = arrived && body.grounded;
        if perched && !mood.panic {
            self.act_left -= dt;
            if self.act_left <= 0.0 {
                let (cx, cy) = body.cell();
                let fresh = s.ground.novel(cx + WIDTH / 2, cy + HEIGHT);
                self.choose_act(fresh, rng);
            }
        } else if !perched {
            self.act = Act::Still;
        }

        (input, self.look(body, s, &mood, dt, rng))
    }

    fn pick(&self, body: &Body, s: &Senses, rng: &mut Rng, mood: &Mood) -> Option<Goal> {
        let Mood {
            scared, choking, ..
        } = *mood;
        let mask = s.mask;
        let (w, h) = (mask.width as i32, mask.height as i32);
        let me = body.centre();
        let mut best = None;
        let mut best_score = f32::MIN;
        for i in 0..CANDIDATES {
            let (x, y) = match s.eyes {
                Some(e) if i % 3 == 0 => (
                    (e.0 + rng.range(-30.0, 30.0)) as i32,
                    (e.1 - rng.range(0.0, 50.0)) as i32,
                ),
                _ => (
                    rng.range(0.0, (w - WIDTH) as f32) as i32,
                    rng.range(0.0, (h - HEIGHT) as f32) as i32,
                ),
            };
            let x = x.clamp(0, w - WIDTH);
            let mut y = y.clamp(0, h - HEIGHT);
            if overlaps(mask, x, y) {
                continue;
            }
            while y + HEIGHT < h && !overlaps(mask, x, y + 1) {
                y += 1;
            }
            let perched = y + HEIGHT < h;
            if !perched {
                y = rng.range(0.0, (h / 2) as f32) as i32;
                if overlaps(mask, x, y) {
                    continue;
                }
            }
            let c = centre_at(x as f32, y as f32);
            if !s.air.reached(c.0 as i32, c.1 as i32) {
                continue;
            }
            let far = dist(c, me);
            let mut score = rng.range(0.0, 12.0) - far * 0.04;
            if !perched {
                score -= 25.0;
            }
            if far < 6.0 && !scared && !choking {
                score -= 10.0;
            }
            if perched && s.ground.novel(x + WIDTH / 2, y + HEIGHT) {
                score += 35.0;
            }
            if let Some(t) = s.threat {
                score += dist(c, t).min(150.0) * if scared { 1.0 } else { 0.15 };
            }
            if let Some(e) = s.eyes
                && !scared
                && !choking
            {
                score += (60.0 - dist(c, e)).max(0.0) * 0.8;
            }
            if choking {
                let edge = c.0.min(c.1).min(w as f32 - c.0).min(h as f32 - c.1);
                score -= edge * 0.8;
            }
            if score > best_score {
                best_score = score;
                best = Some(Goal {
                    x: x as f32,
                    y: y as f32,
                    perch: perched,
                });
            }
        }
        best
    }

    fn steer(&mut self, body: &Body, s: &Senses, panic: bool, rng: &mut Rng) -> Input {
        let mut input = Input::default();
        let Some(Goal { x: gx, y: gy, .. }) = self.goal else {
            return input;
        };
        let mask = s.mask;
        let (dx, dy) = (gx - body.x, gy - body.y);
        let (cx, cy) = body.cell();
        let dir = dx.signum() as i32;
        let near_x = dx.abs() < 1.5;
        if !near_x {
            let pace = if panic { 1.0 } else { 0.75 };
            input.run = (dx / 6.0).clamp(-1.0, 1.0).abs().max(0.4).copysign(dx) * pace;
        }
        if body.grounded {
            let wall = !near_x && overlaps(mask, cx + dir * 2, cy - STEP_UP);
            let gap = !near_x && !ground_within(mask, cx + dir * WIDTH, cy, 10);
            let climb = dy < -4.0 && dx.abs() < 70.0;
            if wall || gap || climb {
                input.jump_pressed = true;
                input.jump_held = dy < -10.0;
            }
        } else {
            input.jump_held = body.vy < 0.0 && dy < -10.0;
            let floor = ground_within(mask, cx, cy, 30);
            let low = cy + HEIGHT > mask.height as i32 - 12;
            let glide = !floor && body.vy > 60.0 && dy < 30.0 && dx.abs() > 4.0;
            let wants_up = dy < -1.5 || glide || (low && !floor);
            if wants_up && self.flap_cd <= 0.0 && body.vy > -40.0 {
                input.flap = true;
                self.flap_cd = if panic { 0.16 } else { 0.26 } * rng.range(0.85, 1.2);
                self.since_flap = 0.0;
                self.squash = self.squash.min(-0.12);
            }
        }
        input
    }

    fn choose_act(&mut self, fresh: bool, rng: &mut Rng) {
        if fresh && self.act != Act::Peck {
            self.act = Act::Peck;
            self.act_left = PECK_BEAT * rng.range(2.0, 4.99).floor();
            return;
        }
        let options: [(Act, f32); 5] = [
            (Act::Still, 2.0),
            (Act::LookBack, 1.6),
            (
                Act::Tilt(rng.range(0.15, 0.35).copysign(rng.range(-1.0, 1.0))),
                2.0,
            ),
            (Act::Peck, 1.0),
            (Act::Hop, 0.35),
        ];
        let total: f32 = options
            .iter()
            .filter(|(a, _)| *a != self.act)
            .map(|(_, w)| w)
            .sum();
        let mut roll = rng.range(0.0, total);
        for (act, weight) in options {
            if act == self.act {
                continue;
            }
            roll -= weight;
            if roll <= 0.0 {
                self.act = act;
                break;
            }
        }
        self.act_left = match self.act {
            Act::Still => rng.range(0.4, 1.6),
            Act::LookBack => rng.range(0.5, 1.4),
            Act::Tilt(_) => rng.range(0.5, 1.1),
            Act::Peck => PECK_BEAT * rng.range(1.0, 3.99).floor(),
            Act::Hop => 0.3,
        };
    }

    fn look(&mut self, body: &Body, s: &Senses, mood: &Mood, dt: f32, rng: &mut Rng) -> Look {
        let Mood {
            scared,
            choking,
            panic,
        } = *mood;
        let me = body.centre();
        if body.grounded && !self.was_grounded {
            self.squash = (self.fall_speed / 260.0 * 0.5).clamp(0.12, 0.45);
        } else if !body.grounded && self.was_grounded && body.vy < -100.0 {
            self.squash = -0.3;
        }
        if !body.grounded {
            self.fall_speed = body.vy;
        }
        self.was_grounded = body.grounded;
        self.squash *= (1.0 - dt * 10.0).max(0.0);

        let moving = body.vx.abs() > 5.0;
        let legs = if !body.grounded {
            Legs::Tuck
        } else if moving && ((self.clock * 8.0) as u32).is_multiple_of(2) {
            Legs::Stride
        } else {
            Legs::Stand
        };
        let beat = ((self.clock * 12.0) as u32).is_multiple_of(2);
        let wings = if !body.grounded {
            if self.since_flap < 0.1 || body.vy >= 0.0 {
                Wings::Down
            } else {
                Wings::Up
            }
        } else if panic || self.joy > 0.9 {
            if beat { Wings::Up } else { Wings::Down }
        } else {
            Wings::Rest
        };
        if self.blink < -0.12 {
            self.blink = rng.range(1.5, 5.0);
        }
        let eyes = if self.joy > 0.0 {
            Eyes::Happy
        } else if panic {
            Eyes::Wide
        } else if self.blink < 0.0 {
            Eyes::Shut
        } else {
            Eyes::Open
        };
        let toward = |p: (f32, f32)| p.0 < me.0;
        let mut face_left = body.facing_left;
        let mut tilt = 0.0;
        if !moving && body.grounded {
            match self.act {
                Act::LookBack => face_left = !face_left,
                Act::Tilt(a) => tilt = a,
                Act::Peck => {
                    let phase = (self.act_left / PECK_BEAT).fract();
                    tilt = 0.6 * (phase * std::f32::consts::PI).sin();
                }
                _ => {}
            }
            if scared && let Some(t) = s.threat {
                face_left = toward(t);
            }
        }
        let shake = if panic {
            0.5
        } else if choking {
            0.5 * (1.0 - s.breath)
        } else {
            0.0
        };
        Look {
            frame: Frame {
                wings,
                legs,
                eyes,
                puffed: scared || s.breath < LOW_AIR,
            },
            face_left,
            tilt,
            squash: self.squash,
            shake,
        }
    }
}
