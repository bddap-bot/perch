use crate::body::Pose;

pub const SIZE: (usize, usize) = (14, 16);

const BODY: [&str; 12] = [
    "..............",
    ".....kkkk.....",
    "....kyyyyk....",
    "...kyyyyyyk...",
    "...kyyyykwkk..",
    "..kyyyyykkkook",
    "..kyyyyyyyyook",
    ".kyyyyyyyyyk..",
    ".kyyyyyyyyyk..",
    ".kybbbyyyyyk..",
    "..kbbbbbyyk...",
    "...kkkkkkk....",
];

const WING_REST: [(usize, &str); 2] = [(7, ".kyddddyyyyk.."), (8, ".kyyddddyyyk..")];
const WING_UP: [(usize, &str); 3] = [
    (2, "dd..kyyyyk...."),
    (3, ".dddyyyyyyk..."),
    (4, "..kddyyykwkk.."),
];
const WING_DOWN: [(usize, &str); 2] = [(8, ".kyyddddyyyk.."), (10, "..kbbdddbyk...")];

const LEGS_STAND: [&str; 4] = [
    "....o..o......",
    "....o..o......",
    "...oo.oo......",
    "..............",
];
const LEGS_STRIDE: [&str; 4] = [
    "....o...o.....",
    "...o....o.....",
    "..oo.....oo...",
    "..............",
];
const LEGS_TUCK: [&str; 4] = [
    "....oo.o......",
    ".....oo.......",
    "..............",
    "..............",
];

fn colour(c: u8) -> [u8; 4] {
    match c {
        b'k' => [40, 30, 36, 255],
        b'y' => [255, 206, 64, 255],
        b'd' => [214, 140, 36, 255],
        b'b' => [255, 240, 196, 255],
        b'o' => [240, 110, 40, 255],
        b'w' => [255, 255, 255, 255],
        _ => [0, 0, 0, 0],
    }
}

pub fn frame(pose: Pose, run_phase: bool) -> Vec<u8> {
    let mut rows: Vec<&str> = BODY.to_vec();
    let (wing, legs): (&[(usize, &str)], _) = match pose {
        Pose::Idle => (&WING_REST, LEGS_STAND),
        Pose::Run if run_phase => (&WING_REST, LEGS_STRIDE),
        Pose::Run => (&WING_REST, LEGS_STAND),
        Pose::Jump => (&WING_UP, LEGS_TUCK),
        Pose::Fall => (&WING_DOWN, LEGS_STAND),
    };
    for &(i, row) in wing {
        rows[i] = row;
    }
    rows.extend(legs);
    rows.iter()
        .flat_map(|row| {
            assert_eq!(row.len(), SIZE.0, "{row}");
            row.bytes().flat_map(colour)
        })
        .collect()
}
