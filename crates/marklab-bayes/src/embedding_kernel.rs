use std::collections::{HashMap, HashSet};

use serde::Serialize;
use thiserror::Error;

use crate::EmbeddingDistanceBin;

#[derive(Clone, Debug)]
pub struct KernelEmbeddingRow {
    pub object_id: String,
    pub biological_unit: String,
    pub split: String,
    pub x_um: f64,
    pub y_um: f64,
    pub embedding: Vec<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingKernelKind {
    Linear,
    Cosine,
    Rbf,
    Laplacian,
}

impl EmbeddingKernelKind {
    pub fn parse(value: &str) -> Result<Self, EmbeddingKernelError> {
        match value {
            "linear" => Ok(Self::Linear),
            "cosine" => Ok(Self::Cosine),
            "rbf" => Ok(Self::Rbf),
            "laplacian" => Ok(Self::Laplacian),
            _ => Err(EmbeddingKernelError::Invalid(
                "kernel must be linear, cosine, rbf, or laplacian".into(),
            )),
        }
    }
}

#[derive(Clone, Debug)]
pub struct EmbeddingKernelSpec {
    pub rows: Vec<KernelEmbeddingRow>,
    pub feature_names: Vec<String>,
    pub kind: EmbeddingKernelKind,
}

#[derive(Clone, Debug, Serialize)]
pub struct EmbeddingKernelArtifact {
    pub kind: EmbeddingKernelKind,
    pub fit_split: &'static str,
    pub training_biological_units: Vec<String>,
    pub feature_names: Vec<String>,
    pub center: Vec<f64>,
    pub scale: Option<f64>,
    pub scale_selection: &'static str,
    pub formula: &'static str,
    pub positive_semidefinite: bool,
    pub preprocessing: &'static str,
}

#[derive(Clone, Debug)]
pub struct KernelMarkCorrelationSpec {
    pub kernel_spec: EmbeddingKernelSpec,
    pub bins: Vec<EmbeddingDistanceBin>,
    pub global_reference_tolerance: f64,
    pub maximum_pair_visits: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct KernelMarkCorrelationRow {
    pub bin_id: String,
    pub lower_um: f64,
    pub upper_um: f64,
    pub upper_inclusive: bool,
    pub pair_count: u64,
    pub raw_similarity: Option<f64>,
    pub normalized_similarity: Option<f64>,
    pub normalization_status: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct KernelMarkCorrelationCurve {
    pub split: String,
    pub object_count: u32,
    pub pair_visits: u64,
    pub global_reference: f64,
    pub rows: Vec<KernelMarkCorrelationRow>,
}

#[derive(Clone, Debug, Serialize)]
pub struct KernelMarkCorrelationResult {
    pub format: &'static str,
    pub version: u32,
    pub coordinate_unit: &'static str,
    pub kernel: EmbeddingKernelArtifact,
    pub curves: Vec<KernelMarkCorrelationCurve>,
}

#[derive(Debug, Error)]
pub enum EmbeddingKernelError {
    #[error("invalid embedding kernel analysis: {0}")]
    Invalid(String),
    #[error("embedding kernel analysis exceeded its numeric range: {0}")]
    Numeric(String),
}

#[derive(Clone, Copy, Default)]
struct StableSum {
    sum: f64,
    correction: f64,
}

impl StableSum {
    fn add(&mut self, value: f64) -> Result<(), EmbeddingKernelError> {
        if !value.is_finite() {
            return Err(EmbeddingKernelError::Numeric(
                "a kernel contribution is non-finite".into(),
            ));
        }
        let next = self.sum + value;
        if !next.is_finite() {
            return Err(EmbeddingKernelError::Numeric(
                "a kernel sum overflowed".into(),
            ));
        }
        self.correction += if self.sum.abs() >= value.abs() {
            (self.sum - next) + value
        } else {
            (value - next) + self.sum
        };
        if !self.correction.is_finite() {
            return Err(EmbeddingKernelError::Numeric(
                "a kernel correction overflowed".into(),
            ));
        }
        self.sum = next;
        Ok(())
    }

    fn total(self) -> Result<f64, EmbeddingKernelError> {
        let result = self.sum + self.correction;
        if result.is_finite() {
            Ok(result)
        } else {
            Err(EmbeddingKernelError::Numeric(
                "a kernel total overflowed".into(),
            ))
        }
    }
}

pub fn build_embedding_kernel(
    spec: &EmbeddingKernelSpec,
) -> Result<EmbeddingKernelArtifact, EmbeddingKernelError> {
    validate_kernel_input(spec)?;
    let training = spec
        .rows
        .iter()
        .filter(|row| row.split == "train")
        .collect::<Vec<_>>();
    let mut center_sums = vec![StableSum::default(); spec.feature_names.len()];
    for row in &training {
        for (sum, value) in center_sums.iter_mut().zip(&row.embedding) {
            sum.add(*value)?;
        }
    }
    let center = center_sums
        .into_iter()
        .map(|sum| sum.total().map(|value| value / training.len() as f64))
        .collect::<Result<Vec<_>, _>>()?;
    let scale = match spec.kind {
        EmbeddingKernelKind::Linear | EmbeddingKernelKind::Cosine => None,
        EmbeddingKernelKind::Rbf | EmbeddingKernelKind::Laplacian => {
            let mut distances = Vec::new();
            for left in 0..training.len() {
                for right in &training[(left + 1)..] {
                    let distance = match spec.kind {
                        EmbeddingKernelKind::Rbf => {
                            squared_distance(&training[left].embedding, &right.embedding)?.sqrt()
                        }
                        EmbeddingKernelKind::Laplacian => {
                            l1_distance(&training[left].embedding, &right.embedding)?
                        }
                        _ => unreachable!("matched scale-fitting kernel"),
                    };
                    if distance > 1e-14 {
                        distances.push(distance);
                    }
                }
            }
            if distances.is_empty() {
                return Err(EmbeddingKernelError::Invalid(
                    "training rows have no positive pair distance for kernel scale selection"
                        .into(),
                ));
            }
            distances.sort_by(f64::total_cmp);
            let middle = distances.len() / 2;
            Some(if distances.len().is_multiple_of(2) {
                distances[middle - 1] + (distances[middle] - distances[middle - 1]) / 2.0
            } else {
                distances[middle]
            })
        }
    };
    if spec.kind == EmbeddingKernelKind::Cosine {
        for row in &spec.rows {
            let norm = squared_centered_norm(&row.embedding, &center)?.sqrt();
            if norm <= 1e-14 {
                return Err(EmbeddingKernelError::Invalid(format!(
                    "cosine kernel row {} has zero centered norm",
                    row.object_id
                )));
            }
        }
    }
    let mut units = training
        .iter()
        .map(|row| row.biological_unit.clone())
        .collect::<Vec<_>>();
    units.sort();
    units.dedup();
    let (selection, formula) = match spec.kind {
        EmbeddingKernelKind::Linear => ("not_applicable", "centered_dot_product"),
        EmbeddingKernelKind::Cosine => ("not_applicable", "centered_cosine_similarity"),
        EmbeddingKernelKind::Rbf => (
            "median_positive_training_pair_euclidean_distance",
            "exp_negative_squared_distance_over_two_bandwidth_squared",
        ),
        EmbeddingKernelKind::Laplacian => (
            "median_positive_training_pair_l1_distance",
            "exp_negative_l1_distance_over_scale",
        ),
    };
    Ok(EmbeddingKernelArtifact {
        kind: spec.kind,
        fit_split: "train",
        training_biological_units: units,
        feature_names: spec.feature_names.clone(),
        center,
        scale,
        scale_selection: selection,
        formula,
        positive_semidefinite: true,
        preprocessing: "training_only_feature_centering",
    })
}

pub fn kernel_mark_correlation(
    mut spec: KernelMarkCorrelationSpec,
) -> Result<KernelMarkCorrelationResult, EmbeddingKernelError> {
    validate_bins(&mut spec.bins)?;
    if !spec.global_reference_tolerance.is_finite()
        || spec.global_reference_tolerance < 0.0
        || spec.maximum_pair_visits == 0
    {
        return Err(EmbeddingKernelError::Invalid(
            "global reference tolerance or pair bound is invalid".into(),
        ));
    }
    spec.kernel_spec
        .rows
        .sort_by(|left, right| left.object_id.cmp(&right.object_id));
    let kernel = build_embedding_kernel(&spec.kernel_spec)?;
    let split_counts = ["train", "validation", "test"]
        .into_iter()
        .filter_map(|split| {
            let count = spec
                .kernel_spec
                .rows
                .iter()
                .filter(|row| row.split == split)
                .count();
            (count > 0).then_some((split, count))
        })
        .collect::<Vec<_>>();
    let scale_fit_pairs = if matches!(
        kernel.kind,
        EmbeddingKernelKind::Rbf | EmbeddingKernelKind::Laplacian
    ) {
        let count = split_counts
            .iter()
            .find(|(split, _)| *split == "train")
            .expect("validated training split")
            .1 as u64;
        count * (count - 1) / 2
    } else {
        0
    };
    let pair_visits = split_counts
        .iter()
        .try_fold(scale_fit_pairs, |total, (_, count)| {
            let count = *count as u64;
            total.checked_add(count * (count - 1) / 2)
        });
    let pair_visits = pair_visits
        .ok_or_else(|| EmbeddingKernelError::Invalid("kernel pair count overflowed".into()))?;
    if pair_visits > spec.maximum_pair_visits
        || pair_visits
            .checked_mul(spec.kernel_spec.feature_names.len() as u64)
            .is_none_or(|work| work > 250_000_000)
    {
        return Err(EmbeddingKernelError::Invalid(format!(
            "{pair_visits} kernel pair visits exceed the declared or fixed resource bound"
        )));
    }

    let mut curves = Vec::with_capacity(split_counts.len());
    for (split, count) in split_counts {
        let rows = spec
            .kernel_spec
            .rows
            .iter()
            .filter(|row| row.split == split)
            .collect::<Vec<_>>();
        let split_pairs = count as u64 * (count as u64 - 1) / 2;
        let mut reference_sum = StableSum::default();
        let mut bin_sums = vec![StableSum::default(); spec.bins.len()];
        let mut bin_counts = vec![0_u64; spec.bins.len()];
        for left in 0..rows.len() {
            for right in &rows[(left + 1)..] {
                let similarity = evaluate_kernel(&rows[left].embedding, &right.embedding, &kernel)?;
                reference_sum.add(similarity)?;
                let distance = (rows[left].x_um - right.x_um).hypot(rows[left].y_um - right.y_um);
                if !distance.is_finite() {
                    return Err(EmbeddingKernelError::Numeric(
                        "a spatial pair distance is non-finite".into(),
                    ));
                }
                if let Some(bin) = find_bin(distance, &spec.bins) {
                    bin_counts[bin] += 1;
                    bin_sums[bin].add(similarity)?;
                }
            }
        }
        let reference = reference_sum.total()? / split_pairs as f64;
        let normalizable = reference > spec.global_reference_tolerance;
        let mut curve_rows = Vec::with_capacity(spec.bins.len());
        for (index, bin) in spec.bins.iter().enumerate() {
            let raw = if bin_counts[index] == 0 {
                None
            } else {
                Some(bin_sums[index].total()? / bin_counts[index] as f64)
            };
            curve_rows.push(KernelMarkCorrelationRow {
                bin_id: bin.bin_id.clone(),
                lower_um: bin.lower_um,
                upper_um: bin.upper_um,
                upper_inclusive: bin.upper_inclusive,
                pair_count: bin_counts[index],
                raw_similarity: raw,
                normalized_similarity: raw.filter(|_| normalizable).map(|value| value / reference),
                normalization_status: if normalizable {
                    "available"
                } else {
                    "zero_global_kernel_reference"
                },
            });
        }
        curves.push(KernelMarkCorrelationCurve {
            split: split.to_owned(),
            object_count: count as u32,
            pair_visits: split_pairs,
            global_reference: reference,
            rows: curve_rows,
        });
    }
    Ok(KernelMarkCorrelationResult {
        format: "marklab.kernel_mark_correlation",
        version: 1,
        coordinate_unit: "micrometer",
        kernel,
        curves,
    })
}

fn validate_kernel_input(spec: &EmbeddingKernelSpec) -> Result<(), EmbeddingKernelError> {
    if !(8..=10_000).contains(&spec.rows.len()) || !(2..=128).contains(&spec.feature_names.len()) {
        return Err(EmbeddingKernelError::Invalid(
            "kernel object count or embedding dimension is invalid".into(),
        ));
    }
    let mut features = HashSet::new();
    if spec.feature_names.iter().any(|name| {
        name.is_empty()
            || name.trim() != name
            || !name.starts_with("embedding_")
            || !features.insert(name.as_str())
    }) {
        return Err(EmbeddingKernelError::Invalid(
            "kernel feature names must be unique exact embedding_* names".into(),
        ));
    }
    let mut identifiers = HashSet::new();
    let mut unit_splits = HashMap::new();
    let mut split_counts = HashMap::<&str, usize>::new();
    for row in &spec.rows {
        if row.object_id.is_empty()
            || row.object_id.trim() != row.object_id
            || !identifiers.insert(row.object_id.as_str())
            || row.biological_unit.is_empty()
            || row.biological_unit.trim() != row.biological_unit
            || !matches!(row.split.as_str(), "train" | "validation" | "test")
            || !row.x_um.is_finite()
            || !row.y_um.is_finite()
            || row.embedding.len() != spec.feature_names.len()
            || row.embedding.iter().any(|value| !value.is_finite())
        {
            return Err(EmbeddingKernelError::Invalid(
                "kernel rows require unique exact identities, closed splits, finite coordinates, and complete finite vectors"
                    .into(),
            ));
        }
        if let Some(previous) = unit_splits.insert(row.biological_unit.as_str(), row.split.as_str())
        {
            if previous != row.split {
                return Err(EmbeddingKernelError::Invalid(
                    "each biological unit must belong to exactly one split".into(),
                ));
            }
        }
        *split_counts.entry(row.split.as_str()).or_default() += 1;
    }
    if split_counts.get("train").copied().unwrap_or(0) < 2
        || !(split_counts.contains_key("validation") || split_counts.contains_key("test"))
        || split_counts.values().any(|count| *count < 2)
    {
        return Err(EmbeddingKernelError::Invalid(
            "kernel fitting requires train and held-out splits with at least two rows each".into(),
        ));
    }
    Ok(())
}

fn validate_bins(bins: &mut [EmbeddingDistanceBin]) -> Result<(), EmbeddingKernelError> {
    if !(1..=256).contains(&bins.len()) {
        return Err(EmbeddingKernelError::Invalid(
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
            return Err(EmbeddingKernelError::Invalid(
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

fn find_bin(distance: f64, bins: &[EmbeddingDistanceBin]) -> Option<usize> {
    bins.iter().position(|bin| {
        distance >= bin.lower_um
            && (distance < bin.upper_um || (bin.upper_inclusive && distance <= bin.upper_um))
    })
}

fn evaluate_kernel(
    left: &[f64],
    right: &[f64],
    artifact: &EmbeddingKernelArtifact,
) -> Result<f64, EmbeddingKernelError> {
    let value = match artifact.kind {
        EmbeddingKernelKind::Linear => centered_dot(left, right, &artifact.center)?,
        EmbeddingKernelKind::Cosine => {
            centered_dot(left, right, &artifact.center)?
                / (squared_centered_norm(left, &artifact.center)?.sqrt()
                    * squared_centered_norm(right, &artifact.center)?.sqrt())
        }
        EmbeddingKernelKind::Rbf => {
            let scale = artifact.scale.expect("RBF scale is fitted");
            (-squared_distance(left, right)? / (2.0 * scale * scale)).exp()
        }
        EmbeddingKernelKind::Laplacian => {
            (-l1_distance(left, right)? / artifact.scale.expect("Laplacian scale is fitted")).exp()
        }
    };
    if value.is_finite() {
        Ok(value)
    } else {
        Err(EmbeddingKernelError::Numeric(
            "kernel evaluation is non-finite".into(),
        ))
    }
}

fn centered_dot(left: &[f64], right: &[f64], center: &[f64]) -> Result<f64, EmbeddingKernelError> {
    let mut sum = StableSum::default();
    for ((left, right), center) in left.iter().zip(right).zip(center) {
        sum.add((left - center) * (right - center))?;
    }
    sum.total()
}

fn squared_centered_norm(values: &[f64], center: &[f64]) -> Result<f64, EmbeddingKernelError> {
    let mut sum = StableSum::default();
    for (value, center) in values.iter().zip(center) {
        sum.add((value - center) * (value - center))?;
    }
    sum.total()
}

fn squared_distance(left: &[f64], right: &[f64]) -> Result<f64, EmbeddingKernelError> {
    let mut sum = StableSum::default();
    for (left, right) in left.iter().zip(right) {
        sum.add((left - right) * (left - right))?;
    }
    sum.total()
}

fn l1_distance(left: &[f64], right: &[f64]) -> Result<f64, EmbeddingKernelError> {
    let mut sum = StableSum::default();
    for (left, right) in left.iter().zip(right) {
        sum.add((left - right).abs())?;
    }
    sum.total()
}
