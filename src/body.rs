use crate::seg::Mask;

pub const WIDTH: i32 = 7;
pub const HEIGHT: i32 = 8;

const GRAVITY: f32 = 620.0;
const HELD_JUMP_GRAVITY: f32 = 340.0;
const JUMP_SPEED: f32 = 190.0;
const RUN_SPEED: f32 = 75.0;
const RUN_ACCEL: f32 = 700.0;
const MAX_FALL: f32 = 260.0;
const STEP_UP: i32 = 3;
const SNAP_DOWN: i32 = 3;
const COYOTE: f32 = 0.08;
const JUMP_BUFFER: f32 = 0.1;

#[derive(Clone, Copy, Default)]
pub struct Input {
    pub run: f32,
    pub jump_pressed: bool,
    pub jump_held: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pose {
    Idle,
    Run,
    Jump,
    Fall,
}

pub struct Body {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub grounded: bool,
    pub facing_left: bool,
    coyote: f32,
    buffered_jump: f32,
}

impl Body {
    pub fn spawn(mask: &Mask) -> Self {
        Self {
            x: (mask.width as i32 - WIDTH) as f32 / 2.0,
            y: -HEIGHT as f32,
            vx: 0.0,
            vy: 0.0,
            grounded: false,
            facing_left: false,
            coyote: 0.0,
            buffered_jump: 0.0,
        }
    }

    pub fn pose(&self) -> Pose {
        if self.grounded {
            if self.vx.abs() > 5.0 {
                Pose::Run
            } else {
                Pose::Idle
            }
        } else if self.vy < 0.0 {
            Pose::Jump
        } else {
            Pose::Fall
        }
    }

    fn cell(&self) -> (i32, i32) {
        (self.x.round() as i32, self.y.round() as i32)
    }

    pub fn step(&mut self, mask: &Mask, input: Input, dt: f32) {
        let dt = dt.min(1.0 / 30.0);
        self.depenetrate(mask);

        let target = input.run.clamp(-1.0, 1.0) * RUN_SPEED;
        let dv = RUN_ACCEL * dt;
        self.vx += (target - self.vx).clamp(-dv, dv);
        if input.run.abs() > 0.1 {
            self.facing_left = input.run < 0.0;
        }

        self.buffered_jump = if input.jump_pressed {
            JUMP_BUFFER
        } else {
            (self.buffered_jump - dt).max(0.0)
        };
        if self.buffered_jump > 0.0 && (self.grounded || self.coyote > 0.0) {
            self.vy = -JUMP_SPEED;
            self.grounded = false;
            self.coyote = 0.0;
            self.buffered_jump = 0.0;
        }

        let gravity = if self.vy < 0.0 && input.jump_held {
            HELD_JUMP_GRAVITY
        } else {
            GRAVITY
        };
        self.vy = (self.vy + gravity * dt).min(MAX_FALL);

        let was_grounded = self.grounded;
        self.move_x(mask, self.vx * dt);
        self.move_y(mask, self.vy * dt);

        if was_grounded && self.vy >= 0.0 && !self.on_ground(mask) {
            let (cx, cy) = self.cell();
            if let Some(d) = (1..=SNAP_DOWN).find(|&d| overlaps(mask, cx, cy + d + 1)) {
                self.y = (cy + d) as f32;
            }
        }

        self.grounded = self.vy >= 0.0 && self.on_ground(mask);
        if self.grounded {
            self.vy = 0.0;
            self.coyote = COYOTE;
        } else {
            self.coyote = (self.coyote - dt).max(0.0);
        }

        if self.y > mask.height as f32 {
            *self = Self::spawn(mask);
        }
    }

    fn on_ground(&self, mask: &Mask) -> bool {
        let (cx, cy) = self.cell();
        overlaps(mask, cx, cy + 1)
    }

    /// A solid that moves into the body lifts it to the nearest free row above.
    fn depenetrate(&mut self, mask: &Mask) {
        let (cx, cy) = self.cell();
        if !overlaps(mask, cx, cy) {
            return;
        }
        if let Some(d) = (1..=cy + HEIGHT).find(|&d| !overlaps(mask, cx, cy - d)) {
            self.y = (cy - d) as f32;
            self.vy = self.vy.min(0.0);
        }
    }

    fn move_x(&mut self, mask: &Mask, dx: f32) {
        let max_x = (mask.width as i32 - WIDTH) as f32;
        let mut left = dx;
        while left.abs() > 0.0 {
            let s = left.clamp(-1.0, 1.0);
            left -= s;
            let nx = (self.x + s).clamp(0.0, max_x);
            let (_, cy) = self.cell();
            let ncx = nx.round() as i32;
            if !overlaps(mask, ncx, cy) {
                self.x = nx;
            } else if let Some(up) = (1..=STEP_UP).find(|&u| !overlaps(mask, ncx, cy - u)) {
                self.x = nx;
                self.y = (cy - up) as f32;
            } else {
                self.vx = 0.0;
                return;
            }
        }
    }

    fn move_y(&mut self, mask: &Mask, dy: f32) {
        let mut left = dy;
        while left.abs() > 0.0 {
            let s = left.clamp(-1.0, 1.0);
            left -= s;
            let (cx, _) = self.cell();
            let ny = self.y + s;
            if overlaps(mask, cx, ny.round() as i32) {
                self.y = self.y.round();
                self.vy = 0.0;
                return;
            }
            self.y = ny;
        }
    }
}

fn overlaps(mask: &Mask, x: i32, y: i32) -> bool {
    (y..y + HEIGHT).any(|cy| (x..x + WIDTH).any(|cx| mask.solid(cx, cy)))
}
