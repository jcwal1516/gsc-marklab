use std::collections::{BTreeMap, HashSet};

use super::{
    numeric::stable_mean, CohortInferenceError, InferenceDesign, MAXIMUM_PATIENTS,
    MAXIMUM_PERMUTATIONS,
};

const HIERARCHICAL_BOOTSTRAP_NAMESPACE: u64 = 0x6869_6572_5f62_6f6f;
const MAXIMUM_BOOTSTRAP_DRAWS: usize = 100_000_000;

/// One finite specimen-level endpoint nested under one patient.
#[derive(Clone, Debug)]
pub struct HierarchicalScalarRecord {
    /// Exact non-empty patient identity.
    pub patient_id: String,
    /// Globally unique non-empty specimen identity.
    pub specimen_id: String,
    /// Finite scalar endpoint.
    pub endpoint: f64,
}

/// Frozen patient-first two-level bootstrap design.
#[derive(Clone, Debug)]
pub struct HierarchicalBootstrapSpec {
    /// Positive bounded replicate count.
    pub replicates: usize,
    /// Base deterministic seed.
    pub seed: u64,
    /// Two-sided percentile interval alpha.
    pub alpha: f64,
}

/// Deterministic nearest-rank percentile interval.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HierarchicalBootstrapInterval {
    /// Lower empirical bound.
    pub lower: f64,
    /// Upper empirical bound.
    pub upper: f64,
    /// Confidence level `1-alpha`.
    pub level: f64,
}

/// Patient-first hierarchical scalar bootstrap result.
#[derive(Clone, Debug, PartialEq)]
pub struct HierarchicalBootstrapResult {
    /// Exact patient-then-nested-specimen resampling design.
    pub inference_design: InferenceDesign,
    /// Number of independent patients.
    pub patient_count: usize,
    /// Number of nested specimens.
    pub specimen_count: usize,
    /// Stable observed specimen-row mean.
    pub observed_mean: f64,
    /// Stored-order replicate means for oracle/reuse consumers.
    pub bootstrap_means: Vec<f64>,
    /// Nearest-rank percentile interval.
    pub interval: HierarchicalBootstrapInterval,
    /// Requested replicate count.
    pub replicates_requested: usize,
    /// Attempted replicate count.
    pub replicates_attempted: usize,
    /// Completed replicate count.
    pub replicates_completed: usize,
    /// Base seed.
    pub seed: u64,
    /// Interval alpha.
    pub alpha: f64,
}

