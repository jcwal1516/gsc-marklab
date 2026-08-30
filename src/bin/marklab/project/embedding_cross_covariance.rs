use std::{fs, io, path::PathBuf};

use marklab_bayes::{embedding_cross_covariance_by_distance, EmbeddingCrossCovarianceSpec};
use marklab_workflow::{
    execute_algorithm, ArtifactRef, ArtifactSchema, CacheKeyMaterial, CacheStatus, ContentDigest,
    DurableProject, DurableProjectLimits, LocalScheduler, MarklabProject, NodeError, NodeId,
    NodeSpec, SchedulerLimits, WorkflowGraph, WorkflowNode,
};
use serde::{Deserialize, Serialize};

use super::{
    bayes, native_runtime_provenance, report_recovery, source_artifact, BayesCliError,
    MAXIMUM_INPUT_BYTES, MAXIMUM_RESULT_BYTES, PROJECT_CONTROL_BYTES, PROJECT_LEDGER_BYTES,
    PROJECT_LEDGER_RECORDS, PROJECT_RECORD_BYTES,
};

const INPUT_KIND: &str = "application/vnd.marklab.source.embedding-points-csv;version=1";
const BINS_KIND: &str = "application/vnd.marklab.source.distance-bins-csv;version=1";
const OUTPUT_KIND: &str =
    "application/vnd.marklab.embedding-cross-covariance-by-distance+json;version=1";
const IMPLEMENTATION_IDENTITY: &str =
    "marklab-project-embedding-cross-covariance-by-distance-node-v1";

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    project_path: PathBuf,
    input_path: PathBuf,
    bins_path: PathBuf,
    output_path: PathBuf,
    maximum_points: usize,
    maximum_dimension: usize,
    maximum_pair_visits: u64,
    maximum_matrix_elements: u64,
    memory_budget_mib: usize,
) -> Result<(), BayesCliError> {
    let memory_bytes = memory_budget_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| BayesCliError::Input("--memory-budget-mib is too large".into()))?;
    if maximum_points < 2
        || maximum_dimension < 2
        || maximum_pair_visits == 0
        || maximum_matrix_elements == 0
        || maximum_matrix_elements > 250_000_000
        || memory_bytes == 0
    {
        return Err(BayesCliError::Input(
            "embedding cross-covariance limits must admit two points and two dimensions, with positive bounded work and memory"
                .into(),
        ));
    }

    let paths = SourcePaths {
        input: input_path,
        bins: bins_path,
    };
    let before = source_artifacts(&paths)?;
    let input_bytes = read_bounded(&paths.input, memory_bytes)?;
    let bins_bytes = read_bounded(&paths.bins, memory_bytes)?;
    let source_bytes = input_bytes
        .len()
        .checked_add(bins_bytes.len())
        .ok_or_else(|| BayesCliError::Input("embedding source-byte total overflowed".into()))?;
    if source_bytes > memory_bytes {
        return Err(BayesCliError::Input(
            "embedding source files exceed the retained-memory budget".into(),
        ));
    }
    let (points, feature_names) = bayes::embedding_spatial::read_points(&input_bytes)?;
    let bins = bayes::embedding_spatial::read_bins(&bins_bytes)?;
    let requirements = preflight(
        points.len(),
        feature_names.len(),
        bins.len(),
        maximum_points,
        maximum_dimension,
        maximum_pair_visits,
        maximum_matrix_elements,
    )?;
    let retained = retained_bytes(source_bytes, points.len(), feature_names.len(), bins.len())?;
    if retained > memory_bytes {
        return Err(BayesCliError::Input(format!(
            "embedding cross-covariance retained-memory estimate {retained} exceeds budget {memory_bytes}"
        )));
    }
    let after = source_artifacts(&paths)?;
    if before != after {
        return Err(BayesCliError::Input(
            "embedding cross-covariance sources changed while prepared".into(),
        ));
    }

    let input_sha256 = after[0].digest().to_string();
    let bins_sha256 = after[1].digest().to_string();
    let node = EmbeddingCrossCovarianceProjectNode::new(
        paths,
        after.clone(),
        EmbeddingCrossCovarianceSpec {
            points,
            feature_names,
            bins,
            maximum_pair_visits,
            maximum_matrix_elements,
        },
        requirements,
        input_sha256,
        bins_sha256,
        maximum_points,
        maximum_dimension,
        memory_budget_mib,
    )?;
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    for artifact in &after {
        project
            .register_reference(artifact.clone())
            .map_err(|error| BayesCliError::Input(error.to_string()))?;
    }
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let execution = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        ArtifactSchema::new("marklab.embedding_cross_covariance", 1)
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    bayes::publish_json(&output_path, &execution.output)?;
    let cache_status = match execution.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!("project embedding-cross-covariance-by-distance cache_status={cache_status}");
    Ok(())
}

