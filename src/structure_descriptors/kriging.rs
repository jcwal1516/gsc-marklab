//! Ordinary kriging of one cell-level value with a declared variogram and held-out checks.
//!
//! The semivariogram is declared, not fitted here: `γ(h) = b + c (1 − e^{−h/a})`
//! (exponential), `b + c (1.5 h/a − 0.5 (h/a)³)` for `h ≤ a` and `b + c` beyond (spherical), or
//! `b + c (1 − e^{−h²/a²})` (Gaussian), with nugget `b`, partial sill `c` and range parameter
//! `a`, and `γ(0) = 0`. A prediction at `x₀` uses the observations within the declared search
//! radius, nearest first (ties by input order), up to `maximum_neighbors`; with fewer than
//! `minimum_neighbors` it is unavailable. The weights solve the ordinary-kriging system
//! `[Γ 1; 1ᵀ 0][λ; μ] = [γ₀; 1]` and the kriging variance is `Σ λᵢ γ₀ᵢ + μ`, the error
//! variance for a new observation at `x₀` (it includes the nugget).
//!
//! Held-out check: square blocks of the declared width tile the plane from the declared
//! origin. Each nonempty block is left out in turn and its observations are predicted from
//! the rest. The summary reports RMSE, mean error, the mean and mean square of the
//! standardized errors `(z − ẑ)/σ` and the coverage of `ẑ ± 1.959964 σ`. A well-specified
//! model has mean squared standardized error near 1 and coverage near 0.95.

use serde::{Deserialize, Serialize};

use crate::errors::{MarklabError, Result};

/// Result format of [`ordinary_kriging_from_json`].
pub const ORDINARY_KRIGING_FORMAT: &str = "marklab.ordinary_kriging";
const MAXIMUM_INPUT_BYTES: usize = 16 * 1024 * 1024;
const Z_975: f64 = 1.959_963_984_540_054;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    format: String,
    version: u32,
    coordinate_frame_id: String,
    value_name: String,
    value_unit: String,
    observations: Vec<Observation>,
    variogram: Variogram,
    neighborhood: Neighborhood,
    cross_validation: BlockDesign,
    targets: Vec<Target>,
    limits: Limits,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    id: String,
    x_um: f64,
    y_um: f64,
    value: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Target {
    id: String,
    x_um: f64,
    y_um: f64,
}

/// Declared semivariogram model.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Variogram {
    pub model: VariogramModel,
    pub range_um: f64,
    pub partial_sill: f64,
    pub nugget: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VariogramModel {
    Exponential,
    Spherical,
    Gaussian,
}

impl Variogram {
    /// Semivariance at a positive lag; `γ(0) = 0` is applied by the caller.
    fn at(&self, lag: f64) -> f64 {
        let scaled = lag / self.range_um;
        let shape = match self.model {
            VariogramModel::Exponential => 1.0 - (-scaled).exp(),
            VariogramModel::Spherical if lag <= self.range_um => {
                1.5 * scaled - 0.5 * scaled.powi(3)
            }
            VariogramModel::Spherical => 1.0,
            VariogramModel::Gaussian => 1.0 - (-scaled * scaled).exp(),
        };
        self.nugget + self.partial_sill * shape
    }
}

/// Declared neighbor search.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Neighborhood {
    pub search_radius_um: f64,
    pub minimum_neighbors: usize,
    pub maximum_neighbors: usize,
}

/// Declared held-out blocks.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BlockDesign {
    pub block_width_um: f64,
    pub origin_um: [f64; 2],
}

#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct Limits {
    maximum_observations: usize,
    maximum_targets: usize,
    maximum_distance_evaluations: u64,
}

/// One kriging prediction.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KrigingPrediction {
    pub id: String,
    pub x_um: f64,
    pub y_um: f64,
    pub neighbors: usize,
    pub prediction: Option<f64>,
    pub kriging_variance: Option<f64>,
    pub unavailable: Option<String>,
}

