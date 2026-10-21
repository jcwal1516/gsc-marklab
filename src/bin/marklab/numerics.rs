use std::{
    fs,
    path::{Path, PathBuf},
};

use clap::Subcommand;
use marklab_numerics::{evaluate_stable_primitives, StablePrimitivesSpec};
use serde::Serialize;

use super::exclusive_json_output::{publish_pretty_json, ExclusiveJsonOutputError};
use crate::cli_error::{into_marklab_error, CliError};

const MAXIMUM_INPUT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Subcommand)]
pub(crate) enum NumericsCommand {
    /// Measure distance-dependent enrichment between ordered cell classes.
    CategoricalCoOccurrence(super::co_occurrence::Arguments),
    /// Compare categorical edge counts with a fixed-count label null.
    CategoricalEnrichment {
        /// Strict categorical enrichment JSON request.
        #[arg(long)]
        input: PathBuf,
        /// New JSON result file; it must not already exist.
        #[arg(long)]
        out: PathBuf,
    },
    /// Evaluate numerically stable probability and matrix primitives.
    StablePrimitives {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Measure local spatial association in multivariate observations.
    LocalMultivariateMoran(super::local_multivariate::Arguments),
    /// Measure bond-orientational (k-atic) order of cell positions.
    BondOrientationalOrder {
        /// Strict marklab.bond_orientational_order_request JSON.
        #[arg(long)]
        input: PathBuf,
        /// New JSON result file; it must not already exist.
        #[arg(long)]
        out: PathBuf,
    },
    /// Compute the structure factor and hyperuniformity indices of cells in a rectangle.
    Hyperuniformity {
        /// Strict marklab.hyperuniformity_request JSON.
        #[arg(long)]
        input: PathBuf,
        /// New JSON result file; it must not already exist.
        #[arg(long)]
        out: PathBuf,
    },
    /// Predict a cell-level value by ordinary kriging, with leave-one-block-out checks.
    OrdinaryKriging {
        /// Strict marklab.ordinary_kriging_request JSON.
        #[arg(long)]
        input: PathBuf,
        /// New JSON result file; it must not already exist.
        #[arg(long)]
        out: PathBuf,
    },
    /// Measure box-counting dimension and gliding-box lacunarity of a declared binary mask.
    LacunarityFractalDimension {
        /// Strict marklab.lacunarity_fractal_dimension_request JSON.
        #[arg(long)]
        input: PathBuf,
        /// New JSON result file; it must not already exist.
        #[arg(long)]
        out: PathBuf,
    },
    /// Estimate mutual information between two cell marks with a spatially blocked null.
    MutualInformation {
        /// Strict marklab.mutual_information_request JSON.
        #[arg(long)]
        input: PathBuf,
        /// New JSON result file; it must not already exist.
        #[arg(long)]
        out: PathBuf,
    },
    /// Estimate the Landy–Szalay two-point correlation against a declared random catalog.
    LandySzalay {
        /// Strict marklab.landy_szalay_request JSON.
        #[arg(long)]
        input: PathBuf,
        /// New JSON result file; it must not already exist.
        #[arg(long)]
        out: PathBuf,
    },
    /// Build the Mapper graph of cell features over a one-dimensional lens.
    MapperGraph {
        /// Strict marklab.mapper_graph_request JSON.
        #[arg(long)]
        input: PathBuf,
        /// New JSON result file; it must not already exist.
        #[arg(long)]
        out: PathBuf,
    },
    /// Measure Morisita quadrat dispersion of cells inside a tissue window.
    MorisitaDispersion {
        /// Strict marklab.morisita_dispersion_request JSON.
        #[arg(long)]
        input: PathBuf,
        /// New JSON result file; it must not already exist.
        #[arg(long)]
        out: PathBuf,
    },
}

pub(crate) fn run_cli(command: NumericsCommand) -> Result<(), CliError> {
    match command {
        NumericsCommand::CategoricalCoOccurrence(args) => super::co_occurrence::run(args),
        NumericsCommand::CategoricalEnrichment { input, out } => {
            let bytes = super::input_file::read_regular_file(
                &input,
                MAXIMUM_INPUT_BYTES,
                "categorical enrichment input must be a regular file within 16 MiB",
            )?;
            let result = marklab::marks::categorical_enrichment_from_json(&bytes)
                .map_err(|error| CliError::Input(error.to_string()))?;
            drop(bytes);
            publish_json(&out, &result)
        }
        NumericsCommand::StablePrimitives { input, out } => run(input, out),
        NumericsCommand::LocalMultivariateMoran(args) => {
            super::local_multivariate::run_direct(args)
        }
        NumericsCommand::BondOrientationalOrder { input, out } => {
            let bytes = super::input_file::read_regular_file(
                &input,
                MAXIMUM_INPUT_BYTES,
                "bond-orientational order input must be a regular file within 16 MiB",
            )?;
            let result = marklab::structure::bond_orientational_order_from_json(&bytes)
                .map_err(|error| CliError::Input(error.to_string()))?;
            publish_json(&out, &result)
        }
        NumericsCommand::Hyperuniformity { input, out } => {
            let bytes = super::input_file::read_regular_file(
                &input,
                MAXIMUM_INPUT_BYTES,
                "hyperuniformity input must be a regular file within 16 MiB",
            )?;
            let result = marklab::structure::hyperuniformity_from_json(&bytes)
                .map_err(|error| CliError::Input(error.to_string()))?;
            publish_json(&out, &result)
        }
        NumericsCommand::OrdinaryKriging { input, out } => {
            let bytes = super::input_file::read_regular_file(
                &input,
                MAXIMUM_INPUT_BYTES,
                "kriging input must be a regular file within 16 MiB",
            )?;
            let result = marklab::structure::ordinary_kriging_from_json(&bytes)
                .map_err(|error| CliError::Input(error.to_string()))?;
            publish_json(&out, &result)
        }
        NumericsCommand::LacunarityFractalDimension { input, out } => {
            let bytes = super::input_file::read_regular_file(
                &input,
                MAXIMUM_INPUT_BYTES,
                "lacunarity input must be a regular file within 16 MiB",
            )?;
            let result = marklab::structure::lacunarity_fractal_dimension_from_json(&bytes)
                .map_err(|error| CliError::Input(error.to_string()))?;
            publish_json(&out, &result)
        }
        NumericsCommand::MutualInformation { input, out } => {
            let bytes = super::input_file::read_regular_file(
                &input,
                MAXIMUM_INPUT_BYTES,
                "mutual-information input must be a regular file within 16 MiB",
            )?;
            let result = marklab::marks::mutual_information_from_json(&bytes)
                .map_err(|error| CliError::Input(error.to_string()))?;
            publish_json(&out, &result)
        }
        NumericsCommand::LandySzalay { input, out } => {
            let bytes = super::input_file::read_regular_file(
                &input,
                MAXIMUM_INPUT_BYTES,
                "Landy-Szalay input must be a regular file within 16 MiB",
            )?;
            let result = marklab::structure::landy_szalay_from_json(&bytes)
                .map_err(|error| CliError::Input(error.to_string()))?;
            publish_json(&out, &result)
        }
        NumericsCommand::MapperGraph { input, out } => {
            let bytes = super::input_file::read_regular_file(
                &input,
                MAXIMUM_INPUT_BYTES,
                "Mapper input must be a regular file within 16 MiB",
            )?;
            let result = marklab::structure::mapper_graph_from_json(&bytes)
                .map_err(|error| CliError::Input(error.to_string()))?;
            publish_json(&out, &result)
        }
        NumericsCommand::MorisitaDispersion { input, out } => {
            let bytes = super::input_file::read_regular_file(
                &input,
                MAXIMUM_INPUT_BYTES,
                "Morisita input must be a regular file within 16 MiB",
            )?;
            let result = marklab::structure::morisita_dispersion_from_json(&bytes)
                .map_err(|error| CliError::Input(error.to_string()))?;
            publish_json(&out, &result)
        }
    }
}

fn run(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let metadata = fs::metadata(&input).map_err(|source| CliError::Io {
        path: input.clone(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(CliError::Input(
            "input must be a regular file within 16 MiB".into(),
        ));
    }
    let bytes = fs::read(&input).map_err(|source| CliError::Io {
        path: input,
        source,
    })?;
    let spec: StablePrimitivesSpec = serde_json::from_slice(&bytes)?;
    let result =
        evaluate_stable_primitives(spec).map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn publish_json(path: &Path, result: &impl Serialize) -> Result<(), CliError> {
    publish_pretty_json(path, result, "numerics").map_err(|error| match error {
        ExclusiveJsonOutputError::OutputExists => {
            CliError::Input(format!("output already exists: {}", path.display()))
        }
        ExclusiveJsonOutputError::OutputMustNameFile => {
            CliError::Input("output must name a file".into())
        }
        ExclusiveJsonOutputError::Io { path, source } => CliError::Io { path, source },
        ExclusiveJsonOutputError::Json(error) => CliError::Json(error),
    })
}

pub(crate) fn execute(command: NumericsCommand) -> marklab::Result<()> {
    run_cli(command).map_err(into_marklab_error)
}
