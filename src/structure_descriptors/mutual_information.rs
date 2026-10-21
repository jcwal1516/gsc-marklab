//! Mutual information between two continuous cell marks, with a spatially aware null.
//!
//! * **Estimate:** Kraskov, Stögbauer and Grassberger (2004) algorithm 1. Each mark is divided
//!   by its population standard deviation; `ε_i` is the Chebyshev distance from cell `i` to its
//!   `k`-th nearest cell in the joint space, and `n_x(i)`, `n_y(i)` count other cells strictly
//!   closer than `ε_i` in each mark. `I = ψ(N) + ψ(k) − ⟨ψ(n_x + 1)⟩ − ⟨ψ(n_y + 1)⟩` in nats,
//!   reported unclipped: a negative value is estimator bias near independence.
//! * **Null, one of:** `block_permutation` permutes the second mark only among cells of the same
//!   declared square block, holding arrangement at scales larger than a block fixed (a block
//!   wider than the tissue gives the ordinary permutation test; autocorrelation within blocks
//!   still inflates it). `variogram_matching` replaces the second mark by surrogates that keep
//!   its semivariogram and marginal distribution (see `surrogates`).
//!
//! The estimator assumes continuous marks: a cell with at least `k` other cells at identical
//! values of both marks has `ε_i = 0`, and the request is rejected.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use statrs::function::gamma::digamma;

use super::surrogates::VariogramSurrogates;
use crate::common::seeds::{derive_seed, SeedEndpoint};
use crate::errors::{MarklabError, Result};
use crate::permutation::labels::deterministic_shuffle;

/// Result format of [`mutual_information_from_json`].
pub const MUTUAL_INFORMATION_FORMAT: &str = "marklab.mutual_information";
const MAXIMUM_INPUT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    format: String,
    version: u32,
    coordinate_frame_id: String,
    first_name: String,
    second_name: String,
    cells: Vec<Cell>,
    neighbors_k: usize,
    null: NullModel,
    permutations: usize,
    seed: u64,
    limits: Limits,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cell {
    id: String,
    x_um: f64,
    y_um: f64,
    first: f64,
    second: f64,
}

/// Declared null model for the second mark.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "snake_case")]
pub enum NullModel {
    /// Permute within square blocks of the declared width from the declared origin.
    BlockPermutation { origin_um: [f64; 2], width_um: f64 },
    /// Variogram-matching surrogates over the declared smoothing neighbor counts and lag-bin
    /// edges `(e_b, e_{b+1}]`.
    VariogramMatching {
        smoothing_neighbors: Vec<usize>,
        lag_edges_um: Vec<f64>,
    },
}

#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct Limits {
    maximum_cells: usize,
    maximum_distance_evaluations: u64,
    maximum_variogram_pairs: usize,
}

/// Mutual information of two marks and its blocked permutation null.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MutualInformationResult {
    pub format: String,
    pub version: u32,
    pub coordinate_frame_id: String,
    pub first_name: String,
    pub second_name: String,
    pub cells: usize,
    pub neighbors_k: usize,
    /// Cells sharing their first (second) mark value with another cell.
    pub tied_first_cells: usize,
    pub tied_second_cells: usize,
    /// KSG algorithm 1 estimate in nats, unclipped.
    pub estimate_nats: f64,
    pub null: NullModel,
    pub occupied_blocks: Option<usize>,
    /// Blocks holding one cell, whose second mark the permutation cannot move.
    pub singleton_blocks: Option<usize>,
    pub variogram_pairs: Option<usize>,
    /// How many surrogates used each declared smoothing neighbor count.
    pub selected_smoothing_neighbors: Option<Vec<usize>>,
    /// Mean `R²` of the observed semivariogram on the surrogates' smoothed semivariograms.
    pub mean_variogram_fit_r_squared: Option<f64>,
    /// Permutations or surrogates.
    pub permutations: usize,
    pub seed: u64,
    pub null_mean_nats: Option<f64>,
    pub null_standard_deviation_nats: Option<f64>,
    /// `estimate − null mean`: the dependence beyond what the null arrangement produces.
    pub excess_over_null_nats: Option<f64>,
    /// `(1 + #{null ≥ estimate}) / (permutations + 1)`.
    pub upper_p_value: Option<f64>,
    pub claim_scope: String,
}

