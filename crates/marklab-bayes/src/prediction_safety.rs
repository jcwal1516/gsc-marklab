use serde::Serialize;
use std::collections::HashSet;
use thiserror::Error;

const MAXIMUM_OOD_UNITS: usize = 100_000;
const MAXIMUM_OOD_DIMENSIONS: usize = 128;

#[derive(Clone, Copy, Debug)]
pub struct AbstentionPolicy {
    pub maximum_uncertainty: f64,
    pub maximum_ood_score: f64,
}

#[derive(Clone, Debug)]
pub struct PredictionForAbstention {
    pub prediction_id: String,
    pub prediction: f64,
    pub uncertainty: f64,
    pub ood_score: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AbstentionStatus {
    Retained,
    Abstained,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AbstentionDecision {
    pub prediction_id: String,
    pub status: AbstentionStatus,
    pub prediction: Option<f64>,
    pub uncertainty: f64,
    pub ood_score: f64,
    pub reasons: Vec<&'static str>,
}

#[derive(Debug, Error)]
pub enum PredictionSafetyError {
    #[error("invalid abstention input: {0}")]
    Invalid(String),
}

#[derive(Clone, Debug)]
pub struct OodRepresentationUnit {
    pub unit_id: String,
    pub split: String,
    pub domain: String,
    pub representation: Vec<f64>,
}

#[derive(Clone, Debug)]
pub struct MahalanobisOodSpec {
    pub units: Vec<OodRepresentationUnit>,
    pub feature_names: Vec<String>,
    pub shrinkage: f64,
    pub validation_quantile: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct OodUnitScore {
    pub unit_id: String,
    pub domain: String,
    pub score: f64,
    pub exceeds_threshold: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MahalanobisOodResult {
    pub format: &'static str,
    pub version: u32,
    pub method: &'static str,
    pub fit_split: &'static str,
    pub threshold_split: &'static str,
    pub feature_names: Vec<String>,
    pub training_mean: Vec<f64>,
    pub shrunk_population_covariance: Vec<f64>,
    pub shrinkage: f64,
    pub validation_quantile: f64,
    pub validation_count: u32,
    pub threshold: f64,
    pub scores: Vec<OodUnitScore>,
}

pub fn apply_abstention(
    input: PredictionForAbstention,
    policy: AbstentionPolicy,
) -> Result<AbstentionDecision, PredictionSafetyError> {
    if input.prediction_id.trim().is_empty()
        || input.prediction_id.trim() != input.prediction_id
        || !input.prediction.is_finite()
        || !input.uncertainty.is_finite()
        || input.uncertainty < 0.0
        || !input.ood_score.is_finite()
        || input.ood_score < 0.0
    {
        return Err(PredictionSafetyError::Invalid(
            "prediction identity and values must be valid and finite".into(),
        ));
    }
    if !policy.maximum_uncertainty.is_finite()
        || policy.maximum_uncertainty < 0.0
        || !policy.maximum_ood_score.is_finite()
        || policy.maximum_ood_score < 0.0
    {
        return Err(PredictionSafetyError::Invalid(
            "abstention thresholds must be finite and nonnegative".into(),
        ));
    }

    let mut reasons = Vec::with_capacity(2);
    if input.uncertainty > policy.maximum_uncertainty {
        reasons.push("uncertainty_exceeds_threshold");
    }
    if input.ood_score > policy.maximum_ood_score {
        reasons.push("ood_score_exceeds_threshold");
    }
    let status = if reasons.is_empty() {
        AbstentionStatus::Retained
    } else {
        AbstentionStatus::Abstained
    };
    Ok(AbstentionDecision {
        prediction_id: input.prediction_id,
        status,
        prediction: (status == AbstentionStatus::Retained).then_some(input.prediction),
        uncertainty: input.uncertainty,
        ood_score: input.ood_score,
        reasons,
    })
}

pub fn mahalanobis_ood_score(
    mut spec: MahalanobisOodSpec,
) -> Result<MahalanobisOodResult, PredictionSafetyError> {
    let dimension = spec.feature_names.len();
    if !((2..=MAXIMUM_OOD_DIMENSIONS).contains(&dimension)
        && !spec.units.is_empty()
        && spec.units.len() <= MAXIMUM_OOD_UNITS
        && (0.0..=1.0).contains(&spec.shrinkage)
        && 0.0 < spec.validation_quantile
        && spec.validation_quantile < 1.0)
    {
        return Err(PredictionSafetyError::Invalid(
            "OOD dimensions, shrinkage, or validation quantile are invalid".into(),
        ));
    }
    let mut feature_names = HashSet::with_capacity(dimension);
    if spec
        .feature_names
        .iter()
        .any(|name| !name.starts_with("embedding_") || !feature_names.insert(name.as_str()))
    {
        return Err(PredictionSafetyError::Invalid(
            "OOD feature names must be unique embedding_ names".into(),
        ));
    }
    spec.units
        .sort_by(|left, right| left.unit_id.cmp(&right.unit_id));
    let mut ids = HashSet::with_capacity(spec.units.len());
    for unit in &spec.units {
        if unit.unit_id.trim().is_empty()
            || unit.domain.trim().is_empty()
            || !ids.insert(unit.unit_id.as_str())
            || !matches!(unit.split.as_str(), "train" | "validation" | "test")
            || unit.representation.len() != dimension
            || unit.representation.iter().any(|value| !value.is_finite())
        {
            return Err(PredictionSafetyError::Invalid(
                "OOD unit identity, split, domain, or representation is invalid".into(),
            ));
        }
    }
    let training = spec
        .units
        .iter()
        .filter(|unit| unit.split == "train")
        .collect::<Vec<_>>();
    let validation = spec
        .units
        .iter()
        .filter(|unit| unit.split == "validation")
        .collect::<Vec<_>>();
    let test = spec
        .units
        .iter()
        .filter(|unit| unit.split == "test")
        .collect::<Vec<_>>();
    if training.len() < dimension + 1 || validation.len() < 3 || test.is_empty() {
        return Err(PredictionSafetyError::Invalid(
            "OOD requires at least dimension+1 training, three validation, and one test unit"
                .into(),
        ));
    }
    let training_domains = training
        .iter()
        .map(|unit| unit.domain.as_str())
        .collect::<HashSet<_>>();
    if validation
        .iter()
        .any(|unit| training_domains.contains(unit.domain.as_str()))
    {
        return Err(PredictionSafetyError::Invalid(
            "OOD validation domains must be held out from training domains".into(),
        ));
    }

    let mut mean = vec![0.0; dimension];
    for unit in &training {
        for (total, value) in mean.iter_mut().zip(&unit.representation) {
            *total += value;
        }
    }
    for value in &mut mean {
        *value /= training.len() as f64;
    }
    let mut covariance = vec![0.0; dimension * dimension];
    for unit in &training {
        for row in 0..dimension {
            let row_value = unit.representation[row] - mean[row];
            for column in 0..dimension {
                covariance[row * dimension + column] +=
                    row_value * (unit.representation[column] - mean[column]);
            }
        }
    }
    for value in &mut covariance {
        *value /= training.len() as f64;
    }
    for row in 0..dimension {
        if covariance[row * dimension + row] <= 0.0 {
            return Err(PredictionSafetyError::Invalid(
                "every OOD feature must vary in training".into(),
            ));
        }
        for column in 0..dimension {
            if row != column {
                covariance[row * dimension + column] *= 1.0 - spec.shrinkage;
            }
        }
    }
    let lower = cholesky(&covariance, dimension)?;
    let score = |unit: &OodRepresentationUnit| {
        let centered = unit
            .representation
            .iter()
            .zip(&mean)
            .map(|(value, center)| value - center)
            .collect::<Vec<_>>();
        solve_lower(&lower, dimension, &centered)
            .iter()
            .map(|value| value * value)
            .sum::<f64>()
            .sqrt()
    };
    let mut validation_scores = validation
        .iter()
        .map(|unit| score(unit))
        .collect::<Vec<_>>();
    if validation_scores.iter().any(|value| !value.is_finite()) {
        return Err(PredictionSafetyError::Invalid(
            "OOD validation score is non-finite".into(),
        ));
    }
    validation_scores.sort_by(f64::total_cmp);
    let rank = (spec.validation_quantile * validation_scores.len() as f64).ceil() as usize;
    let threshold = validation_scores[rank.saturating_sub(1)];
    let scores = test
        .iter()
        .map(|unit| {
            let value = score(unit);
            OodUnitScore {
                unit_id: unit.unit_id.clone(),
                domain: unit.domain.clone(),
                score: value,
                exceeds_threshold: value > threshold,
            }
        })
        .collect::<Vec<_>>();
    if scores.iter().any(|row| !row.score.is_finite()) {
        return Err(PredictionSafetyError::Invalid(
            "OOD test score is non-finite".into(),
        ));
    }
    Ok(MahalanobisOodResult {
        format: "marklab.ood_score",
        version: 1,
        method: "mahalanobis_shrinkage",
        fit_split: "train",
        threshold_split: "validation",
        feature_names: spec.feature_names,
        training_mean: mean,
        shrunk_population_covariance: covariance,
        shrinkage: spec.shrinkage,
        validation_quantile: spec.validation_quantile,
        validation_count: validation_scores.len() as u32,
        threshold,
        scores,
    })
}

fn cholesky(matrix: &[f64], dimension: usize) -> Result<Vec<f64>, PredictionSafetyError> {
    let mut lower = vec![0.0; matrix.len()];
    for row in 0..dimension {
        for column in 0..=row {
            let prior = (0..column)
                .map(|index| lower[row * dimension + index] * lower[column * dimension + index])
                .sum::<f64>();
            if row == column {
                let diagonal = matrix[row * dimension + row] - prior;
                if !diagonal.is_finite() || diagonal <= 0.0 {
                    return Err(PredictionSafetyError::Invalid(
                        "shrunk training covariance is not positive definite".into(),
                    ));
                }
                lower[row * dimension + column] = diagonal.sqrt();
            } else {
                lower[row * dimension + column] =
                    (matrix[row * dimension + column] - prior) / lower[column * dimension + column];
            }
        }
    }
    Ok(lower)
}

fn solve_lower(lower: &[f64], dimension: usize, right: &[f64]) -> Vec<f64> {
    let mut solution = vec![0.0; dimension];
    for row in 0..dimension {
        let prior = (0..row)
            .map(|column| lower[row * dimension + column] * solution[column])
            .sum::<f64>();
        solution[row] = (right[row] - prior) / lower[row * dimension + row];
    }
    solution
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equality_is_retained_but_exceedance_abstains() {
        let policy = AbstentionPolicy {
            maximum_uncertainty: 0.25,
            maximum_ood_score: 1.0,
        };
        let retained = apply_abstention(
            PredictionForAbstention {
                prediction_id: "boundary".into(),
                prediction: 0.5,
                uncertainty: 0.25,
                ood_score: 1.0,
            },
            policy,
        )
        .expect("retained");
        assert_eq!(retained.status, AbstentionStatus::Retained);
        let abstained = apply_abstention(
            PredictionForAbstention {
                prediction_id: "outside".into(),
                prediction: 0.5,
                uncertainty: 0.250_001,
                ood_score: 1.0,
            },
            policy,
        )
        .expect("abstained");
        assert_eq!(abstained.status, AbstentionStatus::Abstained);
        assert_eq!(abstained.prediction, None);
    }
}
