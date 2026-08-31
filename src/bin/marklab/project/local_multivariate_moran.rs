use std::path::PathBuf;

use marklab::LocalMultivariateMoranResult;
use marklab_workflow::{
    execute_algorithm, ArtifactRef, ArtifactSchema, CacheKeyMaterial, CacheStatus, ContentDigest,
    DurableProject, DurableProjectLimits, LocalScheduler, MarklabProject, NodeError, NodeId,
    NodeSpec, SchedulerLimits, WorkflowGraph, WorkflowNode,
};

use super::{
    bayes, native_runtime_provenance, report_recovery, source_artifact, BayesCliError,
    MAXIMUM_RESULT_BYTES, PROJECT_CONTROL_BYTES, PROJECT_LEDGER_BYTES, PROJECT_LEDGER_RECORDS,
    PROJECT_RECORD_BYTES,
};
use crate::local_multivariate::{self, PreparedLocalMultivariateMoran};

const INPUT_KIND: &str = "application/vnd.marklab.source.local-multivariate-points+csv;version=1";
const WINDOW_KIND: &str = "application/vnd.marklab.source.observation-window+geojson;version=1";
const OUTPUT_KIND: &str = "application/vnd.marklab.local-multivariate-moran+json;version=1";
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-local-multivariate-moran-node-v1";

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    project_path: PathBuf,
    input_path: PathBuf,
    window_path: PathBuf,
    radius_um: f64,
    permutations: usize,
    seed: u64,
    maximum_points: usize,
    maximum_dimension: usize,
    maximum_directed_edges: usize,
    maximum_permutation_edge_evaluations: usize,
    memory_budget_mib: usize,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let memory_budget_bytes = memory_budget_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| BayesCliError::Input("--memory-budget-mib is too large".into()))?;
    let paths = SourcePaths {
        input: input_path,
        window: window_path,
    };
    let before = source_artifacts(&paths)?;
    let prepared = local_multivariate::prepare(&paths.input, &paths.window, memory_budget_bytes)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let after = source_artifacts(&paths)?;
    if before != after {
        return Err(BayesCliError::Input(
            "local multivariate Moran sources changed while prepared".into(),
        ));
    }
    let node = LocalMultivariateMoranProjectNode::new(
        paths,
        after.clone(),
        prepared,
        radius_um,
        permutations,
        seed,
        maximum_points,
        maximum_dimension,
        maximum_directed_edges,
        maximum_permutation_edge_evaluations,
        memory_budget_bytes,
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
        ArtifactSchema::new("marklab.local_multivariate_moran", 1)
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    bayes::publish_json(&output_path, &execution.output)?;
    let cache_status = match execution.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!("project local-multivariate-moran cache_status={cache_status}");
    Ok(())
}

#[derive(Clone)]
struct SourcePaths {
    input: PathBuf,
    window: PathBuf,
}

fn source_artifacts(paths: &SourcePaths) -> Result<Vec<ArtifactRef>, BayesCliError> {
    Ok(vec![
        source_artifact(&paths.input, INPUT_KIND)?,
        source_artifact(&paths.window, WINDOW_KIND)?,
    ])
}

struct LocalMultivariateMoranProjectNode {
    spec: NodeSpec,
    paths: SourcePaths,
    input_artifacts: Vec<ArtifactRef>,
    prepared: PreparedLocalMultivariateMoran,
    radius_um: f64,
    permutations: usize,
    seed: u64,
    maximum_points: usize,
    maximum_dimension: usize,
    maximum_directed_edges: usize,
    maximum_permutation_edge_evaluations: usize,
    memory_budget_bytes: usize,
    configuration_digest: ContentDigest,
    execution_policy: Vec<u8>,
}

