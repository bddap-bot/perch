mod common;

use common::{Gap, INSIDE, in_pocket, ring};
use perch::air::Air;
use perch::baddie::Baddie;
use perch::seg::Mask;

const DT: f32 = 1.0 / 60.0;
const PREY: (f32, f32) = (INSIDE.0 as f32 + 0.5, INSIDE.1 as f32 + 0.5);

fn hunt(mask: &Mask, seconds: f32) -> (Baddie, bool) {
    hunt_from(mask, (55.5, 2.5), seconds)
}

fn hunt_from(mask: &Mask, from: (f32, f32), seconds: f32) -> (Baddie, bool) {
    let mut air = Air::default();
    air.fill(mask, [INSIDE]);
    let mut b = Baddie::new(from.0, from.1);
    for _ in 0..(seconds / DT) as usize {
        let caught = b.hunt(mask, &air, PREY, 20.0, DT);
        let (x, y) = b.cell();
        assert!(!mask.solid(x, y));
        if caught {
            return (b, true);
        }
    }
    (b, false)
}

#[test]
fn reaches_the_bird_through_a_gap() {
    assert!(hunt(&ring(Gap::Near), 10.0).1);
}

#[test]
fn goes_around_to_a_gap_on_the_far_side() {
    assert!(hunt_from(&ring(Gap::Far), (55.5, PREY.1), 10.0).1);
}

#[test]
fn a_seal_keeps_it_out() {
    let (b, caught) = hunt(&ring(Gap::None), 10.0);
    assert!(!caught);
    assert!(!in_pocket(b.cell()), "{:?}", b.cell());
}

#[test]
fn sealed_out_it_presses_against_the_wall_nearest_the_bird() {
    let (b, _) = hunt(&ring(Gap::None), 10.0);
    let (x, y) = b.cell();
    assert!((18..42).contains(&x) && (8..32).contains(&y), "{x},{y}");
}
