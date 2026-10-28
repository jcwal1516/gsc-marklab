//! The one-dimensional self-organizing map of R `learn_pattern`: kohonen 3.0.13 `som` on a
//! rectangular `xdim × 1` grid with a Gaussian neighbourhood, trained online.
//!
//! kohonen is GPL-licensed, so this is re-implemented from its documented and measured
//! behaviour, not its code. Each of `rlen × n` steps presents one row drawn uniformly with
//! replacement. The learning rate and the radius fall linearly per step from their start to
//! their end values, and a radius below 1 acts as 0.5. Every node moves toward the row by
//! `alpha · exp(-d² / (2 r²))`, where `d` is its grid distance to the nearest node (wrapping
//! around on a ring). Fixed presentation orders reproduce kohonen's codes to 1e-12.

/// The nodes of a chain or a ring.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SomGrid {
    pub nodes: usize,
    pub toroidal: bool,
}

impl SomGrid {
    pub(crate) fn distance(&self, first: usize, second: usize) -> usize {
        let distance = first.abs_diff(second);
        if self.toroidal {
            distance.min(self.nodes - distance)
        } else {
            distance
        }
    }

    /// kohonen's default starting radius, R `quantile(unit.distances(grid), 2/3)` (type 7) over
    /// every ordered pair of nodes.
    pub(crate) fn default_radius(&self) -> f64 {
        let nodes = self.nodes as u64;
        // How many ordered pairs lie at each grid distance.
        let mut counts = vec![0_u64; self.nodes];
        if self.toroidal {
            for offset in 0..self.nodes {
                counts[offset.min(self.nodes - offset)] += nodes;
            }
        } else {
            counts[0] = nodes;
            for (distance, count) in counts.iter_mut().enumerate().skip(1) {
                *count = 2 * (nodes - distance as u64);
            }
        }
        let order_statistic = |position: u64| {
            let mut seen = 0;
            for (distance, count) in counts.iter().enumerate() {
                seen += count;
                if position < seen {
                    return distance as f64;
                }
            }
            unreachable!("position within the pairs")
        };
        let index = ((nodes * nodes - 1) as f64) * (2.0 / 3.0);
        let lower = order_statistic(index.floor() as u64);
        let upper = order_statistic(index.ceil() as u64);
        let fraction = index - index.floor();
        if fraction > 0.0 && upper != lower {
            (1.0 - fraction) * lower + fraction * upper
        } else {
            lower
        }
    }
}

/// Learning rates and radii at the first step and at the end of training.
pub(crate) struct Schedule {
    pub alpha: [f64; 2],
    pub radius: [f64; 2],
    pub steps: usize,
}

/// Train `codes` (node-major, `dims` values per node) on `data` (row-major), presenting the
/// rows `next_row` returns.
pub(crate) fn train(
    grid: SomGrid,
    codes: &mut [f64],
    data: &[f64],
    dims: usize,
    schedule: &Schedule,
    mut next_row: impl FnMut() -> usize,
) {
    let farthest = if grid.toroidal {
        grid.nodes / 2
    } else {
        grid.nodes - 1
    };
    let mut weights = vec![0.0; farthest + 1];
    for step in 0..schedule.steps {
        let fraction = step as f64 / schedule.steps as f64;
        let alpha = schedule.alpha[0] - (schedule.alpha[0] - schedule.alpha[1]) * fraction;
        let radius = schedule.radius[0] - (schedule.radius[0] - schedule.radius[1]) * fraction;
        let radius = if radius < 1.0 { 0.5 } else { radius };
        let row = next_row();
        let point = &data[row * dims..(row + 1) * dims];
        let winner = nearest(codes, dims, point);
        // Weights fall with distance; once one underflows to zero the rest do too, and a zero
        // weight leaves a node unchanged.
        let scale = 2.0 * radius * radius;
        let mut reach = weights.len();
        for (distance, weight) in weights.iter_mut().enumerate() {
            *weight = (-((distance * distance) as f64) / scale).exp();
            if *weight == 0.0 {
                reach = distance;
                break;
            }
        }
        for (node, code) in codes.chunks_exact_mut(dims).enumerate() {
            let distance = grid.distance(winner, node);
            if distance >= reach {
                continue;
            }
            let rate = alpha * weights[distance];
            for (value, &target) in code.iter_mut().zip(point) {
                *value += rate * (target - *value);
            }
        }
    }
}

/// The node with the smallest squared Euclidean distance to `point`; the first on ties.
pub(crate) fn nearest(codes: &[f64], dims: usize, point: &[f64]) -> usize {
    let mut best = (f64::INFINITY, 0);
    for (node, code) in codes.chunks_exact(dims).enumerate() {
        let distance = code
            .iter()
            .zip(point)
            .map(|(value, target)| (value - target) * (value - target))
            .sum::<f64>();
        if distance < best.0 {
            best = (distance, node);
        }
    }
    best.1
}

