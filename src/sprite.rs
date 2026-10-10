pub const BIRD: (usize, usize) = (14, 16);
pub const BADDIE: (usize, usize) = (14, 14);
pub const PUFF: (usize, usize) = (9, 8);
pub const BUBBLE: (usize, usize) = (4, 4);

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
    "....oo.oo.....",
    "..............",
];
const LEGS_STRIDE: [&str; 4] = [
    "....o...o.....",
    "...o....o.....",
    "...oo....oo...",
    "..............",
];
const LEGS_TUCK: [&str; 4] = [
    "....o.oo......",
    ".....oo.......",
    "..............",
    "..............",
];

const EYES_SHUT: [(usize, usize, u8); 1] = [(4, 9, b'k')];
const EYES_HAPPY: [(usize, usize, u8); 2] = [(3, 9, b'k'), (4, 9, b'y')];
const EYES_WIDE: [(usize, usize, u8); 4] = [(3, 8, b'w'), (3, 9, b'w'), (4, 8, b'w'), (4, 9, b'w')];
const FLUFF: [(usize, usize, u8); 10] = [
    (0, 5, b'k'),
    (0, 7, b'k'),
    (1, 6, b'y'),
    (6, 1, b'k'),
    (7, 0, b'k'),
    (7, 1, b'y'),
    (8, 1, b'y'),
    (8, 0, b'k'),
    (9, 0, b'k'),
    (9, 12, b'k'),
];

const BADDIE_ROWS: [[&str; 14]; 2] = [
    [
        "..k........k..",
        ".kpk......kpk.",
        ".kppkkkkkkppk.",
        "kppppppppppppk",
        "kpkkppppppkkpk",
        "kpwwkppppkwwpk",
        "kpwrwppppwrwpk",
        "kppppppppppppk",
        "kpkwkwkkwkwkpk",
        "kppkkkkkkkkppk",
        ".kppppppppppk.",
        "..kppppppppk..",
        "..kpk....kpk..",
        ".kk........kk.",
    ],
    [
        "..............",
        "k.k........k.k",
        "kpkpkkkkkkpkpk",
        "kppppppppppppk",
        "kpkkppppppkkpk",
        "kpwwkppppkwwpk",
        "kpwrwppppwrwpk",
        "kppppppppppppk",
        "kpwkwkwwkwkwpk",
        "kppkkkkkkkkppk",
        ".kppppppppppk.",
        "..kppppppppk..",
        ".kpk......kpk.",
        "kk..........kk",
    ],
];

const PUFF_ROWS: [&str; 8] = [
    "...ww....",
    ".wwwww.w.",
    "wwwwwwww.",
    ".wwwwwwww",
    "wwwwwwww.",
    ".wwwwwww.",
    "..ww.ww..",
    ".........",
];

const BUBBLE_ROWS: [&str; 4] = [".cc.", "cwac", "caac", ".cc."];

fn colour(c: u8) -> [u8; 4] {
    match c {
        b'k' => [40, 30, 36, 255],
        b'y' => [255, 206, 64, 255],
        b'd' => [214, 140, 36, 255],
        b'b' => [255, 240, 196, 255],
        b'o' => [240, 110, 40, 255],
        b'w' => [255, 255, 255, 255],
        b'p' => [124, 58, 168, 255],
        b'r' => [230, 30, 50, 255],
        b'c' => [150, 220, 255, 255],
        b'a' => [150, 220, 255, 90],
        _ => [0, 0, 0, 0],
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Wings {
    #[default]
    Rest,
    Up,
    Down,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Legs {
    #[default]
    Stand,
    Stride,
    Tuck,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Eyes {
    #[default]
    Open,
    Shut,
    Happy,
    Wide,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Frame {
    pub wings: Wings,
    pub legs: Legs,
    pub eyes: Eyes,
    pub puffed: bool,
}

impl Frame {
    const WINGS: [Wings; 3] = [Wings::Rest, Wings::Up, Wings::Down];
    const LEGS: [Legs; 3] = [Legs::Stand, Legs::Stride, Legs::Tuck];
    const EYES: [Eyes; 4] = [Eyes::Open, Eyes::Shut, Eyes::Happy, Eyes::Wide];
    pub const COUNT: usize = Self::WINGS.len() * Self::LEGS.len() * Self::EYES.len() * 2;

    pub fn index(self) -> usize {
        ((self.wings as usize * Self::LEGS.len() + self.legs as usize) * Self::EYES.len()
            + self.eyes as usize)
            * 2
            + self.puffed as usize
    }

    pub fn all() -> impl Iterator<Item = Frame> {
        Self::WINGS.into_iter().flat_map(|wings| {
            Self::LEGS.into_iter().flat_map(move |legs| {
                Self::EYES.into_iter().flat_map(move |eyes| {
                    [false, true].map(|puffed| Frame {
                        wings,
                        legs,
                        eyes,
                        puffed,
                    })
                })
            })
        })
    }
}

fn paint(rows: &[impl AsRef<[u8]>], width: usize) -> Vec<u8> {
    rows.iter()
        .flat_map(|row| {
            let row = row.as_ref();
            assert_eq!(row.len(), width);
            row.iter().flat_map(|&c| colour(c))
        })
        .collect()
}

pub fn bird(frame: Frame) -> Vec<u8> {
    let mut rows: Vec<Vec<u8>> = BODY.iter().map(|r| r.as_bytes().to_vec()).collect();
    let wing: &[(usize, &str)] = match frame.wings {
        Wings::Rest => &WING_REST,
        Wings::Up => &WING_UP,
        Wings::Down => &WING_DOWN,
    };
    for &(i, row) in wing {
        rows[i] = row.as_bytes().to_vec();
    }
    let eyes: &[(usize, usize, u8)] = match frame.eyes {
        Eyes::Open => &[],
        Eyes::Shut => &EYES_SHUT,
        Eyes::Happy => &EYES_HAPPY,
        Eyes::Wide => &EYES_WIDE,
    };
    let fluff: &[(usize, usize, u8)] = if frame.puffed { &FLUFF } else { &[] };
    for &(r, c, v) in eyes.iter().chain(fluff) {
        rows[r][c] = v;
    }
    let legs = match frame.legs {
        Legs::Stand => LEGS_STAND,
        Legs::Stride => LEGS_STRIDE,
        Legs::Tuck => LEGS_TUCK,
    };
    rows.extend(legs.map(|r| r.as_bytes().to_vec()));
    paint(&rows, BIRD.0)
}

pub fn baddie(step: usize) -> Vec<u8> {
    paint(&BADDIE_ROWS[step], BADDIE.0)
}

pub fn puff() -> Vec<u8> {
    paint(&PUFF_ROWS, PUFF.0)
}

pub fn bubble() -> Vec<u8> {
    paint(&BUBBLE_ROWS, BUBBLE.0)
}
