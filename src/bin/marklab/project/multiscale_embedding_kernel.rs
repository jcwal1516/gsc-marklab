use std::{collections::HashSet, fs, io, path::PathBuf};

use marklab_bayes::{
    multiscale_embedding_kernel, MultiscaleBaseKernel, MultiscaleEmbeddingKernelResult,
    MultiscaleEmbeddingKernelSpec, MultiscaleEmbeddingSummary, MultiscaleKernelError,
    MultiscaleKernelWeight,
};
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

const INPUT_KIND: &str =
    "application/vnd.marklab.source.multiscale-embedding-summaries-csv;version=1";
const WEIGHTS_KIND: &str = "application/vnd.marklab.source.multiscale-kernel-weights-csv;version=1";
const OUTPUT_KIND: &str = "application/vnd.marklab.multiscale-embedding-kernel+json;version=1";
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-multiscale-embedding-kernel-node-v1";

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    project_path: PathBuf,
    input_path: PathBuf,
    weights_path: PathBuf,
    sample_a: String,
    sample_b: String,
    base_kernel: String,
    kernel_scale: Option<f64>,
    maximum_summaries: usize,
    maximum_dimension: usize,
    maximum_component_scale_visits: u64,
    memory_budget_mib: usize,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let memory_bytes = memory_budget_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| BayesCliError::Input("--memory-budget-mib is too large".into()))?;
    if maximum_summaries == 0
        || maximum_dimension == 0
        || maximum_component_scale_visits == 0
        || memory_bytes == 0
    {
        return Err(BayesCliError::Input(
            "multiscale summary, dimension, work, and memory limits must be positive".into(),
        ));
    }
    let kernel = MultiscaleBaseKernel::parse(&base_kernel).map_err(map_error)?;
    let paths = SourcePaths {
        input: input_path,
        weights: weights_path,
    };
    let input_bytes = read_bounded(&paths.input, memory_bytes)?;
    let weight_bytes = read_bounded(&paths.weights, memory_bytes)?;
    let source_bytes = input_bytes
        .len()
        .checked_add(weight_bytes.len())
        .ok_or_else(|| BayesCliError::Input("multiscale source-byte total overflowed".into()))?;
    if source_bytes > memory_bytes {
        return Err(BayesCliError::Input(
            "multiscale source files exceed the retained-memory budget".into(),
        ));
    }
    let prepared_artifacts = source_artifacts_from_bytes(&input_bytes, &weight_bytes)?;
    let (summaries, feature_names) = read_summaries(&input_bytes)?;
    let weights = read_weights(&weight_bytes)?;
    let requirements = preflight(
        &summaries,
        &feature_names,
        &weights,
        &sample_a,
        &sample_b,
        kernel,
        kernel_scale,
        maximum_summaries,
        maximum_dimension,
        maximum_component_scale_visits,
    )?;
    let retained = retained_bytes(
        source_bytes,
        summaries.len(),
        feature_names.len(),
        weights.len(),
    )?;
    if retained > memory_bytes {
        return Err(BayesCliError::Input(format!(
            "multiscale kernel retained-memory estimate {retained} exceeds budget {memory_bytes}"
        )));
    }
    if source_artifacts(&paths)? != prepared_artifacts {
        return Err(BayesCliError::Input(
            "multiscale kernel sources changed while prepared".into(),
        ));
    }
    drop(input_bytes);
    drop(weight_bytes);

    let input_sha256 = prepared_artifacts[0].digest().to_string();
    let weights_sha256 = prepared_artifacts[1].digest().to_string();
    let analysis = MultiscaleEmbeddingKernelSpec {
        summaries,
        feature_names,
        weights,
        sample_a,
        sample_b,
        base_kernel: kernel,
        kernel_scale,
        maximum_component_scale_visits,
    };
    let node = MultiscaleEmbeddingKernelProjectNode::new(
        paths,
        prepared_artifacts.clone(),
        analysis,
        requirements,
        input_sha256,
        weights_sha256,
        base_kernel,
        maximum_summaries,
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
    for artifact in &prepared_artifacts {
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
        ArtifactSchema::new("marklab.multiscale_embedding_kernel", 1)
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    bayes::publish_json(&output_path, &execution.output)?;
    let cache_status = match execution.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!("project multiscale-embedding-kernel cache_status={cache_status}");
    Ok(())
}

#[derive(Clone)]
struct SourcePaths {
    input: PathBuf,
    weights: PathBuf,
}

fn source_artifacts(paths: &SourcePaths) -> Result<Vec<ArtifactRef>, BayesCliError> {
    Ok(vec![
        source_artifact(&paths.input, INPUT_KIND)?,
        source_artifact(&paths.weights, WEIGHTS_KIND)?,
    ])
}

fn source_artifacts_from_bytes(
    input_bytes: &[u8],
    weight_bytes: &[u8],
) -> Result<Vec<ArtifactRef>, BayesCliError> {
    Ok(vec![
        ArtifactRef::from_bytes(INPUT_KIND, input_bytes)
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
        ArtifactRef::from_bytes(WEIGHTS_KIND, weight_bytes)
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
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
            "multiscale source must be a regular file within {maximum} bytes: {}",
            path.display()
        )));
    }
    fs::read(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })
}

