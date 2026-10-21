use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    io,
};

use marklab_embeddings::analysis::{
    kernel_mark_correlation, EmbeddingDistanceBin, EmbeddingKernelError, EmbeddingKernelKind,
    EmbeddingKernelSpec, KernelEmbeddingRow, KernelMarkCorrelationSpec,
};
use marklab_project::ContentDigest;
use marklab_workflow::NodeError;
use serde::{Deserialize, Serialize};

use super::cli_schema::{KernelMarkCorrelationLimits, KernelMarkCorrelationProjectArgs};
use super::{
    bayes,
    durable::{self, Codec, NodeContract, Source, SourceNode},
    CliError, MAXIMUM_INPUT_BYTES, MAXIMUM_RESULT_BYTES,
};

const INPUT_KIND: &str = "application/vnd.marklab.source.kernel-embedding-points-csv;version=1";
const BINS_KIND: &str = "application/vnd.marklab.source.distance-bins-csv;version=1";
const OUTPUT_KIND: &str = "application/vnd.marklab.kernel-mark-correlation+json;version=1";
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-kernel-mark-correlation-node-v1";

pub(super) fn run(args: KernelMarkCorrelationProjectArgs) -> Result<(), CliError> {
    let KernelMarkCorrelationProjectArgs {
        project: project_path,
        input: input_path,
        bins: bins_path,
        kernel,
        global_reference_tolerance,
        limits,
        out: output_path,
    } = args;
    let KernelMarkCorrelationLimits {
        maximum_points,
        maximum_dimension,
        maximum_pair_visits,
        memory_budget_mib,
    } = limits;
    const LABEL: &str = "kernel mark-correlation sources";
    let memory_bytes = memory_budget_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| CliError::Input("--memory-budget-mib is too large".into()))?;
    if memory_bytes == 0 {
        return Err(CliError::Input("memory budget must be positive".into()));
    }
    if maximum_points == 0 || maximum_dimension == 0 || maximum_pair_visits == 0 {
        return Err(CliError::Input(
            "kernel mark-correlation point, dimension, and pair-work limits must be positive"
                .into(),
        ));
    }
    if !global_reference_tolerance.is_finite() || global_reference_tolerance < 0.0 {
        return Err(CliError::Input(
            "global reference tolerance must be finite and nonnegative".into(),
        ));
    }
    let kernel_kind = EmbeddingKernelKind::parse(&kernel).map_err(map_error)?;
    let input_bytes = read_bounded(&input_path, memory_bytes)?;
    let bins_bytes = read_bounded(&bins_path, memory_bytes)?;
    let source_bytes = input_bytes
        .len()
        .checked_add(bins_bytes.len())
        .ok_or_else(|| CliError::Input("kernel source-byte total overflowed".into()))?;
    if source_bytes > memory_bytes {
        return Err(CliError::Input(
            "kernel source files exceed the retained-memory budget".into(),
        ));
    }
    let sources = vec![
        Source::from_bytes(input_path, INPUT_KIND, &input_bytes)?,
        Source::from_bytes(bins_path, BINS_KIND, &bins_bytes)?,
    ];
    let (rows, feature_names) = read_rows(&input_bytes)?;
    let bins = bayes::embedding_spatial::read_bins(&bins_bytes)?;
    let requirements = preflight(
        &rows,
        &feature_names,
        &bins,
        kernel_kind,
        maximum_points,
        maximum_dimension,
        maximum_pair_visits,
    )?;
    let retained = retained_bytes(
        source_bytes,
        rows.len(),
        feature_names.len(),
        bins.len(),
        requirements.scale_fit_pairs,
    )?;
    if retained > memory_bytes {
        return Err(CliError::Input(format!(
            "kernel mark-correlation retained-memory estimate {retained} exceeds budget {memory_bytes}"
        )));
    }
    Source::ensure_all_unchanged(&sources, LABEL)?;

    let input_sha256 = sources[0].artifact().digest().to_string();
    let bins_sha256 = sources[1].artifact().digest().to_string();
    let analysis = KernelMarkCorrelationSpec {
        kernel_spec: EmbeddingKernelSpec {
            rows,
            feature_names,
            kind: kernel_kind,
        },
        bins,
        global_reference_tolerance,
        maximum_pair_visits,
    };
    let (analysis, contract) = KernelMarkCorrelation::new(
        analysis,
        requirements,
        input_sha256,
        bins_sha256,
        kernel,
        limits,
    )?;
    let node = SourceNode::new(
        contract,
        LABEL,
        sources,
        analysis,
        KernelMarkCorrelation::execute,
        |analysis, output| analysis.validate(output).map_err(NodeError::decode),
    )?;

    let run = durable::execute_node(
        &project_path,
        &node,
        ("marklab.kernel_mark_correlation", 1),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "kernel-mark-correlation",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

fn read_bounded(path: &std::path::Path, memory_bytes: usize) -> Result<Vec<u8>, CliError> {
    let maximum = (MAXIMUM_INPUT_BYTES as usize).min(memory_bytes);
    crate::input_file::read_regular_file(
        path,
        maximum as u64,
        &format!(
            "kernel source must be a regular file within {maximum} bytes: {}",
            path.display()
        ),
    )
}

fn read_rows(bytes: &[u8]) -> Result<(Vec<KernelEmbeddingRow>, Vec<String>), CliError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes);
    let headers = reader.headers()?.clone();
    if headers.len() < 7
        || headers.iter().take(5).collect::<Vec<_>>()
            != ["object_id", "biological_unit", "split", "x_um", "y_um"]
    {
        return Err(CliError::Input(
            "kernel input requires object_id,biological_unit,split,x_um,y_um and at least two embedding_* columns"
                .into(),
        ));
    }
    let feature_names = headers
        .iter()
        .skip(5)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record?;
        let parse = |index: usize| {
            record[index].parse::<f64>().map_err(|_| {
                CliError::Input(format!(
                    "kernel numeric value in column {} is invalid",
                    &headers[index]
                ))
            })
        };
        rows.push(KernelEmbeddingRow {
            object_id: record[0].to_owned(),
            biological_unit: record[1].to_owned(),
            split: record[2].to_owned(),
            x_um: parse(3)?,
            y_um: parse(4)?,
            embedding: (5..record.len())
                .map(parse)
                .collect::<Result<Vec<_>, _>>()?,
        });
    }
    Ok((rows, feature_names))
}

