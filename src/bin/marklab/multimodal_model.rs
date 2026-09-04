use std::path::PathBuf;

use clap::{Parser, Subcommand};

use super::topology::TopologyCliError;

#[path = "multimodal_model/matrix.rs"]
mod matrix;
#[path = "multimodal_model/multiresolution.rs"]
mod multiresolution;
#[path = "multimodal_model/paired.rs"]
mod paired;
#[path = "multimodal_model/robust.rs"]
mod robust;
#[path = "multimodal_model/schema.rs"]
mod schema;
#[path = "multimodal_model/spatial_latent.rs"]
mod spatial_latent;
#[path = "multimodal_model/spatial_matrix.rs"]
mod spatial_matrix;
#[path = "multimodal_model/tensor.rs"]
mod tensor;
#[path = "multimodal_model/validation.rs"]
mod validation;
#[path = "multimodal_model/worker_assets.rs"]
mod worker_assets;

use matrix::{run_hierarchical_factor, run_matrix_factor, run_mofa};
use multiresolution::run_multiresolution_factor;
use paired::{run_bayesian_pcca, run_pcca};
use robust::{run_dropout_robust, run_joint_pathology};
use spatial_latent::run_spatial_latent_factor;
use spatial_matrix::run_spatial_matrix_factor;
use tensor::run_tensor_factor;
use validation::run_validation_suite;

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct MultimodalModelCli {
    #[command(subcommand)]
    command: MultimodalTopLevel,
}

#[derive(Debug, Subcommand)]
enum MultimodalTopLevel {
    Multimodal {
        #[command(subcommand)]
        command: MultimodalModelCommand,
    },
}

#[derive(Debug, Subcommand)]
enum MultimodalModelCommand {
    Pcca {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    BayesianPcca {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Mofa {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    MatrixFactor {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    HierarchicalFactor {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    SpatialMatrixFactor {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    TensorFactor {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    SpatialLatentFactor {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    MultiresolutionFactor {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    DropoutRobust {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    JointPathology {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    CompareModels {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        outer_folds: u32,
        #[arg(long)]
        inner_folds: u32,
        #[arg(long)]
        ridge_alphas: String,
        #[arg(long)]
        permutations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    Validate {
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
}

pub(crate) fn run_cli() -> Result<(), TopologyCliError> {
    match MultimodalModelCli::parse().command {
        MultimodalTopLevel::Multimodal {
            command: MultimodalModelCommand::Pcca { input, out },
        } => run_pcca(input, out),
        MultimodalTopLevel::Multimodal {
            command: MultimodalModelCommand::BayesianPcca { input, out },
        } => run_bayesian_pcca(input, out),
        MultimodalTopLevel::Multimodal {
            command: MultimodalModelCommand::Mofa { input, out },
        } => run_mofa(input, out),
        MultimodalTopLevel::Multimodal {
            command: MultimodalModelCommand::MatrixFactor { input, out },
        } => run_matrix_factor(input, out),
        MultimodalTopLevel::Multimodal {
            command: MultimodalModelCommand::HierarchicalFactor { input, out },
        } => run_hierarchical_factor(input, out),
        MultimodalTopLevel::Multimodal {
            command: MultimodalModelCommand::SpatialMatrixFactor { input, out },
        } => run_spatial_matrix_factor(input, out),
        MultimodalTopLevel::Multimodal {
            command: MultimodalModelCommand::TensorFactor { input, out },
        } => run_tensor_factor(input, out),
        MultimodalTopLevel::Multimodal {
            command: MultimodalModelCommand::SpatialLatentFactor { input, out },
        } => run_spatial_latent_factor(input, out),
        MultimodalTopLevel::Multimodal {
            command: MultimodalModelCommand::MultiresolutionFactor { input, out },
        } => run_multiresolution_factor(input, out),
        MultimodalTopLevel::Multimodal {
            command: MultimodalModelCommand::DropoutRobust { input, out },
        } => run_dropout_robust(input, out),
        MultimodalTopLevel::Multimodal {
            command: MultimodalModelCommand::JointPathology { input, out },
        } => run_joint_pathology(input, out),
        MultimodalTopLevel::Multimodal {
            command:
                MultimodalModelCommand::CompareModels {
                    input,
                    outer_folds,
                    inner_folds,
                    ridge_alphas,
                    permutations,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => super::bayes::complementarity::run_multimodal_comparison(
            input,
            outer_folds,
            inner_folds,
            ridge_alphas,
            permutations,
            seed,
            timeout_seconds,
            out,
        )
        .map_err(|error| TopologyCliError::Backend(error.to_string())),
        MultimodalTopLevel::Multimodal {
            command:
                MultimodalModelCommand::Validate {
                    seed,
                    timeout_seconds,
                    out,
                },
        } => run_validation_suite(seed, timeout_seconds, out),
    }
}

pub(crate) fn into_marklab_error(error: TopologyCliError) -> marklab::MarklabError {
    marklab::MarklabError::Validation(error.to_string())
}

pub(crate) fn cli_route() -> crate::command_tree::Route {
    crate::command_tree::Route::new::<MultimodalModelCli>(|| run_cli().map_err(into_marklab_error))
}