#[derive(Clone)]
struct SourcePaths {
    input: PathBuf,
    bins: PathBuf,
}

fn source_artifacts(paths: &SourcePaths) -> Result<Vec<ArtifactRef>, BayesCliError> {
    Ok(vec![
        source_artifact(&paths.input, INPUT_KIND)?,
        source_artifact(&paths.bins, BINS_KIND)?,
    ])
}

fn read_bounded(path: &std::path::Path, memory_bytes: usize) -> Result<Vec<u8>, BayesCliError> {
    let metadata = fs::metadata(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })?;
    let maximum = (MAXIMUM_INPUT_BYTES as usize).min(memory_bytes);
    if !metadata.is_file() || metadata.len() > maximum as u64 {
        return Err(BayesCliError::Input(format!(
            "embedding source must be a regular file within {maximum} bytes: {}",
            path.display()
        )));
    }
    fs::read(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })
}

#[derive(Clone, Copy)]
struct CrossCovarianceRequirements {
    pair_visits: u64,
    matrix_operations: u64,
    stored_matrix_elements: u64,
}

#[allow(clippy::too_many_arguments)]
fn preflight(
    points: usize,
    dimension: usize,
    bins: usize,
    maximum_points: usize,
    maximum_dimension: usize,
    maximum_pair_visits: u64,
    maximum_matrix_elements: u64,
) -> Result<CrossCovarianceRequirements, BayesCliError> {
    if !(2..=10_000).contains(&points) || !(2..=4_096).contains(&dimension) {
        return Err(BayesCliError::Input(
            "object count or embedding dimension is outside the supported bounds".into(),
        ));
    }
    if !(1..=256).contains(&bins) {
        return Err(BayesCliError::Input(
            "distance bin count is outside 1..=256".into(),
        ));
    }
    if points > maximum_points {
        return Err(BayesCliError::Input(format!(
            "point count exceeds maximum_points {maximum_points}"
        )));
    }
    if dimension > maximum_dimension {
        return Err(BayesCliError::Input(format!(
            "embedding dimension exceeds maximum_dimension {maximum_dimension}"
        )));
    }
    let point_count = u64::try_from(points)
        .map_err(|_| BayesCliError::Input("point count exceeds u64".into()))?;
    let pair_visits = point_count
        .checked_mul(point_count.saturating_sub(1))
        .and_then(|value| value.checked_div(2))
        .ok_or_else(|| BayesCliError::Input("pair count overflowed".into()))?;
    if pair_visits > maximum_pair_visits {
        return Err(BayesCliError::Input(format!(
            "{pair_visits} unordered pair visits exceed maximum_pair_visits {maximum_pair_visits}"
        )));
    }
    let dimension = u64::try_from(dimension)
        .map_err(|_| BayesCliError::Input("embedding dimension exceeds u64".into()))?;
    let elements_per_matrix = dimension
        .checked_mul(dimension)
        .ok_or_else(|| BayesCliError::Input("matrix size overflowed".into()))?;
    let stored_matrix_elements = elements_per_matrix
        .checked_mul(
            u64::try_from(bins)
                .map_err(|_| BayesCliError::Input("distance bin count exceeds u64".into()))?,
        )
        .ok_or_else(|| BayesCliError::Input("matrix artifact size overflowed".into()))?;
    if stored_matrix_elements > 1_000_000 {
        return Err(BayesCliError::Input(format!(
            "{stored_matrix_elements} stored matrix elements exceed the fixed ceiling 1000000"
        )));
    }
    let matrix_operations = elements_per_matrix
        .checked_mul(pair_visits)
        .ok_or_else(|| BayesCliError::Input("matrix work overflowed".into()))?;
    if matrix_operations > maximum_matrix_elements {
        return Err(BayesCliError::Input(format!(
            "{matrix_operations} pair-matrix element operations exceed maximum_matrix_elements {maximum_matrix_elements}"
        )));
    }
    Ok(CrossCovarianceRequirements {
        pair_visits,
        matrix_operations,
        stored_matrix_elements,
    })
}