/// One held-out observation.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HeldOutPrediction {
    pub block: [i64; 2],
    pub observed: f64,
    /// `(z − ẑ) / σ`, when a positive kriging variance is available.
    pub standardized_error: Option<f64>,
    pub prediction: KrigingPrediction,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HeldOutSummary {
    pub available: usize,
    pub unavailable: usize,
    pub rmse: Option<f64>,
    /// Mean of `z − ẑ`.
    pub mean_error: Option<f64>,
    pub mean_standardized_error: Option<f64>,
    pub mean_squared_standardized_error: Option<f64>,
    pub coverage_95: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BlockCrossValidation {
    pub design: BlockDesign,
    pub blocks: usize,
    pub held_out: Vec<HeldOutPrediction>,
    pub summary: HeldOutSummary,
}

/// Ordinary kriging predictions and the leave-one-block-out check.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrdinaryKrigingResult {
    pub format: String,
    pub version: u32,
    pub coordinate_frame_id: String,
    pub value_name: String,
    pub value_unit: String,
    pub variogram: Variogram,
    pub neighborhood: Neighborhood,
    pub observations: usize,
    pub targets: Vec<KrigingPrediction>,
    pub cross_validation: BlockCrossValidation,
    pub distance_evaluations: u64,
    pub claim_scope: String,
}

fn invalid(message: impl Into<String>) -> MarklabError {
    MarklabError::Validation(message.into())
}

struct Kriging {
    xs: Vec<f64>,
    ys: Vec<f64>,
    values: Vec<f64>,
    variogram: Variogram,
    neighborhood: Neighborhood,
    evaluations: u64,
    maximum_evaluations: u64,
}

impl Kriging {
    /// Predict at `(x, y)` from the observations whose `available` flag is set.
    fn predict(
        &mut self,
        id: &str,
        x: f64,
        y: f64,
        available: &[bool],
    ) -> Result<KrigingPrediction> {
        self.evaluations = self
            .evaluations
            .checked_add(self.xs.len() as u64)
            .filter(|count| *count <= self.maximum_evaluations)
            .ok_or_else(|| invalid("kriging distance evaluations exceed the declared limit"))?;
        let mut candidates = (0..self.xs.len())
            .filter(|&index| available[index])
            .map(|index| (self.distance(index, x, y), index))
            .filter(|(distance, _)| *distance <= self.neighborhood.search_radius_um)
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| left.0.total_cmp(&right.0).then(left.1.cmp(&right.1)));
        candidates.truncate(self.neighborhood.maximum_neighbors);
        let neighbors = candidates.len();
        let mut prediction = KrigingPrediction {
            id: id.to_owned(),
            x_um: x,
            y_um: y,
            neighbors,
            prediction: None,
            kriging_variance: None,
            unavailable: None,
        };
        if neighbors < self.neighborhood.minimum_neighbors {
            prediction.unavailable = Some(format!(
                "{neighbors} observations within the search radius; at least {} are required",
                self.neighborhood.minimum_neighbors
            ));
            return Ok(prediction);
        }
        let size = neighbors + 1;
        let mut matrix = vec![vec![1.0; size]; size];
        matrix[neighbors][neighbors] = 0.0;
        for (row, (_, first)) in candidates.iter().enumerate() {
            for (column, (_, second)) in candidates.iter().enumerate() {
                matrix[row][column] = if row == column {
                    0.0
                } else {
                    let lag = self.pair_distance(*first, *second);
                    if lag == 0.0 {
                        0.0
                    } else {
                        self.variogram.at(lag)
                    }
                };
            }
        }
        let mut right = candidates
            .iter()
            .map(|(distance, _)| {
                if *distance == 0.0 {
                    0.0
                } else {
                    self.variogram.at(*distance)
                }
            })
            .collect::<Vec<_>>();
        right.push(1.0);
        let Some(solution) = marklab_numerics::dense::gauss_jordan_solve(&matrix, &right) else {
            prediction.unavailable = Some("the kriging system is numerically singular".into());
            return Ok(prediction);
        };
        let estimate = candidates
            .iter()
            .zip(&solution)
            .map(|((_, index), weight)| weight * self.values[*index])
            .sum::<f64>();
        let variance = right[..neighbors]
            .iter()
            .zip(&solution)
            .map(|(gamma, weight)| gamma * weight)
            .sum::<f64>()
            + solution[neighbors];
        if !estimate.is_finite() || !variance.is_finite() {
            prediction.unavailable = Some("the kriging solution is not finite".into());
            return Ok(prediction);
        }
        prediction.prediction = Some(estimate);
        prediction.kriging_variance = Some(variance);
        Ok(prediction)
    }

    fn distance(&self, index: usize, x: f64, y: f64) -> f64 {
        (self.xs[index] - x).hypot(self.ys[index] - y)
    }

    fn pair_distance(&self, first: usize, second: usize) -> f64 {
        self.distance(first, self.xs[second], self.ys[second])
    }
}