#[derive(Clone, Copy)]
struct Requirements {
    pair_visits: u64,
    scale_fit_pairs: u64,
}

fn preflight(
    rows: &[KernelEmbeddingRow],
    feature_names: &[String],
    bins: &[EmbeddingDistanceBin],
    kernel_kind: EmbeddingKernelKind,
    maximum_points: usize,
    maximum_dimension: usize,
    maximum_pair_visits: u64,
) -> Result<Requirements, CliError> {
    if rows.len() > maximum_points {
        return Err(CliError::Input(format!(
            "point count exceeds maximum_points {maximum_points}"
        )));
    }
    if feature_names.len() > maximum_dimension {
        return Err(CliError::Input(format!(
            "embedding dimension exceeds maximum_dimension {maximum_dimension}"
        )));
    }
    validate_input(rows, feature_names)?;
    validate_bins(bins)?;

    let dimension = feature_names.len();
    let split_count = |split: &str| rows.iter().filter(|row| row.split == split).count() as u64;
    let train = split_count("train");
    let mut unit_counts = HashMap::<(&str, &str), u64>::new();
    for row in rows {
        *unit_counts
            .entry((&row.split, &row.biological_unit))
            .or_default() += 1;
    }
    let mut split_pairs = HashMap::<&str, u64>::new();
    for ((split, _), count) in unit_counts {
        *split_pairs.entry(split).or_default() += count * (count - 1) / 2;
    }
    if split_pairs.values().any(|pairs| *pairs == 0) {
        return Err(CliError::Input(
            "every split requires at least one within-biological-unit pair".into(),
        ));
    }
    let curve_pairs = split_pairs
        .values()
        .try_fold(0_u64, |total, count| total.checked_add(*count))
        .ok_or_else(|| CliError::Input("kernel pair count overflowed".into()))?;
    let scale_fit_pairs = if matches!(
        kernel_kind,
        EmbeddingKernelKind::Rbf | EmbeddingKernelKind::Laplacian
    ) {
        train
            .checked_mul(train.saturating_sub(1))
            .and_then(|value| value.checked_div(2))
            .ok_or_else(|| CliError::Input("kernel scale-fit pair count overflowed".into()))?
    } else {
        0
    };
    let pair_visits = curve_pairs
        .checked_add(scale_fit_pairs)
        .ok_or_else(|| CliError::Input("kernel pair count overflowed".into()))?;
    if pair_visits > maximum_pair_visits {
        return Err(CliError::Input(format!(
            "{pair_visits} kernel pair visits exceed maximum_pair_visits {maximum_pair_visits}"
        )));
    }
    let work = pair_visits
        .checked_mul(dimension as u64)
        .ok_or_else(|| CliError::Input("kernel element work overflowed".into()))?;
    if work > 250_000_000 {
        return Err(CliError::Input(format!(
            "{work} kernel element operations exceed the fixed ceiling 250000000"
        )));
    }
    Ok(Requirements {
        pair_visits,
        scale_fit_pairs,
    })
}