struct EmbeddingCrossCovarianceProjectNode {
    spec: NodeSpec,
    paths: SourcePaths,
    input_artifacts: Vec<ArtifactRef>,
    analysis: EmbeddingCrossCovarianceSpec,
    requirements: CrossCovarianceRequirements,
    input_sha256: String,
    bins_sha256: String,
    configuration_digest: ContentDigest,
    execution_policy: Vec<u8>,
}

impl EmbeddingCrossCovarianceProjectNode {
    #[allow(clippy::too_many_arguments)]
    fn new(
        paths: SourcePaths,
        input_artifacts: Vec<ArtifactRef>,
        analysis: EmbeddingCrossCovarianceSpec,
        requirements: CrossCovarianceRequirements,
        input_sha256: String,
        bins_sha256: String,
        maximum_points: usize,
        maximum_dimension: usize,
        memory_budget_mib: usize,
    ) -> Result<Self, BayesCliError> {
        if input_artifacts.len() != 2 {
            return Err(BayesCliError::Input(
                "embedding cross-covariance source identities are incomplete".into(),
            ));
        }
        let configuration_digest = ContentDigest::from_framed([
            b"marklab-embedding-cross-covariance-configuration-v1".as_slice(),
            maximum_points.to_string().as_bytes(),
            maximum_dimension.to_string().as_bytes(),
            analysis.maximum_pair_visits.to_string().as_bytes(),
            analysis.maximum_matrix_elements.to_string().as_bytes(),
            memory_budget_mib.to_string().as_bytes(),
        ]);
        let execution_policy = format!(
            "serial;complete-vector-cross-covariance;physical-distance-bins;undirected-symmetrization;compensated-sums;maximum_points={maximum_points};maximum_dimension={maximum_dimension};maximum_pair_visits={};maximum_matrix_elements={};memory_budget_mib={memory_budget_mib}",
            analysis.maximum_pair_visits, analysis.maximum_matrix_elements
        )
        .into_bytes();
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("embedding-cross-covariance")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "embedding_cross_covariance_by_distance",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            paths,
            input_artifacts,
            analysis,
            requirements,
            input_sha256,
            bins_sha256,
            configuration_digest,
            execution_policy,
        })
    }

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn validate(&self, output: &CrossCovarianceDocument) -> io::Result<()> {
        let dimension = self.analysis.feature_names.len();
        if output.format != "marklab.embedding_cross_covariance_by_distance"
            || output.version != 1
            || output.input_sha256 != self.input_sha256
            || output.bins_sha256 != self.bins_sha256
            || output.coordinate_unit != "micrometer"
            || output.object_count as usize != self.analysis.points.len()
            || output.embedding_dimension as usize != dimension
            || output.feature_names != self.analysis.feature_names
            || output.pair_visits != self.requirements.pair_visits
            || output.pair_visits > self.analysis.maximum_pair_visits
            || output.global_mean.len() != dimension
            || output.global_mean.iter().any(|value| !value.is_finite())
            || output.symmetrization != "undirected_average_c_plus_c_transpose_over_two"
            || output.summaries.len() != self.analysis.bins.len()
            || output.matrix_artifact.format != "marklab.embedding_cross_covariance_matrix_artifact"
            || output.matrix_artifact.version != 1
            || output.matrix_artifact.feature_names != self.analysis.feature_names
            || output.matrix_artifact.matrices.len() != self.analysis.bins.len()
            || self.requirements.matrix_operations > self.analysis.maximum_matrix_elements
            || self.requirements.stored_matrix_elements > 1_000_000
        {
            return Err(invalid(
                "decoded embedding cross-covariance identity differs",
            ));
        }

        let mut assigned_pairs = 0_u64;
        for (index, ((summary, matrix), bin)) in output
            .summaries
            .iter()
            .zip(&output.matrix_artifact.matrices)
            .zip(&self.analysis.bins)
            .enumerate()
        {
            if summary.bin_id != bin.bin_id
                || summary.lower_um.to_bits() != bin.lower_um.to_bits()
                || summary.upper_um.to_bits() != bin.upper_um.to_bits()
                || summary.upper_inclusive != (index + 1 == output.summaries.len())
                || matrix.bin_id != bin.bin_id
                || matrix.pair_count != summary.pair_count
                || summary.trace.is_some() != (summary.pair_count > 0)
                || summary.frobenius_norm.is_some() != (summary.pair_count > 0)
                || matrix.matrix.is_some() != (summary.pair_count > 0)
            {
                return Err(invalid("decoded cross-covariance bin identity differs"));
            }
            assigned_pairs = assigned_pairs
                .checked_add(summary.pair_count)
                .ok_or_else(|| invalid("decoded pair counts overflowed"))?;
            if let Some(values) = &matrix.matrix {
                validate_matrix(values, summary, dimension)?;
            }
        }
        if assigned_pairs > output.pair_visits {
            return Err(invalid("decoded bin pair counts exceed visited pairs"));
        }
        Ok(())
    }
}

