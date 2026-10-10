use crate::seg::Mask;

const UNREACHED: u16 = u16::MAX;

#[derive(Default)]
pub struct Air {
    width: usize,
    height: usize,
    dist: Vec<u16>,
    queue: Vec<u32>,
    breathes: bool,
}

impl Air {
    pub fn breathes(&self) -> bool {
        self.breathes
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        (x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height)
            .then(|| y as usize * self.width + x as usize)
    }

    pub fn fill(&mut self, mask: &Mask, seeds: impl IntoIterator<Item = (i32, i32)>) {
        (self.width, self.height) = (mask.width, mask.height);
        self.dist.clear();
        self.dist.resize(mask.cells.len(), UNREACHED);
        self.queue.clear();
        let mut breathes = false;
        for (x, y) in seeds {
            match self.index(x, y) {
                None => breathes = true,
                Some(i) if !mask.cells[i] && self.dist[i] == UNREACHED => {
                    self.dist[i] = 0;
                    self.queue.push(i as u32);
                }
                Some(_) => {}
            }
        }
        let (w, h) = (self.width, self.height);
        let mut head = 0;
        while head < self.queue.len() {
            let i = self.queue[head] as usize;
            head += 1;
            let (x, y) = (i % w, i / w);
            breathes |= x == 0 || y == 0 || x == w - 1 || y == h - 1;
            let next = self.dist[i].saturating_add(1);
            for (ok, j) in [
                (x > 0, i.wrapping_sub(1)),
                (x + 1 < w, i + 1),
                (y > 0, i.wrapping_sub(w)),
                (y + 1 < h, i + w),
            ] {
                if ok && !mask.cells[j] && self.dist[j] == UNREACHED {
                    self.dist[j] = next;
                    self.queue.push(j as u32);
                }
            }
        }
        self.breathes = breathes;
    }

    pub fn dist(&self, x: i32, y: i32) -> Option<u16> {
        self.index(x, y)
            .map(|i| self.dist[i])
            .filter(|&d| d != UNREACHED)
    }

    pub fn reached(&self, x: i32, y: i32) -> bool {
        self.dist(x, y).is_some()
    }

    pub fn downhill(&self, x: i32, y: i32, toward: (f32, f32)) -> Option<(i32, i32)> {
        let here = self.dist(x, y)?;
        let off = |(cx, cy): (i32, i32)| {
            (cx as f32 + 0.5 - toward.0).powi(2) + (cy as f32 + 0.5 - toward.1).powi(2)
        };
        [(1, 0), (-1, 0), (0, 1), (0, -1)]
            .into_iter()
            .map(|(dx, dy)| (x + dx, y + dy))
            .filter_map(|c| Some((self.dist(c.0, c.1)?, c)))
            .filter(|&(d, _)| d < here)
            .min_by(|a, b| a.0.cmp(&b.0).then(off(a.1).total_cmp(&off(b.1))))
            .map(|(_, c)| c)
    }

    pub fn pocket(&self) -> impl Iterator<Item = usize> + '_ {
        self.queue.iter().map(|&i| i as usize)
    }
}
