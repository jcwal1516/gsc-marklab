use std::borrow::Cow;

use marklab_project::ContentDigest;
use marklab_sbi::{smc_abc_growth_front, SmcAbcGrowthFrontResult, SmcAbcGrowthFrontSpec};
use marklab_simulation::GrowthFrontInitialPoint;
use marklab_workflow::NodeError;
use serde::{Deserialize, Serialize};

use super::{
    durable::{self, Codec, NodeContract, Source, SourceNode},
    CliError, SmcAbcGrowthFrontProjectArgs, MAXIMUM_INPUT_BYTES, MAXIMUM_RESULT_BYTES,
};

const INPUT_KIND: &str = "application/vnd.marklab.source.smc-abc-growth-front+json;version=1";
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

pub(super) fn run(arguments: SmcAbcGrowthFrontProjectArgs) -> Result<(), CliError> {
    const LABEL: &str = "SMC-ABC input";
    let input_bytes = crate::input_file::read_regular_file(
        &arguments.input,
        MAXIMUM_INPUT_BYTES,
        "SMC-ABC input must be a regular file within 16 MiB",
    )?;
    let source = Source::from_bytes(arguments.input.clone(), INPUT_KIND, &input_bytes)?;
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
    source.ensure_unchanged(LABEL)?;
    let contract = NodeContract {
        id: "smc-abc-growth-front",
        kind: "simulation_based_inference_smc_abc_growth_front",
        version: 1,
        configuration_digest: ContentDigest::from_framed([
            IMPLEMENTATION_IDENTITY.as_bytes(),
            configuration_bytes.as_slice(),
        ]),
        execution_policy: Cow::Borrowed(b"native-safe-rust-bounded-smc-abc-growth-front-v1"),
        implementation_identity: IMPLEMENTATION_IDENTITY,
        output_kind: "application/vnd.marklab.smc-abc-growth-front+json;version=1",
        codec: Codec::Json,
    };
    let node = SourceNode::new(
        contract,
        LABEL,
        vec![source],
        spec,
        |spec| smc_abc_growth_front(spec.clone()).map_err(NodeError::execution),
        |spec, output| {
            validate_output(spec, output)
                .map_err(|message| NodeError::decode(CliError::Backend(message)))
        },
    )?;
    let run = durable::execute_node(
        &arguments.project,
        &node,
        ("marklab.smc_abc_growth_front", 1),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "smc-abc-growth-front",
        &arguments.out,
        &run.output,
        run.cache_status,
    )
}

fn validate_output(
    analysis: &SmcAbcGrowthFrontSpec,
    output: &SmcAbcGrowthFrontResult,
) -> Result<(), String> {
    let expected_work = (analysis.epsilon_schedule.len() as u64)
        .checked_mul(u64::from(analysis.maximum_proposals_per_stage))
        .and_then(|work| work.checked_mul(analysis.maximum_cell_steps_per_proposal))
        .ok_or_else(|| "SMC-ABC cached work identity overflows".to_owned())?;
    let weights = output
        .particles
        .iter()
        .map(|particle| particle.weight)
        .sum::<f64>();
    let stages_match = output.stages.len() == analysis.epsilon_schedule.len()
        && output
            .stages
            .iter()
            .zip(&analysis.epsilon_schedule)
            .enumerate()
            .all(|(index, (stage, epsilon))| {
                stage.stage == index as u32 + 1
                    && stage.epsilon.to_bits() == epsilon.to_bits()
                    && stage.proposals_attempted <= analysis.maximum_proposals_per_stage
            });
    let finite = output.particles.iter().all(|row| {
        row.growth_rate_per_time.is_finite() && row.distance.is_finite() && row.weight.is_finite()
    }) && output.posterior.growth_rate_mean.is_finite()
        && output.posterior.growth_rate_sd.is_finite();
    if output.version != 1
        || !stages_match
        || output.particles.len() != analysis.particles as usize
        || output.maximum_total_declared_cell_steps != expected_work
        || output.total_simulations
            > u64::from(analysis.maximum_proposals_per_stage)
                * analysis.epsilon_schedule.len() as u64
        || !finite
        || (weights - 1.0).abs() > 1e-12
    {
        return Err("SMC-ABC cached result identity, bounds, or finite policy differs".into());
    }
    Ok(())
}
