use perch::seg::Mask;

pub const W: usize = 60;
pub const H: usize = 40;
pub const INSIDE: (i32, i32) = (30, 20);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Gap {
    None,
    Near,
    Far,
}

pub fn ring(gap: Gap) -> Mask {
    let mut mask = Mask::empty(W, H);
    for y in 10..30 {
        for x in 20..40 {
            let wall = !(22..38).contains(&x) || !(12..28).contains(&y);
            let side = match gap {
                Gap::None => false,
                Gap::Near => x >= 38,
                Gap::Far => x < 22,
            };
            let opening = side && (18..21).contains(&y);
            mask.cells[y * W + x] = wall && !opening;
        }
    }
    mask
}

pub fn in_pocket((x, y): (i32, i32)) -> bool {
    (22..38).contains(&x) && (12..28).contains(&y)
}
