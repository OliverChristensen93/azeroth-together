//! What stands in the horde's way: trees, rocks, walls and buildings, as a grid of blocked cells
//! over the arena. The sim has no world geometry of its own, so the host's client fills the grid
//! from its collision ([`Obstacles::set`]), a few cells at a time as the terrain streams in; until
//! it does, enemies walk straight as before.
//!
//! An enemy that can see its hero along open cells walks straight at it. One that cannot follows
//! a flow field: every open cell's distance in cells to the nearest hero, refreshed a few times a
//! second ([`Obstacles::flow`]), downhill around the obstacle.

use std::collections::VecDeque;

use crate::math::V2;

/// A cell's distance when no hero can be reached from it.
const FAR: u16 = u16::MAX;

/// The blocked grid and its flow field, over a square of cells around the arena's centre.
#[derive(Clone, Debug, Default)]
pub struct Obstacles {
    /// The world position of cell (0, 0)'s corner, the cell size in yards, and the side in cells.
    origin: V2,
    cell: f32,
    side: usize,
    blocked: Vec<bool>,
    /// Any cell blocked at all: with none, nothing needs steering.
    any: bool,
    /// Each cell's distance in cells to the nearest hero, [`FAR`] when none can be reached.
    dist: Vec<u16>,
    /// Whether [`Obstacles::dist`] has been worked out at least once.
    flowed: bool,
}

impl Obstacles {
    /// An open grid of `cell`-yard cells covering `radius` yards around `center`.
    pub fn new(center: V2, radius: f32, cell: f32) -> Self {
        let cell = cell.max(0.25);
        let side = ((radius * 2.0) / cell).ceil() as usize + 1;
        let origin = center - V2::new(side as f32 * cell * 0.5, side as f32 * cell * 0.5);
        Self {
            origin,
            cell,
            side,
            blocked: vec![false; side * side],
            any: false,
            dist: vec![FAR; side * side],
            flowed: false,
        }
    }

    /// The side of the grid, in cells: cells run 0..side on each axis.
    pub fn side(&self) -> usize {
        self.side
    }

    pub fn cell_size(&self) -> f32 {
        self.cell
    }

    /// The world position at the middle of cell (`x`, `y`).
    pub fn cell_center(&self, x: usize, y: usize) -> V2 {
        self.origin + V2::new((x as f32 + 0.5) * self.cell, (y as f32 + 0.5) * self.cell)
    }

    /// The cell `pos` falls in, if it is on the grid.
    pub fn cell_at(&self, pos: V2) -> Option<(usize, usize)> {
        let off = pos - self.origin;
        let (x, y) = (off.x / self.cell, off.y / self.cell);
        if x < 0.0 || y < 0.0 {
            return None;
        }
        let (x, y) = (x as usize, y as usize);
        (x < self.side && y < self.side).then_some((x, y))
    }

    fn ix(&self, x: usize, y: usize) -> usize {
        y * self.side + x
    }

    /// Mark cell (`x`, `y`) blocked or open.
    pub fn set(&mut self, x: usize, y: usize, blocked: bool) {
        if x < self.side && y < self.side {
            let i = self.ix(x, y);
            self.blocked[i] = blocked;
            self.any |= blocked;
        }
    }

    /// Whether anything is blocked yet.
    pub fn any(&self) -> bool {
        self.any
    }

    /// Whether `pos` is in a blocked cell; off the grid is open.
    pub fn blocked_at(&self, pos: V2) -> bool {
        self.cell_at(pos)
            .is_some_and(|(x, y)| self.blocked[self.ix(x, y)])
    }

    /// Whether the straight line from `from` to `to` crosses no blocked cell.
    pub fn clear_line(&self, from: V2, to: V2) -> bool {
        let d = to - from;
        let steps = (d.length() / (self.cell * 0.5)).ceil().max(1.0) as usize;
        (1..=steps).all(|k| !self.blocked_at(from + d * (k as f32 / steps as f32)))
    }

    /// Work out every cell's distance to the nearest of `heroes`, through open cells.
    pub fn flow(&mut self, heroes: &[V2]) {
        self.dist.iter_mut().for_each(|d| *d = FAR);
        let mut queue = VecDeque::new();
        for &h in heroes {
            if let Some((x, y)) = self.cell_at(h) {
                let i = self.ix(x, y);
                if self.dist[i] != 0 {
                    self.dist[i] = 0;
                    queue.push_back((x, y));
                }
            }
        }
        while let Some((x, y)) = queue.pop_front() {
            let next = self.dist[self.ix(x, y)].saturating_add(1);
            for (nx, ny) in self.neighbours4(x, y) {
                let i = self.ix(nx, ny);
                if !self.blocked[i] && self.dist[i] > next {
                    self.dist[i] = next;
                    queue.push_back((nx, ny));
                }
            }
        }
        self.flowed = true;
    }

