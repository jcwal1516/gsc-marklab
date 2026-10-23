use std::path::PathBuf;

use clap::Subcommand;
use marklab::pathology::{
    analyze_pathology_architecture, analyze_pathology_composition, analyze_pathology_maps,
    publish_pathology_architecture, publish_pathology_composition, publish_pathology_maps,
};
use marklab::pathology::{analyze_pathology_scan, publish_pathology_scan};
use marklab::pathology::{analyze_pathology_spatial_study, publish_pathology_spatial_study};

// The library's own architecture ceilings, compiled into the CLI so both enforce one rule.
#[allow(dead_code)]
#[path = "../../pathology_architecture/host_memory.rs"]
mod host_memory;

#[derive(Subcommand)]
pub(crate) enum PathologyCommand {
    /// Search for binary phenotype enrichment across declared centers and radii.
    Scan {
        /// Declared analysis recipe.
        #[arg(long)]
        recipe: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Context-conditioned spatial curves and area-weighted patient comparison.
    SpatialStudy {
        /// Declared analysis recipe.
        #[arg(long)]
        recipe: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Compare patient composition and density, with tile-size/offset sensitivity.
    Composition {
        /// Declared analysis recipe.
        #[arg(long)]
        recipe: PathBuf,
        /// Destination for the analysis output.
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
    /// Measure epithelial architecture in distance bands from a derived invasive front.
    Architecture {
        /// Strict version-one JSON recipe; see docs/pathology-architecture.md.
        #[arg(long)]
        recipe: PathBuf,
        /// New or empty output directory for results, tables, overlays and report.
        #[arg(long)]
        out: PathBuf,
    },
}

pub(crate) fn run(command: PathologyCommand) -> marklab::Result<()> {
    let (recipe, out) = match &command {
        PathologyCommand::Maps { recipe, out }
        | PathologyCommand::Architecture { recipe, out }
        | PathologyCommand::Composition { recipe, out }
        | PathologyCommand::SpatialStudy { recipe, out } => (recipe, out),
        PathologyCommand::Scan { recipe, out } => (recipe, out),
    };
    let (maximum_bytes, message) = match &command {
        PathologyCommand::Architecture { .. } => {
            let maximum = host_memory::admitted_resources().recipe_bytes;
            (
                maximum,
                format!("pathology architecture recipe must be a regular file of at most {maximum} bytes on this host"),
            )
        }
        _ => (
            64 * 1024 * 1024,
            "pathology recipe must be a regular file of at most 64 MiB".to_owned(),
        ),
    };
    let bytes = super::bayes::read_regular_file(recipe, maximum_bytes, &message)
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
        PathologyCommand::Architecture { .. } => {
            publish_pathology_architecture(&analyze_pathology_architecture(&bytes)?, out)
        }
        PathologyCommand::Composition { .. } => {
            publish_pathology_composition(&analyze_pathology_composition(&bytes)?, out)
        }
    }
}
