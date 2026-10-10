use perch::seg::{CAPTURE_FRAMES, Segmenter};

const W: usize = 240;
const H: usize = 180;

type Rect = (usize, usize, usize, usize);

struct Noise(u64);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) as f32 / (1u64 << 31) as f32) - 0.5
    }
}

fn room(x: usize, y: usize) -> [f32; 3] {
    let (fx, fy) = (x as f32, y as f32);
    let mut c = [150.0 + fy * 0.3, 135.0 + fx * 0.1, 115.0];
    if (x / 30 + y / 45).is_multiple_of(2) {
        c = [c[0] * 0.8, c[1] * 0.9, c[2] * 1.2];
    }
    if (60..110).contains(&x) && (20..90).contains(&y) {
        c = [60.0, 90.0, 140.0];
    }
    if (160..230).contains(&x) && (100..170).contains(&y) {
        c = [120.0, 70.0, 45.0];
    }
    if x.is_multiple_of(17) {
        c = c.map(|v| v * 0.85);
    }
    c
}

struct Scene {
    noise: Noise,
    gain: [f32; 3],
    shadow: Option<(Rect, f32)>,
    camouflage: Option<Rect>,
    objects: Vec<(Rect, [f32; 3])>,
}

impl Scene {
    fn new() -> Self {
        Self {
            noise: Noise(7),
            gain: [1.0; 3],
            shadow: None,
            camouflage: None,
            objects: Vec::new(),
        }
    }

    fn frame(&mut self) -> Vec<u8> {
        let mut out = Vec::with_capacity(W * H * 4);
        for y in 0..H {
            for x in 0..W {
                let inside = |r: Rect| (r.0..r.2).contains(&x) && (r.1..r.3).contains(&y);
                let mut c = room(x, y);
                for &(r, colour) in &self.objects {
                    if inside(r) {
                        c = colour.map(|v| v * if (x + y) % 9 == 0 { 0.9 } else { 1.0 });
                    }
                }
                if self.camouflage.is_some_and(inside) {
                    let weave = if (x / 3 + y / 3) % 2 == 0 { 1.18 } else { 0.84 };
                    c = c.map(|v| v * weave);
                }
                let shade = self.shadow.map_or(1.0, |(r, penumbra)| {
                    let depth = [x as f32 - r.0 as f32, r.2 as f32 - 1.0 - x as f32]
                        .into_iter()
                        .chain([y as f32 - r.1 as f32, r.3 as f32 - 1.0 - y as f32])
                        .fold(f32::INFINITY, f32::min);
                    let t = ((depth + 1.0) / (penumbra + 1.0)).clamp(0.0, 1.0);
                    1.0 - 0.4 * t
                });
                for (v, gain) in c.into_iter().zip(self.gain) {
                    let n = self.noise.next() * 10.0;
                    out.push((v * gain * shade + n).clamp(0.0, 255.0) as u8);
                }
                out.push(255);
            }
        }
        out
    }
}

fn captured(scene: &mut Scene) -> Segmenter {
    let mut seg = Segmenter::new(W, H);
    seg.begin_capture();
    for i in 0..CAPTURE_FRAMES {
        let done = seg.add_capture_frame(&scene.frame());
        assert_eq!(done, i + 1 == CAPTURE_FRAMES);
    }
    assert!(seg.has_background());
    seg
}

fn around(outer: Rect, inner: Rect) -> [Rect; 4] {
    [
        (outer.0, outer.1, outer.2, inner.1),
        (outer.0, inner.3, outer.2, outer.3),
        (outer.0, inner.1, inner.0, inner.3),
        (inner.2, inner.1, outer.2, inner.3),
    ]
}

fn settle(seg: &mut Segmenter, scene: &mut Scene) {
    for _ in 0..4 {
        seg.segment(&scene.frame());
    }
}

fn coverage(seg: &Segmenter, r: Rect) -> f32 {
    let mut hit = 0;
    for y in r.1..r.3 {
        for x in r.0..r.2 {
            hit += seg.mask.cells[y * W + x] as usize;
        }
    }
    hit as f32 / ((r.2 - r.0) * (r.3 - r.1)) as f32
}

fn outside(seg: &Segmenter, rects: &[Rect]) -> f32 {
    let mut hit = 0;
    let mut total = 0;
    for y in 0..H {
        for x in 0..W {
            let near = rects
                .iter()
                .any(|r| x + 2 >= r.0 && x < r.2 + 2 && y + 2 >= r.1 && y < r.3 + 2);
            if !near {
                total += 1;
                hit += seg.mask.cells[y * W + x] as usize;
            }
        }
    }
    hit as f32 / total as f32
}

