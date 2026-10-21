use std::path::PathBuf;

use clap::ValueEnum;
use serde::Deserialize;

use super::schema::HeInputFormat;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum LogLevel {
    Info,
    Debug,
}

impl LogLevel {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Debug => "debug",
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct ManifestRow {
    pub(super) id: String,
    pub(super) cells: PathBuf,
    pub(super) mask: PathBuf,
}

#[derive(Debug, Deserialize)]
pub(super) struct LandmarkRow {
    pub(super) source_x_um: f64,
    pub(super) source_y_um: f64,
    pub(super) target_x_um: f64,
    pub(super) target_y_um: f64,
}

#[derive(clap::Args, Debug)]
pub struct AnalyzeRequest {
    /// Cell table (CSV or Parquet).
    #[arg(long)]
    pub(super) cells: PathBuf,
    /// Tissue mask (GeoJSON).
    #[arg(long)]
    pub(super) mask: PathBuf,
    /// Analysis configuration (TOML).
    #[arg(long)]
    pub(super) config: PathBuf,
    /// Output directory.
    #[arg(long)]
    pub(super) out: PathBuf,
    /// Number of worker threads (default: all available cores).
    #[arg(long)]
    pub(super) threads: Option<usize>,
    #[command(flatten)]
    pub(super) observability: ObservabilityOptions,
    /// Write a heap profile to this file.
    #[arg(long)]
    pub(super) heap_profile: Option<PathBuf>,
}

#[derive(clap::Args, Debug)]
pub struct ClassicalRequest {
    /// Cell table (CSV or Parquet).
    #[arg(long)]
    pub(super) cells: PathBuf,
    /// Tissue mask (GeoJSON).
    #[arg(long)]
    pub(super) mask: PathBuf,
    /// Output directory.
    #[arg(long)]
    pub(super) out: PathBuf,
    /// Largest distance evaluated, in µm.
    #[arg(long)]
    pub(super) r_max_um: f64,
    /// Number of distance steps up to --r-max-um.
    #[arg(long)]
    pub(super) r_steps: usize,
    /// Number of simulated null patterns.
    #[arg(long)]
    pub(super) simulations: usize,
    /// Random seed; the same seed gives the same result.
    #[arg(long)]
    pub(super) seed: u64,
    /// Significance level.
    #[arg(long)]
    pub(super) alpha: f64,
    /// Memory budget in MiB.
    #[arg(long)]
    pub(super) memory_budget_mib: usize,
    /// Maximum number of cell pairs visited.
    #[arg(long = "max-pair-visits", value_name = "MAX_PAIR_VISITS")]
    pub(super) maximum_pair_visits: usize,
    /// Maximum number of random points drawn for CSR simulations.
    #[arg(long = "max-csr-draws", value_name = "MAX_CSR_DRAWS")]
    pub(super) maximum_csr_draws: usize,
}

#[derive(clap::Args, Debug)]
pub struct NearestSpaceRequest {
    /// Cell table (CSV or Parquet).
    #[arg(long)]
    pub(super) cells: PathBuf,
    /// Tissue mask (GeoJSON).
    #[arg(long)]
    pub(super) mask: PathBuf,
    /// Output directory.
    #[arg(long)]
    pub(super) out: PathBuf,
    /// Largest distance evaluated, in µm.
    #[arg(long)]
    pub(super) r_max_um: f64,
    /// Number of distance steps up to --r-max-um.
    #[arg(long)]
    pub(super) r_steps: usize,
    /// Number of empty-space probe locations along x.
    #[arg(long)]
    pub(super) probe_grid_x: usize,
    /// Number of empty-space probe locations along y.
    #[arg(long)]
    pub(super) probe_grid_y: usize,
    /// Number of simulated null patterns.
    #[arg(long)]
    pub(super) simulations: usize,
    /// Random seed; the same seed gives the same result.
    #[arg(long)]
    pub(super) seed: u64,
    /// Significance level.
    #[arg(long)]
    pub(super) alpha: f64,
    /// Smallest admitted empty-space survival probability in the J ratio.
    #[arg(long)]
    pub(super) j_denominator_epsilon: f64,
    /// Memory budget in MiB.
    #[arg(long)]
    pub(super) memory_budget_mib: usize,
    /// Maximum number of nearest-neighbour queries.
    #[arg(long = "max-nearest-queries", value_name = "MAX_NEAREST_QUERIES")]
    pub(super) maximum_nearest_queries: usize,
    /// Maximum number of random points drawn for CSR simulations.
    #[arg(long = "max-csr-draws", value_name = "MAX_CSR_DRAWS")]
    pub(super) maximum_csr_draws: usize,
}

#[derive(clap::Args, Debug)]
pub struct MultimodalAnalyzeRequest {
    /// H&E cell table used in the multimodal analysis.
    #[arg(long)]
    pub(super) he_cells: PathBuf,
    /// IHC cell table used in the multimodal analysis.
    #[arg(long)]
    pub(super) ihc_cells: PathBuf,
    /// Matched landmarks used to register the modalities.
    #[arg(long)]
    pub(super) landmarks: PathBuf,
    /// TOML configuration; registration.transform="rigid" fits rotation and translation only.
    #[arg(long)]
    pub(super) config: PathBuf,
    /// Output directory.
    #[arg(long)]
    pub(super) out: PathBuf,
    /// Case identifier to attach to the analysis.
    #[arg(long)]
    pub(super) case_id: String,
    /// Timepoint label to attach to the analysis.
    #[arg(long)]
    pub(super) timepoint: String,
    /// Protein or marker identifier to attach to the analysis.
    #[arg(long)]
    pub(super) protein: String,
    /// Input format of the H&E cell table.
    #[arg(long, value_enum, default_value_t = HeInputFormat::HeCsv)]
    pub(super) he_format: HeInputFormat,
    /// Minimum CellViT classification probability for retaining a cell.
    #[arg(long, default_value_t = 0.5)]
    pub(super) cellvit_min_probability: f64,
}

#[derive(Debug, Deserialize)]
pub(super) struct MultimodalManifestRow {
    pub(super) id: String,
    pub(super) he_cells: Option<PathBuf>,
    pub(super) ihc_cells: Option<PathBuf>,
    pub(super) landmarks: Option<PathBuf>,
    pub(super) config: Option<PathBuf>,
    pub(super) case_id: Option<String>,
    pub(super) timepoint: Option<String>,
    pub(super) protein: Option<String>,
    pub(super) pre: Option<PathBuf>,
    pub(super) post: Option<PathBuf>,
}

#[derive(clap::Args, Debug, Default)]
pub(super) struct ObservabilityOptions {
    /// Log level.
    #[arg(long, value_enum)]
    pub(super) log: Option<LogLevel>,
    /// Write a JSON trace of the run to this file.
    #[arg(long)]
    pub(super) trace_json: Option<PathBuf>,
    /// Write stage timings to this file.
    #[arg(long)]
    pub(super) timings: Option<PathBuf>,
}