fn read_summaries(
    bytes: &[u8],
) -> Result<(Vec<MultiscaleEmbeddingSummary>, Vec<String>), BayesCliError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes);
    let headers = reader.headers()?.clone();
    if headers.len() < 3 || headers.iter().take(2).collect::<Vec<_>>() != ["sample_id", "scale_um"]
    {
        return Err(BayesCliError::Input(
            "multiscale summaries require sample_id,scale_um and embedding_* columns".into(),
        ));
    }
    let feature_names = headers
        .iter()
        .skip(2)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record?;
        rows.push(MultiscaleEmbeddingSummary {
            sample_id: record[0].to_owned(),
            scale_um: record[1]
                .parse()
                .map_err(|_| BayesCliError::Input("summary scale is invalid".into()))?,
            embedding: (2..record.len())
                .map(|index| {
                    record[index].parse::<f64>().map_err(|_| {
                        BayesCliError::Input(format!(
                            "summary value {} is invalid",
                            &headers[index]
                        ))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
        });
    }
    Ok((rows, feature_names))
}

fn read_weights(bytes: &[u8]) -> Result<Vec<MultiscaleKernelWeight>, BayesCliError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes);
    if reader.headers()?.iter().collect::<Vec<_>>() != ["scale_um", "weight"] {
        return Err(BayesCliError::Input(
            "multiscale weight headers must be scale_um,weight".into(),
        ));
    }
    reader
        .records()
        .map(|record| {
            let record = record?;
            Ok(MultiscaleKernelWeight {
                scale_um: record[0]
                    .parse()
                    .map_err(|_| BayesCliError::Input("weight scale is invalid".into()))?,
                weight: record[1]
                    .parse()
                    .map_err(|_| BayesCliError::Input("scale weight is invalid".into()))?,
            })
        })
        .collect()
}

#[derive(Clone)]
struct Requirements {
    sorted_weights: Vec<(f64, f64)>,
    component_scale_visits: u64,
}

