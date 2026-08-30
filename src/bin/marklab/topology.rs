use std::path::PathBuf;

use clap::{Parser, Subcommand};
use thiserror::Error;

#[path = "topology/direct_commands.rs"]
mod direct_commands;
#[path = "topology/io.rs"]
mod io;
#[path = "topology/witness_bottleneck.rs"]
mod witness_bottleneck;
#[path = "topology/witness_persistence.rs"]
mod witness_persistence;
#[path = "topology/witness_stability.rs"]
mod witness_stability;
#[path = "topology/worker_process.rs"]
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
    PreparedWitnessPersistenceBottleneckStability,
};
use witness_persistence::run_witness_persistence;
pub(crate) use witness_persistence::{
    execute_witness_persistence, prepare_witness_persistence, PreparedWitnessPersistence,
};
use witness_stability::run_witness_persistence_stability;
pub(crate) use witness_stability::{
    execute_witness_persistence_stability, prepare_witness_persistence_stability,
    validate_witness_persistence_stability_result, PreparedWitnessPersistenceStability,
};
pub(crate) use worker_process::run_worker;

use witness_bottleneck::run_witness_persistence_bottleneck_stability;

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct TopologyCli {
    #[command(subcommand)]
    command: TopologyTopLevel,
}

#[derive(Debug, Subcommand)]
enum TopologyTopLevel {
    Topology {
        #[command(subcommand)]
        command: TopologyCommand,
    },
}

#[derive(Debug, Subcommand)]
enum TopologyCommand {
    AlphaPersistence {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    WitnessPersistence {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    WitnessPersistenceStability {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    WitnessPersistenceBottleneckStability {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    RasterMorphology {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Connectivity {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    ComparePersistence {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Stability {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Validate {
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Debug, Error)]
pub(crate) enum TopologyCliError {
    #[error("invalid topology input: {0}")]
    Input(String),
    #[error("topology backend failed: {0}")]
    Backend(String),
    #[error("failed to access {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid topology JSON: {0}")]
    Json(#[from] serde_json::Error),
}

pub(crate) fn run_cli() -> Result<(), TopologyCliError> {
    match TopologyCli::parse().command {
        TopologyTopLevel::Topology {
            command: TopologyCommand::AlphaPersistence { input, out },
        } => run_alpha_persistence(input, out),
        TopologyTopLevel::Topology {
            command: TopologyCommand::WitnessPersistence { input, out },
        } => run_witness_persistence(input, out),
        TopologyTopLevel::Topology {
            command: TopologyCommand::WitnessPersistenceStability { input, out },
        } => run_witness_persistence_stability(input, out),
        TopologyTopLevel::Topology {
            command: TopologyCommand::WitnessPersistenceBottleneckStability { input, out },
        } => run_witness_persistence_bottleneck_stability(input, out),
        TopologyTopLevel::Topology {
            command: TopologyCommand::RasterMorphology { input, out },
        } => run_raster_morphology(input, out),
        TopologyTopLevel::Topology {
            command: TopologyCommand::Connectivity { input, out },
        } => run_connectivity(input, out),
        TopologyTopLevel::Topology {
            command: TopologyCommand::ComparePersistence { input, out },
        } => run_compare_persistence(input, out),
        TopologyTopLevel::Topology {
            command: TopologyCommand::Stability { input, out },
        } => run_stability(input, out),
        TopologyTopLevel::Topology {
            command: TopologyCommand::Validate { out },
        } => run_validation(out),
    }
}

pub(crate) fn into_marklab_error(error: TopologyCliError) -> marklab::MarklabError {
    marklab::MarklabError::Validation(error.to_string())
}