/// Parse a `marklab.ordinary_kriging_request` version 1 and krige it.
pub fn ordinary_kriging_from_json(input: &[u8]) -> Result<OrdinaryKrigingResult> {
    if input.len() > MAXIMUM_INPUT_BYTES {
        return Err(invalid("ordinary kriging input exceeds 16 MiB"));
    }
    let request: Request = serde_json::from_slice(input)?;
    let (variogram, neighborhood, design, limits) = (
        request.variogram,
        request.neighborhood,
        request.cross_validation,
        request.limits,
    );
    let positive = |value: f64| value.is_finite() && value > 0.0;
    if request.format != "marklab.ordinary_kriging_request"
        || request.version != 1
        || request.coordinate_frame_id.trim().is_empty()
        || request.value_name.trim().is_empty()
        || !positive(variogram.range_um)
        || !positive(variogram.partial_sill)
        || !(variogram.nugget.is_finite() && variogram.nugget >= 0.0)
        || !positive(neighborhood.search_radius_um)
        || neighborhood.minimum_neighbors < 1
        || neighborhood.maximum_neighbors < neighborhood.minimum_neighbors
        || neighborhood.maximum_neighbors > 512
        || !positive(design.block_width_um)
        || !design.origin_um.iter().all(|value| value.is_finite())
        || !(2..=limits.maximum_observations).contains(&request.observations.len())
        || request.targets.len() > limits.maximum_targets
    {
        return Err(invalid(
            "ordinary kriging format, variogram, neighborhood, blocks or limits are invalid",
        ));
    }
    let mut ids = std::collections::HashSet::new();
    let finite = |values: &[f64]| values.iter().all(|value| value.is_finite());
    if !request.observations.iter().all(|row| {
        !row.id.trim().is_empty()
            && ids.insert(row.id.as_str())
            && finite(&[row.x_um, row.y_um, row.value])
    }) || !request
        .targets
        .iter()
        .all(|row| !row.id.trim().is_empty() && finite(&[row.x_um, row.y_um]))
    {
        return Err(invalid(
            "kriging observations need unique identifiers and finite coordinates and values",
        ));
    }
    let mut kriging = Kriging {
        xs: request.observations.iter().map(|row| row.x_um).collect(),
        ys: request.observations.iter().map(|row| row.y_um).collect(),
        values: request.observations.iter().map(|row| row.value).collect(),
        variogram,
        neighborhood,
        evaluations: 0,
        maximum_evaluations: limits.maximum_distance_evaluations,
    };
    let everything = vec![true; request.observations.len()];
    let targets = request
        .targets
        .iter()
        .map(|target| kriging.predict(&target.id, target.x_um, target.y_um, &everything))
        .collect::<Result<Vec<_>>>()?;
    let block = |x: f64, y: f64| {
        [
            ((x - design.origin_um[0]) / design.block_width_um).floor() as i64,
            ((y - design.origin_um[1]) / design.block_width_um).floor() as i64,
        ]
    };
    let blocks = request
        .observations
        .iter()
        .map(|row| block(row.x_um, row.y_um))
        .collect::<Vec<_>>();
    let distinct = blocks
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let mut held_out = Vec::with_capacity(blocks.len());
    for (index, row) in request.observations.iter().enumerate() {
        let available = blocks
            .iter()
            .map(|other| *other != blocks[index])
            .collect::<Vec<_>>();
        let prediction = kriging.predict(&row.id, row.x_um, row.y_um, &available)?;
        let standardized_error = match (prediction.prediction, prediction.kriging_variance) {
            (Some(estimate), Some(variance)) if variance > 0.0 => {
                Some((row.value - estimate) / variance.sqrt())
            }
            _ => None,
        };
        held_out.push(HeldOutPrediction {
            block: blocks[index],
            observed: row.value,
            standardized_error,
            prediction,
        });
    }
    let summary = summarize(&held_out);
    Ok(OrdinaryKrigingResult {
        format: ORDINARY_KRIGING_FORMAT.into(),
        version: 1,
        coordinate_frame_id: request.coordinate_frame_id,
        value_name: request.value_name,
        value_unit: request.value_unit,
        variogram,
        neighborhood,
        observations: request.observations.len(),
        targets,
        cross_validation: BlockCrossValidation {
            design,
            blocks: distinct,
            held_out,
            summary,
        },
        distance_evaluations: kriging.evaluations,
        claim_scope: "declared_variogram_ordinary_kriging_with_block_held_out_checks".into(),
    })
}