#[allow(clippy::too_many_arguments)]
fn preflight(
    summaries: &[MultiscaleEmbeddingSummary],
    feature_names: &[String],
    weights: &[MultiscaleKernelWeight],
    sample_a: &str,
    sample_b: &str,
    base_kernel: MultiscaleBaseKernel,
    kernel_scale: Option<f64>,
    maximum_summaries: usize,
    maximum_dimension: usize,
    maximum_component_scale_visits: u64,
) -> Result<Requirements, BayesCliError> {
    if sample_a.is_empty()
        || sample_b.is_empty()
        || sample_a == sample_b
        || !(1..=128).contains(&feature_names.len())
        || !(2..=64).contains(&weights.len())
    {
        return Err(BayesCliError::Input(
            "dimensions or sample identities are invalid".into(),
        ));
    }
    if summaries.len() > maximum_summaries {
        return Err(BayesCliError::Input(format!(
            "multiscale summary count exceeds maximum_summaries {maximum_summaries}"
        )));
    }
    if feature_names.len() > maximum_dimension {
        return Err(BayesCliError::Input(format!(
            "multiscale embedding dimension exceeds maximum_dimension {maximum_dimension}"
        )));
    }
    let mut names = HashSet::new();
    if feature_names
        .iter()
        .any(|name| !name.starts_with("embedding_") || !names.insert(name.as_str()))
    {
        return Err(BayesCliError::Input("feature names are invalid".into()));
    }
    if matches!(
        base_kernel,
        MultiscaleBaseKernel::Rbf | MultiscaleBaseKernel::Laplacian
    ) {
        if kernel_scale.is_none_or(|value| !value.is_finite() || value <= 0.0) {
            return Err(BayesCliError::Input(
                "RBF/Laplacian kernel scale is invalid".into(),
            ));
        }
    } else if kernel_scale.is_some() {
        return Err(BayesCliError::Input(
            "linear/cosine kernels do not accept a scale".into(),
        ));
    }
    let mut sorted_weights = weights
        .iter()
        .map(|row| (row.scale_um, row.weight))
        .collect::<Vec<_>>();
    sorted_weights.sort_by(|left, right| left.0.total_cmp(&right.0));
    if sorted_weights.iter().any(|(scale, weight)| {
        !scale.is_finite()
            || *scale <= 0.0
            || !weight.is_finite()
            || *weight <= 0.0
            || *weight >= 1.0
    }) || sorted_weights.windows(2).any(|rows| rows[0].0 == rows[1].0)
    {
        return Err(BayesCliError::Input("scale weights are invalid".into()));
    }
    let weight_sum = sorted_weights.iter().map(|row| row.1).sum::<f64>();
    if !weight_sum.is_finite() || (weight_sum - 1.0).abs() > 1e-12 {
        return Err(BayesCliError::Input("scale weights must sum to one".into()));
    }
    let component_scale_visits = (weights.len() as u64)
        .checked_mul(feature_names.len() as u64)
        .ok_or_else(|| BayesCliError::Input("component-scale work overflowed".into()))?;
    if component_scale_visits > maximum_component_scale_visits
        || maximum_component_scale_visits > 250_000_000
    {
        return Err(BayesCliError::Input(
            "component-scale work exceeds its bound".into(),
        ));
    }
    let mut sorted_summaries = summaries.iter().collect::<Vec<_>>();
    sorted_summaries.sort_by(|left, right| {
        left.sample_id
            .cmp(&right.sample_id)
            .then_with(|| left.scale_um.total_cmp(&right.scale_um))
    });
    if summaries.len() != weights.len() * 2 {
        return Err(BayesCliError::Input(
            "both samples require every weighted scale".into(),
        ));
    }
    let selected = |id: &str| {
        sorted_summaries
            .iter()
            .copied()
            .filter(|row| row.sample_id == id)
            .collect::<Vec<_>>()
    };
    let a = selected(sample_a);
    let b = selected(sample_b);
    if a.len() != weights.len() || b.len() != weights.len() {
        return Err(BayesCliError::Input(
            "input contains unknown samples or missing scales".into(),
        ));
    }
    for (index, (scale, _)) in sorted_weights.iter().enumerate() {
        if a[index].scale_um.to_bits() != scale.to_bits()
            || b[index].scale_um.to_bits() != scale.to_bits()
            || a[index].embedding.len() != feature_names.len()
            || b[index].embedding.len() != feature_names.len()
            || a[index]
                .embedding
                .iter()
                .chain(&b[index].embedding)
                .any(|value| !value.is_finite())
        {
            return Err(BayesCliError::Input(
                "sample scales or vectors differ".into(),
            ));
        }
        if base_kernel == MultiscaleBaseKernel::Cosine {
            let a_norm = a[index]
                .embedding
                .iter()
                .map(|value| value * value)
                .sum::<f64>();
            let b_norm = b[index]
                .embedding
                .iter()
                .map(|value| value * value)
                .sum::<f64>();
            if !a_norm.is_finite() || !b_norm.is_finite() || (a_norm * b_norm).sqrt() <= 1e-14 {
                return Err(BayesCliError::Input(
                    "cosine summary has zero or non-finite norm".into(),
                ));
            }
        }
    }
    Ok(Requirements {
        sorted_weights,
        component_scale_visits,
    })
}

