//! SPACE's randomized census for raster specimens (R `census_patches` and `random_census` on
//! images). Within each disc, each layer's points form contiguous patches, 8-connected on the
//! grid, of one class or of bright or dim intensity. A randomized census shuffles a layer's
//! patches across discs and refills every disc to its own count of the layer's points, splitting
//! a patch that straddles two discs and its scalar totals in proportion to the split.

use crate::common::seeds::{derive_seed, SeedEndpoint};
use crate::permutation::labels::deterministic_shuffle;

/// One layer's patches over every disc, in disc order.
pub(crate) struct LayerPatches {
    /// Per disc, the layer's points: classified points of an object channel, every point for
    /// an independent scalar.
    pub capacities: Vec<u32>,
    pub areas: Vec<u32>,
    /// Each patch's class (object layers) or 0.
    pub classes: Vec<u32>,
    /// Per patch, `width` scalar totals over its points.
    pub totals: Vec<f64>,
    pub width: usize,
}

impl LayerPatches {
    /// Patches of `rows` (each disc's points, by `offsets`) with equal `state`; `totals` gives a
    /// point's `width` scalar values given its state.
    pub(crate) fn new(
        offsets: &[usize],
        rows: &[u32],
        steps: &[[u32; 2]],
        width: usize,
        state: impl Fn(u32) -> u32,
        totals: impl Fn(u32, u32, &mut [f64]),
    ) -> Self {
        let mut patches = Self {
            capacities: Vec::with_capacity(offsets.len() - 1),
            areas: Vec::new(),
            classes: Vec::new(),
            totals: Vec::new(),
            width,
        };
        let mut labeller = Labeller::default();
        for disc in offsets.windows(2) {
            let members = &rows[disc[0]..disc[1]];
            patches.capacities.push(members.len() as u32);
            let first = patches.areas.len();
            let labels = labeller.label(members, steps, &state);
            let count = labels.iter().max().map_or(0, |label| *label as usize + 1);
            patches.areas.resize(first + count, 0);
            patches.classes.resize(first + count, 0);
            patches.totals.resize((first + count) * width, 0.0);
            for (&row, &label) in members.iter().zip(labels) {
                let patch = first + label as usize;
                patches.areas[patch] += 1;
                patches.classes[patch] = state(row);
                totals(
                    row,
                    state(row),
                    &mut patches.totals[patch * width..(patch + 1) * width],
                );
            }
        }
        patches
    }

    pub(crate) fn len(&self) -> usize {
        self.areas.len()
    }

    /// Shuffle the patches with `seed` and refill the discs in order, calling
    /// `place(disc, patch, area)` for each whole patch or piece placed.
    pub(crate) fn refill(
        &self,
        seed: u64,
        order: &mut Vec<u32>,
        mut place: impl FnMut(usize, usize, u32),
    ) {
        order.clear();
        order.extend(0..self.len() as u32);
        deterministic_shuffle(order, seed);
        let mut discs = self
            .capacities
            .iter()
            .enumerate()
            .filter(|(_, capacity)| **capacity > 0)
            .map(|(disc, &capacity)| (disc, capacity));
        let Some((mut disc, mut remaining)) = discs.next() else {
            return;
        };
        for &patch in order.iter() {
            let mut area = self.areas[patch as usize];
            while area > 0 {
                let piece = area.min(remaining);
                place(disc, patch as usize, piece);
                area -= piece;
                remaining -= piece;
                if remaining == 0 {
                    match discs.next() {
                        Some(next) => (disc, remaining) = next,
                        // The capacities sum to the areas, so only the last patch ends here.
                        None => return,
                    }
                }
            }
        }
    }

    /// A piece's share of a patch total: R converts totals to per-point means before splitting
    /// and multiplies them back by each piece's size.
    pub(crate) fn piece_total(&self, patch: usize, column: usize, area: u32) -> f64 {
        self.totals[patch * self.width + column] / f64::from(self.areas[patch]) * f64::from(area)
    }
}

/// Seed of one layer's shuffle in one randomized census, as the cell-table census seeds it.
pub(crate) fn layer_seed(seed: u64, layer: usize) -> u64 {
    derive_seed(seed, SeedEndpoint::SpaceCisNull, layer)
}