impl LocalMultivariateMoranProjectNode {
    #[allow(clippy::too_many_arguments)]
    fn new(
        paths: SourcePaths,
        input_artifacts: Vec<ArtifactRef>,
        prepared: PreparedLocalMultivariateMoran,
        radius_um: f64,
        permutations: usize,
        seed: u64,
        maximum_points: usize,
        maximum_dimension: usize,
        maximum_directed_edges: usize,
        maximum_permutation_edge_evaluations: usize,
        memory_budget_bytes: usize,
        memory_budget_mib: usize,
    ) -> Result<Self, BayesCliError> {
        if input_artifacts.len() != 2 {
            return Err(BayesCliError::Input(
                "local multivariate Moran source identities are incomplete".into(),
            ));
        }
        let configuration_digest = ContentDigest::from_framed([
            b"marklab-local-multivariate-moran-configuration-v1".as_slice(),
            radius_um.to_bits().to_be_bytes().as_slice(),
            permutations.to_string().as_bytes(),
            seed.to_string().as_bytes(),
            maximum_points.to_string().as_bytes(),
            maximum_dimension.to_string().as_bytes(),
            maximum_directed_edges.to_string().as_bytes(),
            maximum_permutation_edge_evaluations.to_string().as_bytes(),
            memory_budget_mib.to_string().as_bytes(),
        ]);
        let execution_policy = format!(
            "serial;complete-multivariate-rows-within-strata;row-standardized-radius-weights;single-step-max-abs;permutations={permutations};seed={seed};maximum_points={maximum_points};maximum_dimension={maximum_dimension};maximum_directed_edges={maximum_directed_edges};maximum_permutation_edge_evaluations={maximum_permutation_edge_evaluations};memory_budget_mib={memory_budget_mib}"
        )
        .into_bytes();
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("local-multivariate-moran")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "local_multivariate_moran",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            paths,
            input_artifacts,
            prepared,
            radius_um,
            permutations,
            seed,
            maximum_points,
            maximum_dimension,
            maximum_directed_edges,
            maximum_permutation_edge_evaluations,
            memory_budget_bytes,
            configuration_digest,
            execution_policy,
        })
    }

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn validate_decoded(&self, result: &LocalMultivariateMoranResult) -> Result<(), NodeError> {
        let expected_stratum_count = self
            .prepared
            .points
            .iter()
            .map(|point| point.permutation_stratum.as_str())
            .collect::<std::collections::HashSet<_>>()
            .len();
        let maximum = result
            .locations
            .iter()
            .map(|row| row.statistic.abs())
            .fold(0.0_f64, f64::max);
        let denominator = self.permutations as f64 + 1.0;
        if result.format != "marklab.local_multivariate_moran"
            || result.version != 1
            || result.statistical_unit != "complete_multivariate_mark_row"
            || result.population_claim != "within_specimen_field_diagnostic_only"
            || result.null != "complete_rows_randomly_labeled_within_declared_strata"
            || result.multiplicity != "single_step_max_abs_over_all_locations"
            || result.feature_names != self.prepared.feature_names
            || result.feature_means.len() != self.prepared.feature_names.len()
            || result.feature_standard_deviations.len() != self.prepared.feature_names.len()
            || result.feature_means.iter().any(|value| !value.is_finite())
            || result
                .feature_standard_deviations
                .iter()
                .any(|value| !value.is_finite() || *value <= 0.0)
            || result.radius_um.to_bits() != self.radius_um.to_bits()
            || result.point_count != self.prepared.points.len()
            || result.dimension != self.prepared.feature_names.len()
            || result.stratum_count != expected_stratum_count
            || result.permutations_requested != self.permutations
            || result.permutations_completed != self.permutations
            || result.seed != self.seed
            || result.window_sha256 != self.prepared.window.descriptor().logical_digest.to_string()
            || !is_sha256(&result.weights_sha256)
            || result.directed_edge_count > self.maximum_directed_edges
            || result.permutation_edge_evaluations
                != result.directed_edge_count.saturating_mul(self.permutations)
            || result.permutation_edge_evaluations > self.maximum_permutation_edge_evaluations
            || result.retained_memory_bytes > self.memory_budget_bytes
            || result.locations.len() != self.prepared.points.len()
            || result
                .locations
                .iter()
                .zip(&self.prepared.points)
                .any(|(row, point)| {
                    row.point_id != point.point_id
                        || row.x_um.to_bits() != point.x_um.to_bits()
                        || row.y_um.to_bits() != point.y_um.to_bits()
                })
            || !result.global_max_abs_statistic.is_finite()
            || result.global_max_abs_statistic.to_bits() != maximum.to_bits()
            || !result.global_max_abs_p_value.is_finite()
            || !(0.0..=1.0).contains(&result.global_max_abs_p_value)
            || !permutation_probability(result.global_max_abs_p_value, denominator)
            || result.locations.iter().any(|row| {
                !row.statistic.is_finite()
                    || !row.raw_p_value.is_finite()
                    || !row.adjusted_p_value.is_finite()
                    || row.raw_p_value > row.adjusted_p_value
                    || !(0.0..=1.0).contains(&row.raw_p_value)
                    || !(0.0..=1.0).contains(&row.adjusted_p_value)
                    || !permutation_probability(row.raw_p_value, denominator)
                    || !permutation_probability(row.adjusted_p_value, denominator)
            })
        {
            return Err(NodeError::decode(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "decoded local multivariate Moran result does not match its cache-bound request",
            )));
        }
        Ok(())
    }
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn permutation_probability(value: f64, denominator: f64) -> bool {
    let count = value * denominator;
    (count - count.round()).abs() <= 1e-9 && count >= 1.0 && count <= denominator
}

impl WorkflowNode for LocalMultivariateMoranProjectNode {
    type Output = LocalMultivariateMoranResult;

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
                "local multivariate Moran sources changed after preparation",
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
        local_multivariate::evaluate(
            &self.prepared,
            self.radius_um,
            self.permutations,
            self.seed,
            self.maximum_points,
            self.maximum_dimension,
            self.maximum_directed_edges,
            self.maximum_permutation_edge_evaluations,
            self.memory_budget_bytes,
        )
        .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        serde_json::to_vec(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let result: LocalMultivariateMoranResult =
            serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        self.validate_decoded(&result)?;
        Ok(result)
    }

    fn output_kind(&self) -> &'static str {
        OUTPUT_KIND
    }
}