fn invalid(message: impl Into<String>) -> MarklabError {
    MarklabError::Validation(message.into())
}

/// Parse a `marklab.mutual_information_request` version 1, estimate and test it.
pub fn mutual_information_from_json(input: &[u8]) -> Result<MutualInformationResult> {
    if input.len() > MAXIMUM_INPUT_BYTES {
        return Err(invalid("mutual-information input exceeds 16 MiB"));
    }
    let request: Request = serde_json::from_slice(input)?;
    let n = request.cells.len();
    let k = request.neighbors_k;
    let draws = request.permutations as u64;
    // Worst case of the joint neighbor scan: every cell visits every other cell.
    let mut work = (n as u64)
        .saturating_mul(n as u64)
        .saturating_mul(draws + 1);
    let valid_null = match &request.null {
        NullModel::BlockPermutation {
            origin_um,
            width_um,
        } => width_um.is_finite() && *width_um > 0.0 && origin_um.iter().all(|v| v.is_finite()),
        NullModel::VariogramMatching {
            smoothing_neighbors,
            lag_edges_um,
        } => {
            let largest = smoothing_neighbors.last().copied().unwrap_or(0);
            // Each draw smooths and bins once per candidate; pairs are bounded by their limit.
            let per_candidate = (n as u64)
                .saturating_mul(largest as u64 + 1)
                .saturating_add(request.limits.maximum_variogram_pairs as u64);
            work = work.saturating_add(
                draws
                    .saturating_mul(smoothing_neighbors.len() as u64)
                    .saturating_mul(per_candidate),
            );
            (1..=16).contains(&smoothing_neighbors.len())
                && smoothing_neighbors[0] >= 1
                && smoothing_neighbors.windows(2).all(|pair| pair[0] < pair[1])
                && largest < n.min(1025)
                && (2..=65).contains(&lag_edges_um.len())
                && lag_edges_um.iter().all(|edge| edge.is_finite())
                && lag_edges_um[0] >= 0.0
                && lag_edges_um.windows(2).all(|pair| pair[0] < pair[1])
        }
    };
    if request.format != "marklab.mutual_information_request"
        || request.version != 1
        || [
            &request.coordinate_frame_id,
            &request.first_name,
            &request.second_name,
        ]
        .iter()
        .any(|text| text.trim().is_empty())
        || !(1..=64).contains(&k)
        || n <= k
        || n > request.limits.maximum_cells
        || request.permutations > 9_999
        || work > request.limits.maximum_distance_evaluations
        || !valid_null
    {
        return Err(invalid(
            "mutual-information format, k (1..=64, below the cell count), null model, \
             permutations (at most 9999) or limits are invalid",
        ));
    }
    let mut ids = std::collections::HashSet::new();
    if !request.cells.iter().all(|cell| {
        !cell.id.trim().is_empty()
            && ids.insert(cell.id.as_str())
            && [cell.x_um, cell.y_um, cell.first, cell.second]
                .iter()
                .all(|value| value.is_finite())
    }) {
        return Err(invalid(
            "cells need unique identifiers and finite positions and marks",
        ));
    }
    let first = standardized(request.cells.iter().map(|cell| cell.first), "first")?;
    let mut second = standardized(request.cells.iter().map(|cell| cell.second), "second")?;
    let estimator = Estimator::new(&first, &second, k);
    let estimate = estimator.estimate(&second).ok_or_else(|| {
        invalid(format!(
            "a cell has at least {k} other cells with identical values of both marks; the \
             estimator needs continuous marks"
        ))
    })?;

    let original = second.clone();
    let mut null = Vec::with_capacity(request.permutations);
    let mut blocked = None;
    let mut matched = None;
    match &request.null {
        NullModel::BlockPermutation {
            origin_um,
            width_um,
        } => {
            let mut members = BTreeMap::<(i64, i64), Vec<usize>>::new();
            for (index, cell) in request.cells.iter().enumerate() {
                let column = ((cell.x_um - origin_um[0]) / width_um).floor() as i64;
                let row = ((cell.y_um - origin_um[1]) / width_um).floor() as i64;
                members.entry((column, row)).or_default().push(index);
            }
            let endpoint = SeedEndpoint::MutualInformationBlockPermutation;
            let mut values = Vec::new();
            for permutation in 0..request.permutations {
                let stream = derive_seed(request.seed, endpoint, permutation);
                for (block, cells) in members.values().enumerate() {
                    values.clear();
                    values.extend(cells.iter().map(|&cell| original[cell]));
                    deterministic_shuffle(&mut values, derive_seed(stream, endpoint, block));
                    for (&cell, &value) in cells.iter().zip(&values) {
                        second[cell] = value;
                    }
                }
                null.push(null_estimate(&estimator, &second)?);
            }
            let singletons = members.values().filter(|cells| cells.len() == 1).count();
            blocked = Some((members.len(), singletons));
        }
        NullModel::VariogramMatching {
            smoothing_neighbors,
            lag_edges_um,
        } => {
            let xs = request
                .cells
                .iter()
                .map(|cell| cell.x_um)
                .collect::<Vec<_>>();
            let ys = request
                .cells
                .iter()
                .map(|cell| cell.y_um)
                .collect::<Vec<_>>();
            let surrogates = VariogramSurrogates::new(
                &xs,
                &ys,
                &original,
                smoothing_neighbors,
                lag_edges_um,
                request.limits.maximum_variogram_pairs,
            )?;
            let mut selected = vec![0_usize; smoothing_neighbors.len()];
            let mut fit_total = 0.0;
            for draw in 0..request.permutations {
                let seed = derive_seed(
                    request.seed,
                    SeedEndpoint::MutualInformationVariogramSurrogate,
                    draw,
                );
                let used = surrogates.draw(seed, &mut second)?;
                selected[used.candidate] += 1;
                fit_total += used.r_squared;
                null.push(null_estimate(&estimator, &second)?);
            }
            let mean_fit =
                (request.permutations > 0).then(|| fit_total / request.permutations as f64);
            matched = Some((surrogates.pairs(), selected, mean_fit));
        }
    }
    let summary = (!null.is_empty()).then(|| {
        let count = null.len() as f64;
        let mean = null.iter().sum::<f64>() / count;
        let variance = null.iter().map(|value| (value - mean).powi(2)).sum::<f64>() / count;
        let exceed = null.iter().filter(|&&value| value >= estimate).count();
        (mean, variance.sqrt(), (1 + exceed) as f64 / (count + 1.0))
    });
    Ok(MutualInformationResult {
        format: MUTUAL_INFORMATION_FORMAT.into(),
        version: 1,
        coordinate_frame_id: request.coordinate_frame_id,
        first_name: request.first_name,
        second_name: request.second_name,
        cells: n,
        neighbors_k: k,
        tied_first_cells: tied_cells(&first),
        tied_second_cells: tied_cells(&original),
        estimate_nats: estimate,
        null: request.null,
        occupied_blocks: blocked.map(|value| value.0),
        singleton_blocks: blocked.map(|value| value.1),
        variogram_pairs: matched.as_ref().map(|value| value.0),
        mean_variogram_fit_r_squared: matched.as_ref().and_then(|value| value.2),
        selected_smoothing_neighbors: matched.map(|value| value.1),
        permutations: request.permutations,
        seed: request.seed,
        null_mean_nats: summary.map(|value| value.0),
        null_standard_deviation_nats: summary.map(|value| value.1),
        excess_over_null_nats: summary.map(|value| estimate - value.0),
        upper_p_value: summary.map(|value| value.2),
        claim_scope: "research_spatial_null_mutual_information".into(),
    })
}

