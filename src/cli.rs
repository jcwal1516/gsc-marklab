use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use crate::{
    geom::mask::TumorMask,
    io::{intermediates::write_analysis_intermediates, load_pattern_path_with_diagnostics},
    multimodal::cell_table::{
        load_cellvit_he_cell_table_csv, load_he_cell_table_csv, load_ihc_cell_table_csv,
        primary_label, CellSection, FusedCell,
    },
    neighborhood::{
        enrichment::{edge_enrichment, edge_enrichment_with_strata, LabelPair},
        graph::{build_spatial_graph, GraphConfig, SpatialGraph},
    },
    permutation::labels::permute_fixed_count,
    prepost::deltas::{compare_multimodal_prepost, compare_prepost},
    registration::{
        landmarks::LandmarkPair,
        transform::{fit_affine, fit_rigid},
    },
    validation::{
        run_multimodal_synthetic_validation, run_synthetic_validation,
        MultimodalSyntheticValidationSummary,
    },
    AnalysisConfig, AnalysisEngine, MarkedPatternResult, MarklabError, MultimodalEngine,
    MultimodalInput, MultimodalResult, NeighborhoodEnrichmentResult, NeighborhoodNullModel,
    OutputWriter, RegistrationTransform, Result, ResultDocument, ThreadSetting, TimingStage,
};
#[cfg(feature = "parquet")]
use crate::{io::parquet::write_pattern_parquet, Pattern, PatternMeta};
use clap::{Parser, Subcommand, ValueEnum};

macro_rules! bail {
    ($($argument:tt)*) => {
        return Err(MarklabError::Validation(format!($($argument)*)))
    };
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
#[path = "cli/validate.rs"]
mod validate;
use serde::{Deserialize, Serialize};

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
    Validate {
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
        Commands::Validate {
            suite,
            replicates,
            out,
        } => validate::run(&suite, replicates, out),
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

#[derive(Debug, Serialize)]
struct NullModelSensitivityResult {
    null_model: String,
    results: Vec<NeighborhoodEnrichmentResult>,
}

#[derive(Debug, Serialize)]
struct RegistrationResidualRecord {
    landmark_index: usize,
    source_x_um: f64,
    source_y_um: f64,
    target_x_um: f64,
    target_y_um: f64,
    transformed_x_um: f64,
    transformed_y_um: f64,
    residual_dx_um: f64,
    residual_dy_um: f64,
    residual_um: f64,
}

#[derive(Debug, Serialize)]
struct CellExtrapolationRecord {
    source_section: String,
    source_cell_id: String,
    x_um_registered: f64,
    y_um_registered: f64,
    outside_landmark_hull: bool,
}

#[derive(Clone, Copy, Debug)]
struct Point2 {
    x: f64,
    y: f64,
}

#[derive(Debug, Default)]
struct ObservabilityOptions {
    log: Option<LogLevel>,
    trace_json: Option<PathBuf>,
    timings: Option<PathBuf>,
}
