use std::{fs, io, path::PathBuf};

use marklab_bayes::{vector_semivariogram, VectorSemivariogramSpec};
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
const WEIGHTS_KIND: &str = "application/vnd.marklab.source.pair-weights-csv;version=1";
const OUTPUT_KIND: &str = "application/vnd.marklab.vector-semivariogram+json;version=1";
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-vector-semivariogram-node-v1";

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    project_path: PathBuf,
    input_path: PathBuf,
    bins_path: PathBuf,
    weights_path: Option<PathBuf>,
    output_path: PathBuf,
    maximum_points: usize,
    maximum_dimension: usize,
    maximum_pair_visits: u64,
    memory_budget_mib: usize,
) -> Result<(), BayesCliError> {
    let memory_bytes = memory_budget_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| BayesCliError::Input("--memory-budget-mib is too large".into()))?;
    if maximum_points < 2 || maximum_dimension < 2 || maximum_pair_visits == 0 || memory_bytes == 0
    {
        return Err(BayesCliError::Input(
            "vector semivariogram limits must admit two points and two dimensions".into(),
        ));
    }
    let paths = SourcePaths {
        input: input_path,
        bins: bins_path,
        weights: weights_path,
    };
    let before = source_artifacts(&paths)?;
    let input_bytes = read_bounded(&paths.input, memory_bytes)?;
    let bins_bytes = read_bounded(&paths.bins, memory_bytes)?;
    let weight_bytes = paths
        .weights
        .as_ref()
        .map(|path| read_bounded(path, memory_bytes))
        .transpose()?;
    let source_bytes = input_bytes
        .len()
        .checked_add(bins_bytes.len())
        .and_then(|value| value.checked_add(weight_bytes.as_ref().map_or(0, Vec::len)))
        .ok_or_else(|| BayesCliError::Input("vector source-byte total overflowed".into()))?;
    if source_bytes > memory_bytes {
        return Err(BayesCliError::Input(
            "vector source files exceed the retained-memory budget".into(),
        ));
    }
    let (points, feature_names) = bayes::embedding_spatial::read_points(&input_bytes)?;
    let bins = bayes::embedding_spatial::read_bins(&bins_bytes)?;
    let weights = weight_bytes
        .as_deref()
        .map(bayes::embedding_spatial::read_weights)
        .transpose()?;
    if points.len() > maximum_points || feature_names.len() > maximum_dimension {
        return Err(BayesCliError::Input(
            "vector point count or embedding dimension exceeds its declared ceiling".into(),
        ));
    }
    let retained = retained_bytes(
        source_bytes,
        points.len(),
        feature_names.len(),
        bins.len(),
        weights.as_ref().map_or(0, Vec::len),
    )?;
    if retained > memory_bytes {
        return Err(BayesCliError::Input(
            "vector semivariogram retained-memory estimate exceeds its budget".into(),
        ));
    }
    let after = source_artifacts(&paths)?;
    if before != after {
        return Err(BayesCliError::Input(
            "vector semivariogram sources changed while prepared".into(),
        ));
    }
    let input_sha256 = after[0].digest().to_string();
    let bins_sha256 = after[1].digest().to_string();
    let weights_sha256 = after.get(2).map(|artifact| artifact.digest().to_string());
    let node = VectorSemivariogramProjectNode::new(
        paths,
        after.clone(),
        VectorSemivariogramSpec {
            points,
            feature_names,
            bins,
            weights,
            maximum_pair_visits,
        },
        input_sha256,
        bins_sha256,
        weights_sha256,
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
        ArtifactSchema::new("marklab.vector_semivariogram", 1)
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    bayes::publish_json(&output_path, &execution.output)?;
    let cache_status = match execution.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!("project vector-semivariogram cache_status={cache_status}");
    Ok(())
}

#[derive(Clone)]
struct SourcePaths {
    input: PathBuf,
    bins: PathBuf,
    weights: Option<PathBuf>,
}