/// A null draw can create joint ties the observed marks lack: reported, not skipped.
fn null_estimate(estimator: &Estimator<'_>, second: &[f64]) -> Result<f64> {
    estimator.estimate(second).ok_or_else(|| {
        invalid("a null draw produced identical joint values; marks are not continuous")
    })
}

/// Divide by the population standard deviation; a constant mark carries no information.
fn standardized(values: impl Iterator<Item = f64>, name: &str) -> Result<Vec<f64>> {
    let values = values.collect::<Vec<_>>();
    let count = values.len() as f64;
    let mean = values.iter().sum::<f64>() / count;
    let deviation = (values
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / count)
        .sqrt();
    if !(deviation.is_finite() && deviation > 0.0) {
        return Err(invalid(format!(
            "the {name} mark is constant or not finite"
        )));
    }
    Ok(values.iter().map(|value| value / deviation).collect())
}

fn tied_cells(values: &[f64]) -> usize {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    sorted
        .iter()
        .enumerate()
        .filter(|&(index, value)| {
            (index > 0 && sorted[index - 1] == *value)
                || sorted.get(index + 1).is_some_and(|next| next == value)
        })
        .count()
}

/// KSG algorithm 1 with the first mark fixed; the second mark may be permuted between calls.
struct Estimator<'a> {
    first: &'a [f64],
    k: usize,
    /// Cell indices ordered by the first mark, and that mark in the same order.
    order: Vec<usize>,
    first_sorted: Vec<f64>,
    /// The second mark's values in order; permutation does not change the multiset.
    second_sorted: Vec<f64>,
    constant: f64,
}

