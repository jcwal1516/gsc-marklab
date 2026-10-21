//! Variogram-matching surrogates of one cell mark (Viladomat et al. 2014; Burt et al. 2020).
//!
//! A surrogate keeps the mark's spatial autocorrelation but breaks its link to any other
//! mark: the values are randomly permuted over the cells, smoothed over each cell's `k`
//! nearest cells with weights `exp(−d / d_k)` (`d_k` the distance to the `k`-th of them; the
//! cell itself has weight 1), and scaled as `√|β| z + √|α| ε` with independent standard normal
//! `ε`, where `α` and `β` come from the least-squares fit of the observed semivariogram on the
//! smoothed one over the declared lag bins. Of the declared `k` values, the one with the
//! smallest residual sum of squares is used. Finally the observed values are reassigned in the
//! surrogate's rank order, so every surrogate has exactly the observed marginal distribution.
//!
//! The semivariogram of lag bin `(e_b, e_{b+1}]` is `Σ (z_i − z_j)² / (2 N_b)` over the `N_b`
//! cell pairs whose distance falls in it.

use crate::common::seeds::{derive_seed, splitmix64, SeedEndpoint};
use crate::errors::{MarklabError, Result};
use crate::geom::spatial_index::SpatialIndex2D;
use crate::permutation::labels::deterministic_shuffle;

fn invalid(message: impl Into<String>) -> MarklabError {
    MarklabError::Validation(message.into())
}

/// Precomputed neighbor lists and lag-binned pairs for one set of cell positions.
pub(crate) struct VariogramSurrogates<'a> {
    values: &'a [f64],
    sorted: Vec<f64>,
    candidates: &'a [usize],
    /// Each cell's nearest cells, nearest first, up to the largest candidate `k`.
    neighbors: Vec<Vec<(usize, f64)>>,
    pairs: Vec<(u32, u32, u16)>,
    pair_counts: Vec<u64>,
    target: Vec<f64>,
}

/// What one surrogate draw used.
pub(crate) struct Draw {
    pub(crate) candidate: usize,
    pub(crate) r_squared: f64,
}

impl<'a> VariogramSurrogates<'a> {
    /// Fails when the cell pairs within the last lag edge exceed `maximum_pairs`, or fewer than
    /// two lag bins hold pairs.
    pub(crate) fn new(
        xs: &[f64],
        ys: &[f64],
        values: &'a [f64],
        candidates: &'a [usize],
        lag_edges: &[f64],
        maximum_pairs: usize,
    ) -> Result<Self> {
        let index = SpatialIndex2D::new(xs, ys)?;
        let largest = candidates.iter().copied().max().unwrap_or(1);
        let mut neighbors = Vec::with_capacity(values.len());
        let mut pairs = Vec::new();
        let (first_edge, last_edge) = (lag_edges[0], lag_edges[lag_edges.len() - 1]);
        let mut pair_counts = vec![0_u64; lag_edges.len() - 1];
        for cell in 0..values.len() {
            neighbors.push(
                index
                    .k_nearest(cell, largest)?
                    .into_iter()
                    .map(|neighbor| (neighbor.index, neighbor.distance_um))
                    .collect::<Vec<_>>(),
            );
            for neighbor in index.within_radius(cell, last_edge)? {
                let distance = neighbor.distance_um;
                if neighbor.index <= cell || distance <= first_edge || distance > last_edge {
                    continue;
                }
                if pairs.len() == maximum_pairs {
                    return Err(invalid(format!(
                        "more than {maximum_pairs} cell pairs lie within the last lag edge"
                    )));
                }
                let bin = lag_edges.partition_point(|&edge| edge < distance) - 1;
                pair_counts[bin] += 1;
                pairs.push((cell as u32, neighbor.index as u32, bin as u16));
            }
        }
        if pair_counts.iter().filter(|&&count| count > 0).count() < 2 {
            return Err(invalid("fewer than two lag bins hold cell pairs"));
        }
        let mut sorted = values.to_vec();
        sorted.sort_by(f64::total_cmp);
        let mut surrogates = Self {
            values,
            sorted,
            candidates,
            neighbors,
            pairs,
            pair_counts,
            target: Vec::new(),
        };
        surrogates.target = surrogates.semivariogram(values);
        Ok(surrogates)
    }

    pub(crate) fn pairs(&self) -> usize {
        self.pairs.len()
    }