fn validate_input(rows: &[KernelEmbeddingRow], feature_names: &[String]) -> Result<(), CliError> {
    if !(8..=10_000).contains(&rows.len()) || !(2..=128).contains(&feature_names.len()) {
        return Err(CliError::Input(
            "kernel object count or embedding dimension is invalid".into(),
        ));
    }
    let mut features = HashSet::new();
    if feature_names.iter().any(|name| {
        name.is_empty()
            || name.trim() != name
            || !name.starts_with("embedding_")
            || !features.insert(name.as_str())
    }) {
        return Err(CliError::Input(
            "kernel feature names must be unique exact embedding_* names".into(),
        ));
    }

    let mut identifiers = HashSet::new();
    let mut unit_splits = HashMap::new();
    let mut split_counts = HashMap::<&str, usize>::new();
    for row in rows {
        if row.object_id.is_empty()
            || row.object_id.trim() != row.object_id
            || !identifiers.insert(row.object_id.as_str())
            || row.biological_unit.is_empty()
            || row.biological_unit.trim() != row.biological_unit
            || !matches!(row.split.as_str(), "train" | "validation" | "test")
            || !row.x_um.is_finite()
            || !row.y_um.is_finite()
            || row.embedding.len() != feature_names.len()
            || row.embedding.iter().any(|value| !value.is_finite())
        {
            return Err(CliError::Input(
                "kernel rows require unique exact identities, closed splits, finite coordinates, and complete finite vectors"
                    .into(),
            ));
        }
        if let Some(previous) = unit_splits.insert(row.biological_unit.as_str(), row.split.as_str())
        {
            if previous != row.split {
                return Err(CliError::Input(
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
        return Err(CliError::Input(
            "kernel fitting requires train and held-out splits with at least two rows each".into(),
        ));
    }
    Ok(())
}

fn validate_bins(bins: &[EmbeddingDistanceBin]) -> Result<(), CliError> {
    if !(1..=256).contains(&bins.len()) {
        return Err(CliError::Input("distance bin count is invalid".into()));
    }
    let mut identifiers = HashSet::new();
    for (index, bin) in bins.iter().enumerate() {
        if bin.bin_id.is_empty()
            || !identifiers.insert(bin.bin_id.as_str())
            || !bin.lower_um.is_finite()
            || !bin.upper_um.is_finite()
            || bin.lower_um < 0.0
            || bin.lower_um >= bin.upper_um
            || (index > 0 && bin.lower_um.to_bits() != bins[index - 1].upper_um.to_bits())
        {
            return Err(CliError::Input(
                "distance bins must be unique, contiguous, and increasing".into(),
            ));
        }
    }
    Ok(())
}

fn retained_bytes(
    source_bytes: usize,
    points: usize,
    dimension: usize,
    bins: usize,
    scale_fit_pairs: u64,
) -> Result<usize, CliError> {
    let embedding_bytes = points
        .checked_mul(dimension)
        .and_then(|value| value.checked_mul(std::mem::size_of::<f64>()))
        .ok_or_else(|| CliError::Input("kernel embedding memory overflowed".into()))?;
    // Radial scale fitting retains all training-pair distances, and stable sorting can retain
    // scratch storage. Three f64 slots per pair conservatively cover both allocations.
    let scale_fit_bytes = usize::try_from(scale_fit_pairs)
        .ok()
        .and_then(|pairs| pairs.checked_mul(3 * std::mem::size_of::<f64>()))
        .ok_or_else(|| CliError::Input("kernel scale-fit memory overflowed".into()))?;
    source_bytes
        .checked_add(embedding_bytes.saturating_mul(2))
        .and_then(|value| value.checked_add(points.saturating_mul(256)))
        .and_then(|value| value.checked_add(dimension.saturating_mul(128)))
        .and_then(|value| value.checked_add(bins.saturating_mul(512)))
        .and_then(|value| value.checked_add(scale_fit_bytes))
        .ok_or_else(|| CliError::Input("kernel retained-memory estimate overflowed".into()))
}

struct KernelMarkCorrelation {
    analysis: KernelMarkCorrelationSpec,
    requirements: Requirements,
    input_sha256: String,
    bins_sha256: String,
}

impl KernelMarkCorrelation {
    fn new(
        analysis: KernelMarkCorrelationSpec,
        requirements: Requirements,
        input_sha256: String,
        bins_sha256: String,
        kernel: String,
        limits: KernelMarkCorrelationLimits,
    ) -> Result<(Self, NodeContract), CliError> {
        let KernelMarkCorrelationLimits {
            maximum_points,
            maximum_dimension,
            memory_budget_mib,
            ..
        } = limits;
        let tolerance_bits = analysis.global_reference_tolerance.to_bits().to_string();
        let configuration_digest = ContentDigest::from_framed([
            b"marklab-kernel-mark-correlation-configuration-v1".as_slice(),
            kernel.as_bytes(),
            tolerance_bits.as_bytes(),
            maximum_points.to_string().as_bytes(),
            maximum_dimension.to_string().as_bytes(),
            analysis.maximum_pair_visits.to_string().as_bytes(),
            memory_budget_mib.to_string().as_bytes(),
        ]);
        let execution_policy = format!(
            "serial;training-only-kernel-fit;split-and-biological-unit-isolated-curves;physical-distance-bins;kernel={kernel};global_reference_tolerance_bits={tolerance_bits};maximum_points={maximum_points};maximum_dimension={maximum_dimension};maximum_pair_visits={};memory_budget_mib={memory_budget_mib}",
            analysis.maximum_pair_visits
        )
        .into_bytes();
        Ok((
            Self {
                analysis,
                requirements,
                input_sha256,
                bins_sha256,
            },
            NodeContract {
                id: "kernel-mark-correlation",
                kind: "kernel_mark_correlation",
                version: 1,
                configuration_digest,
                execution_policy: Cow::Owned(execution_policy),
                implementation_identity: IMPLEMENTATION_IDENTITY,
                output_kind: OUTPUT_KIND,
                codec: Codec::ExactFloatJson,
            },
        ))
    }

    fn validate(&self, output: &Output) -> io::Result<()> {
        let expected_kind = kernel_name(self.analysis.kernel_spec.kind);
        let expected_units = {
            let mut units = self
                .analysis
                .kernel_spec
                .rows
                .iter()
                .filter(|row| row.split == "train")
                .map(|row| row.biological_unit.clone())
                .collect::<Vec<_>>();
            units.sort();
            units.dedup();
            units
        };
        let radial = matches!(expected_kind, "rbf" | "laplacian");
        if output.input_sha256 != self.input_sha256
            || output.bins_sha256 != self.bins_sha256
            || output.format != "marklab.kernel_mark_correlation"
            || output.version != 1
            || output.coordinate_unit != "micrometer"
            || output.kernel.kind != expected_kind
            || output.kernel.fit_split != "train"
            || output.kernel.training_biological_units != expected_units
            || output.kernel.feature_names != self.analysis.kernel_spec.feature_names
            || output.kernel.center.len() != self.analysis.kernel_spec.feature_names.len()
            || output.kernel.center.iter().any(|value| !value.is_finite())
            || output.kernel.scale.is_some() != radial
            || output
                .kernel
                .scale
                .is_some_and(|value| !value.is_finite() || value <= 0.0)
            || !output.kernel.positive_semidefinite
            || output.kernel.preprocessing != "training_only_feature_centering"
        {
            return Err(invalid("decoded kernel mark-correlation identity differs"));
        }
        let (selection, formula) = match expected_kind {
            "linear" => ("not_applicable", "centered_dot_product"),
            "cosine" => ("not_applicable", "centered_cosine_similarity"),
            "rbf" => (
                "median_positive_training_pair_euclidean_distance",
                "exp_negative_squared_distance_over_two_bandwidth_squared",
            ),
            "laplacian" => (
                "median_positive_training_pair_l1_distance",
                "exp_negative_l1_distance_over_scale",
            ),
            _ => return Err(invalid("decoded kernel kind is invalid")),
        };
        if output.kernel.scale_selection != selection || output.kernel.formula != formula {
            return Err(invalid("decoded kernel formula identity differs"));
        }
        let expected_splits = ["train", "validation", "test"]
            .into_iter()
            .filter_map(|split| {
                let count = self
                    .analysis
                    .kernel_spec
                    .rows
                    .iter()
                    .filter(|row| row.split == split)
                    .count();
                (count > 0).then_some((split, count))
            })
            .collect::<Vec<_>>();
        if output.curves.len() != expected_splits.len() {
            return Err(invalid("decoded kernel split count differs"));
        }
        for (curve, (split, count)) in output.curves.iter().zip(expected_splits) {
            let mut unit_counts = HashMap::<&str, u64>::new();
            for row in self
                .analysis
                .kernel_spec
                .rows
                .iter()
                .filter(|row| row.split == split)
            {
                *unit_counts.entry(&row.biological_unit).or_default() += 1;
            }
            let pairs = unit_counts
                .values()
                .map(|count| count * (count - 1) / 2)
                .sum::<u64>();
            if curve.split != split
                || curve.object_count as usize != count
                || curve.pair_visits != pairs
                || !curve.global_reference.is_finite()
                || curve.rows.len() != self.analysis.bins.len()
            {
                return Err(invalid("decoded kernel curve identity differs"));
            }
            let normalizable = curve.global_reference > self.analysis.global_reference_tolerance;
            let mut assigned_pairs = 0_u64;
            for (index, (row, bin)) in curve.rows.iter().zip(&self.analysis.bins).enumerate() {
                if row.bin_id != bin.bin_id
                    || row.lower_um.to_bits() != bin.lower_um.to_bits()
                    || row.upper_um.to_bits() != bin.upper_um.to_bits()
                    || row.upper_inclusive != (index + 1 == self.analysis.bins.len())
                    || row.raw_similarity.is_some() != (row.pair_count > 0)
                    || row.raw_similarity.is_some_and(|value| !value.is_finite())
                    || row.normalized_similarity.is_some() != (row.pair_count > 0 && normalizable)
                    || row
                        .normalized_similarity
                        .is_some_and(|value| !value.is_finite())
                    || row.normalization_status
                        != if normalizable {
                            "available"
                        } else {
                            "zero_global_kernel_reference"
                        }
                {
                    return Err(invalid("decoded kernel bin identity differs"));
                }
                if let (Some(raw), Some(normalized)) =
                    (row.raw_similarity, row.normalized_similarity)
                {
                    if normalized.to_bits() != (raw / curve.global_reference).to_bits() {
                        return Err(invalid("decoded kernel normalization differs"));
                    }
                }
                assigned_pairs = assigned_pairs
                    .checked_add(row.pair_count)
                    .ok_or_else(|| invalid("decoded kernel bin counts overflowed"))?;
            }
            if assigned_pairs > pairs {
                return Err(invalid("decoded kernel bin counts exceed split pairs"));
            }
        }
        if self.requirements.pair_visits > self.analysis.maximum_pair_visits {
            return Err(invalid("decoded kernel result exceeds declared pair work"));
        }
        Ok(())
    }

    fn execute(&self) -> Result<Output, NodeError> {
        let result =
            kernel_mark_correlation(self.analysis.clone()).map_err(NodeError::execution)?;
        Ok(Output::from_result(
            self.input_sha256.clone(),
            self.bins_sha256.clone(),
            result,
        ))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Output {
    input_sha256: String,
    bins_sha256: String,
    format: String,
    version: u32,
    coordinate_unit: String,
    kernel: KernelArtifact,
    curves: Vec<Curve>,
}

impl Output {
    fn from_result(
        input_sha256: String,
        bins_sha256: String,
        result: marklab_embeddings::analysis::KernelMarkCorrelationResult,
    ) -> Self {
        Self {
            input_sha256,
            bins_sha256,
            format: result.format.into(),
            version: result.version,
            coordinate_unit: result.coordinate_unit.into(),
            kernel: KernelArtifact {
                kind: kernel_name(result.kernel.kind).into(),
                fit_split: result.kernel.fit_split.into(),
                training_biological_units: result.kernel.training_biological_units,
                feature_names: result.kernel.feature_names,
                center: result.kernel.center,
                scale: result.kernel.scale,
                scale_selection: result.kernel.scale_selection.into(),
                formula: result.kernel.formula.into(),
                positive_semidefinite: result.kernel.positive_semidefinite,
                preprocessing: result.kernel.preprocessing.into(),
            },
            curves: result
                .curves
                .into_iter()
                .map(|curve| Curve {
                    split: curve.split,
                    object_count: curve.object_count,
                    pair_visits: curve.pair_visits,
                    global_reference: curve.global_reference,
                    rows: curve
                        .rows
                        .into_iter()
                        .map(|row| CurveRow {
                            bin_id: row.bin_id,
                            lower_um: row.lower_um,
                            upper_um: row.upper_um,
                            upper_inclusive: row.upper_inclusive,
                            pair_count: row.pair_count,
                            raw_similarity: row.raw_similarity,
                            normalized_similarity: row.normalized_similarity,
                            normalization_status: row.normalization_status.into(),
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct KernelArtifact {
    kind: String,
    fit_split: String,
    training_biological_units: Vec<String>,
    feature_names: Vec<String>,
    center: Vec<f64>,
    scale: Option<f64>,
    scale_selection: String,
    formula: String,
    positive_semidefinite: bool,
    preprocessing: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Curve {
    split: String,
    object_count: u32,
    pair_visits: u64,
    global_reference: f64,
    rows: Vec<CurveRow>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CurveRow {
    bin_id: String,
    lower_um: f64,
    upper_um: f64,
    upper_inclusive: bool,
    pair_count: u64,
    raw_similarity: Option<f64>,
    normalized_similarity: Option<f64>,
    normalization_status: String,
}

fn kernel_name(kind: EmbeddingKernelKind) -> &'static str {
    match kind {
        EmbeddingKernelKind::Linear => "linear",
        EmbeddingKernelKind::Cosine => "cosine",
        EmbeddingKernelKind::Rbf => "rbf",
        EmbeddingKernelKind::Laplacian => "laplacian",
    }
}

fn map_error(error: EmbeddingKernelError) -> CliError {
    match error {
        EmbeddingKernelError::Invalid(message) => CliError::Input(message),
        EmbeddingKernelError::Numeric(message) => CliError::Backend(message),
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
