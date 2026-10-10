pub const CAPTURE_FRAMES: usize = 9;

const CHROMA_BIAS: f32 = 12.0;
const CHROMA_FLOOR: f32 = 0.04;
const CHROMA_SPREAD: f32 = 3.0;
const LUMA_BIAS: f32 = 24.0;
const CHANNEL_BIAS: f32 = LUMA_BIAS / 3.0;
const SHADOW_FLOOR: f32 = 0.5;
const HIGHLIGHT_CEIL: f32 = 1.45;
const TEXTURE_FLOOR: f32 = 0.05;
const TEXTURE_SPREAD: f32 = 3.0;
const ADAPT_RATE: f32 = 0.03;
const RISE: u8 = 2;
const FALL: u8 = 3;
const MIN_SOLID: usize = 30;
const MAX_HOLE: usize = 50;

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
    texture: f32,
    texture_tolerance: f32,
}

pub struct Segmenter {
    width: usize,
    height: usize,
    state: State,
    raw: Vec<u8>,
    textured: Vec<u8>,
    texture: Vec<f32>,
    luma: Vec<f32>,
    blur: Vec<f32>,
    ratios: Vec<f32>,
    scratch: Vec<u8>,
    held: Vec<bool>,
    run: Vec<u8>,
    region: Vec<usize>,
    pub mask: Mask,
}

impl Segmenter {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            state: State::Empty,
            raw: vec![0; width * height],
            textured: vec![0; width * height],
            texture: vec![0.0; width * height],
            luma: vec![0.0; width * height],
            blur: vec![0.0; width * height],
            ratios: Vec::new(),
            scratch: vec![0; width * height],
            held: vec![false; width * height],
            run: vec![0; width * height],
            region: Vec::new(),
            mask: Mask::empty(width, height),
        }
    }

    pub fn has_background(&self) -> bool {
        matches!(self.state, State::Ready(_))
    }

    pub fn begin_capture(&mut self) {
        self.state = State::Capturing(Vec::with_capacity(CAPTURE_FRAMES));
        self.mask.cells.fill(false);
        self.held.fill(false);
        self.run.fill(0);
    }

    pub fn add_capture_frame(&mut self, rgba: &[u8]) -> bool {
        let State::Capturing(captured) = &mut self.state else {
            return self.has_background();
        };
        captured.push(rgba.to_vec());
        if captured.len() < CAPTURE_FRAMES {
            return false;
        }
        let (w, h) = (self.width, self.height);
        let textures: Vec<Vec<f32>> = captured
            .iter()
            .map(|f| {
                let mut t = vec![0.0; w * h];
                texture(f, 1.0, w, h, &mut t, &mut self.luma, &mut self.blur);
                t
            })
            .collect();
        let mut channel = [0u8; CAPTURE_FRAMES];
        let background = (0..w * h)
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
                let mean = textures.iter().map(|t| t[i]).sum::<f32>() / CAPTURE_FRAMES as f32;
                let deviation = textures.iter().map(|t| (t[i] - mean).abs()).sum::<f32>()
                    / CAPTURE_FRAMES as f32;
                Model {
                    rgb,
                    chroma_tolerance: (CHROMA_SPREAD * spread).max(CHROMA_FLOOR),
                    texture: mean,
                    texture_tolerance: (TEXTURE_SPREAD * deviation).max(TEXTURE_FLOOR),
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
        let (w, h) = (self.width, self.height);
        let gain = global_gain(rgba, background, &mut self.ratios);
        let mean_gain = (gain[0] + gain[1] + gain[2]) / 3.0;
        texture(
            rgba,
            mean_gain,
            w,
            h,
            &mut self.texture,
            &mut self.luma,
            &mut self.blur,
        );
        for (i, model) in background.iter_mut().enumerate() {
            let px = pixel(rgba, i);
            let expected = std::array::from_fn(|c| (model.rgb[c] * gain[c]).min(255.0));
            let coloured = differs(px, expected, model.chroma_tolerance);
            let textured = (self.texture[i] - model.texture).abs() > model.texture_tolerance;
            self.raw[i] = coloured as u8;
            self.textured[i] = textured as u8;
            if !coloured && !textured {
                for c in 0..3 {
                    model.rgb[c] += ADAPT_RATE * (px[c] / gain[c] - model.rgb[c]);
                }
            }
        }
        // A cast shadow's edge is new structure too, but only a few cells wide.
        for op in [
            Op::Erode,
            Op::Erode,
            Op::Erode,
            Op::Dilate,
            Op::Dilate,
            Op::Dilate,
        ] {
            morph(&mut self.textured, &mut self.scratch, w, h, op);
        }
        for (i, &t) in self.textured.iter().enumerate() {
            let fg = self.raw[i] | t != 0;
            if fg == self.held[i] {
                self.run[i] = 0;
            } else {
                self.run[i] += 1;
                if self.run[i] >= if fg { RISE } else { FALL } {
                    self.held[i] = fg;
                    self.run[i] = 0;
                }
            }
            self.raw[i] = self.held[i] as u8;
        }
        for op in [Op::Erode, Op::Dilate] {
            morph(&mut self.raw, &mut self.scratch, w, h, op);
        }
        for (cell, &r) in self.mask.cells.iter_mut().zip(&self.raw) {
            *cell = r != 0;
        }
        let cells = &mut self.mask.cells;
        for solid in [true, false] {
            self.scratch.fill(0);
            for i in 0..w * h {
                if self.scratch[i] != 0 || cells[i] != solid {
                    continue;
                }
                let touches_edge = flood(cells, &mut self.scratch, &mut self.region, w, h, i);
                let small = if solid {
                    self.region.len() < MIN_SOLID
                } else {
                    !touches_edge && self.region.len() < MAX_HOLE
                };
                if small {
                    for &j in &self.region {
                        cells[j] = !solid;
                    }
                }
            }
        }
    }
}

