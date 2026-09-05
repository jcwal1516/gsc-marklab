use std::{fs::File, io::Read, path::PathBuf};

use clap::{Parser, Subcommand};
use marklab::{
    execute_multiplex_study, publish_multiplex_study, MarklabError, MultiplexStudyTarget,
};

#[derive(Parser)]
#[command(name = "marklab")]
struct StudyCli {
    #[command(subcommand)]
    command: StudyTopLevel,
}

#[derive(Subcommand)]
enum StudyTopLevel {
    /// Analyze a declared multiplex panel across slides and independent patients.
    Study {
        #[command(subcommand)]
        command: StudyCommand,
    },
}

#[derive(Subcommand)]
enum StudyCommand {
    /// Inspect supported inputs, design, evidence limits and claim ceiling before running.
    Describe,
    /// Execute or resume a strict multiplex recipe and publish its patient-level report.
    Run {
        /// Version-one JSON recipe with named channels, exact windows and patient identities.
        #[arg(long)]
        recipe: PathBuf,
        /// Local durable project holding verified node outputs and execution provenance.
        #[arg(long)]
        project: PathBuf,
        /// New or empty directory for result.json, report.md and manifest.json.
        #[arg(long)]
        out: PathBuf,
        /// Stop after this many canonically ordered slides; retain durable progress without a report.
        #[arg(long)]
        through_slides: Option<usize>,
    },
}

pub(crate) fn cli_route() -> super::command_tree::Route {
    super::command_tree::Route::new::<StudyCli>(run)
}

fn run() -> marklab::Result<()> {
    let StudyTopLevel::Study { command } = StudyCli::parse().command;
    match command {
        StudyCommand::Describe => {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "profile": "marklab.multiplex_study_recipe", "version": 1,
                    "scientific_status": "experimental",
                    "domain": "exact 2-D polygon windows and physical micrometre coordinates",
                    "marks": "named continuous, binary and categorical protein-assay channels; explicit measured/predicted status and null observations",
                    "spatial_methods": ["global Moran I", "global Geary C"],
                    "spatial_graph": "prespecified radius, binary symmetric or row-standardized; exact observed-row graph",
                    "patient_reduction": "equal-slide mean; no cell or slide pseudoreplication",
                    "inference": "prespecified independent patient groups; complete-family single-step Max-T; finite Monte Carlo permutations",
                    "missingness": "per-channel complete cases; any required unavailable slide blocks patient inference",
                    "admitted_profile_limits": {"recipe_bytes": 16 * 1024 * 1024, "channels":64, "selected_channels":32, "slides":1024},
                    "validated_whole_slide_scale": null,
                    "external_biological_validation": false,
                    "claim_ceiling": "conditional research inference; no clinical or causal claims",
                    "recipe_guide": "docs/multiplex-study.md"
                }))?
            );
        }
        StudyCommand::Run {
            recipe,
            project,
            out,
            through_slides,
        } => {
            let mut bytes = Vec::new();
            File::open(&recipe)
                .map_err(|source| MarklabError::Io {
                    path: recipe.clone(),
                    source,
                })?
                .take(16 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(|source| MarklabError::Io {
                    path: recipe,
                    source,
                })?;
            let target = through_slides.map_or(
                MultiplexStudyTarget::Complete,
                MultiplexStudyTarget::ThroughSlides,
            );
            let runtime = super::project::native_runtime_provenance()
                .map_err(super::bayes::into_marklab_error)?;
            let run = execute_multiplex_study(&project, &bytes, target, runtime)?;
            if let Some(result) = run.result {
                publish_multiplex_study(&result, &out)?;
            } else {
                eprintln!(
                    "study paused after {} slides; resume with the same recipe and project",
                    run.executed_slides + run.restored_slides
                );
            }
            eprintln!(
                "study executed_slides={} restored_slides={} durable_execution_count={}",
                run.executed_slides, run.restored_slides, run.durable_execution_count
            );
        }
    }
    Ok(())
}
