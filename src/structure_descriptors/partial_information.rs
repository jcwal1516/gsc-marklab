//! Discrete two-source partial information decomposition using Williams and Beer's I_min.
//!
//! For each target state t, specific information is the KL divergence
//! D_KL[p(source | t) || p(source)]. I_min averages the smaller of the two specific
//! informations over target states. Unique and synergistic terms follow by subtraction
//! from the two marginal mutual informations and their joint mutual information.
//! All quantities are plug-in estimates in bits, without pseudocounts or significance tests.

use std::collections::HashSet;

use marklab_core::vocabulary::StatisticalUnit;
use marklab_numerics::summation::neumaier_sum;
use serde::{Deserialize, Serialize};

use crate::{MarklabError, Result};

/// Result format for the declared discrete I_min decomposition.
pub const PARTIAL_INFORMATION_FORMAT: &str = "marklab.partial_information_decomposition";
const MAXIMUM_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAXIMUM_STATES: usize = 1_000_000;
const MAXIMUM_OBSERVATIONS: u64 = 1_000_000_000_000;
const ROUNDING_TOLERANCE_BITS: f64 = 1e-12;

/// A declared categorical variable, including how its levels were obtained.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiscreteInformationVariable {
    pub name: String,
    pub levels: Vec<String>,
    /// For continuous features, record the prespecified or training-only discretization.
    pub definition: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    format: String,
    version: u32,
    estimator: String,
    statistical_unit: StatisticalUnit,
    target: DiscreteInformationVariable,
    sources: [DiscreteInformationVariable; 2],
    counts: Vec<JointCount>,
    limits: Limits,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JointCount {
    target: usize,
    source_a: usize,
    source_b: usize,
    count: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Limits {
    maximum_entries: usize,
    maximum_states: usize,
    maximum_observations: u64,
}

/// Specific information about an observed target state from each source.
#[derive(Debug, Serialize)]
pub struct TargetSpecificInformation {
    pub target_level: usize,
    pub probability: f64,
    pub source_a_bits: f64,
    pub source_b_bits: f64,
    pub minimum_bits: f64,
}

/// Descriptive empirical two-source decomposition; every information quantity is in bits.
#[derive(Debug, Serialize)]
pub struct PartialInformationDecompositionResult {
    pub format: &'static str,
    pub version: u32,
    pub estimator: &'static str,
    pub statistical_unit: StatisticalUnit,
    pub target: DiscreteInformationVariable,
    pub sources: [DiscreteInformationVariable; 2],
    pub observations: u64,
    pub declared_states: usize,
    pub observed_states: usize,
    pub target_entropy_bits: f64,
    pub source_a_information_bits: f64,
    pub source_b_information_bits: f64,
    pub joint_information_bits: f64,
    pub redundancy_bits: f64,
    pub unique_source_a_bits: f64,
    pub unique_source_b_bits: f64,
    pub synergy_bits: f64,
    pub target_specific_information: Vec<TargetSpecificInformation>,
    pub claim_scope: &'static str,
}

fn invalid(message: impl Into<String>) -> MarklabError {
    MarklabError::Validation(format!(
        "partial information decomposition: {}",
        message.into()
    ))
}

fn nonnegative(value: f64) -> Result<f64> {
    if !value.is_finite() || value < -ROUNDING_TOLERANCE_BITS {
        return Err(invalid("information is nonfinite or materially negative"));
    }
    Ok(value.max(0.0))
}

fn valid_variable(variable: &DiscreteInformationVariable) -> bool {
    let mut levels = HashSet::new();
    !variable.name.trim().is_empty()
        && variable.name.trim() == variable.name
        && variable.name.len() <= 256
        && !variable.definition.trim().is_empty()
        && variable.definition.len() <= 8192
        && (1..=256).contains(&variable.levels.len())
        && variable.levels.iter().all(|level| {
            !level.trim().is_empty()
                && level.trim() == level
                && level.len() <= 256
                && levels.insert(level)
        })
}

/// Decode a bounded joint-count table and compute the explicitly named I_min estimator.
///
/// Duplicate joint cells are summed exactly before calculation, so count-row order and
/// splitting an entry do not affect the result. This is a discrete empirical estimator;
/// it does not automatically bin embeddings or claim bias-corrected or causal information.
pub fn partial_information_decomposition_from_json(
    input: &[u8],
) -> Result<PartialInformationDecompositionResult> {
    if input.len() > MAXIMUM_INPUT_BYTES {
        return Err(invalid("input exceeds 16 MiB"));
    }
    let request: Request = serde_json::from_slice(input)?;
    let variables = [&request.target, &request.sources[0], &request.sources[1]];
    let names: HashSet<_> = variables.iter().map(|variable| &variable.name).collect();
    if request.format != "marklab.partial_information_decomposition_request"
        || request.version != 1
        || request.estimator != "williams_beer_i_min"
        || !matches!(
            request.statistical_unit,
            StatisticalUnit::Patient | StatisticalUnit::WithinSpecimenFieldDiagnostic
        )
        || !variables.iter().all(|variable| valid_variable(variable))
        || names.len() != 3
        || !(1..=MAXIMUM_STATES).contains(&request.limits.maximum_states)
        || !(1..=MAXIMUM_STATES).contains(&request.limits.maximum_entries)
        || !(1..=MAXIMUM_OBSERVATIONS).contains(&request.limits.maximum_observations)
        || request.counts.is_empty()
        || request.counts.len() > request.limits.maximum_entries
    {
        return Err(invalid(
            "invalid format, estimator, variable declarations, statistical unit or count limits",
        ));
    }
    let nt = request.target.levels.len();
    let na = request.sources[0].levels.len();
    let nb = request.sources[1].levels.len();
    let states = nt * na * nb;
    if states > request.limits.maximum_states {
        return Err(invalid("declared joint state space exceeds maximum_states"));
    }
    let mut joint = vec![0_u64; states];
    let mut observations = 0_u64;
    for entry in &request.counts {
        if entry.target >= nt || entry.source_a >= na || entry.source_b >= nb || entry.count == 0 {
            return Err(invalid(
                "joint counts require in-range level indices and positive counts",
            ));
        }
        observations = observations
            .checked_add(entry.count)
            .filter(|total| *total <= request.limits.maximum_observations)
            .ok_or_else(|| invalid("observation count exceeds its limit"))?;
        // Each cell count is at most the already checked total count.
        joint[(entry.target * na + entry.source_a) * nb + entry.source_b] += entry.count;
    }
    let mut target = vec![0_u64; nt];
    let mut source_a = vec![0_u64; na];
    let mut source_b = vec![0_u64; nb];
    let mut target_a = vec![0_u64; nt * na];
    let mut target_b = vec![0_u64; nt * nb];
    let mut sources_ab = vec![0_u64; na * nb];
    for (index, count) in joint.iter().copied().enumerate() {
        let (t, a, b) = (index / (na * nb), index / nb % na, index % nb);
        target[t] += count;
        source_a[a] += count;
        source_b[b] += count;
        target_a[t * na + a] += count;
        target_b[t * nb + b] += count;
        sources_ab[a * nb + b] += count;
    }
    let n = observations as f64;
    let specific = |t: usize, marginal: &[u64], table: &[u64]| -> Result<f64> {
        let width = marginal.len();
        nonnegative(neumaier_sum((0..width).filter_map(|source| {
            let joint_count = table[t * width + source];
            if joint_count == 0 {
                return None;
            }
            let conditional = joint_count as f64 / target[t] as f64;
            Some(conditional * (conditional / (marginal[source] as f64 / n)).log2())
        })))
    };
    let mut target_specific_information = Vec::new();
    for (t, count) in target
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, count)| *count > 0)
    {
        let a = specific(t, &source_a, &target_a)?;
        let b = specific(t, &source_b, &target_b)?;
        target_specific_information.push(TargetSpecificInformation {
            target_level: t,
            probability: count as f64 / n,
            source_a_bits: a,
            source_b_bits: b,
            minimum_bits: a.min(b),
        });
    }
    let target_entropy_bits = nonnegative(neumaier_sum(
        target_specific_information
            .iter()
            .map(|row| -row.probability * row.probability.log2()),
    ))?;
    let source_a_information_bits = nonnegative(neumaier_sum(
        target_specific_information
            .iter()
            .map(|row| row.probability * row.source_a_bits),
    ))?;
    let source_b_information_bits = nonnegative(neumaier_sum(
        target_specific_information
            .iter()
            .map(|row| row.probability * row.source_b_bits),
    ))?;
    let redundancy_bits = nonnegative(neumaier_sum(
        target_specific_information
            .iter()
            .map(|row| row.probability * row.minimum_bits),
    ))?;
    let joint_information_bits = nonnegative(neumaier_sum(
        joint
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, count)| *count > 0)
            .map(|(index, count)| {
                let t = index / (na * nb);
                let ab = index % (na * nb);
                let conditional = count as f64 / target[t] as f64;
                count as f64 / n * (conditional / (sources_ab[ab] as f64 / n)).log2()
            }),
    ))?;
    let unique_source_a_bits = nonnegative(source_a_information_bits - redundancy_bits)?;
    let unique_source_b_bits = nonnegative(source_b_information_bits - redundancy_bits)?;
    let synergy_bits = nonnegative(
        joint_information_bits - source_a_information_bits - source_b_information_bits
            + redundancy_bits,
    )?;
    if joint_information_bits > target_entropy_bits + ROUNDING_TOLERANCE_BITS {
        return Err(invalid("joint information exceeds target entropy"));
    }
    Ok(PartialInformationDecompositionResult {
        format: PARTIAL_INFORMATION_FORMAT,
        version: 1,
        estimator: "williams_beer_i_min",
        statistical_unit: request.statistical_unit,
        target: request.target,
        sources: request.sources,
        observations,
        declared_states: states,
        observed_states: joint.iter().filter(|n| **n > 0).count(),
        target_entropy_bits,
        source_a_information_bits,
        source_b_information_bits,
        joint_information_bits,
        redundancy_bits,
        unique_source_a_bits,
        unique_source_b_bits,
        synergy_bits,
        target_specific_information,
        claim_scope:
            "research_empirical_discrete_i_min_no_bias_correction_or_significance_or_causal_claim",
    })
}