#[test]
fn nothing_is_solid_before_capture() {
    let mut scene = Scene::new();
    let mut seg = Segmenter::new(W, H);
    scene.objects.push(((10, 10, 50, 50), [250.0, 20.0, 20.0]));
    settle(&mut seg, &mut scene);
    assert!(seg.mask.cells.iter().all(|&s| !s));
}

#[test]
fn empty_busy_room_stays_empty() {
    let mut scene = Scene::new();
    let mut seg = captured(&mut scene);
    for _ in 0..10 {
        seg.segment(&scene.frame());
    }
    assert!(outside(&seg, &[]) < 0.002, "{}", outside(&seg, &[]));
}

#[test]
fn global_lighting_shift_stays_empty() {
    for gain in [0.6, 0.8, 1.25, 1.4] {
        let mut scene = Scene::new();
        let mut seg = captured(&mut scene);
        scene.gain = [gain; 3];
        settle(&mut seg, &mut scene);
        assert!(
            outside(&seg, &[]) < 0.002,
            "gain {gain}: {}",
            outside(&seg, &[])
        );
    }
}

#[test]
fn exposure_and_white_balance_ramp_is_not_foreground() {
    let mut scene = Scene::new();
    let mut seg = captured(&mut scene);
    for step in 1..=30 {
        let t = step as f32 / 30.0;
        scene.gain = [1.0 - 0.45 * t, 1.0 - 0.3 * t, 1.0 - 0.1 * t];
        seg.segment(&scene.frame());
        assert!(
            outside(&seg, &[]) < 0.002,
            "step {step}: {}",
            outside(&seg, &[])
        );
    }
}

#[test]
fn a_large_object_survives_a_white_balance_shift() {
    let mut scene = Scene::new();
    let mut seg = captured(&mut scene);
    let body = (60, 30, 180, 180);
    scene.objects.push((body, [190.0, 50.0, 40.0]));
    scene.gain = [0.8, 0.9, 1.15];
    for _ in 0..30 {
        seg.segment(&scene.frame());
    }
    assert!(coverage(&seg, body) > 0.95, "{}", coverage(&seg, body));
    assert!(outside(&seg, &[body]) < 0.002, "{}", outside(&seg, &[body]));
}

#[test]
fn cast_shadow_is_not_solid() {
    for penumbra in [0.0, 1.5, 3.0, 6.0] {
        let mut scene = Scene::new();
        let mut seg = captured(&mut scene);
        scene.shadow = Some(((40, 60, 140, 150), penumbra));
        settle(&mut seg, &mut scene);
        assert!(
            outside(&seg, &[]) < 0.002,
            "penumbra {penumbra}: {}",
            outside(&seg, &[])
        );
    }
}

#[test]
fn placed_objects_become_solid() {
    let mut scene = Scene::new();
    let mut seg = captured(&mut scene);
    let book = (20, 120, 90, 140);
    let arm = (100, 40, 200, 60);
    scene.objects.push((book, [200.0, 40.0, 40.0]));
    scene.objects.push((arm, [210.0, 160.0, 130.0]));
    settle(&mut seg, &mut scene);
    assert!(coverage(&seg, book) > 0.95, "{}", coverage(&seg, book));
    assert!(coverage(&seg, arm) > 0.9, "{}", coverage(&seg, arm));
    assert!(outside(&seg, &[book, arm]) < 0.002);
}

#[test]
fn dark_object_on_light_wall_is_solid_despite_matching_chroma() {
    let mut scene = Scene::new();
    let mut seg = captured(&mut scene);
    let r = (120, 10, 150, 40);
    let wall = room(130, 20);
    scene.objects.push((r, wall.map(|v| v * 0.3)));
    settle(&mut seg, &mut scene);
    assert!(coverage(&seg, r) > 0.9, "{}", coverage(&seg, r));
}

#[test]
fn a_textured_object_the_colour_of_the_wall_is_solid() {
    let mut scene = Scene::new();
    let mut seg = captured(&mut scene);
    let r = (60, 110, 110, 150);
    scene.camouflage = Some(r);
    settle(&mut seg, &mut scene);
    assert!(coverage(&seg, r) > 0.9, "{}", coverage(&seg, r));
    assert!(outside(&seg, &[r]) < 0.002, "{}", outside(&seg, &[r]));
}

#[test]
fn a_still_object_is_not_absorbed_into_the_background() {
    let mut scene = Scene::new();
    let mut seg = captured(&mut scene);
    let r = (30, 30, 70, 60);
    let camouflaged = (60, 110, 110, 150);
    scene.objects.push((r, [40.0, 180.0, 60.0]));
    scene.camouflage = Some(camouflaged);
    for _ in 0..600 {
        seg.segment(&scene.frame());
    }
    assert!(coverage(&seg, r) > 0.95, "{}", coverage(&seg, r));
    assert!(
        coverage(&seg, camouflaged) > 0.9,
        "{}",
        coverage(&seg, camouflaged)
    );
}

