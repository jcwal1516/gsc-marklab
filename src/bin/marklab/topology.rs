use std::path::PathBuf;

use clap::Subcommand;

mod direct_commands;
mod io;
mod witness_bottleneck;
mod witness_persistence;
mod witness_stability;
mod worker_process;

use direct_commands::{
    run_alpha_persistence, run_compare_persistence, run_connectivity, run_raster_morphology,
    run_stability, run_validation,
};
pub(crate) use io::{publish_json, read_input, read_required};
pub(crate) use witness_bottleneck::{
    execute_witness_persistence_bottleneck_stability,
    prepare_witness_persistence_bottleneck_stability,
    validate_witness_persistence_bottleneck_stability_result,
};
use witness_persistence::run_witness_persistence;
pub(crate) use witness_persistence::{execute_witness_persistence, prepare_witness_persistence};
use witness_stability::run_witness_persistence_stability;
pub(crate) use witness_stability::{
    execute_witness_persistence_stability, prepare_witness_persistence_stability,
    validate_witness_persistence_stability_result,
};
pub(crate) use worker_process::run_worker;

use crate::cli_error::{into_marklab_error, CliError};
use witness_bottleneck::run_witness_persistence_bottleneck_stability;

#[derive(Debug, Subcommand)]
pub(crate) enum TopologyCommand {
    /// Compute alpha-complex persistence for spatial points.
    AlphaPersistence {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Compute witness-complex persistence from selected landmarks.
    WitnessPersistence {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Assess witness persistence across landmark perturbations.
    WitnessPersistenceStability {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Measure bottleneck stability of witness persistence diagrams.
    WitnessPersistenceBottleneckStability {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Measure morphology and topology of a binary raster mask.
    RasterMorphology {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Measure connectivity across declared spatial thresholds.
    Connectivity {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Compare persistence diagrams between samples.
    ComparePersistence {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Assess persistence-diagram stability under declared perturbations.
    Stability {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Run topological oracles and report the available validation evidence.
    Validate {
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
}

pub(crate) fn run_cli(command: TopologyCommand) -> Result<(), CliError> {
    match command {
        TopologyCommand::AlphaPersistence { input, out } => run_alpha_persistence(input, out),
        TopologyCommand::WitnessPersistence { input, out } => run_witness_persistence(input, out),
        TopologyCommand::WitnessPersistenceStability { input, out } => {
            run_witness_persistence_stability(input, out)
        }
        TopologyCommand::WitnessPersistenceBottleneckStability { input, out } => {
            run_witness_persistence_bottleneck_stability(input, out)
        }
        TopologyCommand::RasterMorphology { input, out } => run_raster_morphology(input, out),
        TopologyCommand::Connectivity { input, out } => run_connectivity(input, out),
        TopologyCommand::ComparePersistence { input, out } => run_compare_persistence(input, out),
        TopologyCommand::Stability { input, out } => run_stability(input, out),
        TopologyCommand::Validate { out } => run_validation(out),
    }
}

pub(crate) fn execute(command: TopologyCommand) -> marklab::Result<()> {
    run_cli(command).map_err(into_marklab_error)
}
