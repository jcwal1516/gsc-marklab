use std::path::PathBuf;

use clap::{Parser, Subcommand};
use marklab::{
    analyze_pathology_composition, analyze_pathology_maps, publish_pathology_composition,
    publish_pathology_maps,
};
use marklab::{analyze_pathology_scan, publish_pathology_scan};
use marklab::{analyze_pathology_spatial_study, publish_pathology_spatial_study};

#[derive(Parser)]
#[command(name = "marklab")]
struct PathologyCli {
    #[command(subcommand)]
    command: TopLevel,
}

#[derive(Subcommand)]
enum TopLevel {
    /// H&E/IHC annotation profiles and within-slide spatial maps.
    Pathology {
        #[command(subcommand)]
        command: PathologyCommand,
    },
}

#[derive(Subcommand)]
enum PathologyCommand {
    /// Search for binary phenotype enrichment across declared centers and radii.
    Scan {
        #[arg(long)]
        recipe: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// Context-conditioned spatial curves and area-weighted patient comparison.
    SpatialStudy {
        #[arg(long)]
        recipe: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// Compare patient composition and density, with tile-size/offset sensitivity.
    Composition {
        #[arg(long)]
        recipe: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// Compute structure-relative densities, phenotype neighborhoods and local Moran maps.
    Maps {
        /// Strict version-one JSON recipe; see docs/pathology-maps.md.
        #[arg(long)]
        recipe: PathBuf,
        /// New or empty output directory for results, overlays and report.
        #[arg(long)]
        out: PathBuf,
    },
}

pub(crate) fn cli_route() -> super::command_tree::Route {
    super::command_tree::Route::new::<PathologyCli>(run)
}

fn run() -> marklab::Result<()> {
    let TopLevel::Pathology { command } = PathologyCli::parse().command;
    let (recipe, out) = match &command {
        PathologyCommand::Maps { recipe, out }
        | PathologyCommand::Composition { recipe, out }
        | PathologyCommand::SpatialStudy { recipe, out } => (recipe, out),
        PathologyCommand::Scan { recipe, out } => (recipe, out),
    };
    let bytes = super::bayes::read_regular_file(
        recipe,
        64 * 1024 * 1024,
        "pathology recipe must be a regular file of at most 64 MiB",
    )
    .map_err(super::bayes::into_marklab_error)?;
    match command {
        PathologyCommand::Scan { .. } => {
            publish_pathology_scan(&analyze_pathology_scan(&bytes)?, out)
        }
        PathologyCommand::SpatialStudy { .. } => {
            publish_pathology_spatial_study(&analyze_pathology_spatial_study(&bytes)?, out)
        }
        PathologyCommand::Maps { .. } => {
            publish_pathology_maps(&analyze_pathology_maps(&bytes)?, out)
        }
        PathologyCommand::Composition { .. } => {
            publish_pathology_composition(&analyze_pathology_composition(&bytes)?, out)
        }
    }
}
