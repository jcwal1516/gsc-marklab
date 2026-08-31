use std::collections::{BTreeMap, HashSet};

use marklab_numerics::extreme_rank_length_envelope;
use rand::{seq::SliceRandom, SeedableRng};
use rand_chacha::ChaCha20Rng;
use serde::Serialize;
use thiserror::Error;

use crate::{
    embedding_spatial::{find_bin, FiniteNeumaierError, FiniteNeumaierSum},
    EmbeddingDistanceBin,
};

#[derive(Clone, Debug)]
pub struct EmbeddingEnvelopeRow {
    pub object_id: String,
    pub permutation_stratum: String,
    pub x_um: f64,
    pub y_um: f64,
    pub embedding: Vec<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingSpatialCurveFunction {
    VectorSemivariogram,
}

impl EmbeddingSpatialCurveFunction {
    pub fn parse(value: &str) -> Result<Self, EmbeddingEnvelopeError> {
        match value {
            "vector_semivariogram" => Ok(Self::VectorSemivariogram),
            _ => Err(EmbeddingEnvelopeError::Invalid(
                "curve must be vector_semivariogram".into(),
            )),
        }
    }
}

#[derive(Clone, Debug)]
pub struct EmbeddingSpatialDependenceSpec {
    pub rows: Vec<EmbeddingEnvelopeRow>,
    pub feature_names: Vec<String>,
    pub bins: Vec<EmbeddingDistanceBin>,
    pub curve_function: EmbeddingSpatialCurveFunction,
    pub permutations: u32,
    pub alpha: f64,
    pub seed: u64,
    pub maximum_pair_visits: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct EmbeddingEnvelopeCurveRow {
    pub bin_id: String,
    pub lower_um: f64,
    pub upper_um: f64,
    pub upper_inclusive: bool,
    pub pair_count: u64,
    pub observed: Option<f64>,
    pub lower: Option<f64>,
    pub upper: Option<f64>,
    pub inference_eligible: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct EmbeddingSpatialDependenceResult {
    pub format: &'static str,
    pub version: u32,
    pub coordinate_unit: &'static str,
    pub curve_function: EmbeddingSpatialCurveFunction,
    pub permutation_policy: &'static str,
    pub feature_names: Vec<String>,
    pub object_count: u32,
    pub stratum_count: u32,
    pub permutations: u32,
    pub alpha: f64,
    pub seed: u64,
    pub pair_visits: u64,
    pub p_global: f64,
    pub erl_depth: f64,
    pub critical_depth: f64,
    pub curve: Vec<EmbeddingEnvelopeCurveRow>,
}

#[derive(Debug, Error)]
pub enum EmbeddingEnvelopeError {
    #[error("invalid embedding spatial envelope: {0}")]
    Invalid(String),
    #[error("embedding spatial envelope exceeded its numeric range: {0}")]
    Numeric(String),
}

impl From<FiniteNeumaierError> for EmbeddingEnvelopeError {
    fn from(error: FiniteNeumaierError) -> Self {
        let message = match error {
            FiniteNeumaierError::NonFiniteInput => "a curve contribution is non-finite",
            FiniteNeumaierError::SumOverflow => "a curve sum overflowed",
            FiniteNeumaierError::CorrectionOverflow => "a curve correction overflowed",
            FiniteNeumaierError::TotalOverflow => "a curve total overflowed",
        };
        Self::Numeric(message.into())
    }
}

type StableSum = FiniteNeumaierSum;

struct PairPlan {
    left: usize,
    right: usize,
    bin: Option<usize>,
}

pub fn test_embedding_spatial_dependence(
    mut spec: EmbeddingSpatialDependenceSpec,
) -> Result<EmbeddingSpatialDependenceResult, EmbeddingEnvelopeError> {
    validate(&mut spec)?;
    spec.rows
        .sort_by(|left, right| left.object_id.cmp(&right.object_id));
    let mut groups = BTreeMap::<String, Vec<usize>>::new();
    for (index, row) in spec.rows.iter().enumerate() {
        groups
            .entry(row.permutation_stratum.clone())
            .or_default()
            .push(index);
    }
    let pair_count = groups.values().try_fold(0_u64, |total, indices| {
        let count = u64::try_from(indices.len())
            .map_err(|_| EmbeddingEnvelopeError::Invalid("pair count exceeds u64".into()))?;
        let pairs = count
            .checked_mul(count.saturating_sub(1))
            .and_then(|value| value.checked_div(2))
            .ok_or_else(|| EmbeddingEnvelopeError::Invalid("pair count overflowed".into()))?;
        total
            .checked_add(pairs)
            .ok_or_else(|| EmbeddingEnvelopeError::Invalid("pair count overflowed".into()))
    })?;
    let pair_visits = pair_count
        .checked_mul(u64::from(spec.permutations) + 1)
        .ok_or_else(|| EmbeddingEnvelopeError::Invalid("pair work overflowed".into()))?;
    if pair_visits > spec.maximum_pair_visits
        || pair_visits
            .checked_mul(spec.feature_names.len() as u64)
            .is_none_or(|work| work > 250_000_000)
    {
        return Err(EmbeddingEnvelopeError::Invalid(format!(
            "{pair_visits} observed/permuted pair visits exceed the declared or fixed resource bound"
        )));
    }
    let pair_capacity = usize::try_from(pair_count)
        .map_err(|_| EmbeddingEnvelopeError::Invalid("pair count exceeds usize".into()))?;
    let mut pairs = Vec::with_capacity(pair_capacity);
    let mut bin_counts = vec![0_u64; spec.bins.len()];
    for indices in groups.values() {
        for (position, &left) in indices.iter().enumerate() {
            for &right in &indices[(position + 1)..] {
                let distance = (spec.rows[left].x_um - spec.rows[right].x_um)
                    .hypot(spec.rows[left].y_um - spec.rows[right].y_um);
                if !distance.is_finite() {
                    return Err(EmbeddingEnvelopeError::Numeric(
                        "a spatial pair distance is non-finite".into(),
                    ));
                }
                let bin = find_bin(distance, &spec.bins);
                if let Some(bin) = bin {
                    bin_counts[bin] += 1;
                }
                pairs.push(PairPlan { left, right, bin });
            }
        }
    }
    let identity = (0..spec.rows.len()).collect::<Vec<_>>();
    let observed = curve(&spec, &pairs, &identity, &bin_counts)?;
    let mut rng = ChaCha20Rng::seed_from_u64(spec.seed);
    let mut null = Vec::with_capacity(spec.permutations as usize);
    for _ in 0..spec.permutations {
        let mut donor = identity.clone();
        for indices in groups.values() {
            let mut shuffled = indices.clone();
            shuffled.shuffle(&mut rng);
            for (&receiver, &source) in indices.iter().zip(&shuffled) {
                donor[receiver] = source;
            }
        }
        null.push(curve(&spec, &pairs, &donor, &bin_counts)?);
    }
    let eligible = bin_counts
        .iter()
        .enumerate()
        .filter_map(|(index, count)| (*count > 0).then_some(index))
        .collect::<Vec<_>>();
    let observed_eligible = eligible
        .iter()
        .map(|index| observed[*index])
        .collect::<Vec<_>>();
    let null_eligible = null
        .iter()
        .map(|row| eligible.iter().map(|index| row[*index]).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let envelope = erl_envelope(&observed_eligible, &null_eligible, spec.alpha)?;
    let mut position = vec![None; spec.bins.len()];
    for (eligible_position, bin) in eligible.iter().enumerate() {
        position[*bin] = Some(eligible_position);
    }
    let curve = spec
        .bins
        .iter()
        .enumerate()
        .map(|(index, bin)| {
            let eligible_position = position[index];
            EmbeddingEnvelopeCurveRow {
                bin_id: bin.bin_id.clone(),
                lower_um: bin.lower_um,
                upper_um: bin.upper_um,
                upper_inclusive: bin.upper_inclusive,
                pair_count: bin_counts[index],
                observed: eligible_position.map(|_| observed[index]),
                lower: eligible_position.map(|position| envelope.lower[position]),
                upper: eligible_position.map(|position| envelope.upper[position]),
                inference_eligible: eligible_position.is_some(),
            }
        })
        .collect();
    Ok(EmbeddingSpatialDependenceResult {
        format: "marklab.embedding_spatial_dependence_envelope",
        version: 1,
        coordinate_unit: "micrometer",
        curve_function: spec.curve_function,
        permutation_policy: "complete_embedding_rows_within_declared_strata",
        feature_names: spec.feature_names,
        object_count: spec.rows.len() as u32,
        stratum_count: groups.len() as u32,
        permutations: spec.permutations,
        alpha: spec.alpha,
        seed: spec.seed,
        pair_visits,
        p_global: envelope.p_global,
        erl_depth: envelope.erl_depth,
        critical_depth: envelope.critical_depth,
        curve,
    })
}

fn validate(spec: &mut EmbeddingSpatialDependenceSpec) -> Result<(), EmbeddingEnvelopeError> {
    if !(8..=10_000).contains(&spec.rows.len())
        || !(2..=128).contains(&spec.feature_names.len())
        || !(20..=10_000).contains(&spec.permutations)
        || !spec.alpha.is_finite()
        || spec.alpha <= 0.0
        || spec.alpha >= 1.0
        || (f64::from(spec.permutations) + 1.0) * spec.alpha < 1.0
        || spec.maximum_pair_visits == 0
    {
        return Err(EmbeddingEnvelopeError::Invalid(
            "envelope dimensions, alpha, permutations, or resource bound are invalid".into(),
        ));
    }
    let mut features = HashSet::new();
    if spec.feature_names.iter().any(|name| {
        name.is_empty()
            || name.trim() != name
            || !name.starts_with("embedding_")
            || !features.insert(name.as_str())
    }) {
        return Err(EmbeddingEnvelopeError::Invalid(
            "envelope feature names must be unique exact embedding_* names".into(),
        ));
    }
    let mut identifiers = HashSet::new();
    let mut strata = BTreeMap::<&str, usize>::new();
    for row in &spec.rows {
        if row.object_id.is_empty()
            || row.object_id.trim() != row.object_id
            || !identifiers.insert(row.object_id.as_str())
            || row.permutation_stratum.is_empty()
            || row.permutation_stratum.trim() != row.permutation_stratum
            || !row.x_um.is_finite()
            || !row.y_um.is_finite()
            || row.embedding.len() != spec.feature_names.len()
            || row.embedding.iter().any(|value| !value.is_finite())
        {
            return Err(EmbeddingEnvelopeError::Invalid(
                "envelope rows require unique exact identities/strata, finite coordinates, and complete finite vectors"
                    .into(),
            ));
        }
        *strata.entry(row.permutation_stratum.as_str()).or_default() += 1;
    }
    if strata.values().any(|count| *count < 2) {
        return Err(EmbeddingEnvelopeError::Invalid(
            "every permutation stratum requires at least two rows".into(),
        ));
    }
    validate_bins(&mut spec.bins)
}

fn validate_bins(bins: &mut [EmbeddingDistanceBin]) -> Result<(), EmbeddingEnvelopeError> {
    if !(1..=256).contains(&bins.len()) {
        return Err(EmbeddingEnvelopeError::Invalid(
            "distance bin count is invalid".into(),
        ));
    }
    let mut identifiers = HashSet::new();
    for index in 0..bins.len() {
        if bins[index].bin_id.is_empty()
            || !identifiers.insert(bins[index].bin_id.as_str())
            || !bins[index].lower_um.is_finite()
            || !bins[index].upper_um.is_finite()
            || bins[index].lower_um < 0.0
            || bins[index].lower_um >= bins[index].upper_um
            || (index > 0 && bins[index].lower_um.to_bits() != bins[index - 1].upper_um.to_bits())
        {
            return Err(EmbeddingEnvelopeError::Invalid(
                "distance bins must be unique, contiguous, and increasing".into(),
            ));
        }
    }
    let final_bin = bins.len() - 1;
    for (index, bin) in bins.iter_mut().enumerate() {
        bin.upper_inclusive = index == final_bin;
    }
    Ok(())
}

fn curve(
    spec: &EmbeddingSpatialDependenceSpec,
    pairs: &[PairPlan],
    donor: &[usize],
    counts: &[u64],
) -> Result<Vec<f64>, EmbeddingEnvelopeError> {
    match spec.curve_function {
        EmbeddingSpatialCurveFunction::VectorSemivariogram => {
            let mut sums = vec![StableSum::default(); spec.bins.len()];
            for pair in pairs {
                let Some(bin) = pair.bin else { continue };
                let left = &spec.rows[donor[pair.left]].embedding;
                let right = &spec.rows[donor[pair.right]].embedding;
                let mut squared = StableSum::default();
                for (left, right) in left.iter().zip(right) {
                    squared.add((left - right) * (left - right))?;
                }
                sums[bin].add(0.5 * squared.total()?)?;
            }
            sums.into_iter()
                .zip(counts)
                .map(|(sum, count)| {
                    if *count == 0 {
                        Ok(0.0)
                    } else {
                        sum.total()
                            .map(|value| value / *count as f64)
                            .map_err(EmbeddingEnvelopeError::from)
                    }
                })
                .collect()
        }
    }
}

struct ErlEnvelope {
    lower: Vec<f64>,
    upper: Vec<f64>,
    p_global: f64,
    erl_depth: f64,
    critical_depth: f64,
}

fn erl_envelope(
    observed: &[f64],
    permutations: &[Vec<f64>],
    alpha: f64,
) -> Result<ErlEnvelope, EmbeddingEnvelopeError> {
    let result = extreme_rank_length_envelope(observed, permutations, alpha)
        .map_err(|error| EmbeddingEnvelopeError::Invalid(error.to_string()))?;
    Ok(ErlEnvelope {
        lower: result.lower,
        upper: result.upper,
        p_global: result.p_global,
        erl_depth: result.observed_depth,
        critical_depth: result.critical_depth,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        erl_envelope, test_embedding_spatial_dependence, EmbeddingEnvelopeRow,
        EmbeddingSpatialCurveFunction, EmbeddingSpatialDependenceSpec,
    };
    use crate::EmbeddingDistanceBin;

    #[test]
    fn identical_curves_match_the_canonical_erl_tie_oracle() {
        let observed = [1.0, 2.0, 3.0];
        let permutations = vec![observed.to_vec(), observed.to_vec(), observed.to_vec()];
        let envelope = erl_envelope(&observed, &permutations, 0.25).unwrap();
        assert_eq!(envelope.lower, observed);
        assert_eq!(envelope.upper, observed);
        assert_eq!(envelope.p_global, 1.0);
        assert_eq!(envelope.erl_depth, 0.625);
    }

    #[test]
    fn spatial_pairs_remain_inside_the_declared_permutation_stratum() {
        let mut rows = Vec::new();
        for (stratum, offset) in [("s1", 0.0), ("s2", 10.0)] {
            for (index, (x, value)) in [(0.0, 0.0), (1.0, 0.1), (3.0, 3.0), (6.0, 3.1)]
                .into_iter()
                .enumerate()
            {
                rows.push(EmbeddingEnvelopeRow {
                    object_id: format!("{stratum}-{index}"),
                    permutation_stratum: stratum.into(),
                    x_um: offset + x,
                    y_um: 0.0,
                    embedding: vec![value, value],
                });
            }
        }
        let result = test_embedding_spatial_dependence(EmbeddingSpatialDependenceSpec {
            rows,
            feature_names: vec!["embedding_0".into(), "embedding_1".into()],
            bins: vec![
                EmbeddingDistanceBin {
                    bin_id: "near".into(),
                    lower_um: 0.0,
                    upper_um: 2.0,
                    upper_inclusive: false,
                },
                EmbeddingDistanceBin {
                    bin_id: "mid".into(),
                    lower_um: 2.0,
                    upper_um: 5.0,
                    upper_inclusive: false,
                },
                EmbeddingDistanceBin {
                    bin_id: "far".into(),
                    lower_um: 5.0,
                    upper_um: 17.0,
                    upper_inclusive: false,
                },
            ],
            curve_function: EmbeddingSpatialCurveFunction::VectorSemivariogram,
            permutations: 39,
            alpha: 0.05,
            seed: 991,
            maximum_pair_visits: 480,
        })
        .expect("within-stratum envelope");

        assert_eq!(result.pair_visits, 480);
        let counts = result
            .curve
            .iter()
            .map(|row| row.pair_count)
            .collect::<Vec<_>>();
        assert_eq!(counts, [2, 6, 4]);
        for (row, expected) in result.curve.iter().zip([0.01, 17.42 / 3.0, 18.61 / 2.0]) {
            assert!((row.observed.expect("eligible bin") - expected).abs() < 1e-12);
        }
    }
}
