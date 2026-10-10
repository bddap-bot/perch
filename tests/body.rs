use perch::body::{Body, HEIGHT, Input, Pose, WIDTH};
use perch::seg::Mask;

const W: usize = 120;
const H: usize = 90;
const DT: f32 = 1.0 / 60.0;

fn fill(mask: &mut Mask, x0: i32, y0: i32, x1: i32, y1: i32) {
    for y in y0..y1 {
        for x in x0..x1 {
            mask.cells[y as usize * W + x as usize] = true;
        }
    }
}

fn inside(mask: &Mask, b: &Body) -> bool {
    let (x, y) = (b.x.round() as i32, b.y.round() as i32);
    (y..y + HEIGHT).any(|cy| (x..x + WIDTH).any(|cx| mask.solid(cx, cy)))
}

fn idle() -> Input {
    Input::default()
}

#[test]
fn falls_and_lands_on_top_of_a_platform() {
    let mut mask = Mask::empty(W, H);
    fill(&mut mask, 40, 60, 80, 70);
    let mut b = Body::spawn(&mask);
    for _ in 0..180 {
        b.step(&mask, idle(), DT);
        assert!(!inside(&mask, &b));
    }
    assert!(b.grounded);
    assert_eq!(b.y.round() as i32, 60 - HEIGHT);
    assert_eq!(b.pose(), Pose::Idle);
}

#[test]
fn a_one_pixel_ledge_stops_a_terminal_fall() {
    let mut mask = Mask::empty(W, H);
    fill(&mut mask, 0, 85, W as i32, 86);
    let mut b = Body::spawn(&mask);
    b.vy = 10_000.0;
    for _ in 0..240 {
        b.step(&mask, idle(), DT);
        assert!(!inside(&mask, &b));
    }
    assert!(b.grounded);
    assert_eq!(b.y.round() as i32, 85 - HEIGHT);
}

#[test]
fn a_rising_solid_lifts_the_body_without_overlap() {
    let mut b = None;
    for top in (20..80).rev() {
        let mut mask = Mask::empty(W, H);
        fill(&mut mask, 30, top, 90, H as i32);
        let body = b.get_or_insert_with(|| Body::spawn(&mask));
        body.step(&mask, idle(), DT);
        assert!(!inside(&mask, body), "top {top}");
    }
    let body = b.unwrap();
    assert!(body.grounded);
    assert_eq!(body.y.round() as i32, 20 - HEIGHT);
}

#[test]
fn a_solid_appearing_over_the_body_pops_it_out_on_top() {
    let mut mask = Mask::empty(W, H);
    fill(&mut mask, 0, 80, W as i32, H as i32);
    let mut b = Body::spawn(&mask);
    for _ in 0..120 {
        b.step(&mask, idle(), DT);
    }
    let x = b.x.round() as i32;
    fill(&mut mask, x - 5, 50, x + 15, 80);
    b.step(&mask, idle(), DT);
    assert!(!inside(&mask, &b));
    assert_eq!(b.y.round() as i32, 50 - HEIGHT);
}

#[test]
fn falling_off_the_bottom_respawns_at_the_top() {
    let mask = Mask::empty(W, H);
    let mut b = Body::spawn(&mask);
    let mut lowest = f32::MIN;
    let mut respawned = false;
    for _ in 0..300 {
        let before = b.y;
        b.step(&mask, idle(), DT);
        lowest = lowest.max(b.y);
        respawned |= b.y < before;
    }
    assert!(respawned);
    assert!(lowest <= H as f32 + 5.0);
}

#[test]
fn runs_up_small_steps_and_stops_at_walls() {
    let mut mask = Mask::empty(W, H);
    fill(&mut mask, 0, 80, W as i32, H as i32);
    fill(&mut mask, 70, 78, W as i32, 80);
    fill(&mut mask, 100, 40, W as i32, 78);
    let mut b = Body::spawn(&mask);
    b.x = 10.0;
    let right = Input {
        run: 1.0,
        ..Input::default()
    };
    for _ in 0..300 {
        b.step(&mask, right, DT);
        assert!(!inside(&mask, &b));
    }
    assert!(b.grounded);
    assert_eq!(b.y.round() as i32, 78 - HEIGHT);
    assert_eq!(b.x.round() as i32, 100 - WIDTH);
}

#[test]
fn jumps_onto_a_higher_ledge() {
    let mut mask = Mask::empty(W, H);
    fill(&mut mask, 0, 80, W as i32, H as i32);
    fill(&mut mask, 60, 60, W as i32, 80);
    let mut b = Body::spawn(&mask);
    b.x = 20.0;
    for _ in 0..120 {
        b.step(&mask, idle(), DT);
    }
    for i in 0..120 {
        let input = Input {
            run: 1.0,
            jump_pressed: i == 0,
            jump_held: i < 30,
        };
        b.step(&mask, input, DT);
        assert!(!inside(&mask, &b));
    }
    assert!(b.grounded);
    assert_eq!(b.y.round() as i32, 60 - HEIGHT);
}

#[test]
fn stays_inside_the_side_edges() {
    let mut mask = Mask::empty(W, H);
    fill(&mut mask, 0, 80, W as i32, H as i32);
    let mut b = Body::spawn(&mask);
    let left = Input {
        run: -1.0,
        ..Input::default()
    };
    for _ in 0..600 {
        b.step(&mask, left, DT);
    }
    assert_eq!(b.x, 0.0);
    assert!(b.facing_left);
}
