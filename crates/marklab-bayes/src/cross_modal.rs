use std::collections::{BTreeMap, HashMap, HashSet};

use rand::{seq::SliceRandom, SeedableRng};
use rand_chacha::ChaCha20Rng;
use serde::Serialize;
use thiserror::Error;

use crate::EmbeddingDistanceBin;

#[derive(Clone, Debug)]
pub struct EmbeddingModalityRow {
    pub object_id: String,
    pub source_section: String,
    pub compartment: String,
    pub embedding: Vec<f64>,
}

#[derive(Clone, Debug)]
pub struct CrossModalPair {
    pub a_object_id: String,
    pub b_object_id: String,
    pub source_section: String,
    pub compartment: String,
    pub bin_id: String,
    pub weight: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossModalMeanPolicy {
    Global,
    CompartmentStratified,
}

impl CrossModalMeanPolicy {
    pub fn parse(value: &str) -> Result<Self, CrossModalCovarianceError> {
        match value {
            "global" => Ok(Self::Global),
            "compartment_stratified" => Ok(Self::CompartmentStratified),
            _ => Err(CrossModalCovarianceError::Invalid(
                "mean_policy must be global or compartment_stratified".into(),
            )),
        }
    }
}

#[derive(Clone, Debug)]
pub struct CrossModalCovarianceSpec {
    pub a_rows: Vec<EmbeddingModalityRow>,
    pub a_feature_names: Vec<String>,
    pub b_rows: Vec<EmbeddingModalityRow>,
    pub b_feature_names: Vec<String>,
    pub pairs: Vec<CrossModalPair>,
    pub bins: Vec<EmbeddingDistanceBin>,
    pub mean_policy: CrossModalMeanPolicy,
    pub permutations: u32,
    pub seed: u64,
    pub maximum_pairs: u64,
    pub maximum_component_pair_visits: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct CrossModalCovarianceSummary {
    pub bin_id: String,
    pub lower_um: f64,
    pub upper_um: f64,
    pub upper_inclusive: bool,
    pub pair_count: u64,
    pub weight_sum: f64,
    pub frobenius_norm: Option<f64>,
    pub permutation_mean: Option<f64>,
    pub permutation_sd: Option<f64>,
    pub max_t_adjusted_p: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CrossModalCovarianceMatrix {
    pub bin_id: String,
    pub pair_count: u64,
    pub matrix: Option<Vec<Vec<f64>>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CrossModalCovarianceMatrixArtifact {
    pub format: &'static str,
    pub version: u32,
    pub a_feature_names: Vec<String>,
    pub b_feature_names: Vec<String>,
    pub matrices: Vec<CrossModalCovarianceMatrix>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CrossModalCovarianceResult {
    pub format: &'static str,
    pub version: u32,
    pub mean_policy: CrossModalMeanPolicy,
    pub inference_policy: &'static str,
    pub multiplicity_control: &'static str,
    pub a_object_count: u32,
    pub b_object_count: u32,
    pub a_dimension: u32,
    pub b_dimension: u32,
    pub pair_count: u64,
    pub permutations: u32,
    pub seed: u64,
    pub summaries: Vec<CrossModalCovarianceSummary>,
    pub matrix_artifact: CrossModalCovarianceMatrixArtifact,
}

#[derive(Debug, Error)]
pub enum CrossModalCovarianceError {
    #[error("invalid cross-modal covariance analysis: {0}")]
    Invalid(String),
    #[error("cross-modal covariance analysis exceeded its numeric range: {0}")]
    Numeric(String),
}

#[derive(Clone, Copy, Default)]
struct StableSum {
    sum: f64,
    correction: f64,
}

impl StableSum {
    fn add(&mut self, value: f64) -> Result<(), CrossModalCovarianceError> {
        if !value.is_finite() {
            return Err(CrossModalCovarianceError::Numeric(
                "a covariance contribution is non-finite".into(),
            ));
        }
        let next = self.sum + value;
        if !next.is_finite() {
            return Err(CrossModalCovarianceError::Numeric(
                "a covariance sum overflowed".into(),
            ));
        }
        self.correction += if self.sum.abs() >= value.abs() {
            (self.sum - next) + value
        } else {
            (value - next) + self.sum
        };
        if !self.correction.is_finite() {
            return Err(CrossModalCovarianceError::Numeric(
                "a covariance correction overflowed".into(),
            ));
        }
        self.sum = next;
        Ok(())
    }

    fn total(self) -> Result<f64, CrossModalCovarianceError> {
        let result = self.sum + self.correction;
        if result.is_finite() {
            Ok(result)
        } else {
            Err(CrossModalCovarianceError::Numeric(
                "a covariance total overflowed".into(),
            ))
        }
    }
}

struct ResolvedPair {
    a_index: usize,
    b_index: usize,
    bin_index: usize,
    weight: f64,
}

pub fn cross_modal_covariance_by_distance(
    mut spec: CrossModalCovarianceSpec,
) -> Result<CrossModalCovarianceResult, CrossModalCovarianceError> {
    let a_dimension = validate_modality(&mut spec.a_rows, &spec.a_feature_names, "A")?;
    let b_dimension = validate_modality(&mut spec.b_rows, &spec.b_feature_names, "B")?;
    validate_bins(&mut spec.bins)?;
    if !(20..=10_000).contains(&spec.permutations)
        || spec.maximum_pairs == 0
        || spec.pairs.is_empty()
        || spec.pairs.len() > 1_000_000
        || spec.pairs.len() as u64 > spec.maximum_pairs
    {
        return Err(CrossModalCovarianceError::Invalid(
            "pair or permutation controls are invalid".into(),
        ));
    }
    let work = (spec.permutations as u64 + 1)
        .checked_mul(spec.pairs.len() as u64)
        .and_then(|value| value.checked_mul(a_dimension as u64))
        .and_then(|value| value.checked_mul(b_dimension as u64))
        .ok_or_else(|| {
            CrossModalCovarianceError::Invalid("component-pair work overflowed".into())
        })?;
    let stored = spec
        .bins
        .len()
        .checked_mul(a_dimension)
        .and_then(|value| value.checked_mul(b_dimension))
        .ok_or_else(|| {
            CrossModalCovarianceError::Invalid("matrix artifact size overflowed".into())
        })?;
    if spec.maximum_component_pair_visits == 0
        || work > spec.maximum_component_pair_visits
        || spec.maximum_component_pair_visits > 250_000_000
        || stored > 1_000_000
    {
        return Err(CrossModalCovarianceError::Invalid(format!(
            "{work} component-pair visits exceed the declared or fixed resource bound"
        )));
    }

    let resolved = resolve_pairs(&spec)?;
    let a_centered = centered_rows(&spec.a_rows, a_dimension, spec.mean_policy)?;
    let b_centered = centered_rows(&spec.b_rows, b_dimension, spec.mean_policy)?;
    let identity = (0..spec.b_rows.len()).collect::<Vec<_>>();
    let observed = accumulate(
        &resolved,
        &spec.bins,
        &a_centered,
        &b_centered,
        &identity,
        a_dimension,
        b_dimension,
    )?;

    let mut groups = BTreeMap::<(String, String), Vec<usize>>::new();
    for (index, row) in spec.b_rows.iter().enumerate() {
        groups
            .entry((row.source_section.clone(), row.compartment.clone()))
            .or_default()
            .push(index);
    }
    if groups.values().any(|indices| indices.len() < 2) {
        return Err(CrossModalCovarianceError::Invalid(
            "every B source-section/compartment permutation stratum requires at least two rows"
                .into(),
        ));
    }
    let mut rng = ChaCha20Rng::seed_from_u64(spec.seed);
    let mut null = vec![vec![None; spec.bins.len()]; spec.permutations as usize];
    for permutation in &mut null {
        let mut donor = identity.clone();
        for indices in groups.values() {
            let mut shuffled = indices.clone();
            shuffled.shuffle(&mut rng);
            for (&receiver, &source) in indices.iter().zip(&shuffled) {
                donor[receiver] = source;
            }
        }
        let permuted = accumulate(
            &resolved,
            &spec.bins,
            &a_centered,
            &b_centered,
            &donor,
            a_dimension,
            b_dimension,
        )?;
        for (bin, value) in permutation.iter_mut().enumerate() {
            *value = permuted.frobenius[bin];
        }
    }
    let inference = max_t(&observed.frobenius, &null)?;

    let mut summaries = Vec::with_capacity(spec.bins.len());
    let mut matrices = Vec::with_capacity(spec.bins.len());
    for (bin_index, bin) in spec.bins.iter().enumerate() {
        summaries.push(CrossModalCovarianceSummary {
            bin_id: bin.bin_id.clone(),
            lower_um: bin.lower_um,
            upper_um: bin.upper_um,
            upper_inclusive: bin.upper_inclusive,
            pair_count: observed.pair_counts[bin_index],
            weight_sum: observed.weight_sums[bin_index],
            frobenius_norm: observed.frobenius[bin_index],
            permutation_mean: inference[bin_index].0,
            permutation_sd: inference[bin_index].1,
            max_t_adjusted_p: inference[bin_index].2,
        });
        matrices.push(CrossModalCovarianceMatrix {
            bin_id: bin.bin_id.clone(),
            pair_count: observed.pair_counts[bin_index],
            matrix: observed.matrices[bin_index].clone(),
        });
    }

    Ok(CrossModalCovarianceResult {
        format: "marklab.cross_modal_covariance_by_distance",
        version: 1,
        mean_policy: spec.mean_policy,
        inference_policy: "source_section_compartment_random_labeling",
        multiplicity_control: "single_step_max_t",
        a_object_count: spec.a_rows.len() as u32,
        b_object_count: spec.b_rows.len() as u32,
        a_dimension: a_dimension as u32,
        b_dimension: b_dimension as u32,
        pair_count: resolved.len() as u64,
        permutations: spec.permutations,
        seed: spec.seed,
        summaries,
        matrix_artifact: CrossModalCovarianceMatrixArtifact {
            format: "marklab.cross_modal_covariance_matrix_artifact",
            version: 1,
            a_feature_names: spec.a_feature_names,
            b_feature_names: spec.b_feature_names,
            matrices,
        },
    })
}

fn validate_modality(
    rows: &mut [EmbeddingModalityRow],
    feature_names: &[String],
    label: &str,
) -> Result<usize, CrossModalCovarianceError> {
    if !(2..=10_000).contains(&rows.len()) || !(1..=128).contains(&feature_names.len()) {
        return Err(CrossModalCovarianceError::Invalid(format!(
            "modality {label} dimensions are invalid"
        )));
    }
    let mut features = HashSet::new();
    if feature_names.iter().any(|name| {
        name.is_empty()
            || name.trim() != name
            || !name.starts_with("embedding_")
            || !features.insert(name.as_str())
    }) {
        return Err(CrossModalCovarianceError::Invalid(format!(
            "modality {label} feature names are invalid"
        )));
    }
    rows.sort_by(|left, right| left.object_id.cmp(&right.object_id));
    let mut identifiers = HashSet::new();
    for row in rows {
        if row.object_id.is_empty()
            || row.object_id.trim() != row.object_id
            || !identifiers.insert(row.object_id.as_str())
            || row.source_section.is_empty()
            || row.source_section.trim() != row.source_section
            || row.compartment.is_empty()
            || row.compartment.trim() != row.compartment
            || row.embedding.len() != feature_names.len()
            || row.embedding.iter().any(|value| !value.is_finite())
        {
            return Err(CrossModalCovarianceError::Invalid(format!(
                "modality {label} rows are invalid"
            )));
        }
    }
    Ok(feature_names.len())
}

fn validate_bins(bins: &mut [EmbeddingDistanceBin]) -> Result<(), CrossModalCovarianceError> {
    if !(1..=256).contains(&bins.len()) {
        return Err(CrossModalCovarianceError::Invalid(
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
            return Err(CrossModalCovarianceError::Invalid(
                "distance bins must be exact, unique, contiguous, and increasing".into(),
            ));
        }
    }
    let final_bin = bins.len() - 1;
    for (index, bin) in bins.iter_mut().enumerate() {
        bin.upper_inclusive = index == final_bin;
    }
    Ok(())
}

fn resolve_pairs(
    spec: &CrossModalCovarianceSpec,
) -> Result<Vec<ResolvedPair>, CrossModalCovarianceError> {
    let a = spec
        .a_rows
        .iter()
        .enumerate()
        .map(|(index, row)| (row.object_id.as_str(), index))
        .collect::<HashMap<_, _>>();
    let b = spec
        .b_rows
        .iter()
        .enumerate()
        .map(|(index, row)| (row.object_id.as_str(), index))
        .collect::<HashMap<_, _>>();
    let bins = spec
        .bins
        .iter()
        .enumerate()
        .map(|(index, bin)| (bin.bin_id.as_str(), index))
        .collect::<HashMap<_, _>>();
    let mut seen = HashSet::new();
    let mut resolved = Vec::with_capacity(spec.pairs.len());
    for pair in &spec.pairs {
        let Some(&a_index) = a.get(pair.a_object_id.as_str()) else {
            return Err(CrossModalCovarianceError::Invalid(format!(
                "pair references unknown A object {}",
                pair.a_object_id
            )));
        };
        let Some(&b_index) = b.get(pair.b_object_id.as_str()) else {
            return Err(CrossModalCovarianceError::Invalid(format!(
                "pair references unknown B object {}",
                pair.b_object_id
            )));
        };
        let Some(&bin_index) = bins.get(pair.bin_id.as_str()) else {
            return Err(CrossModalCovarianceError::Invalid(format!(
                "pair references unknown bin {}",
                pair.bin_id
            )));
        };
        if !seen.insert((a_index, b_index))
            || !pair.weight.is_finite()
            || pair.weight <= 0.0
            || spec.a_rows[a_index].source_section != pair.source_section
            || spec.b_rows[b_index].source_section != pair.source_section
            || spec.a_rows[a_index].compartment != pair.compartment
            || spec.b_rows[b_index].compartment != pair.compartment
        {
            return Err(CrossModalCovarianceError::Invalid(
                "pairs must be unique, positive-weight, and exactly section/compartment bound"
                    .into(),
            ));
        }
        resolved.push(ResolvedPair {
            a_index,
            b_index,
            bin_index,
            weight: pair.weight,
        });
    }
    resolved.sort_by_key(|pair| (pair.bin_index, pair.a_index, pair.b_index));
    Ok(resolved)
}

fn centered_rows(
    rows: &[EmbeddingModalityRow],
    dimension: usize,
    policy: CrossModalMeanPolicy,
) -> Result<Vec<Vec<f64>>, CrossModalCovarianceError> {
    let mut sums = BTreeMap::<String, (Vec<StableSum>, u64)>::new();
    for row in rows {
        let key = match policy {
            CrossModalMeanPolicy::Global => String::new(),
            CrossModalMeanPolicy::CompartmentStratified => row.compartment.clone(),
        };
        let entry = sums
            .entry(key)
            .or_insert_with(|| (vec![StableSum::default(); dimension], 0));
        entry.1 += 1;
        for (sum, value) in entry.0.iter_mut().zip(&row.embedding) {
            sum.add(*value)?;
        }
    }
    let means = sums
        .into_iter()
        .map(|(key, (sums, count))| {
            let mean = sums
                .into_iter()
                .map(|sum| sum.total().map(|value| value / count as f64))
                .collect::<Result<Vec<_>, _>>()?;
            Ok((key, mean))
        })
        .collect::<Result<BTreeMap<_, _>, CrossModalCovarianceError>>()?;
    rows.iter()
        .map(|row| {
            let key = match policy {
                CrossModalMeanPolicy::Global => "",
                CrossModalMeanPolicy::CompartmentStratified => row.compartment.as_str(),
            };
            Ok(row
                .embedding
                .iter()
                .zip(&means[key])
                .map(|(value, mean)| value - mean)
                .collect())
        })
        .collect()
}

struct Accumulated {
    pair_counts: Vec<u64>,
    weight_sums: Vec<f64>,
    matrices: Vec<Option<Vec<Vec<f64>>>>,
    frobenius: Vec<Option<f64>>,
}

fn accumulate(
    pairs: &[ResolvedPair],
    bins: &[EmbeddingDistanceBin],
    a: &[Vec<f64>],
    b: &[Vec<f64>],
    b_donor: &[usize],
    a_dimension: usize,
    b_dimension: usize,
) -> Result<Accumulated, CrossModalCovarianceError> {
    let elements = a_dimension * b_dimension;
    let mut sums = vec![StableSum::default(); bins.len() * elements];
    let mut weights = vec![StableSum::default(); bins.len()];
    let mut counts = vec![0_u64; bins.len()];
    for pair in pairs {
        counts[pair.bin_index] += 1;
        weights[pair.bin_index].add(pair.weight)?;
        let donor = b_donor[pair.b_index];
        let offset = pair.bin_index * elements;
        for row in 0..a_dimension {
            for column in 0..b_dimension {
                sums[offset + row * b_dimension + column]
                    .add(pair.weight * a[pair.a_index][row] * b[donor][column])?;
            }
        }
    }
    let weight_sums = weights
        .into_iter()
        .map(StableSum::total)
        .collect::<Result<Vec<_>, _>>()?;
    let mut matrices = Vec::with_capacity(bins.len());
    let mut frobenius = Vec::with_capacity(bins.len());
    for bin in 0..bins.len() {
        if counts[bin] == 0 {
            matrices.push(None);
            frobenius.push(None);
            continue;
        }
        let mut matrix = vec![vec![0.0; b_dimension]; a_dimension];
        let mut squared = StableSum::default();
        for row in 0..a_dimension {
            for column in 0..b_dimension {
                let value =
                    sums[bin * elements + row * b_dimension + column].total()? / weight_sums[bin];
                if !value.is_finite() {
                    return Err(CrossModalCovarianceError::Numeric(
                        "a normalized cross-modal covariance is non-finite".into(),
                    ));
                }
                matrix[row][column] = value;
                squared.add(value * value)?;
            }
        }
        matrices.push(Some(matrix));
        frobenius.push(Some(squared.total()?.sqrt()));
    }
    Ok(Accumulated {
        pair_counts: counts,
        weight_sums,
        matrices,
        frobenius,
    })
}

type MaxTCell = (Option<f64>, Option<f64>, Option<f64>);

fn max_t(
    observed: &[Option<f64>],
    null: &[Vec<Option<f64>>],
) -> Result<Vec<MaxTCell>, CrossModalCovarianceError> {
    let mut means = vec![None; observed.len()];
    let mut standard_deviations = vec![None; observed.len()];
    let mut observed_t = vec![None; observed.len()];
    for bin in 0..observed.len() {
        let Some(observed_value) = observed[bin] else {
            continue;
        };
        let values = null
            .iter()
            .map(|row| row[bin].expect("nonempty observed bin is nonempty under relabeling"))
            .collect::<Vec<_>>();
        let mut sum = StableSum::default();
        for value in &values {
            sum.add(*value)?;
        }
        let mean = sum.total()? / values.len() as f64;
        let mut squares = StableSum::default();
        for value in &values {
            squares.add((value - mean) * (value - mean))?;
        }
        let sd = (squares.total()? / (values.len() - 1) as f64).sqrt();
        means[bin] = Some(mean);
        if sd > 1e-14 && sd.is_finite() {
            standard_deviations[bin] = Some(sd);
            observed_t[bin] = Some(((observed_value - mean) / sd).abs());
        }
    }
    let maxima = null
        .iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .filter_map(|(bin, value)| {
                    standard_deviations[bin]
                        .zip(*value)
                        .map(|(sd, value)| ((value - means[bin].expect("mean exists")) / sd).abs())
                })
                .fold(0.0_f64, f64::max)
        })
        .collect::<Vec<_>>();
    Ok((0..observed.len())
        .map(|bin| {
            let adjusted = observed_t[bin].map(|statistic| {
                (1 + maxima.iter().filter(|value| **value >= statistic).count()) as f64
                    / (null.len() + 1) as f64
            });
            (means[bin], standard_deviations[bin], adjusted)
        })
        .collect())
}