#[test]
fn a_moving_object_leaves_no_trail() {
    let mut scene = Scene::new();
    let mut seg = captured(&mut scene);
    let mut r = (0, 0, 0, 0);
    for step in 0..40 {
        r = (10 + step * 4, 80, 50 + step * 4, 110);
        scene.objects = vec![(r, [30.0, 200.0, 220.0])];
        seg.segment(&scene.frame());
    }
    settle(&mut seg, &mut scene);
    assert!(coverage(&seg, r) > 0.95);
    assert!(outside(&seg, &[r]) < 0.002, "{}", outside(&seg, &[r]));
}

#[test]
fn recapture_absorbs_what_was_placed() {
    let mut scene = Scene::new();
    let mut seg = captured(&mut scene);
    let r = (30, 30, 70, 60);
    scene.objects.push((r, [40.0, 180.0, 60.0]));
    settle(&mut seg, &mut scene);
    assert!(coverage(&seg, r) > 0.95);
    seg = captured(&mut scene);
    settle(&mut seg, &mut scene);
    assert!(coverage(&seg, r) < 0.01);
}

#[test]
fn a_one_frame_speck_is_never_a_platform() {
    let mut scene = Scene::new();
    let mut seg = captured(&mut scene);
    let speck = (100, 100, 110, 110);
    settle(&mut seg, &mut scene);
    scene.objects.push((speck, [250.0, 250.0, 30.0]));
    seg.segment(&scene.frame());
    scene.objects.clear();
    for _ in 0..4 {
        assert!(coverage(&seg, speck) == 0.0, "{}", coverage(&seg, speck));
        seg.segment(&scene.frame());
    }
}

#[test]
fn solid_needs_two_frames_and_clearing_needs_three() {
    let mut scene = Scene::new();
    let mut seg = captured(&mut scene);
    let r = (100, 100, 130, 130);
    scene.objects.push((r, [250.0, 250.0, 30.0]));
    seg.segment(&scene.frame());
    assert!(coverage(&seg, r) == 0.0, "{}", coverage(&seg, r));
    seg.segment(&scene.frame());
    assert!(coverage(&seg, r) > 0.95, "{}", coverage(&seg, r));
    scene.objects.clear();
    for _ in 0..2 {
        seg.segment(&scene.frame());
        assert!(coverage(&seg, r) > 0.95, "{}", coverage(&seg, r));
    }
    seg.segment(&scene.frame());
    assert!(coverage(&seg, r) == 0.0, "{}", coverage(&seg, r));
}

#[test]
fn specks_too_small_to_stand_on_are_dropped() {
    let mut scene = Scene::new();
    let mut seg = captured(&mut scene);
    let speck = (100, 100, 105, 105);
    let ledge = (40, 40, 46, 45);
    scene.objects.push((speck, [250.0, 250.0, 30.0]));
    scene.objects.push((ledge, [250.0, 250.0, 30.0]));
    settle(&mut seg, &mut scene);
    assert!(coverage(&seg, speck) == 0.0, "{}", coverage(&seg, speck));
    assert!(coverage(&seg, ledge) > 0.95, "{}", coverage(&seg, ledge));
}

#[test]
fn a_hole_too_small_for_the_bird_is_filled_even_around_a_speck() {
    let mut scene = Scene::new();
    let mut seg = captured(&mut scene);
    let body = (60, 40, 140, 140);
    let hole = (90, 70, 97, 77);
    let wall = room(93, 73);
    let luma = wall[0] + wall[1] + wall[2];
    let same_luma = [luma - 200.0, 100.0, 100.0];
    for part in around(body, hole) {
        scene.objects.push((part, same_luma));
    }
    scene.objects.push(((92, 72, 95, 75), same_luma));
    settle(&mut seg, &mut scene);
    assert!(coverage(&seg, hole) == 1.0, "{}", coverage(&seg, hole));
    assert!(coverage(&seg, body) > 0.9, "{}", coverage(&seg, body));
}

#[test]
fn a_pocket_the_bird_fits_in_stays_open() {
    let mut scene = Scene::new();
    let mut seg = captured(&mut scene);
    let hands = (60, 40, 140, 140);
    let pocket = (90, 70, 102, 82);
    for part in around(hands, pocket) {
        scene.objects.push((part, [40.0, 50.0, 70.0]));
    }
    settle(&mut seg, &mut scene);
    assert!(coverage(&seg, pocket) == 0.0, "{}", coverage(&seg, pocket));
}