/// The eight grid neighbours (R `patch_3D`'s 26 in a single plane).
const NEIGHBOURS: [(i64, i64); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

/// 8-connected labelling of the members of one disc, reusing its buffers across discs.
#[derive(Default)]
struct Labeller {
    grid: Vec<u32>,
    parents: Vec<u32>,
    labels: Vec<u32>,
    numbers: Vec<u32>,
}

impl Labeller {
    /// Each member's patch, numbered from 0 in order of each patch's first member.
    fn label(&mut self, members: &[u32], steps: &[[u32; 2]], state: impl Fn(u32) -> u32) -> &[u32] {
        self.labels.clear();
        if members.is_empty() {
            return &self.labels;
        }
        let (mut low, mut high) = ([u32::MAX; 2], [0_u32; 2]);
        for &row in members {
            for axis in 0..2 {
                low[axis] = low[axis].min(steps[row as usize][axis]);
                high[axis] = high[axis].max(steps[row as usize][axis]);
            }
        }
        let columns = (high[0] - low[0] + 1) as usize;
        let rows = (high[1] - low[1] + 1) as usize;
        // Member index + 1 at each grid position of the bounding box, 0 where none.
        self.grid.clear();
        self.grid.resize(columns * rows, 0);
        for (member, &row) in members.iter().enumerate() {
            let [x, y] = steps[row as usize];
            self.grid[(y - low[1]) as usize * columns + (x - low[0]) as usize] = member as u32 + 1;
        }
        self.parents.clear();
        self.parents.extend(0..members.len() as u32);
        for (member, &row) in members.iter().enumerate() {
            let [x, y] = steps[row as usize];
            let (x, y) = (i64::from(x - low[0]), i64::from(y - low[1]));
            for (dx, dy) in NEIGHBOURS {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= columns as i64 || ny >= rows as i64 {
                    continue;
                }
                let neighbour = self.grid[ny as usize * columns + nx as usize];
                if neighbour != 0 && state(members[neighbour as usize - 1]) == state(row) {
                    union(&mut self.parents, member as u32, neighbour - 1);
                }
            }
        }
        self.numbers.clear();
        self.numbers.resize(members.len(), u32::MAX);
        let mut next = 0;
        for member in 0..members.len() as u32 {
            let root = find(&mut self.parents, member) as usize;
            if self.numbers[root] == u32::MAX {
                self.numbers[root] = next;
                next += 1;
            }
            self.labels.push(self.numbers[root]);
        }
        &self.labels
    }
}

fn find(parents: &mut [u32], mut node: u32) -> u32 {
    while parents[node as usize] != node {
        let grandparent = parents[parents[node as usize] as usize];
        parents[node as usize] = grandparent;
        node = grandparent;
    }
    node
}

fn union(parents: &mut [u32], first: u32, second: u32) {
    let (first, second) = (find(parents, first), find(parents, second));
    // The smaller index becomes the root, so labels do not depend on the scan.
    let (root, child) = (first.min(second), first.max(second));
    parents[child as usize] = root;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagonal_contact_joins_and_gaps_or_classes_separate() {
        // A 4 x 3 grid, rows listed top to bottom (A, B classes, . absent):
        //   A . . B
        //   . A . B
        //   . . . A
        // The two diagonal As form one patch; the B column another; the last A touches only
        // B, so it is a third.
        let steps = [[0, 0], [3, 0], [1, 1], [3, 1], [3, 2]];
        let class = [0, 1, 0, 1, 0];
        let members = [0, 1, 2, 3, 4];
        let mut labeller = Labeller::default();
        let labels = labeller.label(&members, &steps, |row| class[row as usize]);
        assert_eq!(labels, [0, 1, 0, 1, 2]);
    }

    #[test]
    fn refilling_keeps_each_discs_count_and_splits_totals_in_proportion() {
        // Two discs: the first holds a class-0 patch of 3 points (total 6) and a class-1 patch
        // of 1 (total 10); the second a class-1 patch of 2 (total 4).
        let steps = [[0, 0], [1, 0], [2, 0], [9, 9], [0, 5], [1, 5]];
        let class = [0, 0, 0, 1, 1, 1];
        let value = [2.0, 2.0, 2.0, 10.0, 2.0, 2.0];
        let patches = LayerPatches::new(
            &[0, 4, 6],
            &[0, 1, 2, 3, 4, 5],
            &steps,
            1,
            |row| class[row as usize],
            |row, _, totals| totals[0] += value[row as usize],
        );
        assert_eq!(patches.capacities, [4, 2]);
        assert_eq!(patches.areas, [3, 1, 2]);
        assert_eq!(patches.classes, [0, 1, 1]);
        assert_eq!(patches.totals, [6.0, 10.0, 4.0]);
        for seed in 0..20 {
            let mut filled = [0_u32; 2];
            let mut mass = 0.0;
            patches.refill(seed, &mut Vec::new(), |disc, patch, area| {
                filled[disc] += area;
                mass += patches.piece_total(patch, 0, area);
            });
            assert_eq!(filled, [4, 2]);
            assert!((mass - 20.0).abs() < 1e-12);
        }
    }
}
