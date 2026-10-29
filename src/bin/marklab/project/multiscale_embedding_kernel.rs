use std::{borrow::Cow, io};

use crate::codecs::multiscale_kernel::{read_summaries, read_weights};

use marklab_embeddings::analysis::{
    multiscale_embedding_kernel, MultiscaleBaseKernel, MultiscaleEmbeddingKernelResult,
    MultiscaleEmbeddingKernelSpec, MultiscaleKernelError,
};
use marklab_project::ContentDigest;
use marklab_workflow::NodeError;
use serde::{Deserialize, Serialize};

use super::cli_schema::{MultiscaleEmbeddingKernelLimits, MultiscaleEmbeddingKernelProjectArgs};
use super::{
    durable::{self, Codec, NodeContract, Source, SourceNode},
    CliError, MAXIMUM_INPUT_BYTES, MAXIMUM_RESULT_BYTES,
};

const INPUT_KIND: &str =
    "application/vnd.marklab.source.multiscale-embedding-summaries-csv;version=1";
const WEIGHTS_KIND: &str = "application/vnd.marklab.source.multiscale-kernel-weights-csv;version=1";
const OUTPUT_KIND: &str = "application/vnd.marklab.multiscale-embedding-kernel+json;version=1";
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-multiscale-embedding-kernel-node-v1";

pub(super) fn run(args: MultiscaleEmbeddingKernelProjectArgs) -> Result<(), CliError> {
    let MultiscaleEmbeddingKernelProjectArgs {
        project: project_path,
        input: input_path,
        weights: weights_path,
        sample_a,
        sample_b,
        base_kernel,
        kernel_scale,
        limits,
        out: output_path,
    } = args;
    let MultiscaleEmbeddingKernelLimits {
        maximum_summaries,
        maximum_dimension,
        maximum_component_scale_visits,
        memory_budget_mib,
    } = limits;
    const LABEL: &str = "multiscale kernel sources";
    let memory_bytes = memory_budget_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| CliError::Input("--memory-budget-mib is too large".into()))?;
    if maximum_summaries == 0
        || maximum_dimension == 0
        || maximum_component_scale_visits == 0
        || memory_bytes == 0
    {
        return Err(CliError::Input(
            "multiscale summary, dimension, work, and memory limits must be positive".into(),
        ));
    }
    let kernel = MultiscaleBaseKernel::parse(&base_kernel).map_err(map_error)?;
    let input_bytes = read_bounded(&input_path, memory_bytes)?;
    let weight_bytes = read_bounded(&weights_path, memory_bytes)?;
    let source_bytes = input_bytes
        .len()
        .checked_add(weight_bytes.len())
        .ok_or_else(|| CliError::Input("multiscale source-byte total overflowed".into()))?;
    if source_bytes > memory_bytes {
        return Err(CliError::Input(
            "multiscale source files exceed the retained-memory budget".into(),
        ));
    }
    let sources = vec![
        Source::from_bytes(input_path, INPUT_KIND, &input_bytes)?,
        Source::from_bytes(weights_path, WEIGHTS_KIND, &weight_bytes)?,
    ];
    let (summaries, feature_names) = read_summaries(&input_bytes)?;
    let weights = read_weights(&weight_bytes)?;
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
    preflight(&analysis, limits)?;
    let retained = retained_bytes(
        source_bytes,
        analysis.summaries.len(),
        analysis.feature_names.len(),
        analysis.weights.len(),
    )?;
    if retained > memory_bytes {
        return Err(CliError::Input(format!(
            "multiscale kernel retained-memory estimate {retained} exceeds budget {memory_bytes}"
        )));
    }
    Source::ensure_all_unchanged(&sources, LABEL)?;
    drop(input_bytes);
    drop(weight_bytes);

    let input_sha256 = sources[0].artifact().digest().to_string();
    let weights_sha256 = sources[1].artifact().digest().to_string();
    let (analysis, contract) = MultiscaleEmbeddingKernel::new(
        analysis,
        input_sha256,
        weights_sha256,
        base_kernel,
        limits,
    )?;
    let node = SourceNode::new(
        contract,
        LABEL,
        sources,
        analysis,
        MultiscaleEmbeddingKernel::execute,
        |analysis, output| analysis.validate(output).map_err(NodeError::decode),
    )?;

    let run = durable::execute_node(
        &project_path,
        &node,
        ("marklab.multiscale_embedding_kernel", 1),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "multiscale-embedding-kernel",
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
            "multiscale source must be a regular file within {maximum} bytes: {}",
            path.display()
        ),
    )
}

fn preflight(
    analysis: &MultiscaleEmbeddingKernelSpec,
    limits: MultiscaleEmbeddingKernelLimits,
) -> Result<(), CliError> {
    if analysis.summaries.len() > limits.maximum_summaries {
        return Err(CliError::Input(format!(
            "multiscale summary count exceeds maximum_summaries {}",
            limits.maximum_summaries
        )));
    }
    if analysis.feature_names.len() > limits.maximum_dimension {
        return Err(CliError::Input(format!(
            "multiscale embedding dimension exceeds maximum_dimension {}",
            limits.maximum_dimension
        )));
    }
    Ok(())
}

fn retained_bytes(
    source_bytes: usize,
    summaries: usize,
    dimension: usize,
    scales: usize,
) -> Result<usize, CliError> {
    let analysis = source_bytes
        .checked_add(summaries.saturating_mul(256 + dimension.saturating_mul(8)))
        .and_then(|value| value.checked_add(dimension.saturating_mul(128)))
        .and_then(|value| value.checked_add(scales.saturating_mul(128)))
        .ok_or_else(|| CliError::Input("multiscale analysis memory overflowed".into()))?;
    analysis
        .checked_mul(2)
        .and_then(|value| value.checked_add(scales.saturating_mul(512)))
        .ok_or_else(|| CliError::Input("multiscale retained-memory estimate overflowed".into()))
}

struct MultiscaleEmbeddingKernel {
    expected: Output,
}

impl MultiscaleEmbeddingKernel {
    fn new(
        analysis: MultiscaleEmbeddingKernelSpec,
        input_sha256: String,
        weights_sha256: String,
        base_kernel: String,
        limits: MultiscaleEmbeddingKernelLimits,
    ) -> Result<(Self, NodeContract), CliError> {
        let MultiscaleEmbeddingKernelLimits {
            maximum_summaries,
            maximum_dimension,
            memory_budget_mib,
            ..
        } = limits;
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
        Ok((
            Self {
                expected: Output::from_result(
                    input_sha256,
                    weights_sha256,
                    multiscale_embedding_kernel(analysis).map_err(map_error)?,
                ),
            },
            NodeContract {
                id: "multiscale-embedding-kernel",
                kind: "multiscale_embedding_kernel",
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
        let expected_bytes = marklab::exact_float_json::encode(&self.expected)
            .map_err(|error| invalid(&error.to_string()))?;
        let actual_bytes = marklab::exact_float_json::encode(output)
            .map_err(|error| invalid(&error.to_string()))?;
        (actual_bytes == expected_bytes)
            .then_some(())
            .ok_or_else(|| invalid("decoded multiscale kernel result differs"))
    }

    fn execute(&self) -> Result<Output, NodeError> {
        Ok(self.expected.clone())
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

fn map_error(error: MultiscaleKernelError) -> CliError {
    match error {
        MultiscaleKernelError::Invalid(message) => CliError::Input(message),
        MultiscaleKernelError::Numeric => CliError::Backend(error.to_string()),
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