impl<'a> Estimator<'a> {
    fn new(first: &'a [f64], second: &[f64], k: usize) -> Self {
        let mut order = (0..first.len()).collect::<Vec<_>>();
        order.sort_by(|&a, &b| first[a].total_cmp(&first[b]));
        let first_sorted = order.iter().map(|&index| first[index]).collect();
        let mut second_sorted = second.to_vec();
        second_sorted.sort_by(f64::total_cmp);
        let n = first.len() as f64;
        Self {
            first,
            k,
            order,
            first_sorted,
            second_sorted,
            constant: digamma(n) + digamma(k as f64),
        }
    }

    /// `None` when some cell's `k`-th joint neighbor is at distance zero.
    fn estimate(&self, second: &[f64]) -> Option<f64> {
        let n = self.first.len();
        let mut nearest = Vec::with_capacity(self.k);
        let (mut sum_x, mut sum_y) = (0.0, 0.0);
        for (rank, &cell) in self.order.iter().enumerate() {
            let (a, b) = (self.first[cell], second[cell]);
            nearest.clear();
            // Scan outward in first-mark order; |Δa| bounds the Chebyshev distance.
            let (mut left, mut right) = (rank, rank + 1);
            loop {
                let bound = (nearest.len() == self.k).then(|| nearest[self.k - 1]);
                let left_gap = (left > 0).then(|| a - self.first_sorted[left - 1]);
                let right_gap = (right < n).then(|| self.first_sorted[right] - a);
                let step = match (left_gap, right_gap) {
                    (Some(l), Some(r)) => Some(if l <= r { (l, true) } else { (r, false) }),
                    (Some(l), None) => Some((l, true)),
                    (None, Some(r)) => Some((r, false)),
                    (None, None) => None,
                };
                let Some((gap, go_left)) = step else { break };
                if bound.is_some_and(|bound| gap >= bound) {
                    break;
                }
                let other = if go_left {
                    left -= 1;
                    self.order[left]
                } else {
                    right += 1;
                    self.order[right - 1]
                };
                let distance = (self.first[other] - a).abs().max((second[other] - b).abs());
                let position = nearest.partition_point(|&value| value <= distance);
                if position < self.k {
                    if nearest.len() == self.k {
                        nearest.pop();
                    }
                    nearest.insert(position, distance);
                }
            }
            let epsilon = nearest[self.k - 1];
            if epsilon <= 0.0 {
                return None;
            }
            sum_x += digamma(strictly_within(&self.first_sorted, a, epsilon) as f64);
            sum_y += digamma(strictly_within(&self.second_sorted, b, epsilon) as f64);
        }
        Some(self.constant - (sum_x + sum_y) / n as f64)
    }
}

/// `1 + #{j ≠ i : |v_j − v_i| < ε}`: the count of sorted values within `ε` of `value`,
/// which includes the cell itself. Differences are compared exactly as computed.
fn strictly_within(sorted: &[f64], value: f64, epsilon: f64) -> usize {
    let lower = sorted.partition_point(|&other| value - other >= epsilon);
    let upper = sorted.partition_point(|&other| other - value < epsilon);
    upper - lower
}