    fn neighbours4(&self, x: usize, y: usize) -> impl Iterator<Item = (usize, usize)> {
        let side = self.side as isize;
        [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)]
            .into_iter()
            .map(move |(dx, dy)| (x as isize + dx, y as isize + dy))
            .filter(move |&(nx, ny)| nx >= 0 && ny >= 0 && nx < side && ny < side)
            .map(|(nx, ny)| (nx as usize, ny as usize))
    }

    /// The way an enemy at `pos` should walk to reach `target`: straight when the line is open,
    /// else downhill along the flow field. None when there is nothing better than straight (no
    /// obstacles, no flow yet, or no way through from here).
    pub fn steer(&self, pos: V2, target: V2) -> Option<V2> {
        if !self.any || !self.flowed || self.clear_line(pos, target) {
            return None;
        }
        let (x, y) = self.cell_at(pos)?;
        let here = self.dist[self.ix(x, y)];
        if here == FAR || self.blocked[self.ix(x, y)] {
            return None;
        }
        // The lowest of the eight around; a diagonal only where both its sides are open, so a
        // body does not cut a corner through a trunk.
        let mut best: Option<((usize, usize), u16)> = None;
        for dy in -1isize..=1 {
            for dx in -1isize..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let (nx, ny) = (x as isize + dx, y as isize + dy);
                if nx < 0 || ny < 0 || nx >= self.side as isize || ny >= self.side as isize {
                    continue;
                }
                let (nx, ny) = (nx as usize, ny as usize);
                if self.blocked[self.ix(nx, ny)] {
                    continue;
                }
                if dx != 0 && dy != 0 {
                    let side_a = self.blocked[self.ix(nx, y)];
                    let side_b = self.blocked[self.ix(x, ny)];
                    if side_a || side_b {
                        continue;
                    }
                }
                let d = self.dist[self.ix(nx, ny)];
                if d < here && best.is_none_or(|(_, b)| d < b) {
                    best = Some(((nx, ny), d));
                }
            }
        }
        let ((nx, ny), _) = best?;
        Some((self.cell_center(nx, ny) - pos).normalize_or_zero())
    }

    /// `from` moved by `step`, but not into a blocked cell: it slides along the obstacle on
    /// whichever axis stays open, or stays put.
    pub fn slide(&self, from: V2, step: V2) -> V2 {
        if !self.any || self.blocked_at(from) {
            return from + step;
        }
        let to = from + step;
        if !self.blocked_at(to) {
            return to;
        }
        let x = from + V2::new(step.x, 0.0);
        if !self.blocked_at(x) {
            return x;
        }
        let y = from + V2::new(0.0, step.y);
        if !self.blocked_at(y) {
            return y;
        }
        from
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A wall across x = 5, from y = -6 to 6, on a grid around the origin.
    fn walled() -> Obstacles {
        let mut o = Obstacles::new(V2::ZERO, 20.0, 1.0);
        for k in -6..=6 {
            let (x, y) = o.cell_at(V2::new(5.2, k as f32 + 0.5)).unwrap();
            o.set(x, y, true);
        }
        o
    }

    #[test]
    fn an_open_line_is_walked_straight() {
        let mut o = walled();
        o.flow(&[V2::new(0.0, 15.0)]);
        assert_eq!(o.steer(V2::new(0.0, 0.0), V2::new(0.0, 15.0)), None);
    }

    #[test]
    fn a_wall_in_the_way_is_walked_around() {
        let mut o = walled();
        let hero = V2::new(10.0, 0.5);
        o.flow(&[hero]);
        let mut pos = V2::new(0.0, 0.5);
        for _ in 0..400 {
            let dir = o
                .steer(pos, hero)
                .unwrap_or_else(|| (hero - pos).normalize_or_zero());
            pos = o.slide(pos, dir * 0.2);
            assert!(!o.blocked_at(pos), "walked into the wall at {pos:?}");
            if pos.distance(hero) < 1.0 {
                return;
            }
        }
        panic!("never got round the wall: at {pos:?}");
    }

    #[test]
    fn a_blocked_step_slides_along_the_wall() {
        let o = walled();
        let from = V2::new(4.5, 0.5);
        let to = o.slide(from, V2::new(1.0, 0.3));
        assert!(!o.blocked_at(to));
        assert!(to.y > from.y);
    }
}