#[derive(Clone, Debug)]
pub struct BootstrapEquivalenceSpec {
    pub lower_margin: f64,
    pub upper_margin: f64,
    pub margin_rationale: String,
    pub bootstrap: HierarchicalBootstrapSpec,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BootstrapEquivalenceResult {
    pub bootstrap: HierarchicalBootstrapResult,
    pub lower_margin: f64,
    pub upper_margin: f64,
    pub margin_rationale: String,
    pub equivalent: bool,
}

pub fn bootstrap_equivalence(
    records: &[HierarchicalScalarRecord],
    spec: &BootstrapEquivalenceSpec,
) -> Result<BootstrapEquivalenceResult, CohortInferenceError> {
    if !(spec.lower_margin.is_finite()
        && spec.upper_margin.is_finite()
        && spec.lower_margin < spec.upper_margin)
        || spec.margin_rationale.is_empty()
        || spec.margin_rationale.trim() != spec.margin_rationale
    {
        return Err(CohortInferenceError::InvalidInput(
            "bootstrap equivalence requires ordered finite margins and an exact rationale".into(),
        ));
    }
    let bootstrap = hierarchical_bootstrap(records, &spec.bootstrap)?;
    let equivalent = bootstrap.interval.lower > spec.lower_margin
        && bootstrap.interval.upper < spec.upper_margin;
    Ok(BootstrapEquivalenceResult {
        bootstrap,
        lower_margin: spec.lower_margin,
        upper_margin: spec.upper_margin,
        margin_rationale: spec.margin_rationale.clone(),
        equivalent,
    })
}

/// Resample patients first and specimens within each sampled patient occurrence.
pub fn hierarchical_bootstrap(
    records: &[HierarchicalScalarRecord],
    spec: &HierarchicalBootstrapSpec,
) -> Result<HierarchicalBootstrapResult, CohortInferenceError> {
    validate_spec(spec)?;
    let grouped = validate_and_group(records)?;
    let patient_count = grouped.len();
    let max_specimens = grouped
        .iter()
        .map(|specimens| specimens.len())
        .max()
        .unwrap_or(0);
    let maximum_draws = patient_count
        .checked_mul(max_specimens)
        .and_then(|draws| draws.checked_mul(spec.replicates))
        .ok_or_else(draw_limit_error)?;
    if maximum_draws > MAXIMUM_BOOTSTRAP_DRAWS {
        return Err(draw_limit_error());
    }
    let observed_values = grouped.iter().flatten().copied().collect::<Vec<_>>();
    let observed_mean = stable_mean(&observed_values)?;
    let inference_design = InferenceDesign::hierarchical_bootstrap(
        &grouped.iter().map(Vec::len).collect::<Vec<_>>(),
        spec.replicates,
        spec.seed,
        HIERARCHICAL_BOOTSTRAP_NAMESPACE,
    )
    .map_err(|error| CohortInferenceError::InvalidInput(error.to_string()))?;
    let mut bootstrap_means = Vec::with_capacity(spec.replicates);
    let mut sampled_values = Vec::with_capacity(patient_count * max_specimens);
    for replicate in 0..spec.replicates {
        sampled_values.clear();
        sampled_values.extend(
            inference_design
                .hierarchical_resample_indices(replicate)
                .map_err(|error| CohortInferenceError::InvalidInput(error.to_string()))?
                .iter()
                .map(|index| observed_values[*index]),
        );
        bootstrap_means.push(stable_mean(&sampled_values)?);
    }
    let mut ordered = bootstrap_means.clone();
    ordered.sort_by(f64::total_cmp);
    let interval = HierarchicalBootstrapInterval {
        lower: nearest_rank(&ordered, spec.alpha / 2.0),
        upper: nearest_rank(&ordered, 1.0 - spec.alpha / 2.0),
        level: 1.0 - spec.alpha,
    };
    Ok(HierarchicalBootstrapResult {
        inference_design,
        patient_count,
        specimen_count: records.len(),
        observed_mean,
        bootstrap_means,
        interval,
        replicates_requested: spec.replicates,
        replicates_attempted: spec.replicates,
        replicates_completed: spec.replicates,
        seed: spec.seed,
        alpha: spec.alpha,
    })
}

fn validate_spec(spec: &HierarchicalBootstrapSpec) -> Result<(), CohortInferenceError> {
    if spec.replicates == 0 || spec.replicates > MAXIMUM_PERMUTATIONS {
        return Err(CohortInferenceError::InvalidInput(format!(
            "bootstrap replicates must be between 1 and {MAXIMUM_PERMUTATIONS}"
        )));
    }
    if !(spec.alpha.is_finite() && spec.alpha > 0.0 && spec.alpha < 0.5) {
        return Err(CohortInferenceError::InvalidInput(
            "bootstrap alpha must be finite and strictly between zero and one half".into(),
        ));
    }
    Ok(())
}

fn validate_and_group(
    records: &[HierarchicalScalarRecord],
) -> Result<Vec<Vec<f64>>, CohortInferenceError> {
    if records.len() < 2 || records.len() > MAXIMUM_PATIENTS {
        return Err(CohortInferenceError::InvalidInput(format!(
            "hierarchical bootstrap requires 2 to {MAXIMUM_PATIENTS} specimen rows"
        )));
    }
    let mut specimens = HashSet::with_capacity(records.len());
    let mut grouped = BTreeMap::<&str, BTreeMap<&str, f64>>::new();
    for record in records {
        if record.patient_id.trim().is_empty() || record.patient_id.trim() != record.patient_id {
            return Err(CohortInferenceError::InvalidInput(
                "patient_id must be non-empty without surrounding whitespace".into(),
            ));
        }
        if record.specimen_id.trim().is_empty() || record.specimen_id.trim() != record.specimen_id {
            return Err(CohortInferenceError::InvalidInput(
                "specimen_id must be non-empty without surrounding whitespace".into(),
            ));
        }
        if !specimens.insert(record.specimen_id.as_str()) {
            return Err(CohortInferenceError::InvalidInput(format!(
                "duplicate specimen_id: {}",
                record.specimen_id
            )));
        }
        if !record.endpoint.is_finite() {
            return Err(CohortInferenceError::InvalidInput(format!(
                "specimen {} endpoint must be finite",
                record.specimen_id
            )));
        }
        grouped
            .entry(record.patient_id.as_str())
            .or_default()
            .insert(record.specimen_id.as_str(), record.endpoint);
    }
    if grouped.len() < 2 {
        return Err(CohortInferenceError::InvalidInput(
            "hierarchical bootstrap requires at least two patients".into(),
        ));
    }
    Ok(grouped
        .into_values()
        .map(|specimens| specimens.into_values().collect())
        .collect())
}

fn nearest_rank(ordered: &[f64], probability: f64) -> f64 {
    let rank = (probability * ordered.len() as f64).ceil() as usize;
    ordered[rank.saturating_sub(1).min(ordered.len() - 1)]
}

fn draw_limit_error() -> CohortInferenceError {
    CohortInferenceError::InvalidInput(format!(
        "hierarchical bootstrap exceeds the {MAXIMUM_BOOTSTRAP_DRAWS}-draw limit"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observed_mean_and_counts_match_hand_fixture() {
        let records = [
            ("p-1", "s-1", 1.0),
            ("p-1", "s-2", 3.0),
            ("p-2", "s-3", 5.0),
            ("p-2", "s-4", 7.0),
        ]
        .into_iter()
        .map(|(patient, specimen, endpoint)| HierarchicalScalarRecord {
            patient_id: patient.into(),
            specimen_id: specimen.into(),
            endpoint,
        })
        .collect::<Vec<_>>();
        let result = hierarchical_bootstrap(
            &records,
            &HierarchicalBootstrapSpec {
                replicates: 99,
                seed: 59,
                alpha: 0.05,
            },
        )
        .expect("bootstrap result");
        assert_eq!(result.observed_mean, 4.0);
        assert_eq!(result.patient_count, 2);
        assert_eq!(result.specimen_count, 4);
    }

    #[test]
    fn equivalent_specimen_rows_are_order_invariant() {
        let records = [
            ("p-1", "s-1", 1.0),
            ("p-1", "s-2", 7.0),
            ("p-1", "s-3", 20.0),
            ("p-2", "s-4", 2.0),
            ("p-2", "s-5", 11.0),
            ("p-2", "s-6", 30.0),
        ]
        .into_iter()
        .map(|(patient, specimen, endpoint)| HierarchicalScalarRecord {
            patient_id: patient.into(),
            specimen_id: specimen.into(),
            endpoint,
        })
        .collect::<Vec<_>>();
        let reordered = records.iter().rev().cloned().collect::<Vec<_>>();
        let spec = HierarchicalBootstrapSpec {
            replicates: 99,
            seed: 59,
            alpha: 0.05,
        };
        let first = hierarchical_bootstrap(&records, &spec).expect("first result");
        let second = hierarchical_bootstrap(&reordered, &spec).expect("reordered result");
        assert_eq!(first, second);
    }
}
