use std::path::PathBuf;

use clap::Subcommand;

mod matrix;
mod multiresolution;
mod paired;
mod robust;
mod schema;
mod spatial_latent;
mod spatial_matrix;
mod tensor;
mod validation;
mod worker_assets;

use crate::cli_error::{into_marklab_error, CliError};
use matrix::{run_hierarchical_factor, run_matrix_factor, run_mofa};
use multiresolution::run_multiresolution_factor;
use paired::{run_bayesian_pcca, run_pcca};
use robust::{run_dropout_robust, run_joint_pathology};
use spatial_latent::run_spatial_latent_factor;
use spatial_matrix::run_spatial_matrix_factor;
use tensor::run_tensor_factor;
use validation::run_validation_suite;

#[derive(Debug, Subcommand)]
pub(crate) enum MultimodalModelCommand {
    /// Learn nonnegative modality kernel weights with nested patient cross-validation.
    KernelFusion {
        /// Strict patient features, target, fold assignments and resource limits.
        #[arg(long)]
        input: PathBuf,
        /// New JSON result file; it must not already exist.
        #[arg(long)]
        out: PathBuf,
    },
    /// Estimate shared structure with probabilistic canonical correlation analysis.
    Pcca {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Fit a Bayesian model of shared variation between modalities.
    BayesianPcca {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Fit shared latent factors across multiple data modalities.
    Mofa {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Fit a low-rank factor model to a measurement matrix.
    MatrixFactor {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Fit latent factors while accounting for hierarchical sampling.
    HierarchicalFactor {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Fit matrix factors with spatial structure.
    SpatialMatrixFactor {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Factor a tensor of multiaxis measurements.
    TensorFactor {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Fit shared latent factors with spatial dependence.
    SpatialLatentFactor {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Fit factors shared across declared measurement resolutions.
    MultiresolutionFactor {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Fit multimodal factors while accounting for missing measurements.
    DropoutRobust {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Fit a joint model of declared pathology measurements.
    JointPathology {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Compare nested ridge models that add cell, patch, neighbor and measured features to
    /// a baseline, with patient-level nested cross-validation.
    CompareModels(crate::bayes::TestCellPatchComplementarityArgs),
    /// Run multimodal model checks and report the available validation evidence.
    Validate {
        /// Random seed for reproducible simulation or inference.
        #[arg(long)]
        seed: u64,
        /// Maximum worker execution time in seconds.
        #[arg(long)]
        timeout_seconds: u64,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
}

pub(crate) fn run_cli(command: MultimodalModelCommand) -> Result<(), CliError> {
    match command {
        MultimodalModelCommand::KernelFusion { input, out } => run_kernel_fusion(input, out),
        MultimodalModelCommand::Pcca { input, out } => run_pcca(input, out),
        MultimodalModelCommand::BayesianPcca { input, out } => run_bayesian_pcca(input, out),
        MultimodalModelCommand::Mofa { input, out } => run_mofa(input, out),
        MultimodalModelCommand::MatrixFactor { input, out } => run_matrix_factor(input, out),
        MultimodalModelCommand::HierarchicalFactor { input, out } => {
            run_hierarchical_factor(input, out)
        }
        MultimodalModelCommand::SpatialMatrixFactor { input, out } => {
            run_spatial_matrix_factor(input, out)
        }
        MultimodalModelCommand::TensorFactor { input, out } => run_tensor_factor(input, out),
        MultimodalModelCommand::SpatialLatentFactor { input, out } => {
            run_spatial_latent_factor(input, out)
        }
        MultimodalModelCommand::MultiresolutionFactor { input, out } => {
            run_multiresolution_factor(input, out)
        }
        MultimodalModelCommand::DropoutRobust { input, out } => run_dropout_robust(input, out),
        MultimodalModelCommand::JointPathology { input, out } => run_joint_pathology(input, out),
        MultimodalModelCommand::CompareModels(args) => {
            super::bayes::complementarity::run_multimodal_comparison(args)
                .map_err(|error| CliError::Backend(error.to_string()))
        }
        MultimodalModelCommand::Validate {
            seed,
            timeout_seconds,
            out,
        } => run_validation_suite(seed, timeout_seconds, out),
    }
}

fn run_kernel_fusion(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = super::input_file::read_regular_file(
        &input,
        16 * 1024 * 1024,
        "kernel-fusion input must be a regular file within 16 MiB",
    )?;
    let spec: marklab_prediction::KernelFusionSpec = serde_json::from_slice(&bytes)?;
    let result = marklab_prediction::learn_multimodal_kernels(spec)
        .map_err(|error| CliError::Input(error.to_string()))?;
    use super::exclusive_json_output::{publish_pretty_json, ExclusiveJsonOutputError};
    publish_pretty_json(&out, &result, "kernel-fusion").map_err(|error| match error {
        ExclusiveJsonOutputError::OutputExists => {
            CliError::Input(format!("output already exists: {}", out.display()))
        }
        ExclusiveJsonOutputError::OutputMustNameFile => {
            CliError::Input("output must name a file".into())
        }
        ExclusiveJsonOutputError::Io { path, source } => CliError::Io { path, source },
        ExclusiveJsonOutputError::Json(error) => CliError::Json(error),
    })
}

pub(crate) fn execute(command: MultimodalModelCommand) -> marklab::Result<()> {
    run_cli(command).map_err(into_marklab_error)
}
