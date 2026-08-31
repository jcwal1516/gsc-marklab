use std::path::PathBuf;

use clap::ValueEnum;
use serde::Deserialize;

use super::schema::HeInputFormat;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(super) enum LogLevel {
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

#[derive(Debug)]
pub(super) struct AnalyzeRequest {
    pub(super) cells: PathBuf,
    pub(super) mask: PathBuf,
    pub(super) config: PathBuf,
    pub(super) out: PathBuf,
    pub(super) threads: Option<usize>,
    pub(super) observability: ObservabilityOptions,
    pub(super) heap_profile: Option<PathBuf>,
}

#[derive(Debug)]
pub(super) struct ClassicalRequest {
    pub(super) cells: PathBuf,
    pub(super) mask: PathBuf,
    pub(super) out: PathBuf,
    pub(super) r_max_um: f64,
    pub(super) r_steps: usize,
    pub(super) simulations: usize,
    pub(super) seed: u64,
    pub(super) alpha: f64,
    pub(super) memory_budget_mib: usize,
    pub(super) maximum_pair_visits: usize,
    pub(super) maximum_csr_draws: usize,
}

#[derive(Debug)]
pub(super) struct NearestSpaceRequest {
    pub(super) cells: PathBuf,
    pub(super) mask: PathBuf,
    pub(super) out: PathBuf,
    pub(super) r_max_um: f64,
    pub(super) r_steps: usize,
    pub(super) probe_grid_x: usize,
    pub(super) probe_grid_y: usize,
    pub(super) simulations: usize,
    pub(super) seed: u64,
    pub(super) alpha: f64,
    pub(super) j_denominator_epsilon: f64,
    pub(super) memory_budget_mib: usize,
    pub(super) maximum_nearest_queries: usize,
    pub(super) maximum_csr_draws: usize,
}

#[derive(Debug)]
pub(super) struct MultimodalAnalyzeRequest {
    pub(super) he_cells: PathBuf,
    pub(super) ihc_cells: PathBuf,
    pub(super) landmarks: PathBuf,
    pub(super) config: PathBuf,
    pub(super) out: PathBuf,
    pub(super) case_id: String,
    pub(super) timepoint: String,
    pub(super) protein: String,
    pub(super) he_format: HeInputFormat,
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

#[derive(Debug, Default)]
pub(super) struct ObservabilityOptions {
    pub(super) log: Option<LogLevel>,
    pub(super) trace_json: Option<PathBuf>,
    pub(super) timings: Option<PathBuf>,
}
