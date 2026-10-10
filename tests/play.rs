use perch::baddie::Baddie;
use perch::body::{HEIGHT, WIDTH};
use perch::play::{BREATH_SECONDS, Loss, Round};
use perch::seg::Mask;

const W: usize = 240;
const H: usize = 180;
const DT: f32 = 1.0 / 60.0;

fn fill(mask: &mut Mask, x0: usize, y0: usize, x1: usize, y1: usize, on: bool) {
    for y in y0..y1 {
        for x in x0..x1 {
            mask.cells[y * W + x] = on;
        }
    }
}

fn floor() -> Mask {
    let mut mask = Mask::empty(W, H);
    fill(&mut mask, 0, 150, W, H, true);
    mask
}

fn boxed(gap: bool) -> Mask {
    let mut mask = floor();
    fill(&mut mask, 90, 100, 150, 150, true);
    fill(&mut mask, 94, 104, 146, 150, false);
    if gap {
        fill(&mut mask, 146, 120, 150, 140, false);
    }
    mask
}

fn round_in_box(mask: &Mask) -> Round {
    let mut round = Round::new(mask, 7);
    round.body.x = 115.0;
    round.body.y = (150 - HEIGHT) as f32;
    round
}

fn inside_box(round: &Round) -> bool {
    let (x, y) = (round.body.x, round.body.y);
    x >= 94.0 && x + WIDTH as f32 <= 146.0 && y >= 104.0
}

#[test]
fn a_sealed_bird_suffocates() {
    let mask = boxed(false);
    let mut round = round_in_box(&mask);
    for _ in 0..((BREATH_SECONDS + 1.0) / DT) as usize {
        round.step(&mask, None, DT);
        if round.lost.is_some() {
            break;
        }
        assert!(inside_box(&round));
    }
    assert_eq!(round.lost, Some(Loss::Suffocated));
}

#[test]
fn opening_a_gap_refills_the_air() {
    let sealed = boxed(false);
    let open = boxed(true);
    let mut round = round_in_box(&sealed);
    for _ in 0..(3.0 / DT) as usize {
        round.step(&sealed, None, DT);
    }
    let low = round.breath;
    assert!(low < 0.6, "{low}");
    for _ in 0..(2.0 / DT) as usize {
        round.step(&open, None, DT);
    }
    assert!(round.breath > 0.95, "{}", round.breath);
    assert_ne!(round.lost, Some(Loss::Suffocated));
}

#[test]
fn the_bird_wanders_and_stays_on_screen() {
    let mut mask = floor();
    fill(&mut mask, 30, 110, 70, 150, true);
    fill(&mut mask, 170, 90, 200, 150, true);
    let mut round = Round::new(&mask, 3);
    let mut xs = Vec::new();
    for i in 0..(20.0 / DT) as usize {
        round.step(&mask, None, DT);
        round.baddies.clear();
        if i % 60 == 0 {
            xs.push(round.body.x);
        }
        assert!(round.body.y < (H as i32 - HEIGHT) as f32);
    }
    let (lo, hi) = xs
        .iter()
        .fold((f32::MAX, f32::MIN), |(lo, hi), &x| (lo.min(x), hi.max(x)));
    assert!(hi - lo > 40.0, "{lo}..{hi}");
}

#[test]
fn the_bird_heads_for_the_eyes() {
    let mut mask = floor();
    fill(&mut mask, 30, 110, 70, 150, true);
    fill(&mut mask, 170, 90, 200, 150, true);
    let mut round = Round::new(&mask, 5);
    let eyes = (50.0, 90.0);
    let mut near = 0;
    for _ in 0..(20.0 / DT) as usize {
        round.step(&mask, Some(eyes), DT);
        round.baddies.clear();
        let (x, y) = round.body.centre();
        near += ((x - eyes.0).hypot(y - eyes.1) < 40.0) as u32;
    }
    assert!(near as f32 * DT > 8.0, "{}", near as f32 * DT);
}

#[test]
fn baddies_arrive_and_catch_an_exposed_bird() {
    let mask = floor();
    let mut round = Round::new(&mask, 11);
    for _ in 0..(120.0 / DT) as usize {
        round.step(&mask, None, DT);
        if round.lost.is_some() {
            break;
        }
    }
    assert_eq!(round.lost, Some(Loss::Caught));
}

#[test]
fn a_moving_hand_swats_a_baddie() {
    let mut mask = floor();
    let mut round = Round::new(&mask, 13);
    for _ in 0..(6.0 / DT) as usize {
        round.step(&mask, None, DT);
        round.observe(&mask);
    }
    assert!(!round.baddies.is_empty());
    let b = &round.baddies[0];
    let (bx, by) = (b.x.round() as usize, b.y.round() as usize);
    fill(
        &mut mask,
        bx.saturating_sub(10),
        by + 2,
        (bx + 10).min(W),
        (by + 5).min(H),
        true,
    );
    let before = round.baddies.len();
    round.observe(&mask);
    round.step(&mask, None, DT);
    assert!(round.baddies.len() < before);
    assert!(!round.poofs.is_empty());
}

#[test]
fn flicker_at_a_still_edge_does_not_swat() {
    let mut mask = floor();
    fill(&mut mask, 100, 60, 140, 61, true);
    let mut round = Round::new(&mask, 13);
    round.step(&mask, None, DT);
    round.baddies.push(Baddie::new(120.5, 58.5));
    for on in [false, true] {
        fill(&mut mask, 100, 60, 140, 61, on);
        round.observe(&mask);
        round.step(&mask, None, DT);
    }
    assert_eq!(round.baddies.len(), 1);
    assert!(round.poofs.is_empty());
    fill(&mut mask, 100, 56, 140, 57, true);
    round.observe(&mask);
    round.step(&mask, None, DT);
    assert!(round.baddies.is_empty());
    assert_eq!(round.poofs.len(), 1);
}
