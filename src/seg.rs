pub const CAPTURE_FRAMES: usize = 9;

const CHROMA_BIAS: f32 = 12.0;
const CHROMA_FLOOR: f32 = 0.04;
const CHROMA_SPREAD: f32 = 3.0;
const LUMA_BIAS: f32 = 24.0;
const CHANNEL_BIAS: f32 = LUMA_BIAS / 3.0;
const SHADOW_FLOOR: f32 = 0.5;
const HIGHLIGHT_CEIL: f32 = 1.45;
const ADAPT_RATE: f32 = 0.03;

pub struct Mask {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<bool>,
}

impl Mask {
    pub fn empty(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            cells: vec![false; width * height],
        }
    }

    pub fn solid(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return false;
        }
        self.cells[y as usize * self.width + x as usize]
    }
}

enum State {
    Empty,
    Capturing(Vec<Vec<u8>>),
    Ready(Vec<Model>),
}

struct Model {
    rgb: [f32; 3],
    chroma_tolerance: f32,
}

pub struct Segmenter {
    width: usize,
    height: usize,
    state: State,
    raw: Vec<u8>,
    ratios: Vec<f32>,
    scratch: Vec<u8>,
    pub mask: Mask,
}

impl Segmenter {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            state: State::Empty,
            raw: vec![0; width * height],
            ratios: Vec::new(),
            scratch: vec![0; width * height],
            mask: Mask::empty(width, height),
        }
    }

    pub fn has_background(&self) -> bool {
        matches!(self.state, State::Ready(_))
    }

    pub fn begin_capture(&mut self) {
        self.state = State::Capturing(Vec::with_capacity(CAPTURE_FRAMES));
        self.mask.cells.fill(false);
    }

    pub fn add_capture_frame(&mut self, rgba: &[u8]) -> bool {
        let State::Capturing(captured) = &mut self.state else {
            return self.has_background();
        };
        captured.push(rgba.to_vec());
        if captured.len() < CAPTURE_FRAMES {
            return false;
        }
        let mut channel = [0u8; CAPTURE_FRAMES];
        let background = (0..self.width * self.height)
            .map(|i| {
                let rgb = std::array::from_fn(|c| {
                    for (slot, frame) in channel.iter_mut().zip(captured.iter()) {
                        *slot = frame[i * 4 + c];
                    }
                    channel.sort_unstable();
                    channel[CAPTURE_FRAMES / 2] as f32
                });
                let centre = chroma(rgb);
                let spread = captured
                    .iter()
                    .map(|f| chroma_shift(chroma(pixel(f, i)), centre))
                    .sum::<f32>()
                    / CAPTURE_FRAMES as f32;
                Model {
                    rgb,
                    chroma_tolerance: (CHROMA_SPREAD * spread).max(CHROMA_FLOOR),
                }
            })
            .collect();
        self.state = State::Ready(background);
        true
    }

    pub fn segment(&mut self, rgba: &[u8]) {
        let State::Ready(background) = &mut self.state else {
            self.mask.cells.fill(false);
            return;
        };
        let gain = global_gain(rgba, background, &mut self.ratios);
        for (i, model) in background.iter_mut().enumerate() {
            let px = pixel(rgba, i);
            let expected = std::array::from_fn(|c| (model.rgb[c] * gain[c]).min(255.0));
            let solid = differs(px, expected, model.chroma_tolerance);
            self.raw[i] = solid as u8;
            if !solid {
                for c in 0..3 {
                    model.rgb[c] += ADAPT_RATE * (px[c] / gain[c] - model.rgb[c]);
                }
            }
        }
        for op in [Op::Erode, Op::Dilate, Op::Dilate, Op::Erode] {
            morph(
                &mut self.raw,
                &mut self.scratch,
                self.width,
                self.height,
                op,
            );
        }
        for (cell, &r) in self.mask.cells.iter_mut().zip(&self.raw) {
            *cell = r != 0;
        }
    }
}

/// Exposure and white-balance drift scale each channel of every background
/// pixel by roughly the same factor, so the per-channel median ratio over the
/// frame recovers it while foreground stays a minority.
fn global_gain(rgba: &[u8], background: &[Model], ratios: &mut Vec<f32>) -> [f32; 3] {
    std::array::from_fn(|c| {
        ratios.clear();
        ratios.extend(background.iter().enumerate().step_by(7).map(|(i, model)| {
            (rgba[i * 4 + c] as f32 + CHANNEL_BIAS) / (model.rgb[c] + CHANNEL_BIAS)
        }));
        if ratios.is_empty() {
            return 1.0;
        }
        let mid = ratios.len() / 2;
        let (_, median, _) = ratios.select_nth_unstable_by(mid, f32::total_cmp);
        median.clamp(0.25, 4.0)
    })
}

/// Shadows and highlights move luma but keep chromaticity, so a pixel counts
/// as foreground only on a chroma shift or a luma change too large for a shadow.
fn differs(px: [f32; 3], bg: [f32; 3], chroma_tolerance: f32) -> bool {
    chroma_shift(chroma(px), chroma(bg)) > chroma_tolerance
        || !(SHADOW_FLOOR..=HIGHLIGHT_CEIL).contains(&luma_ratio(px, bg))
}

fn pixel(rgba: &[u8], i: usize) -> [f32; 3] {
    [
        rgba[i * 4] as f32,
        rgba[i * 4 + 1] as f32,
        rgba[i * 4 + 2] as f32,
    ]
}

fn chroma(p: [f32; 3]) -> [f32; 3] {
    let sum = p[0] + p[1] + p[2] + 3.0 * CHROMA_BIAS;
    p.map(|c| (c + CHROMA_BIAS) / sum)
}

fn chroma_shift(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).abs() + (a[1] - b[1]).abs() + (a[2] - b[2]).abs()
}

fn luma_ratio(px: [f32; 3], bg: [f32; 3]) -> f32 {
    (px[0] + px[1] + px[2] + LUMA_BIAS) / (bg[0] + bg[1] + bg[2] + LUMA_BIAS)
}

#[derive(Clone, Copy)]
enum Op {
    Erode,
    Dilate,
}

fn morph(cells: &mut [u8], scratch: &mut [u8], w: usize, h: usize, op: Op) {
    let pick = |a: u8, b: u8| match op {
        Op::Erode => a.min(b),
        Op::Dilate => a.max(b),
    };
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let mut v = cells[i];
            if x > 0 {
                v = pick(v, cells[i - 1]);
            }
            if x + 1 < w {
                v = pick(v, cells[i + 1]);
            }
            scratch[i] = v;
        }
    }
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let mut v = scratch[i];
            if y > 0 {
                v = pick(v, scratch[i - w]);
            }
            if y + 1 < h {
                v = pick(v, scratch[i + w]);
            }
            cells[i] = v;
        }
    }
}
