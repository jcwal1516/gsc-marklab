use std::path::PathBuf;

use marklab_cohort::{hierarchical_gatekeeping_max_t, MaxTCorrection, MaxTPermutationSpec};

use super::{input::validate_input_file_with_message, publication::publish_json, CohortError};
use crate::{
    cli_error::CliError,
    codecs::hierarchical_max_t::{read_input, Output},
};

#[derive(Debug, clap::Args)]
pub(crate) struct RunArgs {
    /// CSV with patient_id, group, family_order, family, endpoint and value columns.
    #[arg(long)]
    pub(super) input: PathBuf,
    /// Label of the first group in the contrast.
    #[arg(long)]
    pub(super) group_a: String,
    /// Label of the second group in the contrast.
    #[arg(long)]
    pub(super) group_b: String,
    /// Number of randomized label or residual assignments.
    #[arg(long)]
    pub(super) permutations: usize,
    /// Random seed for reproducible simulation or inference.
    #[arg(long)]
    pub(super) seed: u64,
    /// Significance level or excluded tail probability for the reported inference.
    #[arg(long)]
    pub(super) alpha: f64,
    /// Apply step-down rather than single-step Max-T within each opened family.
    #[arg(long)]
    pub(super) step_down: bool,
    /// Destination for the analysis output.
    #[arg(long)]
    pub(super) out: PathBuf,
}

pub(super) fn run(args: RunArgs) -> Result<(), CohortError> {
    validate_input_file_with_message(
        &args.input,
        "hierarchical Max-T input must be a regular file within 16 MiB",
    )?;
    let file =
        std::fs::File::open(&args.input).map_err(|error| CohortError::Input(error.to_string()))?;
    let input = read_input(file).map_err(|error| match error {
        CliError::Input(message) => CohortError::Input(message),
        CliError::Csv(error) => CohortError::Input(error.to_string()),
        error => CohortError::Input(error.to_string()),
    })?;
    let correction = if args.step_down {
        MaxTCorrection::StepDown
    } else {
        MaxTCorrection::SingleStep
    };
    let result = hierarchical_gatekeeping_max_t(
        &input.patients,
        &input.families,
        &MaxTPermutationSpec {
            group_a: args.group_a,
            group_b: args.group_b,
            permutations: args.permutations,
            seed: args.seed,
            alpha: args.alpha,
        },
        correction,
    )?;
    publish_json(&args.out, &Output::from_result(result))
}
