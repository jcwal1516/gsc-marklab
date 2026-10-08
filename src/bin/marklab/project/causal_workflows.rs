use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use marklab_causal::{
    estimate_gaussian_expected_information_gain, randomized_binary_interference, GaussianEigResult,
    GaussianEigSpec, RandomizedInterferenceResult, RandomizedInterferenceSpec,
};
use marklab_workflow::{
    execute_algorithm, ArtifactRef, ArtifactSchema, CacheKeyMaterial, CacheStatus, ContentDigest,
    DurableProject, DurableProjectLimits, LocalScheduler, MarklabProject, NodeError, NodeId,
    NodeSpec, SchedulerLimits, WorkflowGraph, WorkflowNode,
};

use super::{
    bayes, native_runtime_provenance, report_recovery, source_artifact, BayesCliError,
    MAXIMUM_INPUT_BYTES, MAXIMUM_RESULT_BYTES, PROJECT_CONTROL_BYTES, PROJECT_LEDGER_BYTES,
    PROJECT_LEDGER_RECORDS, PROJECT_RECORD_BYTES,
};

const INTERFERENCE_INPUT_KIND: &str =
    "application/vnd.marklab.source.causal-randomized-interference+json;version=1";
const INTERFERENCE_OUTPUT_KIND: &str =
    "application/vnd.marklab.causal-randomized-interference+json;version=1";
const INTERFERENCE_IMPLEMENTATION_IDENTITY: &str =
    "marklab-project-causal-randomized-interference-node-v1";
const EIG_INPUT_KIND: &str = "application/vnd.marklab.source.gaussian-eig+json;version=1";
const EIG_OUTPUT_KIND: &str = "application/vnd.marklab.gaussian-eig+json;version=1";
const EIG_IMPLEMENTATION_IDENTITY: &str = "marklab-project-gaussian-eig-node-v1";

pub(super) fn run_randomized_interference(
    project_path: PathBuf,
    input_path: PathBuf,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let (before, request_bytes) = read_stable_input(
        &input_path,
        INTERFERENCE_INPUT_KIND,
        "causal randomized-interference",
    )?;
    let analysis: RandomizedInterferenceSpec = serde_json::from_slice(&request_bytes)?;
    let node = RandomizedInterferenceProjectNode::new(
        input_path,
        before.clone(),
        analysis,
        request_bytes,
    )?;
    let execution = execute_interference(&project_path, before, &node)?;
    bayes::publish_json(&output_path, &execution.output)?;
    let status = match execution.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!("project causal-randomized-interference cache_status={status}");
    Ok(())
}

pub(super) fn run_gaussian_eig(
    project_path: PathBuf,
    input_path: PathBuf,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let (before, request_bytes) = read_stable_input(&input_path, EIG_INPUT_KIND, "Gaussian EIG")?;
    let analysis: GaussianEigSpec = serde_json::from_slice(&request_bytes)?;
    let node = GaussianEigProjectNode::new(input_path, before.clone(), analysis, request_bytes)?;
    let execution = execute_eig(&project_path, before, &node)?;
    bayes::publish_json(&output_path, &execution.output)?;
    let status = match execution.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!("project gaussian-eig cache_status={status}");
    Ok(())
}

fn read_stable_input(
    input_path: &Path,
    kind: &str,
    label: &str,
) -> Result<(ArtifactRef, Vec<u8>), BayesCliError> {
    let before = source_artifact(input_path, kind)?;
    let metadata = fs::metadata(input_path).map_err(|source| BayesCliError::Io {
        path: input_path.to_path_buf(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(BayesCliError::Input(format!(
            "{label} input must be a regular file within 16 MiB"
        )));
    }
    let bytes = fs::read(input_path).map_err(|source| BayesCliError::Io {
        path: input_path.to_path_buf(),
        source,
    })?;
    let after = source_artifact(input_path, kind)?;
    if before != after {
        return Err(BayesCliError::Input(format!(
            "{label} input changed while the durable request was prepared"
        )));
    }
    Ok((before, bytes))
}

fn open_project(
    project_path: &Path,
    input: ArtifactRef,
) -> Result<(DurableProject, MarklabProject, LocalScheduler), BayesCliError> {
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let durable = DurableProject::open_or_create(project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(input)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    Ok((durable, project, scheduler))
}

fn execute_interference(
    project_path: &Path,
    input: ArtifactRef,
    node: &RandomizedInterferenceProjectNode,
) -> Result<marklab_workflow::NodeRun<RandomizedInterferenceResult>, BayesCliError> {
    let (mut durable, mut project, scheduler) = open_project(project_path, input)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        node,
        &scheduler,
        ArtifactSchema::new("marklab.causal_randomized_interference", 1)
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
        native_runtime_provenance()?,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))
}

fn execute_eig(
    project_path: &Path,
    input: ArtifactRef,
    node: &GaussianEigProjectNode,
) -> Result<marklab_workflow::NodeRun<GaussianEigResult>, BayesCliError> {
    let (mut durable, mut project, scheduler) = open_project(project_path, input)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        node,
        &scheduler,
        ArtifactSchema::new("marklab.gaussian_expected_information_gain", 1)
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
        native_runtime_provenance()?,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))
}

struct RandomizedInterferenceProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    analysis: RandomizedInterferenceSpec,
    request_bytes: Vec<u8>,
}

