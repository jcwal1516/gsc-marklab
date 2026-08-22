use std::{
    fs,
    path::{Component, Path, PathBuf},
};

#[cfg(feature = "parquet")]
use crate::{io::parquet::write_filtered_pattern_export_parquet, Pattern, PatternMeta};
use crate::{permutation::labels::permute_fixed_count, MarklabError, Result};
use clap::{Parser, Subcommand, ValueEnum};

macro_rules! bail {
    ($($argument:tt)*) => {
        return Err(MarklabError::Validation(format!($($argument)*)))
    };
}

fn batch_output_path(root: &Path, raw_id: &str) -> Result<PathBuf> {
    let id = raw_id.trim();
    let mut components = Path::new(id).components();
    let is_single_normal_component = matches!(components.next(), Some(Component::Normal(_)))
        && components.next().is_none()
        && !id.contains('/')
        && !id.contains('\\');
    if !is_single_normal_component {
        return Err(MarklabError::Validation(
            "batch manifest id must be one non-empty path component without separators, '.' or '..'"
                .into(),
        ));
    }

    let target = root.join(id);
    if let Ok(metadata) = fs::symlink_metadata(&target) {
        if metadata.file_type().is_symlink() {
            return Err(MarklabError::Validation(format!(
                "batch output target may not be a symbolic link: {}",
                target.display()
            )));
        }
        if root.exists() {
            let canonical_root =
                fs::canonicalize(root).map_err(|source| MarklabError::io(root, source))?;
            let canonical_target =
                fs::canonicalize(&target).map_err(|source| MarklabError::io(&target, source))?;
            if !canonical_target.starts_with(&canonical_root) {
                return Err(MarklabError::Validation(format!(
                    "batch output target escapes the configured root: {}",
                    target.display()
                )));
            }
        }
    }
    Ok(target)
}

#[path = "cli/analyze.rs"]
mod analyze;
#[path = "cli/batch.rs"]
mod batch;
#[path = "cli/multimodal.rs"]
mod multimodal;
#[path = "cli/prepost.rs"]
mod prepost;
#[path = "cli/profile.rs"]
mod profile;
#[path = "cli/simulate.rs"]
mod simulate;
#[cfg(feature = "wsi")]
#[path = "cli/slide.rs"]
mod slide;
#[path = "cli/smoke.rs"]
mod smoke;
use serde::Deserialize;

