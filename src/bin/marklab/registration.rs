use std::path::PathBuf;

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

use self::{
    atlas::run_atlas, landmark_uncertainty::run_landmark_uncertainty,
    lddmm_landmarks::run_lddmm_landmarks, nonrigid::run_nonrigid,
    probabilistic_svf::run_probabilistic_svf, svf::run_svf,
};
use super::topology::TopologyCliError;

mod atlas;
mod landmark_uncertainty;
mod lddmm_landmarks;
mod nonrigid;
mod probabilistic_svf;
mod svf;

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct RegistrationCli {
    #[command(subcommand)]
    command: RegistrationTopLevel,
}

#[derive(Debug, Subcommand)]
enum RegistrationTopLevel {
    Registration {
        #[command(subcommand)]
        command: RegistrationCommand,
    },
}

#[derive(Debug, Subcommand)]
enum RegistrationCommand {
    Nonrigid {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Svf {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    LddmmLandmarks {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    ProbabilisticSvf {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    LandmarkUncertainty {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Atlas {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RegistrationImage {
    frame: String,
    spacing_um: [f64; 2],
    pixels: Vec<Vec<f64>>,
}

pub(crate) fn run_cli() -> Result<(), TopologyCliError> {
    match RegistrationCli::parse().command {
        RegistrationTopLevel::Registration {
            command: RegistrationCommand::Nonrigid { input, out },
        } => run_nonrigid(input, out),
        RegistrationTopLevel::Registration {
            command: RegistrationCommand::Svf { input, out },
        } => run_svf(input, out),
        RegistrationTopLevel::Registration {
            command: RegistrationCommand::LddmmLandmarks { input, out },
        } => run_lddmm_landmarks(input, out),
        RegistrationTopLevel::Registration {
            command: RegistrationCommand::ProbabilisticSvf { input, out },
        } => run_probabilistic_svf(input, out),
        RegistrationTopLevel::Registration {
            command: RegistrationCommand::LandmarkUncertainty { input, out },
        } => run_landmark_uncertainty(input, out),
        RegistrationTopLevel::Registration {
            command: RegistrationCommand::Atlas { input, out },
        } => run_atlas(input, out),
    }
}

pub(crate) fn into_marklab_error(error: TopologyCliError) -> marklab::MarklabError {
    marklab::MarklabError::Validation(error.to_string())
}
