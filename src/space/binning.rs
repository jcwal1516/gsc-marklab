//! SPACE's bin count and nearest-grid-point rounding (R `measure_cisMI` and `round_column`).

/// R: `min(floor(((ss + 1) / 10)^(1 / depth)), max_bins)`, where `ss` is the smallest disc
/// count at the radii analysed. `b^depth <= (ss + 1) / 10` therefore holds by construction.
pub(crate) fn bin_count(minimum_discs: usize, depth: usize, max_bins: usize) -> usize {
    let raw = ((minimum_discs as f64 + 1.0) / 10.0)
        .powf(1.0 / depth as f64)
        .floor();
    (raw as usize).min(max_bins)
}

/// The fewest discs for which [`bin_count`] reaches two bins at `depth`.
pub(crate) fn minimum_discs_for_two_bins(depth: usize) -> usize {
    (10_usize.saturating_mul(1_usize << depth.min(60))).saturating_sub(1)
}

/// The smallest and largest value (R `min` and `max` of a census column).
pub(crate) fn min_max(values: &[f64]) -> (f64, f64) {
    values
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), value| {
            (low.min(*value), high.max(*value))
        })
}

/// Nearest-point rounding onto R's `seq(minimum, maximum, length.out = bins)`.
///
/// R rounds with `FNN::knnx.index`, an ANN kd-tree (sliding-midpoint splits, one point per
/// leaf, exact search, first point found kept on a tie). Equidistant values, common for
/// percentages, therefore go up or down depending on the tree. This reproduces that tree in
/// one dimension so ties resolve as in R.
pub(crate) struct NearestGrid {
    points: Vec<f64>,
    nodes: Vec<Node>,
}

enum Node {
    Leaf(usize),
    Split {
        cut: f64,
        lower_bound: f64,
        upper_bound: f64,
        low: usize,
        high: usize,
    },
}

impl NearestGrid {
    /// `None` for a constant variable, which R leaves unrounded and which occupies one bin.
    pub(crate) fn new(minimum: f64, maximum: f64, bins: usize) -> Option<Self> {
        if minimum == maximum {
            return None;
        }
        // R: c(from, from + seq_len(n1 - 1) * ((to - from) / n1), to).
        let steps = bins - 1;
        let by = (maximum - minimum) / steps as f64;
        let mut points = Vec::with_capacity(bins);
        points.push(minimum);
        points.extend((1..steps).map(|step| minimum + step as f64 * by));
        points.push(maximum);
        let mut grid = Self {
            points,
            nodes: Vec::with_capacity(2 * bins),
        };
        grid.build(0, bins, minimum, maximum);
        Some(grid)
    }

    pub(crate) fn points(&self) -> &[f64] {
        &self.points
    }

    /// ANN `rkd_tree` with `sl_midpt_split` over the sorted points `start..end` in the cell
    /// `[lower, upper]`; returns the node index.
    fn build(&mut self, start: usize, end: usize, lower: f64, upper: f64) -> usize {
        let count = end - start;
        if count == 1 {
            self.nodes.push(Node::Leaf(start));
            return self.nodes.len() - 1;
        }
        let ideal = (lower + upper) / 2.0;
        let (minimum, maximum) = (self.points[start], self.points[end - 1]);
        let cut = ideal.clamp(minimum, maximum);
        let slice = &self.points[start..end];
        let below = slice.partition_point(|point| *point < cut);
        let at_or_below = slice.partition_point(|point| *point <= cut);
        let low_count = if ideal < minimum {
            1
        } else if ideal > maximum {
            count - 1
        } else if below > count / 2 {
            below
        } else if at_or_below < count / 2 {
            at_or_below
        } else {
            count / 2
        };
        let position = self.nodes.len();
        self.nodes.push(Node::Leaf(usize::MAX));
        let low = self.build(start, start + low_count, lower, cut);
        let high = self.build(start + low_count, end, cut, upper);
        self.nodes[position] = Node::Split {
            cut,
            lower_bound: lower,
            upper_bound: upper,
            low,
            high,
        };
        position
    }

    /// Zero-based index of the grid point nearest to `value`, with ANN's exact search order.
    pub(crate) fn nearest(&self, value: f64) -> u8 {
        let (first, last) = (self.points[0], self.points[self.points.len() - 1]);
        let outside = if value < first {
            first - value
        } else if value > last {
            value - last
        } else {
            0.0
        };
        let mut best = (f64::INFINITY, usize::MAX);
        self.search(0, value, outside * outside, &mut best);
        // Input scalars are bounded so squared distances stay finite and a point is found.
        debug_assert!(best.1 < self.points.len(), "no grid point for {value}");
        best.1 as u8
    }

    fn search(&self, node: usize, value: f64, box_distance: f64, best: &mut (f64, usize)) {
        match self.nodes[node] {
            Node::Leaf(point) => {
                let difference = value - self.points[point];
                let distance = difference * difference;
                if distance < best.0 {
                    *best = (distance, point);
                }
            }
            Node::Split {
                cut,
                lower_bound,
                upper_bound,
                low,
                high,
            } => {
                let cut_difference = value - cut;
                let (near, far, box_difference) = if cut_difference < 0.0 {
                    (low, high, lower_bound - value)
                } else {
                    (high, low, value - upper_bound)
                };
                self.search(near, value, box_distance, best);
                let box_difference = box_difference.max(0.0);
                let far_distance = box_distance
                    + (cut_difference * cut_difference - box_difference * box_difference);
                if far_distance < best.0 {
                    self.search(far, value, far_distance, best);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bin_count_follows_the_r_formula_including_floating_cube_roots() {
        assert_eq!(bin_count(999, 2, 100), 10);
        assert_eq!(bin_count(7999, 3, 100), 9);
        // 1000^(1/3) is 9.999999999999998 in binary floating point, as in R.
        assert_eq!(bin_count(9999, 3, 100), 9);
        assert_eq!(bin_count(1_000_000, 1, 100), 100);
        assert_eq!(bin_count(78, 3, 100), 1);
        assert_eq!(bin_count(minimum_discs_for_two_bins(3), 3, 100), 2);
    }

    #[test]
    fn rounding_matches_r_fnn_including_its_tie_breaking() {
        // Expected indices are R 4.5.2 FNN 1.1.4.1 knnx.index results (one-based, minus one).
        let grid = NearestGrid::new(0.0, 100.0, 3).unwrap();
        assert_eq!(grid.points(), &[0.0, 50.0, 100.0]);
        let observed = [25.0, 75.0, 50.0, 0.0, 100.0].map(|value| grid.nearest(value));
        assert_eq!(observed, [0, 2, 1, 0, 2]);
        let grid = NearestGrid::new(0.0, 10.0, 6).unwrap();
        let observed = [1.0, 3.0, 5.0, 7.0, 9.0].map(|value| grid.nearest(value));
        assert_eq!(observed, [0, 2, 3, 3, 5]);
        let grid = NearestGrid::new(-1.0, 1.0, 5).unwrap();
        let observed = [-0.75, -0.25, 0.25, 0.75].map(|value| grid.nearest(value));
        assert_eq!(observed, [0, 1, 2, 4]);
        assert!(NearestGrid::new(3.0, 3.0, 4).is_none());
    }
}