impl RandomizedInterferenceProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        analysis: RandomizedInterferenceSpec,
        request_bytes: Vec<u8>,
    ) -> Result<Self, BayesCliError> {
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("causal-randomized-interference")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "causal_randomized_binary_interference",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            analysis,
            request_bytes,
        })
    }

    fn validate_output(&self, output: &RandomizedInterferenceResult) -> Result<(), String> {
        let clusters = self
            .analysis
            .units
            .iter()
            .map(|unit| unit.cluster_id.as_str())
            .collect::<HashSet<_>>();
        let probabilities_finite = output.exposure_probabilities.iter().all(|row| {
            let values = &row.probabilities;
            [
                values.untreated_neighbor_untreated,
                values.untreated_neighbor_treated,
                values.treated_neighbor_untreated,
                values.treated_neighbor_treated,
            ]
            .into_iter()
            .all(|value| value.is_finite() && (0.0..=1.0).contains(&value))
        });
        if output.version != 1
            || output.unit_count != self.analysis.units.len()
            || output.cluster_count != clusters.len()
            || output.design_provenance != self.analysis.design_provenance
            || output.graph_provenance != self.analysis.graph_provenance
            || output.assignment_states > self.analysis.maximum_assignment_states
            || output.unit_assignment_evaluations
                > self.analysis.maximum_unit_assignment_evaluations
            || output.observed_exposures.len() != self.analysis.units.len()
            || output.exposure_probabilities.len() != self.analysis.units.len()
            || output.randomization_test.high != self.analysis.test_exposure_high
            || output.randomization_test.low != self.analysis.test_exposure_low
            || output.randomization_test.null_values.len() != self.analysis.permutations
            || !output.randomization_test.observed.is_finite()
            || !output.randomization_test.p_value.is_finite()
            || !(0.0..=1.0).contains(&output.randomization_test.p_value)
            || !probabilities_finite
        {
            return Err(
                "randomized-interference cached result identity, bounds, or finite policy differs"
                    .into(),
            );
        }
        Ok(())
    }
}

impl WorkflowNode for RandomizedInterferenceProjectNode {
    type Output = RandomizedInterferenceResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed =
            source_artifact(&self.input_path, INTERFERENCE_INPUT_KIND).map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "randomized-interference input no longer matches its durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: ContentDigest::from_framed([
                INTERFERENCE_IMPLEMENTATION_IDENTITY.as_bytes(),
                self.request_bytes.as_slice(),
            ]),
            execution_policy: b"native-safe-rust-bounded-randomized-interference-v1",
            implementation_identity: INTERFERENCE_IMPLEMENTATION_IDENTITY,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        randomized_binary_interference(self.analysis.clone()).map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        serde_json::to_vec_pretty(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: RandomizedInterferenceResult =
            serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        self.validate_output(&output)
            .map_err(|message| NodeError::decode(BayesCliError::Backend(message)))?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        INTERFERENCE_OUTPUT_KIND
    }
}

struct GaussianEigProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    analysis: GaussianEigSpec,
    request_bytes: Vec<u8>,
}

impl GaussianEigProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        analysis: GaussianEigSpec,
        request_bytes: Vec<u8>,
    ) -> Result<Self, BayesCliError> {
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("gaussian-eig")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "causal_gaussian_expected_information_gain",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            analysis,
            request_bytes,
        })
    }

    fn validate_output(&self, output: &GaussianEigResult) -> Result<(), String> {
        let expected_work = (self.analysis.outer_samples as u64)
            .checked_mul(self.analysis.inner_samples as u64 + 1)
            .ok_or_else(|| "Gaussian EIG work identity overflows".to_owned())?;
        let analytic = 0.5
            * (1.0
                + self.analysis.sensitivity.powi(2) * self.analysis.prior_sd.powi(2)
                    / self.analysis.noise_sd.powi(2))
            .ln();
        let finite = [
            output.estimate,
            output.monte_carlo_se,
            output.analytic_eig,
            output.signed_bias_against_analytic,
            output.absolute_bias_against_analytic,
        ]
        .into_iter()
        .all(f64::is_finite)
            && output.outer_values.iter().all(|row| {
                [
                    row.theta,
                    row.simulated_observation,
                    row.log_numerator,
                    row.log_denominator,
                    row.information_value,
                ]
                .into_iter()
                .all(f64::is_finite)
            });
        if output.version != 1
            || output.candidate_id != self.analysis.candidate_id
            || output.outer_samples != self.analysis.outer_samples
            || output.inner_samples != self.analysis.inner_samples
            || output.outer_values.len() != self.analysis.outer_samples
            || !output
                .outer_values
                .iter()
                .enumerate()
                .all(|(index, row)| row.outer_index == index)
            || output.likelihood_evaluations != expected_work
            || output.likelihood_evaluations > self.analysis.maximum_likelihood_evaluations
            || (output.analytic_eig - analytic).abs() > 1.0e-15
            || !finite
        {
            return Err(
                "Gaussian EIG cached result identity, bounds, or finite policy differs".into(),
            );
        }
        Ok(())
    }
}

impl WorkflowNode for GaussianEigProjectNode {
    type Output = GaussianEigResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed =
            source_artifact(&self.input_path, EIG_INPUT_KIND).map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "Gaussian EIG input no longer matches its durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: ContentDigest::from_framed([
                EIG_IMPLEMENTATION_IDENTITY.as_bytes(),
                self.request_bytes.as_slice(),
            ]),
            execution_policy: b"native-safe-rust-bounded-gaussian-eig-v1",
            implementation_identity: EIG_IMPLEMENTATION_IDENTITY,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        estimate_gaussian_expected_information_gain(self.analysis.clone())
            .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        serde_json::to_vec_pretty(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: GaussianEigResult = serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        self.validate_output(&output)
            .map_err(|message| NodeError::decode(BayesCliError::Backend(message)))?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        EIG_OUTPUT_KIND
    }
}