/// Exposure and white-balance drift scale each channel of every background
/// pixel by roughly the same factor, so the per-channel median ratio over the
/// frame recovers it while foreground stays a minority.
fn global_gain(rgba: &[u8], background: &[Model], ratios: &mut Vec<f32>) -> [f32; 3] {
    std::array::from_fn(|c| {
        ratios.clear();
        ratios.extend(
            background.iter().enumerate().step_by(7).map(|(i, model)| {
                (pixel(rgba, i)[c] + CHANNEL_BIAS) / (model.rgb[c] + CHANNEL_BIAS)
            }),
        );
        if ratios.is_empty() {
            return 1.0;
        }
        let mid = ratios.len() / 2;
        let (_, median, _) = ratios.select_nth_unstable_by(mid, f32::total_cmp);
        median.clamp(0.25, 4.0)
    })
}

/// How far log luma strays from its 5x5 mean, averaged over 3x3. Log luma
/// makes it blind to exposure, and a smooth shading ramp, such as a soft
/// shadow's edge, equals its local mean.
fn texture(
    rgba: &[u8],
    gain: f32,
    w: usize,
    h: usize,
    out: &mut [f32],
    luma: &mut [f32],
    blur: &mut [f32],
) {
    for (i, l) in luma.iter_mut().enumerate() {
        let [r, g, b] = pixel(rgba, i);
        *l = (r + g + b + LUMA_BIAS * gain).ln();
    }
    out.copy_from_slice(luma);
    box_mean(out, blur, w, h, 2);
    for (o, l) in out.iter_mut().zip(luma.iter()) {
        *o = (l - *o).abs();
    }
    box_mean(out, blur, w, h, 1);
}

fn box_mean(cells: &mut [f32], blur: &mut [f32], w: usize, h: usize, r: usize) {
    for y in 0..h {
        let row = &cells[y * w..y * w + w];
        for x in 0..w {
            let (a, b) = (x.saturating_sub(r), (x + r).min(w - 1));
            blur[y * w + x] = row[a..=b].iter().sum::<f32>() / (b - a + 1) as f32;
        }
    }
    for y in 0..h {
        let (a, b) = (y.saturating_sub(r), (y + r).min(h - 1));
        for x in 0..w {
            cells[y * w + x] = (a..=b).map(|r| blur[r * w + x]).sum::<f32>() / (b - a + 1) as f32;
        }
    }
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

/// Solid regions join diagonally and gaps do not, so a thin diagonal limb
/// stays one region and a gap sealed at a corner still counts as enclosed.
fn flood(
    cells: &[bool],
    seen: &mut [u8],
    region: &mut Vec<usize>,
    w: usize,
    h: usize,
    start: usize,
) -> bool {
    let value = cells[start];
    region.clear();
    region.push(start);
    seen[start] = 1;
    let mut edge = false;
    let mut next = 0;
    while let Some(&i) = region.get(next) {
        next += 1;
        let (x, y) = (i % w, i / w);
        edge |= x == 0 || y == 0 || x + 1 == w || y + 1 == h;
        for (dx, dy) in [
            (-1, -1),
            (0, -1),
            (1, -1),
            (-1, 0),
            (1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
        ] {
            let (nx, ny) = (x as isize + dx, y as isize + dy);
            if (dx != 0 && dy != 0 && !value)
                || nx < 0
                || ny < 0
                || nx >= w as isize
                || ny >= h as isize
            {
                continue;
            }
            let j = ny as usize * w + nx as usize;
            if seen[j] == 0 && cells[j] == value {
                seen[j] = 1;
                region.push(j);
            }
        }
    }
    edge
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