fn retained_bytes(
    source_bytes: usize,
    summaries: usize,
    dimension: usize,
    scales: usize,
) -> Result<usize, BayesCliError> {
    let analysis = source_bytes
        .checked_add(summaries.saturating_mul(256 + dimension.saturating_mul(8)))
        .and_then(|value| value.checked_add(dimension.saturating_mul(128)))
        .and_then(|value| value.checked_add(scales.saturating_mul(128)))
        .ok_or_else(|| BayesCliError::Input("multiscale analysis memory overflowed".into()))?;
    analysis
        .checked_mul(2)
        .and_then(|value| value.checked_add(scales.saturating_mul(512)))
        .ok_or_else(|| {
            BayesCliError::Input("multiscale retained-memory estimate overflowed".into())
        })
}

struct MultiscaleEmbeddingKernelProjectNode {
    spec: NodeSpec,
    paths: SourcePaths,
    input_artifacts: Vec<ArtifactRef>,
    analysis: MultiscaleEmbeddingKernelSpec,
    requirements: Requirements,
    input_sha256: String,
    weights_sha256: String,
    configuration_digest: ContentDigest,
    execution_policy: Vec<u8>,
}

impl MultiscaleEmbeddingKernelProjectNode {
    #[allow(clippy::too_many_arguments)]
    fn new(
        paths: SourcePaths,
        input_artifacts: Vec<ArtifactRef>,
        analysis: MultiscaleEmbeddingKernelSpec,
        requirements: Requirements,
        input_sha256: String,
        weights_sha256: String,
        base_kernel: String,
        maximum_summaries: usize,
        maximum_dimension: usize,
        memory_budget_mib: usize,
    ) -> Result<Self, BayesCliError> {
        if input_artifacts.len() != 2 {
            return Err(BayesCliError::Input(
                "multiscale kernel source identities are incomplete".into(),
            ));
        }
        let kernel_scale_identity = analysis
            .kernel_scale
            .map_or_else(|| "none".into(), |value| value.to_bits().to_string());
        let configuration_digest = ContentDigest::from_framed([
            b"marklab-multiscale-embedding-kernel-configuration-v1".as_slice(),
            analysis.sample_a.as_bytes(),
            analysis.sample_b.as_bytes(),
            base_kernel.as_bytes(),
            kernel_scale_identity.as_bytes(),
            maximum_summaries.to_string().as_bytes(),
            maximum_dimension.to_string().as_bytes(),
            analysis
                .maximum_component_scale_visits
                .to_string()
                .as_bytes(),
            memory_budget_mib.to_string().as_bytes(),
        ]);
        let execution_policy = format!(
            "serial;caller-prespecified-positive-sum-one-scales;drop-one-scale-sensitivity;base_kernel={base_kernel};kernel_scale_bits={kernel_scale_identity};sample_a={};sample_b={};maximum_summaries={maximum_summaries};maximum_dimension={maximum_dimension};maximum_component_scale_visits={};memory_budget_mib={memory_budget_mib}",
            analysis.sample_a,
            analysis.sample_b,
            analysis.maximum_component_scale_visits
        )
        .into_bytes();
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("multiscale-embedding-kernel")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "multiscale_embedding_kernel",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            paths,
            input_artifacts,
            analysis,
            requirements,
            input_sha256,
            weights_sha256,
            configuration_digest,
            execution_policy,
        })
    }

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn validate(&self, output: &Output) -> io::Result<()> {
        if self.requirements.sorted_weights.len() != self.analysis.weights.len()
            || self.requirements.component_scale_visits
                > self.analysis.maximum_component_scale_visits
        {
            return Err(invalid("multiscale kernel requirements differ"));
        }
        let expected_result =
            multiscale_embedding_kernel(self.analysis.clone()).map_err(|error| {
                invalid(&format!(
                    "multiscale kernel replay validation failed: {error}"
                ))
            })?;
        let expected = Output::from_result(
            self.input_sha256.clone(),
            self.weights_sha256.clone(),
            expected_result,
        );
        let expected_bytes = marklab::exact_float_json::encode(&expected)
            .map_err(|error| invalid(&error.to_string()))?;
        let actual_bytes = marklab::exact_float_json::encode(output)
            .map_err(|error| invalid(&error.to_string()))?;
        (actual_bytes == expected_bytes)
            .then_some(())
            .ok_or_else(|| invalid("decoded multiscale kernel result differs"))
    }
}

