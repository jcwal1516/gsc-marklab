use std::{fs, path::PathBuf};

use marklab_sbi::{smc_abc_growth_front, SmcAbcGrowthFrontResult, SmcAbcGrowthFrontSpec};
use marklab_simulation::GrowthFrontInitialPoint;
use marklab_workflow::{
    execute_algorithm, ArtifactRef, ArtifactSchema, CacheKeyMaterial, CacheStatus, ContentDigest,
    DurableProject, DurableProjectLimits, LocalScheduler, MarklabProject, NodeError, NodeId,
    NodeSpec, SchedulerLimits, WorkflowGraph, WorkflowNode,
};
use serde::{Deserialize, Serialize};

use super::{
    bayes, native_runtime_provenance, report_recovery, source_artifact, BayesCliError,
    SmcAbcGrowthFrontProjectArgs, MAXIMUM_INPUT_BYTES, MAXIMUM_RESULT_BYTES, PROJECT_CONTROL_BYTES,
    PROJECT_LEDGER_BYTES, PROJECT_LEDGER_RECORDS, PROJECT_RECORD_BYTES,
};

const INPUT_KIND: &str = "application/vnd.marklab.source.smc-abc-growth-front+json;version=1";
const OUTPUT_KIND: &str = "application/vnd.marklab.smc-abc-growth-front+json;version=1";
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-smc-abc-growth-front-node-v1";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    initial: Vec<InitialPoint>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InitialPoint {
    position_um: f64,
    density: f64,
}

#[derive(Serialize)]
struct Configuration<'a> {
    diffusion_um2_per_time: f64,
    carrying_capacity: f64,
    final_time: f64,
    time_step: f64,
    front_threshold_fraction: f64,
    observed_final_mass: f64,
    mass_scale: f64,
    growth_rate_prior_min: f64,
    growth_rate_prior_max: f64,
    epsilon_schedule: &'a [f64],
    particles: u32,
    maximum_proposals_per_stage: u32,
    maximum_cell_steps_per_proposal: u64,
    seed: u64,
}