#[derive(Debug, Parser)]
#[command(
    name = "marklab",
    version,
    about = "Spatial statistics for marked cell patterns in pathology"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    #[cfg(feature = "wsi")]
    InspectSlide {
        slide: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    #[cfg(feature = "wsi")]
    ExtractRegion {
        slide: PathBuf,
        #[arg(long, default_value_t = 0)]
        scene: usize,
        #[arg(long, default_value_t = 0)]
        series: usize,
        #[arg(long, default_value_t = 0)]
        level: u32,
        #[arg(long, default_value_t = 0)]
        z: u32,
        #[arg(long, default_value_t = 0)]
        c: u32,
        #[arg(long, default_value_t = 0)]
        t: u32,
        #[arg(long)]
        x: u64,
        #[arg(long)]
        y: u64,
        #[arg(long)]
        width: u32,
        #[arg(long)]
        height: u32,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        force: bool,
    },
    Analyze {
        #[arg(long)]
        cells: PathBuf,
        #[arg(long)]
        mask: PathBuf,
        #[arg(long)]
        config: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        threads: Option<usize>,
        #[arg(long, value_enum)]
        log: Option<LogLevel>,
        #[arg(long)]
        trace_json: Option<PathBuf>,
        #[arg(long)]
        timings: Option<PathBuf>,
        #[arg(long)]
        heap_profile: Option<PathBuf>,
    },
    Batch {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        config: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        threads: Option<usize>,
    },
    Prepost {
        #[arg(long)]
        pre: PathBuf,
        #[arg(long)]
        post: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    ProfilePlan {
        #[arg(long)]
        workload: String,
        #[arg(long)]
        out: PathBuf,
    },
    Simulate {
        #[command(subcommand)]
        command: SimulateCommands,
    },
    Multimodal {
        #[command(subcommand)]
        command: MultimodalCommands,
    },
    Smoke {
        #[arg(long)]
        suite: String,
        #[arg(long)]
        replicates: usize,
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum SimulateCommands {
    RandomLabeling {
        #[arg(long)]
        n: usize,
        #[arg(long)]
        p: f64,
        #[arg(long, default_value_t = 123_456_789)]
        seed: u64,
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum MultimodalCommands {
    Analyze {
        #[arg(long)]
        he_cells: PathBuf,
        #[arg(long)]
        ihc_cells: PathBuf,
        #[arg(long)]
        landmarks: PathBuf,
        /// TOML configuration; registration.transform="rigid" fits rotation and translation only.
        #[arg(long)]
        config: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        case_id: String,
        #[arg(long)]
        timepoint: String,
        #[arg(long)]
        protein: String,
        #[arg(long, value_enum, default_value_t = HeInputFormat::HeCsv)]
        he_format: HeInputFormat,
        #[arg(long, default_value_t = 0.5)]
        cellvit_min_probability: f64,
    },
    Prepost {
        #[arg(long)]
        pre: PathBuf,
        #[arg(long)]
        post: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Batch {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum HeInputFormat {
    HeCsv,
    CellvitCsv,
}

pub fn run_cli() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        #[cfg(feature = "wsi")]
        Commands::InspectSlide { slide, output } => slide::inspect(slide, output),
        #[cfg(feature = "wsi")]
        Commands::ExtractRegion {
            slide,
            scene,
            series,
            level,
            z,
            c,
            t,
            x,
            y,
            width,
            height,
            output,
            force,
        } => slide::extract(slide::ExtractRequest {
            slide,
            region: crate::RegionRequest {
                scene,
                series,
                level,
                plane: crate::PlaneSelection { z, c, t },
                x,
                y,
                width,
                height,
            },
            output,
            force,
        }),
        Commands::Analyze {
            cells,
            mask,
            config,
            out,
            threads,
            log,
            trace_json,
            timings,
            heap_profile,
        } => analyze::run(AnalyzeRequest {
            cells,
            mask,
            config,
            out,
            threads,
            observability: ObservabilityOptions {
                log,
                trace_json,
                timings,
            },
            heap_profile,
        }),
        Commands::Batch {
            manifest,
            config,
            out,
            threads,
        } => batch::run(manifest, config, out, threads),
        Commands::Prepost { pre, post, out } => prepost::run(pre, post, out),
        Commands::ProfilePlan { workload, out } => profile::run(&workload, out),
        Commands::Simulate { command } => match command {
            SimulateCommands::RandomLabeling { n, p, seed, out } => simulate::run(n, p, seed, out),
        },
        Commands::Multimodal { command } => match command {
            MultimodalCommands::Analyze {
                he_cells,
                ihc_cells,
                landmarks,
                config,
                out,
                case_id,
                timepoint,
                protein,
                he_format,
                cellvit_min_probability,
            } => multimodal::analyze::run(MultimodalAnalyzeRequest {
                he_cells,
                ihc_cells,
                landmarks,
                config,
                out,
                case_id,
                timepoint,
                protein,
                he_format,
                cellvit_min_probability,
            }),
            MultimodalCommands::Prepost { pre, post, out } => {
                multimodal::commands::prepost(pre, post, out)
            }
            MultimodalCommands::Batch { manifest, out } => {
                multimodal::commands::batch(manifest, out)
            }
        },
        Commands::Smoke {
            suite,
            replicates,
            out,
        } => smoke::run(&suite, replicates, out),
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum LogLevel {
    Info,
    Debug,
}

impl LogLevel {
    fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Debug => "debug",
        }
    }
}

#[derive(Debug, Deserialize)]
struct ManifestRow {
    id: String,
    cells: PathBuf,
    mask: PathBuf,
}

#[derive(Debug, Deserialize)]
struct LandmarkRow {
    source_x_um: f64,
    source_y_um: f64,
    target_x_um: f64,
    target_y_um: f64,
}

#[derive(Debug)]
struct AnalyzeRequest {
    cells: PathBuf,
    mask: PathBuf,
    config: PathBuf,
    out: PathBuf,
    threads: Option<usize>,
    observability: ObservabilityOptions,
    heap_profile: Option<PathBuf>,
}

#[derive(Debug)]
struct MultimodalAnalyzeRequest {
    he_cells: PathBuf,
    ihc_cells: PathBuf,
    landmarks: PathBuf,
    config: PathBuf,
    out: PathBuf,
    case_id: String,
    timepoint: String,
    protein: String,
    he_format: HeInputFormat,
    cellvit_min_probability: f64,
}

#[derive(Debug, Deserialize)]
struct MultimodalManifestRow {
    id: String,
    he_cells: Option<PathBuf>,
    ihc_cells: Option<PathBuf>,
    landmarks: Option<PathBuf>,
    config: Option<PathBuf>,
    case_id: Option<String>,
    timepoint: Option<String>,
    protein: Option<String>,
    pre: Option<PathBuf>,
    post: Option<PathBuf>,
}

#[derive(Debug, Default)]
struct ObservabilityOptions {
    log: Option<LogLevel>,
    trace_json: Option<PathBuf>,
    timings: Option<PathBuf>,
}

#[cfg(test)]
mod batch_output_path_tests {
    use std::path::Path;

    use super::batch_output_path;

    #[test]
    fn rejects_unsafe_batch_output_ids() {
        let root = Path::new("safe-output");
        for id in [
            "",
            "   ",
            ".",
            "..",
            "../escape",
            "a/b",
            "a\\b",
            "/absolute",
        ] {
            assert!(batch_output_path(root, id).is_err(), "accepted {id:?}");
        }
    }

    #[test]
    fn trims_and_accepts_one_normal_batch_output_component() {
        assert_eq!(
            batch_output_path(Path::new("safe-output"), "  case_001_post  ").expect("valid id"),
            Path::new("safe-output").join("case_001_post")
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_existing_symlink_batch_output_target() {
        use std::{fs, os::unix::fs::symlink};

        let directory = tempfile::tempdir().expect("temp dir");
        let root = directory.path().join("output");
        let outside = directory.path().join("outside");
        fs::create_dir_all(&root).expect("output root");
        fs::create_dir_all(&outside).expect("outside dir");
        symlink(&outside, root.join("case_001")).expect("symlink");

        assert!(batch_output_path(&root, "case_001").is_err());
    }
}