impl WorkflowNode for MultiscaleEmbeddingKernelProjectNode {
    type Output = Output;

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
                "multiscale kernel source identity changed",
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
        let result =
            multiscale_embedding_kernel(self.analysis.clone()).map_err(NodeError::execution)?;
        Ok(Output::from_result(
            self.input_sha256.clone(),
            self.weights_sha256.clone(),
            result,
        ))
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        self.validate(output).map_err(NodeError::encoding)?;
        marklab::exact_float_json::encode(output).map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: Output = marklab::exact_float_json::decode(bytes).map_err(NodeError::decode)?;
        self.validate(&output).map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        OUTPUT_KIND
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Output {
    input_sha256: String,
    weights_sha256: String,
    format: String,
    version: u32,
    sample_a: String,
    sample_b: String,
    feature_names: Vec<String>,
    base_kernel: String,
    kernel_scale: Option<f64>,
    scale_weight_policy: String,
    total: f64,
    scales: Vec<ScaleResult>,
    drop_one_scale_sensitivity: Vec<ScaleSensitivity>,
}

impl Output {
    fn from_result(
        input_sha256: String,
        weights_sha256: String,
        result: MultiscaleEmbeddingKernelResult,
    ) -> Self {
        Self {
            input_sha256,
            weights_sha256,
            format: result.format.into(),
            version: result.version,
            sample_a: result.sample_a,
            sample_b: result.sample_b,
            feature_names: result.feature_names,
            base_kernel: kernel_name(result.base_kernel).into(),
            kernel_scale: result.kernel_scale,
            scale_weight_policy: result.scale_weight_policy.into(),
            total: result.total,
            scales: result
                .scales
                .into_iter()
                .map(|row| ScaleResult {
                    scale_um: row.scale_um,
                    weight: row.weight,
                    raw_kernel: row.raw_kernel,
                    contribution: row.contribution,
                })
                .collect(),
            drop_one_scale_sensitivity: result
                .drop_one_scale_sensitivity
                .into_iter()
                .map(|row| ScaleSensitivity {
                    dropped_scale_um: row.dropped_scale_um,
                    renormalized_total: row.renormalized_total,
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ScaleResult {
    scale_um: f64,
    weight: f64,
    raw_kernel: f64,
    contribution: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ScaleSensitivity {
    dropped_scale_um: f64,
    renormalized_total: f64,
}

fn kernel_name(value: MultiscaleBaseKernel) -> &'static str {
    match value {
        MultiscaleBaseKernel::Linear => "linear",
        MultiscaleBaseKernel::Cosine => "cosine",
        MultiscaleBaseKernel::Rbf => "rbf",
        MultiscaleBaseKernel::Laplacian => "laplacian",
    }
}

fn map_error(error: MultiscaleKernelError) -> BayesCliError {
    match error {
        MultiscaleKernelError::Invalid(message) => BayesCliError::Input(message),
        MultiscaleKernelError::Numeric => BayesCliError::Backend(error.to_string()),
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