/// The node order R `learn_pattern` walks: a chain starts at the end nearer the origin; a ring
/// starts on the shorter side of the run of nodes within 1% of the range of distances to the
/// origin around the nearest node, and proceeds toward the longer side.
pub(crate) fn path_order(codes: &[f64], dims: usize, toroidal: bool) -> Vec<usize> {
    let nodes = codes.len() / dims;
    let norms = codes
        .chunks_exact(dims)
        .map(|code| code.iter().map(|value| value * value).sum::<f64>())
        .collect::<Vec<_>>();
    if !toroidal {
        return if norms[0] > norms[nodes - 1] {
            (0..nodes).rev().collect()
        } else {
            (0..nodes).collect()
        };
    }
    let distances = norms.iter().map(|norm| norm.sqrt()).collect::<Vec<_>>();
    let (mut minimum, mut maximum, mut closest) = (f64::INFINITY, f64::NEG_INFINITY, 0);
    for (node, &distance) in distances.iter().enumerate() {
        if distance < minimum {
            (minimum, closest) = (distance, node);
        }
        maximum = maximum.max(distance);
    }
    let low = distances
        .iter()
        .enumerate()
        .filter(|(_, distance)| **distance < minimum + 0.01 * (maximum - minimum))
        .map(|(node, _)| node)
        .collect::<Vec<_>>();
    // With every node equally far, R finds no node below the threshold and fails; the ring is
    // walked as trained.
    let Some(position) = low.iter().position(|&node| node == closest) else {
        return (0..nodes).collect();
    };
    // R: rle() of c(1, diff(where_low)) up to and from the closest node.
    let steps = std::iter::once(1)
        .chain(low.windows(2).map(|pair| pair[1] - pair[0]))
        .collect::<Vec<_>>();
    let run = |values: &[usize]| {
        values
            .iter()
            .take_while(|value| **value == values[0])
            .count()
    };
    let mut before = steps[..=position].to_vec();
    before.reverse();
    let before = run(&before);
    let after = run(&steps[position..]);
    // R's 1-based positions, converted at the end.
    let closest = closest + 1;
    let order = if before <= after {
        let start = closest + 1 - before;
        (start..=nodes).chain(1..start).collect::<Vec<_>>()
    } else {
        let start = closest + after - 1;
        if start < nodes {
            (1..=start).rev().chain((start + 1..=nodes).rev()).collect()
        } else {
            (1..=nodes).rev().collect()
        }
    };
    order.into_iter().map(|node| node - 1).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(observed: &[f64], expected: &[f64]) {
        for (observed, expected) in observed.iter().zip(expected) {
            assert!(
                (observed - expected).abs() <= 1e-12 * expected.abs().max(1.0),
                "{observed:?} versus {expected:?}"
            );
        }
    }

    #[test]
    fn training_reproduces_kohonen_for_its_presentation_order() {
        // R: set.seed(1); som(matrix(c(0, 100, 200)), somgrid(3, 1, "rectangular",
        // "gaussian"), rlen, alpha = c(0.5, 0.2), radius = 2, init = c(1, 101, 201),
        // normalizeDataLayers = FALSE) presents rows 1,2,2 (rlen 1) and 1,2,2,3,1,3 (rlen 2).
        let chain = SomGrid {
            nodes: 3,
            toroidal: false,
        };
        let data = [0.0, 100.0, 200.0];
        for (order, expected) in [
            (
                &[0, 1, 1][..],
                [13.429881389593142, 70.82281360148386, 116.81834085352973],
            ),
            (
                &[0, 1, 1, 2, 0, 2],
                [28.241718609952272, 105.27909716733974, 157.68118720699886],
            ),
        ] {
            let mut codes = [1.0, 101.0, 201.0];
            let mut rows = order.iter().copied();
            let schedule = Schedule {
                alpha: [0.5, 0.2],
                radius: [2.0, 0.0],
                steps: order.len(),
            };
            train(chain, &mut codes, &data, 1, &schedule, || {
                rows.next().unwrap()
            });
            close(&codes, &expected);
        }
        // A five-node ring in two dimensions: R set.seed(3), rlen 2, alpha c(0.05, 0.01),
        // radius 1.5 presents rows 1,4,2,2,3,3,1,2.
        let ring = SomGrid {
            nodes: 5,
            toroidal: true,
        };
        let data = [0.0, 0.0, 10.0, 0.0, 0.0, 10.0, 10.0, 10.0];
        let mut codes = [1.0, 1.0, 9.0, 1.0, 9.0, 9.0, 1.0, 9.0, 5.0, 5.0];
        let mut rows = [0, 3, 1, 1, 2, 2, 0, 1].into_iter();
        let schedule = Schedule {
            alpha: [0.05, 0.01],
            radius: [1.5, 0.0],
            steps: 8,
        };
        train(ring, &mut codes, &data, 2, &schedule, || {
            rows.next().unwrap()
        });
        close(
            &codes,
            &[
                1.3494752085284996,
                1.0207533487734741,
                8.776905024573798,
                1.150699841182094,
                8.840179731926598,
                8.581461085253348,
                1.2815357983538336,
                8.847830858559394,
                4.8656909459163895,
                4.858084913971133,
            ],
        );
    }

    #[test]
    fn default_radius_is_r_quantile_of_node_distances() {
        for (nodes, toroidal, expected) in [
            (1000, false, 423.0),
            (1000, true, 333.0),
            (7, false, 3.0),
            (7, true, 2.0),
        ] {
            assert_eq!(SomGrid { nodes, toroidal }.default_radius(), expected);
        }
    }

    #[test]
    fn paths_start_where_r_starts_them() {
        // A chain starts at the end nearer the origin.
        assert_eq!(path_order(&[5.0, 3.0, 1.0], 1, false), [2, 1, 0]);
        assert_eq!(path_order(&[1.0, 3.0, 5.0], 1, false), [0, 1, 2]);
        // Ring distances 9, 1, 1, 0, 1, 9: nodes 2 to 5 lie within 1% of the range (0.09) only
        // at node 4, so R starts there and walks on.
        let ring = [9.0, 1.0, 1.0, 0.0, 1.0, 9.0];
        assert_eq!(path_order(&ring, 1, true), [3, 4, 5, 0, 1, 2]);
        // Nodes 2, 3 and 4 equally close: the run before node 2 (the first nearest) is
        // shorter, so the path starts there.
        let ring = [9.0, 5.0, 0.0, 0.0, 0.0, 9.0];
        assert_eq!(path_order(&ring, 1, true), [2, 3, 4, 5, 0, 1]);
    }
}
