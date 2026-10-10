mod common;

use common::{Gap, INSIDE, in_pocket, ring};
use perch::air::Air;

#[test]
fn an_enclosed_pocket_does_not_breathe() {
    let mask = ring(Gap::None);
    let mut air = Air::default();
    air.fill(&mask, [INSIDE]);
    assert!(!air.breathes());
    assert!(air.reached(23, 13));
    assert!(!air.reached(5, 5));
    assert_eq!(air.pocket().count(), 16 * 16);
    assert!(
        air.pocket()
            .all(|i| in_pocket(((i % 60) as i32, (i / 60) as i32)))
    );
}

#[test]
fn a_gap_to_the_edge_breathes() {
    let mask = ring(Gap::Near);
    let mut air = Air::default();
    air.fill(&mask, [INSIDE]);
    assert!(air.breathes());
    assert!(air.reached(5, 5));
    assert_eq!(air.dist(INSIDE.0, INSIDE.1), Some(0));
}

#[test]
fn a_gap_on_any_side_breathes() {
    let mut air = Air::default();
    air.fill(&ring(Gap::Far), [INSIDE]);
    assert!(air.breathes());
}

#[test]
fn a_bird_hanging_off_screen_breathes() {
    let mask = ring(Gap::None);
    let mut air = Air::default();
    air.fill(&mask, [(30, -1)]);
    assert!(air.breathes());
}

#[test]
fn seeds_inside_solid_reach_nothing() {
    let mask = ring(Gap::None);
    let mut air = Air::default();
    air.fill(&mask, [(20, 10)]);
    assert!(!air.breathes());
    assert_eq!(air.pocket().count(), 0);
}

#[test]
fn downhill_walks_to_the_seed() {
    let mask = ring(Gap::Near);
    let mut air = Air::default();
    air.fill(&mask, [INSIDE]);
    let mut at = (55, 5);
    let mut steps = 0;
    while let Some(next) = air.downhill(at.0, at.1, (30.5, 20.5)) {
        assert!(!mask.solid(next.0, next.1));
        at = next;
        steps += 1;
    }
    assert_eq!(at, INSIDE);
    assert_eq!(Some(steps), air.dist(55, 5).map(u32::from));
}