    fn semivariogram(&self, values: &[f64]) -> Vec<f64> {
        let mut sums = vec![0.0; self.pair_counts.len()];
        for &(a, b, bin) in &self.pairs {
            let difference = values[a as usize] - values[b as usize];
            sums[bin as usize] += difference * difference;
        }
        sums.iter()
            .zip(&self.pair_counts)
            .map(|(sum, &count)| {
                if count > 0 {
                    sum / (2.0 * count as f64)
                } else {
                    0.0
                }
            })
            .collect()
    }

    /// Write one surrogate of the values into `out`.
    pub(crate) fn draw(&self, seed: u64, out: &mut [f64]) -> Result<Draw> {
        let endpoint = SeedEndpoint::MutualInformationVariogramSurrogate;
        let mut permuted = self.values.to_vec();
        deterministic_shuffle(&mut permuted, derive_seed(seed, endpoint, 0));
        let mut smoothed = vec![0.0; permuted.len()];
        let mut best: Option<(f64, usize, f64, f64, f64, Vec<f64>)> = None;
        for (candidate, &k) in self.candidates.iter().enumerate() {
            for (cell, neighbors) in self.neighbors.iter().enumerate() {
                let scale = neighbors[k - 1].1;
                let (mut total, mut weight) = (permuted[cell], 1.0);
                for &(other, distance) in &neighbors[..k] {
                    let w = if scale > 0.0 {
                        (-distance / scale).exp()
                    } else {
                        1.0
                    };
                    total += w * permuted[other];
                    weight += w;
                }
                smoothed[cell] = total / weight;
            }
            let fitted = self.semivariogram(&smoothed);
            let Some((alpha, beta, sse, r_squared)) = self.fit(&fitted) else {
                continue;
            };
            if best.as_ref().is_none_or(|current| sse < current.0) {
                best = Some((sse, candidate, alpha, beta, r_squared, smoothed.clone()));
            }
        }
        let (_, candidate, alpha, beta, r_squared, smoothed) = best.ok_or_else(|| {
            invalid("no smoothing candidate gave a semivariogram with variation across lags")
        })?;
        let mut state = derive_seed(seed, endpoint, 1);
        let mut noise = Vec::with_capacity(smoothed.len() + 1);
        while noise.len() < smoothed.len() {
            state = splitmix64(state);
            let u1 = ((state >> 11) as f64 + 0.5) / (1_u64 << 53) as f64;
            state = splitmix64(state);
            let u2 = (state >> 11) as f64 / (1_u64 << 53) as f64;
            let radius = (-2.0 * u1.ln()).sqrt();
            let angle = std::f64::consts::TAU * u2;
            noise.push(radius * angle.cos());
            noise.push(radius * angle.sin());
        }
        let surrogate = smoothed
            .iter()
            .zip(&noise)
            .map(|(z, e)| beta.abs().sqrt() * z + alpha.abs().sqrt() * e)
            .collect::<Vec<_>>();
        let mut order = (0..surrogate.len()).collect::<Vec<_>>();
        order.sort_by(|&a, &b| surrogate[a].total_cmp(&surrogate[b]).then(a.cmp(&b)));
        for (rank, &cell) in order.iter().enumerate() {
            out[cell] = self.sorted[rank];
        }
        Ok(Draw {
            candidate,
            r_squared,
        })
    }

    /// Least squares of the observed semivariogram on `fitted` over bins holding pairs:
    /// (`α`, `β`, residual sum of squares, `R²`).
    fn fit(&self, fitted: &[f64]) -> Option<(f64, f64, f64, f64)> {
        let bins = (0..fitted.len())
            .filter(|&bin| self.pair_counts[bin] > 0)
            .collect::<Vec<_>>();
        let count = bins.len() as f64;
        let mean_x = bins.iter().map(|&bin| fitted[bin]).sum::<f64>() / count;
        let mean_y = bins.iter().map(|&bin| self.target[bin]).sum::<f64>() / count;
        let sxx = bins
            .iter()
            .map(|&b| (fitted[b] - mean_x).powi(2))
            .sum::<f64>();
        let sxy = bins
            .iter()
            .map(|&b| (fitted[b] - mean_x) * (self.target[b] - mean_y))
            .sum::<f64>();
        let syy = bins
            .iter()
            .map(|&b| (self.target[b] - mean_y).powi(2))
            .sum::<f64>();
        if !(sxx > 0.0 && sxx.is_finite()) {
            return None;
        }
        let beta = sxy / sxx;
        let alpha = mean_y - beta * mean_x;
        let sse = (syy - beta * sxy).max(0.0);
        let r_squared = if syy > 0.0 { 1.0 - sse / syy } else { 1.0 };
        Some((alpha, beta, sse, r_squared))
    }
}