fn source_artifacts(paths: &SourcePaths) -> Result<Vec<ArtifactRef>, BayesCliError> {
    let mut artifacts = vec![
        source_artifact(&paths.input, INPUT_KIND)?,
        source_artifact(&paths.bins, BINS_KIND)?,
    ];
    if let Some(path) = &paths.weights {
        artifacts.push(source_artifact(path, WEIGHTS_KIND)?);
    }
    Ok(artifacts)
}

fn read_bounded(path: &std::path::Path, memory_bytes: usize) -> Result<Vec<u8>, BayesCliError> {
    let metadata = fs::metadata(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })?;
    let maximum = (MAXIMUM_INPUT_BYTES as usize).min(memory_bytes);
    if !metadata.is_file() || metadata.len() > maximum as u64 {
        return Err(BayesCliError::Input(format!(
            "vector source must be a regular file within {maximum} bytes: {}",
            path.display()
        )));
    }
    fs::read(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })
}

struct VectorSemivariogramProjectNode {
    spec: NodeSpec,
    paths: SourcePaths,
    input_artifacts: Vec<ArtifactRef>,
    analysis: VectorSemivariogramSpec,
    input_sha256: String,
    bins_sha256: String,
    weights_sha256: Option<String>,
    configuration_digest: ContentDigest,
    execution_policy: Vec<u8>,
}

impl VectorSemivariogramProjectNode {
    #[allow(clippy::too_many_arguments)]
    fn new(
        paths: SourcePaths,
        input_artifacts: Vec<ArtifactRef>,
        analysis: VectorSemivariogramSpec,
        input_sha256: String,
        bins_sha256: String,
        weights_sha256: Option<String>,
        maximum_points: usize,
        maximum_dimension: usize,
        memory_budget_mib: usize,
    ) -> Result<Self, BayesCliError> {
        if input_artifacts.len() != if paths.weights.is_some() { 3 } else { 2 } {
            return Err(BayesCliError::Input(
                "vector source identities are incomplete".into(),
            ));
        }
        let configuration_digest = ContentDigest::from_framed([
            b"marklab-vector-semivariogram-configuration-v1".as_slice(),
            maximum_points.to_string().as_bytes(),
            maximum_dimension.to_string().as_bytes(),
            analysis.maximum_pair_visits.to_string().as_bytes(),
            memory_budget_mib.to_string().as_bytes(),
        ]);
        let execution_policy = format!(
            "serial;complete-vector;physical-distance-bins;compensated-sums;maximum_points={maximum_points};maximum_dimension={maximum_dimension};maximum_pair_visits={};memory_budget_mib={memory_budget_mib}",
            analysis.maximum_pair_visits
        )
        .into_bytes();
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("vector-semivariogram")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "vector_semivariogram",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            paths,
            input_artifacts,
            analysis,
            input_sha256,
            bins_sha256,
            weights_sha256,
            configuration_digest,
            execution_policy,
        })
    }

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn validate(&self, output: &VectorSemivariogramDocument) -> io::Result<()> {
        let point_count = self.analysis.points.len() as u64;
        let pair_visits = point_count
            .checked_mul(point_count - 1)
            .and_then(|value| value.checked_div(2))
            .ok_or_else(|| invalid("vector pair count overflowed"))?;
        if output.format != "marklab.vector_semivariogram"
            || output.version != 1
            || output.input_sha256 != self.input_sha256
            || output.bins_sha256 != self.bins_sha256
            || output.weights_sha256 != self.weights_sha256
            || output.coordinate_unit != "micrometer"
            || output.object_count as usize != self.analysis.points.len()
            || output.embedding_dimension as usize != self.analysis.feature_names.len()
            || output.feature_names != self.analysis.feature_names
            || output.pair_visits != pair_visits
            || output.pair_visits > self.analysis.maximum_pair_visits
            || !output.rotation_invariant
            || output.curve.len() != self.analysis.bins.len()
        {
            return Err(invalid("decoded vector semivariogram identity differs"));
        }
        let expected_weighting = if self.analysis.weights.is_some() {
            "declared_pair_weights"
        } else {
            "unit"
        };
        if output.weighting != expected_weighting {
            return Err(invalid("decoded vector semivariogram weighting differs"));
        }
        for (index, (row, bin)) in output.curve.iter().zip(&self.analysis.bins).enumerate() {
            if row.bin_id != bin.bin_id
                || row.lower_um.to_bits() != bin.lower_um.to_bits()
                || row.upper_um.to_bits() != bin.upper_um.to_bits()
                || row.upper_inclusive != (index + 1 == output.curve.len())
                || !row.weight_sum.is_finite()
                || row.weight_sum < 0.0
                || row
                    .semivariance
                    .is_some_and(|value| !value.is_finite() || value < 0.0)
                || row.semivariance.is_some() != (row.pair_count > 0)
                || row.inference_eligible != (row.pair_count >= 2)
            {
                return Err(invalid("decoded vector semivariogram curve differs"));
            }
        }
        Ok(())
    }
}