fn summarize(held_out: &[HeldOutPrediction]) -> HeldOutSummary {
    let available = held_out
        .iter()
        .filter_map(|row| {
            Some((
                row.observed - row.prediction.prediction?,
                row.standardized_error?,
            ))
        })
        .collect::<Vec<_>>();
    let count = available.len();
    let mean = |values: &mut dyn Iterator<Item = f64>| {
        (count > 0).then(|| values.sum::<f64>() / count as f64)
    };
    HeldOutSummary {
        available: count,
        unavailable: held_out.len() - count,
        rmse: mean(&mut available.iter().map(|(error, _)| error * error)).map(f64::sqrt),
        mean_error: mean(&mut available.iter().map(|(error, _)| *error)),
        mean_standardized_error: mean(&mut available.iter().map(|(_, z)| *z)),
        mean_squared_standardized_error: mean(&mut available.iter().map(|(_, z)| z * z)),
        coverage_95: mean(
            &mut available
                .iter()
                .map(|(_, z)| f64::from(u8::from(z.abs() <= Z_975))),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variogram_models_follow_their_declared_forms() {
        let declared = |model| Variogram {
            model,
            range_um: 10.0,
            partial_sill: 2.0,
            nugget: 0.5,
        };
        let exponential = declared(VariogramModel::Exponential);
        assert_eq!(exponential.at(10.0), 0.5 + 2.0 * (1.0 - (-1.0_f64).exp()));
        let spherical = declared(VariogramModel::Spherical);
        assert_eq!(spherical.at(5.0), 0.5 + 2.0 * (0.75 - 0.0625));
        assert_eq!(spherical.at(20.0), 2.5);
        let gaussian = declared(VariogramModel::Gaussian);
        assert_eq!(gaussian.at(10.0), 0.5 + 2.0 * (1.0 - (-1.0_f64).exp()));
    }
}

#[cfg(test)]
mod system_tests {
    use super::*;

    fn kriging(xs: &[f64], ys: &[f64], values: &[f64], nugget: f64) -> Kriging {
        Kriging {
            xs: xs.to_vec(),
            ys: ys.to_vec(),
            values: values.to_vec(),
            variogram: Variogram {
                model: VariogramModel::Exponential,
                range_um: 10.0,
                partial_sill: 1.0,
                nugget,
            },
            neighborhood: Neighborhood {
                search_radius_um: 100.0,
                minimum_neighbors: 1,
                maximum_neighbors: 8,
            },
            evaluations: 0,
            maximum_evaluations: 1_000,
        }
    }

    #[test]
    fn symmetric_pair_and_exact_interpolation_have_closed_forms() {
        // Two observations 8 um apart, target midway: weights 1/2, mu = g - gamma(8)/2, and the
        // variance is 2 g - gamma(8)/2 with g = gamma(4).
        let mut model = kriging(&[0.0, 8.0], &[0.0, 0.0], &[1.0, 3.0], 0.2);
        let result = model.predict("t", 4.0, 0.0, &[true, true]).unwrap();
        let gamma = |h: f64| 0.2 + (1.0 - (-h / 10.0).exp());
        assert!((result.prediction.unwrap() - 2.0).abs() < 1e-14);
        let expected = 2.0 * gamma(4.0) - gamma(8.0) / 2.0;
        assert!((result.kriging_variance.unwrap() - expected).abs() < 1e-14);
        // Without a nugget, kriging at an observed location returns it with zero variance.
        let mut exact = kriging(&[0.0, 8.0, 3.0], &[0.0, 0.0, 5.0], &[1.0, 3.0, -2.0], 0.0);
        let at_data = exact.predict("o", 3.0, 5.0, &[true, true, true]).unwrap();
        assert!((at_data.prediction.unwrap() + 2.0).abs() < 1e-12);
        assert!(at_data.kriging_variance.unwrap().abs() < 1e-12);
        // Too few neighbors leaves the prediction unavailable.
        let empty = exact
            .predict("x", 3.0, 5.0, &[false, false, false])
            .unwrap();
        assert!(empty.prediction.is_none() && empty.unavailable.is_some());
    }
}