impl WorkflowNode for EmbeddingCrossCovarianceProjectNode {
    type Output = CrossCovarianceDocument;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let current = source_artifacts(&self.paths).map_err(NodeError::input)?;
        if current != self.input_artifacts {
            return Err(NodeError::input(io::Error::new(
                io::ErrorKind::InvalidData,
                "embedding cross-covariance source identity changed",
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self.configuration_digest,
            execution_policy: &self.execution_policy,
            implementation_identity: IMPLEMENTATION_IDENTITY,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        let result = embedding_cross_covariance_by_distance(self.analysis.clone())
            .map_err(NodeError::execution)?;
        Ok(CrossCovarianceDocument {
            format: "marklab.embedding_cross_covariance_by_distance".into(),
            version: 1,
            input_sha256: self.input_sha256.clone(),
            bins_sha256: self.bins_sha256.clone(),
            coordinate_unit: result.coordinate_unit.into(),
            object_count: result.object_count,
            embedding_dimension: result.embedding_dimension,
            feature_names: result.feature_names,
            pair_visits: result.pair_visits,
            global_mean: result.global_mean,
            symmetrization: result.symmetrization.into(),
            summaries: result
                .summaries
                .into_iter()
                .map(|summary| CrossCovarianceSummaryDocument {
                    bin_id: summary.bin_id,
                    lower_um: summary.lower_um,
                    upper_um: summary.upper_um,
                    upper_inclusive: summary.upper_inclusive,
                    pair_count: summary.pair_count,
                    trace: summary.trace,
                    frobenius_norm: summary.frobenius_norm,
                })
                .collect(),
            matrix_artifact: CrossCovarianceMatrixArtifactDocument {
                format: result.matrix_artifact.format.into(),
                version: result.matrix_artifact.version,
                feature_names: result.matrix_artifact.feature_names,
                matrices: result
                    .matrix_artifact
                    .matrices
                    .into_iter()
                    .map(|matrix| CrossCovarianceMatrixDocument {
                        bin_id: matrix.bin_id,
                        pair_count: matrix.pair_count,
                        matrix: matrix.matrix,
                    })
                    .collect(),
            },
        })
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        self.validate(output).map_err(NodeError::encoding)?;
        marklab::exact_float_json::encode(output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: CrossCovarianceDocument =
            marklab::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        self.validate(&output).map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        OUTPUT_KIND
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CrossCovarianceDocument {
    format: String,
    version: u32,
    input_sha256: String,
    bins_sha256: String,
    coordinate_unit: String,
    object_count: u32,
    embedding_dimension: u32,
    feature_names: Vec<String>,
    pair_visits: u64,
    global_mean: Vec<f64>,
    symmetrization: String,
    summaries: Vec<CrossCovarianceSummaryDocument>,
    matrix_artifact: CrossCovarianceMatrixArtifactDocument,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CrossCovarianceSummaryDocument {
    bin_id: String,
    lower_um: f64,
    upper_um: f64,
    upper_inclusive: bool,
    pair_count: u64,
    trace: Option<f64>,
    frobenius_norm: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CrossCovarianceMatrixArtifactDocument {
    format: String,
    version: u32,
    feature_names: Vec<String>,
    matrices: Vec<CrossCovarianceMatrixDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CrossCovarianceMatrixDocument {
    bin_id: String,
    pair_count: u64,
    matrix: Option<Vec<Vec<f64>>>,
}

fn validate_matrix(
    values: &[Vec<f64>],
    summary: &CrossCovarianceSummaryDocument,
    dimension: usize,
) -> io::Result<()> {
    if values.len() != dimension || values.iter().any(|row| row.len() != dimension) {
        return Err(invalid("decoded cross-covariance matrix dimension differs"));
    }
    let mut trace = StableSum::default();
    let mut squared_norm = StableSum::default();
    let mut diagonal_magnitude = 0.0;
    for (row_index, row) in values.iter().enumerate() {
        for (column_index, &value) in row.iter().enumerate() {
            if !value.is_finite() || value.to_bits() != values[column_index][row_index].to_bits() {
                return Err(invalid(
                    "decoded cross-covariance matrix is non-finite or asymmetric",
                ));
            }
            squared_norm.add(value * value)?;
        }
        let diagonal = row[row_index];
        trace.add(diagonal)?;
        diagonal_magnitude += diagonal.abs();
    }
    let trace = trace.total()?;
    let frobenius_norm = squared_norm.total()?.sqrt();
    if !frobenius_norm.is_finite()
        || !within_roundoff(
            summary.trace.unwrap_or(f64::NAN),
            trace,
            diagonal_magnitude,
            dimension,
        )
        || !within_roundoff(
            summary.frobenius_norm.unwrap_or(f64::NAN),
            frobenius_norm,
            frobenius_norm,
            dimension.saturating_mul(dimension),
        )
    {
        return Err(invalid(
            "decoded cross-covariance summaries differ from their matrix",
        ));
    }
    Ok(())
}

fn within_roundoff(left: f64, right: f64, scale: f64, operations: usize) -> bool {
    left.is_finite()
        && right.is_finite()
        && scale.is_finite()
        && (left - right).abs() <= f64::EPSILON * 8.0 * operations.max(1) as f64 * scale.max(1.0)
}

#[derive(Default)]
struct StableSum {
    sum: f64,
    correction: f64,
}

impl StableSum {
    fn add(&mut self, value: f64) -> io::Result<()> {
        if !value.is_finite() {
            return Err(invalid("decoded matrix contribution is non-finite"));
        }
        let next = self.sum + value;
        if !next.is_finite() {
            return Err(invalid("decoded matrix sum overflowed"));
        }
        if self.sum.abs() >= value.abs() {
            self.correction += (self.sum - next) + value;
        } else {
            self.correction += (value - next) + self.sum;
        }
        if !self.correction.is_finite() {
            return Err(invalid("decoded matrix correction overflowed"));
        }
        self.sum = next;
        Ok(())
    }

    fn total(self) -> io::Result<f64> {
        let total = self.sum + self.correction;
        if total.is_finite() {
            Ok(total)
        } else {
            Err(invalid("decoded matrix total overflowed"))
        }
    }
}

fn retained_bytes(
    source_bytes: usize,
    points: usize,
    dimension: usize,
    bins: usize,
) -> Result<usize, BayesCliError> {
    let point_values = points
        .checked_mul(dimension)
        .and_then(|value| value.checked_mul(std::mem::size_of::<f64>()))
        .ok_or_else(|| BayesCliError::Input("embedding point memory overflowed".into()))?;
    let matrix_values = dimension
        .checked_mul(dimension)
        .and_then(|value| value.checked_mul(bins))
        .and_then(|value| value.checked_mul(32))
        .ok_or_else(|| BayesCliError::Input("cross-covariance matrix memory overflowed".into()))?;
    source_bytes
        .checked_add(point_values.saturating_mul(2))
        .and_then(|value| value.checked_add(matrix_values))
        .and_then(|value| value.checked_add(points.saturating_mul(128)))
        .and_then(|value| value.checked_add(bins.saturating_mul(512)))
        .ok_or_else(|| {
            BayesCliError::Input("embedding cross-covariance memory estimate overflowed".into())
        })
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
