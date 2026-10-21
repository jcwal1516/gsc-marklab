use std::{borrow::Cow, io};

use marklab_embeddings::analysis::{vector_semivariogram, VectorSemivariogramSpec};
use marklab_project::ContentDigest;
use marklab_workflow::NodeError;
use serde::{Deserialize, Serialize};

use super::cli_schema::VectorSemivariogramProjectArgs;
use super::{
    bayes,
    durable::{self, Codec, NodeContract, Source, SourceNode},
    CliError, MAXIMUM_INPUT_BYTES, MAXIMUM_RESULT_BYTES,
};

const INPUT_KIND: &str = "application/vnd.marklab.source.embedding-points-csv;version=1";
const BINS_KIND: &str = "application/vnd.marklab.source.distance-bins-csv;version=1";
const WEIGHTS_KIND: &str = "application/vnd.marklab.source.pair-weights-csv;version=1";
const OUTPUT_KIND: &str = "application/vnd.marklab.vector-semivariogram+json;version=1";
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-vector-semivariogram-node-v1";

pub(super) fn run(args: VectorSemivariogramProjectArgs) -> Result<(), CliError> {
    let VectorSemivariogramProjectArgs {
        project: project_path,
        input: input_path,
        bins: bins_path,
        weights: weights_path,
        out: output_path,
        maximum_points,
        maximum_dimension,
        maximum_pair_visits,
        memory_budget_mib,
    } = args;
    const LABEL: &str = "vector semivariogram sources";
    let memory_bytes = memory_budget_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| CliError::Input("--memory-budget-mib is too large".into()))?;
    if maximum_points < 2 || maximum_dimension < 2 || maximum_pair_visits == 0 || memory_bytes == 0
    {
        return Err(CliError::Input(
            "vector semivariogram limits must admit two points and two dimensions".into(),
        ));
    }
    let mut sources = vec![
        Source::identify(input_path, INPUT_KIND)?,
        Source::identify(bins_path, BINS_KIND)?,
    ];
    if let Some(path) = weights_path {
        sources.push(Source::identify(path, WEIGHTS_KIND)?);
    }
    let input_bytes = read_bounded(sources[0].path(), memory_bytes)?;
    let bins_bytes = read_bounded(sources[1].path(), memory_bytes)?;
    let weight_bytes = sources
        .get(2)
        .map(|source| read_bounded(source.path(), memory_bytes))
        .transpose()?;
    let source_bytes = input_bytes
        .len()
        .checked_add(bins_bytes.len())
        .and_then(|value| value.checked_add(weight_bytes.as_ref().map_or(0, Vec::len)))
        .ok_or_else(|| CliError::Input("vector source-byte total overflowed".into()))?;
    if source_bytes > memory_bytes {
        return Err(CliError::Input(
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
        return Err(CliError::Input(
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
        return Err(CliError::Input(
            "vector semivariogram retained-memory estimate exceeds its budget".into(),
        ));
    }
    Source::ensure_all_unchanged(&sources, LABEL)?;
    let input_sha256 = sources[0].artifact().digest().to_string();
    let bins_sha256 = sources[1].artifact().digest().to_string();
    let weights_sha256 = sources
        .get(2)
        .map(|source| source.artifact().digest().to_string());
    let (analysis, contract) = VectorSemivariogram::new(
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
    );
    let node = SourceNode::new(
        contract,
        LABEL,
        sources,
        analysis,
        VectorSemivariogram::execute,
        |analysis, output| analysis.validate(output).map_err(NodeError::decode),
    )?;
    let run = durable::execute_node(
        &project_path,
        &node,
        ("marklab.vector_semivariogram", 1),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "vector-semivariogram",
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
            "vector source must be a regular file within {maximum} bytes: {}",
            path.display()
        ),
    )
}

struct VectorSemivariogram {
    analysis: VectorSemivariogramSpec,
    input_sha256: String,
    bins_sha256: String,
    weights_sha256: Option<String>,
}

impl VectorSemivariogram {
    fn new(
        analysis: VectorSemivariogramSpec,
        input_sha256: String,
        bins_sha256: String,
        weights_sha256: Option<String>,
        maximum_points: usize,
        maximum_dimension: usize,
        memory_budget_mib: usize,
    ) -> (Self, NodeContract) {
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
        (
            Self {
                analysis,
                input_sha256,
                bins_sha256,
                weights_sha256,
            },
            NodeContract {
                id: "vector-semivariogram",
                kind: "vector_semivariogram",
                version: 1,
                configuration_digest,
                execution_policy: Cow::Owned(execution_policy),
                implementation_identity: IMPLEMENTATION_IDENTITY,
                output_kind: OUTPUT_KIND,
                codec: Codec::Json,
            },
        )
    }

    fn execute(&self) -> Result<VectorSemivariogramDocument, NodeError> {
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
) -> Result<usize, CliError> {
    let values = points
        .checked_mul(dimension)
        .and_then(|value| value.checked_mul(std::mem::size_of::<f64>()))
        .ok_or_else(|| CliError::Input("vector value memory overflowed".into()))?;
    source_bytes
        .checked_add(values)
        .and_then(|value| value.checked_add(points.saturating_mul(128)))
        .and_then(|value| value.checked_add(bins.saturating_mul(256)))
        .and_then(|value| value.checked_add(weights.saturating_mul(256)))
        .ok_or_else(|| CliError::Input("vector retained-memory estimate overflowed".into()))
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