pub(super) fn run(arguments: SmcAbcGrowthFrontProjectArgs) -> Result<(), BayesCliError> {
    let before = source_artifact(&arguments.input, INPUT_KIND)?;
    let metadata = fs::metadata(&arguments.input).map_err(|source| BayesCliError::Io {
        path: arguments.input.clone(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(BayesCliError::Input(
            "SMC-ABC input must be a regular file within 16 MiB".into(),
        ));
    }
    let input_bytes = fs::read(&arguments.input).map_err(|source| BayesCliError::Io {
        path: arguments.input.clone(),
        source,
    })?;
    let input: Input = serde_json::from_slice(&input_bytes)?;
    let configuration_bytes = serde_json::to_vec(&Configuration {
        diffusion_um2_per_time: arguments.diffusion_um2_per_time,
        carrying_capacity: arguments.carrying_capacity,
        final_time: arguments.final_time,
        time_step: arguments.time_step,
        front_threshold_fraction: arguments.front_threshold_fraction,
        observed_final_mass: arguments.observed_final_mass,
        mass_scale: arguments.mass_scale,
        growth_rate_prior_min: arguments.growth_rate_prior_min,
        growth_rate_prior_max: arguments.growth_rate_prior_max,
        epsilon_schedule: &arguments.epsilon_schedule,
        particles: arguments.particles,
        maximum_proposals_per_stage: arguments.maximum_proposals_per_stage,
        maximum_cell_steps_per_proposal: arguments.maximum_cell_steps_per_proposal,
        seed: arguments.seed,
    })?;
    let spec = SmcAbcGrowthFrontSpec {
        initial: input
            .initial
            .into_iter()
            .map(|row| GrowthFrontInitialPoint {
                position_um: row.position_um,
                density: row.density,
            })
            .collect(),
        diffusion_um2_per_time: arguments.diffusion_um2_per_time,
        carrying_capacity: arguments.carrying_capacity,
        final_time: arguments.final_time,
        time_step: arguments.time_step,
        front_threshold_fraction: arguments.front_threshold_fraction,
        observed_final_mass: arguments.observed_final_mass,
        mass_scale: arguments.mass_scale,
        growth_rate_prior_min: arguments.growth_rate_prior_min,
        growth_rate_prior_max: arguments.growth_rate_prior_max,
        epsilon_schedule: arguments.epsilon_schedule,
        particles: arguments.particles,
        maximum_proposals_per_stage: arguments.maximum_proposals_per_stage,
        maximum_cell_steps_per_proposal: arguments.maximum_cell_steps_per_proposal,
        seed: arguments.seed,
    };
    let after = source_artifact(&arguments.input, INPUT_KIND)?;
    if before != after {
        return Err(BayesCliError::Input(
            "SMC-ABC input changed while the durable request was prepared".into(),
        ));
    }

    let node = SmcAbcProjectNode::new(arguments.input, before.clone(), spec, configuration_bytes)?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&arguments.project, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
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
        ArtifactSchema::new("marklab.smc_abc_growth_front", 1)
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
        native_runtime_provenance()?,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    bayes::publish_json(&arguments.out, &execution.output)?;
    let cache_status = match execution.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    eprintln!("project smc-abc-growth-front cache_status={cache_status}");
    Ok(())
}

struct SmcAbcProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    analysis: SmcAbcGrowthFrontSpec,
    configuration_bytes: Vec<u8>,
}

impl SmcAbcProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        analysis: SmcAbcGrowthFrontSpec,
        configuration_bytes: Vec<u8>,
    ) -> Result<Self, BayesCliError> {
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new("smc-abc-growth-front")
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                "simulation_based_inference_smc_abc_growth_front",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            analysis,
            configuration_bytes,
        })
    }

    fn validate_output(&self, output: &SmcAbcGrowthFrontResult) -> Result<(), String> {
        let expected_work = (self.analysis.epsilon_schedule.len() as u64)
            .checked_mul(u64::from(self.analysis.maximum_proposals_per_stage))
            .and_then(|work| work.checked_mul(self.analysis.maximum_cell_steps_per_proposal))
            .ok_or_else(|| "SMC-ABC cached work identity overflows".to_owned())?;
        let weights = output
            .particles
            .iter()
            .map(|particle| particle.weight)
            .sum::<f64>();
        let stages_match = output.stages.len() == self.analysis.epsilon_schedule.len()
            && output
                .stages
                .iter()
                .zip(&self.analysis.epsilon_schedule)
                .enumerate()
                .all(|(index, (stage, epsilon))| {
                    stage.stage == index as u32 + 1
                        && stage.epsilon.to_bits() == epsilon.to_bits()
                        && stage.proposals_attempted <= self.analysis.maximum_proposals_per_stage
                });
        let finite = output.particles.iter().all(|row| {
            row.growth_rate_per_time.is_finite()
                && row.distance.is_finite()
                && row.weight.is_finite()
        }) && output.posterior.growth_rate_mean.is_finite()
            && output.posterior.growth_rate_sd.is_finite();
        if output.version != 1
            || !stages_match
            || output.particles.len() != self.analysis.particles as usize
            || output.maximum_total_declared_cell_steps != expected_work
            || output.total_simulations
                > u64::from(self.analysis.maximum_proposals_per_stage)
                    * self.analysis.epsilon_schedule.len() as u64
            || !finite
            || (weights - 1.0).abs() > 1e-12
        {
            return Err("SMC-ABC cached result identity, bounds, or finite policy differs".into());
        }
        Ok(())
    }
}

impl WorkflowNode for SmcAbcProjectNode {
    type Output = SmcAbcGrowthFrontResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(&self.input_path, INPUT_KIND).map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "SMC-ABC input no longer matches its durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: ContentDigest::from_framed([
                IMPLEMENTATION_IDENTITY.as_bytes(),
                self.configuration_bytes.as_slice(),
            ]),
            execution_policy: b"native-safe-rust-bounded-smc-abc-growth-front-v1",
            implementation_identity: IMPLEMENTATION_IDENTITY,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        smc_abc_growth_front(self.analysis.clone()).map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        serde_json::to_vec_pretty(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: SmcAbcGrowthFrontResult =
            serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        self.validate_output(&output)
            .map_err(|message| NodeError::decode(BayesCliError::Backend(message)))?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        OUTPUT_KIND
    }
}
