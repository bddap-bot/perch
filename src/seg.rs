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
/// As a fraction of the empty scene's 5th-to-95th percentile depth range.
const NEAR_MARGIN: f32 = 0.3;
/// Summed RGB difference beyond which a cell has changed since its depth was measured.
const MOVED: f32 = 36.0;
const DEPTH_GAIN_STEP: f32 = 0.05;
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

/// Relative inverse depth, larger nearer, of a camera frame and of the empty
/// scene rendered under that frame's exposure, with the frame's pixels.
pub struct DepthEstimate {
    pub frame: Vec<f32>,
    pub background: Vec<f32>,
    pub pixels: Vec<u8>,
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
    pairs: Vec<(f32, f32)>,
    gain: [f32; 3],
    depth_gain: Option<[f32; 3]>,
    depth: Option<DepthEstimate>,
    quantiles: Vec<f32>,
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
            pairs: Vec::new(),
            gain: [1.0; 3],
            depth_gain: None,
            depth: None,
            quantiles: Vec::new(),
            mask: Mask::empty(width, height),
        }
    }

    pub fn has_background(&self) -> bool {
        matches!(self.state, State::Ready(_))
    }

    pub fn begin_capture(&mut self) {
        self.forget();
        self.state = State::Capturing(Vec::with_capacity(CAPTURE_FRAMES));
    }

    /// Takes no frames until `begin_capture`, so the player can step out of view.
    pub fn forget(&mut self) {
        self.state = State::Empty;
        self.mask.cells.fill(false);
        self.held.fill(false);
        self.run.fill(0);
        self.gain = [1.0; 3];
        self.depth_gain = None;
        self.depth = None;
    }

    /// Monocular depth shifts with exposure, so the empty scene's depth is
    /// measured again, from the background model under the current gain,
    /// after each capture and whenever the gain has moved since.
    pub fn depth_background(&mut self) -> Option<Vec<u8>> {
        let State::Ready(background) = &self.state else {
            return None;
        };
        let gain = self.gain;
        if self
            .depth_gain
            .is_some_and(|then| (0..3).all(|c| (gain[c] / then[c] - 1.0).abs() < DEPTH_GAIN_STEP))
        {
            return None;
        }
        self.depth_gain = Some(gain);
        Some(
            background
                .iter()
                .flat_map(|model| {
                    let [r, g, b] = std::array::from_fn(|c| (model.rgb[c] * gain[c]).min(255.0));
                    [r as u8, g as u8, b as u8, 255]
                })
                .collect(),
        )
    }

    pub fn observe_depth(&mut self, estimate: DepthEstimate) {
        let n = self.width * self.height;
        let sizes = (
            estimate.frame.len(),
            estimate.background.len(),
            estimate.pixels.len(),
        );
        if self.has_background() && sizes == (n, n, 4 * n) {
            self.depth = Some(estimate);
        }
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

    pub fn segment(&mut self, rgba: &[u8], person_mask: Option<&[u8]>) {
        let State::Ready(background) = &mut self.state else {
            self.mask.cells.fill(false);
            return;
        };
        let (w, h) = (self.width, self.height);
        if let Some(mask) = person_mask {
            assert_eq!(mask.len(), w * h);
        }
        let person = |i: usize| person_mask.is_some_and(|m| m[i] != 0);
        let gain = global_gain(rgba, background, person, &mut self.ratios);
        self.gain = gain;
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
            self.raw[i] = (coloured || person(i)) as u8;
            self.textured[i] = textured as u8;
            if !coloured && !textured {
                for c in 0..3 {
                    model.rgb[c] += ADAPT_RATE * (px[c] / gain[c] - model.rgb[c]);
                }
            }
        }
        if let Some(depth) = &self.depth {
            self.quantiles.clear();
            self.quantiles.extend(depth.background.iter().step_by(7));
            self.quantiles.sort_unstable_by(f32::total_cmp);
            let at = |q: f32| self.quantiles[((self.quantiles.len() - 1) as f32 * q) as usize];
            let range = at(0.95) - at(0.05);
            self.pairs.clear();
            self.pairs.extend(
                (0..w * h)
                    .step_by(3)
                    .filter(|&i| self.raw[i] == 0 && self.textured[i] == 0)
                    .map(|i| (depth.background[i], depth.frame[i])),
            );
            if let Some((scale, shift)) = fit(&mut self.pairs, range) {
                let margin = NEAR_MARGIN * range * scale;
                for (i, raw) in self.raw.iter_mut().enumerate() {
                    let [a, b] = [pixel(rgba, i), pixel(&depth.pixels, i)];
                    let still = (0..3).map(|c| (a[c] - b[c]).abs()).sum::<f32>() < MOVED;
                    let expected = scale * depth.background[i] + shift;
                    if still && depth.frame[i] - expected > margin {
                        *raw = 1;
                    }
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
        for op in [Op::Erode, Op::Dilate, Op::Dilate, Op::Erode] {
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

/// Least-squares scale and shift taking the empty scene's depth to the frame's,
/// refit without the samples the first fit leaves more than half the near
/// margin off, which are mostly things that came nearer.
fn fit(pairs: &mut Vec<(f32, f32)>, range: f32) -> Option<(f32, f32)> {
    let solve = |pairs: &[(f32, f32)]| {
        let n = pairs.len() as f32;
        let (sx, sy, sxx, sxy) = pairs.iter().fold((0.0, 0.0, 0.0, 0.0), |a, &(x, y)| {
            (a.0 + x, a.1 + y, a.2 + x * x, a.3 + x * y)
        });
        let scale = (n * sxy - sx * sy) / (n * sxx - sx * sx);
        (pairs.len() >= 100 && scale > 0.0).then(|| (scale, (sy - scale * sx) / n))
    };
    let (scale, shift) = solve(pairs)?;
    pairs.retain(|&(x, y)| (y - scale * x - shift).abs() < 0.5 * NEAR_MARGIN * range * scale);
    solve(pairs)
}

/// Exposure and white-balance drift scale each channel of every background
/// pixel by roughly the same factor, so the per-channel median ratio over the
/// frame recovers it while foreground stays a minority.
fn global_gain(
    rgba: &[u8],
    background: &[Model],
    person: impl Fn(usize) -> bool,
    ratios: &mut Vec<f32>,
) -> [f32; 3] {
    std::array::from_fn(|c| {
        ratios.clear();
        ratios.extend(
            background
                .iter()
                .enumerate()
                .step_by(7)
                .filter(|&(i, _)| !person(i))
                .map(|(i, model)| {
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