impl WorkflowNode for VectorSemivariogramProjectNode {
    type Output = VectorSemivariogramDocument;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let current = source_artifacts(&self.paths).map_err(NodeError::input)?;
        if current != self.input_artifacts {
            return Err(NodeError::input(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "vector source identity changed",
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
        let result = vector_semivariogram(self.analysis.clone()).map_err(NodeError::execution)?;
        Ok(VectorSemivariogramDocument {
            format: "marklab.vector_semivariogram".into(),
            version: 1,
            input_sha256: self.input_sha256.clone(),
            bins_sha256: self.bins_sha256.clone(),
            weights_sha256: self.weights_sha256.clone(),
            coordinate_unit: result.coordinate_unit.into(),
            object_count: result.object_count,
            embedding_dimension: result.embedding_dimension,
            feature_names: result.feature_names,
            pair_visits: result.pair_visits,
            weighting: result.weighting.into(),
            rotation_invariant: result.rotation_invariant,
            curve: result
                .curve
                .into_iter()
                .map(|row| VectorSemivariogramRowDocument {
                    bin_id: row.bin_id,
                    lower_um: row.lower_um,
                    upper_um: row.upper_um,
                    upper_inclusive: row.upper_inclusive,
                    pair_count: row.pair_count,
                    weight_sum: row.weight_sum,
                    semivariance: row.semivariance,
                    inference_eligible: row.inference_eligible,
                })
                .collect(),
        })
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        self.validate(output).map_err(NodeError::encoding)?;
        serde_json::to_vec(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: VectorSemivariogramDocument =
            serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        self.validate(&output).map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        OUTPUT_KIND
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct VectorSemivariogramDocument {
    format: String,
    version: u32,
    input_sha256: String,
    bins_sha256: String,
    weights_sha256: Option<String>,
    coordinate_unit: String,
    object_count: u32,
    embedding_dimension: u32,
    feature_names: Vec<String>,
    pair_visits: u64,
    weighting: String,
    rotation_invariant: bool,
    curve: Vec<VectorSemivariogramRowDocument>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct VectorSemivariogramRowDocument {
    bin_id: String,
    lower_um: f64,
    upper_um: f64,
    upper_inclusive: bool,
    pair_count: u64,
    weight_sum: f64,
    semivariance: Option<f64>,
    inference_eligible: bool,
}

fn retained_bytes(
    source_bytes: usize,
    points: usize,
    dimension: usize,
    bins: usize,
    weights: usize,
) -> Result<usize, BayesCliError> {
    let values = points
        .checked_mul(dimension)
        .and_then(|value| value.checked_mul(std::mem::size_of::<f64>()))
        .ok_or_else(|| BayesCliError::Input("vector value memory overflowed".into()))?;
    source_bytes
        .checked_add(values)
        .and_then(|value| value.checked_add(points.saturating_mul(128)))
        .and_then(|value| value.checked_add(bins.saturating_mul(256)))
        .and_then(|value| value.checked_add(weights.saturating_mul(256)))
        .ok_or_else(|| BayesCliError::Input("vector retained-memory estimate overflowed".into()))
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
