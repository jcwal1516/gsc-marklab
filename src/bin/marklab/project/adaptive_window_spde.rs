use std::path::PathBuf;

use marklab_workflow::{
    execute_algorithm, ArtifactRef, ArtifactSchema, CacheKeyMaterial, CacheStatus, ContentDigest,
    DurableProject, DurableProjectLimits, LocalScheduler, MarklabProject, NodeError, NodeId,
    NodeSpec, SchedulerLimits, WorkflowGraph, WorkflowNode,
};
use serde::{Deserialize, Serialize};

use super::{
    bayes, native_runtime_provenance, report_recovery, BayesCliError, MAXIMUM_RESULT_BYTES,
    PROJECT_CONTROL_BYTES, PROJECT_LEDGER_BYTES, PROJECT_LEDGER_RECORDS, PROJECT_RECORD_BYTES,
};
use crate::bayes_advanced::{self, PreparedAdvancedBayes};

const INPUT_KIND: &str = "application/vnd.marklab.source.adaptive-window-spde+json;version=1";
const LOCK_KIND: &str = "application/vnd.marklab.python-lock+text;version=1";
const WORKER_KIND: &str = "application/vnd.marklab.python-worker+source;version=1";
const OUTPUT_KIND: &str = "application/vnd.marklab.adaptive-window-spde+json;version=1";
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-adaptive-window-spde-node-v1";

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AdaptiveWindowSpdeResult {
    format: String,
    version: u32,
    window: AdaptiveWindowSummary,
    mesh: AdaptiveMeshSummary,
    spatial_factor: AdaptiveSpatialFactorSummary,
    mesh_sensitivity: Vec<AdaptiveMeshSensitivity>,
    limits: AdaptiveSpdeLimits,
    assumptions: Vec<String>,
    limitations: Vec<String>,
    claim_status: String,
    backend: AdaptiveSpdeBackend,
    request_sha256: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AdaptiveWindowSummary {
    frame: String,
    exact_area: f64,
    bounds: [f64; 4],
    component_count: usize,
    hole_count: usize,
    ring_count: usize,
    vertex_count: usize,
    geometry_policy: String,
    canonical_sha256: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AdaptiveMeshSummary {
    basis: String,
    boundary_condition: String,
    alpha: u32,
    kappa: f64,
    tau: f64,
    base_resolution: u32,
    boundary_refinement_levels: u32,
    vertices: Vec<[f64; 2]>,
    triangles: Vec<[usize; 3]>,
    vertex_count: usize,
    triangle_count: usize,
    connected_components: usize,
    mesh_area: f64,
    relative_area_error: f64,
    constant_mass_integral: f64,
    mass_matrix: SparseTriplets,
    stiffness_matrix: SparseTriplets,
    precision_matrix: SparseTriplets,
    minimum_precision_eigenvalue: f64,
    maximum_precision_eigenvalue: f64,
    maximum_projection_row_sum_error: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SparseTriplets {
    rows: Vec<usize>,
    columns: Vec<usize>,
    values: Vec<f64>,
    shape: [usize; 2],
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AdaptiveSpatialFactorSummary {
    inference: String,
    factor_count: u32,
    region_ids: Vec<String>,
    feature_count: usize,
    feature_means: Vec<f64>,
    mesh_field_weights: Vec<f64>,
    projected_region_factor: Vec<f64>,
    loadings: Vec<f64>,
    reconstruction_rmse: f64,
    factor_x_correlation: f64,
    objective: f64,
    gradient_maximum: f64,
    iterations: usize,
    projection: SparseTriplets,
    projection_triangle_visits: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AdaptiveMeshSensitivity {
    boundary_refinement_level: u32,
    boundary_spacing: f64,
    vertex_count: usize,
    triangle_count: usize,
    mesh_area: f64,
    relative_area_error: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AdaptiveSpdeLimits {
    maximum_vertices: usize,
    maximum_triangles: usize,
    maximum_boundary_segment_triangle_checks: u64,
    maximum_projection_triangle_visits: u64,
    memory_budget_mib: usize,
    maximum_result_bytes: usize,
    maximum_iterations: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AdaptiveSpdeBackend {
    name: String,
    scipy_version: String,
    numpy_version: String,
    python_version: String,
    license: String,
    environment_lock_sha256: String,
    worker_sha256: String,
}

pub(super) fn run(
    project_path: PathBuf,
    input_path: PathBuf,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let prepared = bayes_advanced::prepare("adaptive_window_spde", &input_path)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let artifacts = artifacts(&prepared)?;
    let node = AdaptiveWindowSpdeProjectNode::new(input_path, artifacts.clone(), prepared)?;
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
    for artifact in &artifacts {
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
        ArtifactSchema::new("marklab.adaptive_window_spde", 1)
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    bayes::publish_json(&output_path, &execution.output)?;
    let cache_status = match execution.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!("project adaptive-window-spde cache_status={cache_status}");
    Ok(())
}

fn artifacts(prepared: &PreparedAdvancedBayes) -> Result<Vec<ArtifactRef>, BayesCliError> {
    [
        (INPUT_KIND, prepared.input_bytes.as_slice()),
        (LOCK_KIND, prepared.lock_bytes.as_slice()),
        (WORKER_KIND, prepared.worker_bytes.as_slice()),
    ]
    .into_iter()
    .map(|(kind, bytes)| {
        ArtifactRef::from_bytes(kind, bytes)
            .map_err(|error| BayesCliError::Input(error.to_string()))
    })
    .collect()
}

struct AdaptiveWindowSpdeProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: Vec<ArtifactRef>,
    prepared: PreparedAdvancedBayes,
    configuration_digest: ContentDigest,
    execution_policy: Vec<u8>,
}

impl AdaptiveWindowSpdeProjectNode {
    fn new(
        input_path: PathBuf,
        input_artifacts: Vec<ArtifactRef>,
        prepared: PreparedAdvancedBayes,
    ) -> Result<Self, BayesCliError> {
        if input_artifacts.len() != 3 {
            return Err(BayesCliError::Input(
                "adaptive SPDE input/backend identities are incomplete".into(),
            ));
        }
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("adaptive-window-spde")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "adaptive_window_spde",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts,
            configuration_digest: ContentDigest::from_bytes(&prepared.request_bytes),
            execution_policy: b"bounded-single-process-pinned-scipy;deterministic-delaunay;boundary-refined-exact-window-membership;piecewise-linear-alpha-two-spde;fixed-hyperparameter-one-factor-map"
                .to_vec(),
            prepared,
        })
    }

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }
}

impl WorkflowNode for AdaptiveWindowSpdeProjectNode {
    type Output = AdaptiveWindowSpdeResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let current = bayes_advanced::prepare("adaptive_window_spde", &self.input_path)
            .map_err(NodeError::input)?;
        let current_artifacts = artifacts(&current).map_err(NodeError::input)?;
        if current_artifacts != self.input_artifacts
            || current.request_bytes != self.prepared.request_bytes
        {
            return Err(NodeError::input(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "adaptive SPDE source, environment, or worker changed after preparation",
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
        let value = bayes_advanced::execute(&self.prepared).map_err(NodeError::execution)?;
        let result: AdaptiveWindowSpdeResult =
            serde_json::from_value(value).map_err(NodeError::decode)?;
        validate_scientific_result(&result).map_err(NodeError::execution)?;
        Ok(result)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        serde_json::to_vec(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let result: AdaptiveWindowSpdeResult =
            serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        let value = serde_json::to_value(&result).map_err(NodeError::decode)?;
        bayes_advanced::validate_result(&self.prepared, &value).map_err(NodeError::decode)?;
        validate_scientific_result(&result).map_err(NodeError::decode)?;
        Ok(result)
    }

    fn output_kind(&self) -> &'static str {
        OUTPUT_KIND
    }
}

fn validate_scientific_result(result: &AdaptiveWindowSpdeResult) -> Result<(), std::io::Error> {
    if result.format != "marklab.adaptive_window_spde"
        || result.version != 1
        || result.claim_status != "fitted_arbitrary_window_spde_diagnostic"
        || !result.window.exact_area.is_finite()
        || result.window.exact_area <= 0.0
        || !result.mesh.mesh_area.is_finite()
        || result.mesh.mesh_area <= 0.0
        || !result.mesh.minimum_precision_eigenvalue.is_finite()
        || result.mesh.minimum_precision_eigenvalue <= 0.0
        || !result.mesh.maximum_projection_row_sum_error.is_finite()
        || !(0.0..=1e-8).contains(&result.mesh.maximum_projection_row_sum_error)
        || result.mesh_sensitivity.is_empty()
        || result.mesh.vertex_count != result.mesh.vertices.len()
        || result.mesh.triangle_count != result.mesh.triangles.len()
        || result.spatial_factor.mesh_field_weights.len() != result.mesh.vertex_count
        || result.spatial_factor.feature_means.len() != result.spatial_factor.feature_count
        || result.spatial_factor.loadings.len() != result.spatial_factor.feature_count
        || result.spatial_factor.region_ids.len()
            != result.spatial_factor.projected_region_factor.len()
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "decoded adaptive SPDE result is scientifically incoherent",
        ));
    }
    Ok(())
}
