use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use crate::{MarklabError, Result};
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
#[path = "cli/categorical_cross_pair_correlation.rs"]
mod categorical_cross_pair_correlation;
#[path = "cli/categorical_mark_project.rs"]
mod categorical_mark_project;
#[path = "cli/categorical_pair.rs"]
mod categorical_pair;
#[path = "cli/classical.rs"]
mod classical;
#[path = "cli/inhomogeneous_bandwidth_selection.rs"]
mod inhomogeneous_bandwidth_selection;
#[path = "cli/inhomogeneous_pair_correlation.rs"]
mod inhomogeneous_pair_correlation;
#[path = "cli/inhomogeneous_project.rs"]
mod inhomogeneous_project;
#[path = "cli/inhomogeneous_spatial.rs"]
mod inhomogeneous_spatial;
#[path = "cli/isotropic_pair_correlation.rs"]
mod isotropic_pair_correlation;
#[path = "cli/isotropic_spatial.rs"]
mod isotropic_spatial;
#[path = "cli/multimodal.rs"]
mod multimodal;
#[path = "cli/nearest_space.rs"]
mod nearest_space;
#[path = "cli/piecewise_compartment_pair_correlation.rs"]
mod piecewise_compartment_pair_correlation;
#[path = "cli/piecewise_compartment_spatial.rs"]
mod piecewise_compartment_spatial;
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
#[path = "cli/translation_pair_correlation.rs"]
mod translation_pair_correlation;
#[path = "cli/translation_spatial.rs"]
mod translation_spatial;
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
    Classical {
        #[arg(long)]
        cells: PathBuf,
        #[arg(long)]
        mask: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        r_max_um: f64,
        #[arg(long)]
        r_steps: usize,
        #[arg(long)]
        simulations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        max_pair_visits: usize,
        #[arg(long)]
        max_csr_draws: usize,
    },
    NearestSpace {
        #[arg(long)]
        cells: PathBuf,
        #[arg(long)]
        mask: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        r_max_um: f64,
        #[arg(long)]
        r_steps: usize,
        #[arg(long)]
        probe_grid_x: usize,
        #[arg(long)]
        probe_grid_y: usize,
        #[arg(long)]
        simulations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        j_denominator_epsilon: f64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        max_nearest_queries: usize,
        #[arg(long)]
        max_csr_draws: usize,
    },
    Project {
        #[command(subcommand)]
        command: ProjectCommands,
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
    GrowthFront {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        diffusion_um2_per_time: f64,
        #[arg(long)]
        growth_rate_per_time: f64,
        #[arg(long)]
        carrying_capacity: f64,
        #[arg(long)]
        final_time: f64,
        #[arg(long)]
        time_step: f64,
        #[arg(long)]
        front_threshold_fraction: f64,
        #[arg(long)]
        record_every_steps: u32,
        #[arg(long)]
        maximum_cell_steps: u64,
        #[arg(long)]
        out: PathBuf,
    },
    SpatialCompetition {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        diffusion_a_um2_per_time: f64,
        #[arg(long)]
        diffusion_b_um2_per_time: f64,
        #[arg(long)]
        growth_a_per_time: f64,
        #[arg(long)]
        growth_b_per_time: f64,
        #[arg(long)]
        carrying_a: f64,
        #[arg(long)]
        carrying_b: f64,
        #[arg(long)]
        competition_a_from_b: f64,
        #[arg(long)]
        competition_b_from_a: f64,
        #[arg(long)]
        treatment_a_per_time: f64,
        #[arg(long)]
        treatment_b_per_time: f64,
        #[arg(long)]
        final_time: f64,
        #[arg(long)]
        time_step: f64,
        #[arg(long)]
        extinction_threshold_fraction: f64,
        #[arg(long)]
        record_every_steps: u32,
        #[arg(long)]
        maximum_cell_species_steps: u64,
        #[arg(long)]
        out: PathBuf,
    },
    AgentCompetition {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        maximum_events: u32,
        #[arg(long)]
        maximum_agents: u32,
        #[arg(long)]
        maximum_pair_visits: u64,
        #[arg(long)]
        retain_events: u32,
        #[arg(long)]
        out: PathBuf,
    },
    ReactionDiffusion {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        final_time: f64,
        #[arg(long)]
        time_step: f64,
        #[arg(long)]
        record_every_steps: u32,
        #[arg(long)]
        maximum_cell_steps: u64,
        #[arg(long)]
        out: PathBuf,
    },
    EvolveInterface {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        final_time: f64,
        #[arg(long)]
        time_step: f64,
        #[arg(long)]
        reinitialize_every_steps: u32,
        #[arg(long)]
        record_every_steps: u32,
        #[arg(long)]
        maximum_cell_steps: u64,
        #[arg(long)]
        maximum_reinitialization_distance_visits: u64,
        #[arg(long)]
        out: PathBuf,
    },
    VascularTransport {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        final_time: f64,
        #[arg(long)]
        time_step: f64,
        #[arg(long)]
        hypoxia_threshold: f64,
        #[arg(long)]
        record_every_steps: u32,
        #[arg(long)]
        maximum_cell_steps: u64,
        #[arg(long)]
        out: PathBuf,
    },
    MechanisticTissue {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    SummaryMatching {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum ProjectCommands {
    CategoricalPair {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        cells: PathBuf,
        #[arg(long)]
        mask: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        source_level: String,
        #[arg(long)]
        target_level: String,
        #[arg(long, value_delimiter = ',')]
        radii_um: Vec<f64>,
        #[arg(long)]
        permutations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        max_pair_visits: usize,
        #[arg(long)]
        max_null_pair_evaluations: usize,
    },
    CategoricalCrossPairCorrelation {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        cells: PathBuf,
        #[arg(long)]
        mask: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        source_level: String,
        #[arg(long)]
        target_level: String,
        #[arg(long, value_delimiter = ',')]
        radii_um: Vec<f64>,
        #[arg(long)]
        bandwidth_um: f64,
        #[arg(long)]
        permutations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        max_pair_visits: usize,
        #[arg(long)]
        max_null_pair_evaluations: usize,
    },
    InhomogeneousSpatial {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        cells: PathBuf,
        #[arg(long)]
        mask: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, value_delimiter = ',')]
        radii_um: Vec<f64>,
        #[arg(long)]
        bandwidth_um: f64,
        #[arg(long)]
        grid_x: usize,
        #[arg(long)]
        grid_y: usize,
        #[arg(long)]
        simulations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        minimum_intensity_per_um2: f64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        max_probes: usize,
        #[arg(long)]
        max_intensity_evaluations: usize,
        #[arg(long)]
        max_pair_visits: usize,
        #[arg(long)]
        max_null_draws: usize,
    },
    TranslationSpatial {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        cells: PathBuf,
        #[arg(long)]
        mask: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, value_delimiter = ',')]
        radii_um: Vec<f64>,
        #[arg(long)]
        simulations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        max_pair_visits: usize,
        #[arg(long)]
        max_overlap_evaluations: usize,
        #[arg(long)]
        max_overlap_candidate_work: usize,
        #[arg(long)]
        max_overlap_output_vertices: usize,
        #[arg(long)]
        max_csr_draws: usize,
    },
    TranslationPairCorrelation {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        cells: PathBuf,
        #[arg(long)]
        mask: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, value_delimiter = ',')]
        radii_um: Vec<f64>,
        #[arg(long)]
        bandwidth_um: f64,
        #[arg(long)]
        simulations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        max_pair_visits: usize,
        #[arg(long)]
        max_overlap_evaluations: usize,
        #[arg(long)]
        max_overlap_candidate_work: usize,
        #[arg(long)]
        max_overlap_output_vertices: usize,
        #[arg(long)]
        max_csr_draws: usize,
    },
    IsotropicSpatial {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        cells: PathBuf,
        #[arg(long)]
        mask: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, value_delimiter = ',')]
        radii_um: Vec<f64>,
        #[arg(long)]
        simulations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        max_pair_visits: usize,
        #[arg(long)]
        max_visible_arc_evaluations: usize,
        #[arg(long)]
        max_arc_segment_tests: usize,
        #[arg(long)]
        max_arc_membership_queries: usize,
        #[arg(long)]
        max_csr_draws: usize,
    },
    IsotropicPairCorrelation {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        cells: PathBuf,
        #[arg(long)]
        mask: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, value_delimiter = ',')]
        radii_um: Vec<f64>,
        #[arg(long)]
        bandwidth_um: f64,
        #[arg(long)]
        simulations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        max_pair_visits: usize,
        #[arg(long)]
        max_visible_arc_evaluations: usize,
        #[arg(long)]
        max_arc_segment_tests: usize,
        #[arg(long)]
        max_arc_membership_queries: usize,
        #[arg(long)]
        max_csr_draws: usize,
    },
    GaussianBandwidthSelectedSpatial {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        cells: PathBuf,
        #[arg(long)]
        mask: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, value_delimiter = ',')]
        radii_um: Vec<f64>,
        #[arg(long, value_delimiter = ',')]
        candidate_bandwidths_um: Vec<f64>,
        #[arg(long)]
        grid_x: usize,
        #[arg(long)]
        grid_y: usize,
        #[arg(long)]
        simulations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        minimum_intensity_per_um2: f64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        max_probes: usize,
        #[arg(long)]
        max_intensity_evaluations: usize,
        #[arg(long)]
        max_pair_visits: usize,
        #[arg(long)]
        max_null_draws: usize,
        #[arg(long)]
        max_bandwidth_candidates: usize,
        #[arg(long)]
        max_selection_intensity_evaluations: usize,
    },
    PiecewiseCompartmentSpatial {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        cells: PathBuf,
        #[arg(long)]
        observation_mask: PathBuf,
        #[arg(long)]
        negative_mask: PathBuf,
        #[arg(long)]
        positive_mask: PathBuf,
        #[arg(long)]
        negative_compartment_id: String,
        #[arg(long)]
        positive_compartment_id: String,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, value_delimiter = ',')]
        radii_um: Vec<f64>,
        #[arg(long)]
        simulations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        max_boundary_segments: usize,
        #[arg(long)]
        max_compartment_queries: usize,
        #[arg(long)]
        max_pair_visits: usize,
        #[arg(long)]
        max_null_draws: usize,
    },
    PiecewiseCompartmentPairCorrelation {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        cells: PathBuf,
        #[arg(long)]
        observation_mask: PathBuf,
        #[arg(long)]
        negative_mask: PathBuf,
        #[arg(long)]
        positive_mask: PathBuf,
        #[arg(long)]
        negative_compartment_id: String,
        #[arg(long)]
        positive_compartment_id: String,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, value_delimiter = ',')]
        radii_um: Vec<f64>,
        #[arg(long)]
        pair_bandwidth_um: f64,
        #[arg(long)]
        simulations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        max_boundary_segments: usize,
        #[arg(long)]
        max_compartment_queries: usize,
        #[arg(long)]
        max_pair_visits: usize,
        #[arg(long)]
        max_null_draws: usize,
    },
    InhomogeneousPairCorrelation {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        cells: PathBuf,
        #[arg(long)]
        mask: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, value_delimiter = ',')]
        radii_um: Vec<f64>,
        #[arg(long)]
        intensity_bandwidth_um: f64,
        #[arg(long)]
        pair_bandwidth_um: f64,
        #[arg(long)]
        grid_x: usize,
        #[arg(long)]
        grid_y: usize,
        #[arg(long)]
        simulations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        minimum_intensity_per_um2: f64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        max_probes: usize,
        #[arg(long)]
        max_intensity_evaluations: usize,
        #[arg(long)]
        max_pair_visits: usize,
        #[arg(long)]
        max_null_draws: usize,
    },
    Classical {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        cells: PathBuf,
        #[arg(long)]
        mask: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        r_max_um: f64,
        #[arg(long)]
        r_steps: usize,
        #[arg(long)]
        simulations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        max_pair_visits: usize,
        #[arg(long)]
        max_csr_draws: usize,
    },
    NearestSpace {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        cells: PathBuf,
        #[arg(long)]
        mask: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        r_max_um: f64,
        #[arg(long)]
        r_steps: usize,
        #[arg(long)]
        probe_grid_x: usize,
        #[arg(long)]
        probe_grid_y: usize,
        #[arg(long)]
        simulations: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        j_denominator_epsilon: f64,
        #[arg(long)]
        memory_budget_mib: usize,
        #[arg(long)]
        max_nearest_queries: usize,
        #[arg(long)]
        max_csr_draws: usize,
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
        Commands::Classical {
            cells,
            mask,
            out,
            r_max_um,
            r_steps,
            simulations,
            seed,
            alpha,
            memory_budget_mib,
            max_pair_visits,
            max_csr_draws,
        } => classical::run(ClassicalRequest {
            cells,
            mask,
            out,
            r_max_um,
            r_steps,
            simulations,
            seed,
            alpha,
            memory_budget_mib,
            maximum_pair_visits: max_pair_visits,
            maximum_csr_draws: max_csr_draws,
        }),
        Commands::NearestSpace {
            cells,
            mask,
            out,
            r_max_um,
            r_steps,
            probe_grid_x,
            probe_grid_y,
            simulations,
            seed,
            alpha,
            j_denominator_epsilon,
            memory_budget_mib,
            max_nearest_queries,
            max_csr_draws,
        } => nearest_space::run(NearestSpaceRequest {
            cells,
            mask,
            out,
            r_max_um,
            r_steps,
            probe_grid_x,
            probe_grid_y,
            simulations,
            seed,
            alpha,
            j_denominator_epsilon,
            memory_budget_mib,
            maximum_nearest_queries: max_nearest_queries,
            maximum_csr_draws: max_csr_draws,
        }),
        Commands::Project { command } => match command {
            ProjectCommands::CategoricalPair {
                project,
                cells,
                mask,
                out,
                source_level,
                target_level,
                radii_um,
                permutations,
                seed,
                alpha,
                memory_budget_mib,
                max_pair_visits,
                max_null_pair_evaluations,
            } => categorical_pair::run_project(categorical_pair::Request {
                project,
                cells,
                mask,
                out,
                source_level,
                target_level,
                radii_um,
                permutations,
                seed,
                alpha,
                memory_budget_mib,
                maximum_pair_visits: max_pair_visits,
                maximum_null_pair_evaluations: max_null_pair_evaluations,
            }),
            ProjectCommands::CategoricalCrossPairCorrelation {
                project,
                cells,
                mask,
                out,
                source_level,
                target_level,
                radii_um,
                bandwidth_um,
                permutations,
                seed,
                alpha,
                memory_budget_mib,
                max_pair_visits,
                max_null_pair_evaluations,
            } => categorical_cross_pair_correlation::run_project(
                categorical_cross_pair_correlation::Request {
                    project,
                    cells,
                    mask,
                    out,
                    source_level,
                    target_level,
                    radii_um,
                    bandwidth_um,
                    permutations,
                    seed,
                    alpha,
                    memory_budget_mib,
                    maximum_pair_visits: max_pair_visits,
                    maximum_null_pair_evaluations: max_null_pair_evaluations,
                },
            ),
            ProjectCommands::InhomogeneousSpatial {
                project,
                cells,
                mask,
                out,
                radii_um,
                bandwidth_um,
                grid_x,
                grid_y,
                simulations,
                seed,
                alpha,
                minimum_intensity_per_um2,
                memory_budget_mib,
                max_probes,
                max_intensity_evaluations,
                max_pair_visits,
                max_null_draws,
            } => inhomogeneous_spatial::run_project(inhomogeneous_spatial::Request {
                project,
                cells,
                mask,
                out,
                radii_um,
                bandwidth_um,
                integration_grid: [grid_x, grid_y],
                simulations,
                seed,
                alpha,
                minimum_intensity_per_um2,
                memory_budget_mib,
                maximum_probes: max_probes,
                maximum_intensity_evaluations: max_intensity_evaluations,
                maximum_pair_visits: max_pair_visits,
                maximum_null_draws: max_null_draws,
            }),
            ProjectCommands::TranslationSpatial {
                project,
                cells,
                mask,
                out,
                radii_um,
                simulations,
                seed,
                alpha,
                memory_budget_mib,
                max_pair_visits,
                max_overlap_evaluations,
                max_overlap_candidate_work,
                max_overlap_output_vertices,
                max_csr_draws,
            } => translation_spatial::run_project(translation_spatial::Request {
                project,
                cells,
                mask,
                out,
                radii_um,
                simulations,
                seed,
                alpha,
                memory_budget_mib,
                maximum_pair_visits: max_pair_visits,
                maximum_overlap_evaluations: max_overlap_evaluations,
                maximum_overlap_candidate_work: max_overlap_candidate_work,
                maximum_overlap_output_vertices: max_overlap_output_vertices,
                maximum_csr_draws: max_csr_draws,
            }),
            ProjectCommands::TranslationPairCorrelation {
                project,
                cells,
                mask,
                out,
                radii_um,
                bandwidth_um,
                simulations,
                seed,
                alpha,
                memory_budget_mib,
                max_pair_visits,
                max_overlap_evaluations,
                max_overlap_candidate_work,
                max_overlap_output_vertices,
                max_csr_draws,
            } => translation_pair_correlation::run_project(translation_pair_correlation::Request {
                project,
                cells,
                mask,
                out,
                radii_um,
                bandwidth_um,
                simulations,
                seed,
                alpha,
                memory_budget_mib,
                maximum_pair_visits: max_pair_visits,
                maximum_overlap_evaluations: max_overlap_evaluations,
                maximum_overlap_candidate_work: max_overlap_candidate_work,
                maximum_overlap_output_vertices: max_overlap_output_vertices,
                maximum_csr_draws: max_csr_draws,
            }),
            ProjectCommands::IsotropicSpatial {
                project,
                cells,
                mask,
                out,
                radii_um,
                simulations,
                seed,
                alpha,
                memory_budget_mib,
                max_pair_visits,
                max_visible_arc_evaluations,
                max_arc_segment_tests,
                max_arc_membership_queries,
                max_csr_draws,
            } => isotropic_spatial::run_project(isotropic_spatial::Request {
                project,
                cells,
                mask,
                out,
                radii_um,
                simulations,
                seed,
                alpha,
                memory_budget_mib,
                maximum_pair_visits: max_pair_visits,
                maximum_visible_arc_evaluations: max_visible_arc_evaluations,
                maximum_arc_segment_tests: max_arc_segment_tests,
                maximum_arc_membership_queries: max_arc_membership_queries,
                maximum_csr_draws: max_csr_draws,
            }),
            ProjectCommands::IsotropicPairCorrelation {
                project,
                cells,
                mask,
                out,
                radii_um,
                bandwidth_um,
                simulations,
                seed,
                alpha,
                memory_budget_mib,
                max_pair_visits,
                max_visible_arc_evaluations,
                max_arc_segment_tests,
                max_arc_membership_queries,
                max_csr_draws,
            } => isotropic_pair_correlation::run_project(isotropic_pair_correlation::Request {
                project,
                cells,
                mask,
                out,
                radii_um,
                bandwidth_um,
                simulations,
                seed,
                alpha,
                memory_budget_mib,
                maximum_pair_visits: max_pair_visits,
                maximum_visible_arc_evaluations: max_visible_arc_evaluations,
                maximum_arc_segment_tests: max_arc_segment_tests,
                maximum_arc_membership_queries: max_arc_membership_queries,
                maximum_csr_draws: max_csr_draws,
            }),
            ProjectCommands::GaussianBandwidthSelectedSpatial {
                project,
                cells,
                mask,
                out,
                radii_um,
                candidate_bandwidths_um,
                grid_x,
                grid_y,
                simulations,
                seed,
                alpha,
                minimum_intensity_per_um2,
                memory_budget_mib,
                max_probes,
                max_intensity_evaluations,
                max_pair_visits,
                max_null_draws,
                max_bandwidth_candidates,
                max_selection_intensity_evaluations,
            } => inhomogeneous_bandwidth_selection::run_project(
                inhomogeneous_bandwidth_selection::Request {
                    project,
                    cells,
                    mask,
                    out,
                    radii_um,
                    candidate_bandwidths_um,
                    integration_grid: [grid_x, grid_y],
                    simulations,
                    seed,
                    alpha,
                    minimum_intensity_per_um2,
                    memory_budget_mib,
                    maximum_probes: max_probes,
                    maximum_intensity_evaluations: max_intensity_evaluations,
                    maximum_pair_visits: max_pair_visits,
                    maximum_null_draws: max_null_draws,
                    maximum_bandwidth_candidates: max_bandwidth_candidates,
                    maximum_selection_intensity_evaluations: max_selection_intensity_evaluations,
                },
            ),
            ProjectCommands::PiecewiseCompartmentSpatial {
                project,
                cells,
                observation_mask,
                negative_mask,
                positive_mask,
                negative_compartment_id,
                positive_compartment_id,
                out,
                radii_um,
                simulations,
                seed,
                alpha,
                memory_budget_mib,
                max_boundary_segments,
                max_compartment_queries,
                max_pair_visits,
                max_null_draws,
            } => {
                piecewise_compartment_spatial::run_project(piecewise_compartment_spatial::Request {
                    project,
                    cells,
                    observation_mask,
                    negative_mask,
                    positive_mask,
                    negative_compartment_id,
                    positive_compartment_id,
                    out,
                    radii_um,
                    simulations,
                    seed,
                    alpha,
                    memory_budget_mib,
                    maximum_boundary_segments: max_boundary_segments,
                    maximum_compartment_queries: max_compartment_queries,
                    maximum_pair_visits: max_pair_visits,
                    maximum_null_draws: max_null_draws,
                })
            }
            ProjectCommands::PiecewiseCompartmentPairCorrelation {
                project,
                cells,
                observation_mask,
                negative_mask,
                positive_mask,
                negative_compartment_id,
                positive_compartment_id,
                out,
                radii_um,
                pair_bandwidth_um,
                simulations,
                seed,
                alpha,
                memory_budget_mib,
                max_boundary_segments,
                max_compartment_queries,
                max_pair_visits,
                max_null_draws,
            } => piecewise_compartment_pair_correlation::run_project(
                piecewise_compartment_pair_correlation::Request {
                    project,
                    cells,
                    observation_mask,
                    negative_mask,
                    positive_mask,
                    negative_compartment_id,
                    positive_compartment_id,
                    out,
                    radii_um,
                    pair_bandwidth_um,
                    simulations,
                    seed,
                    alpha,
                    memory_budget_mib,
                    maximum_boundary_segments: max_boundary_segments,
                    maximum_compartment_queries: max_compartment_queries,
                    maximum_pair_visits: max_pair_visits,
                    maximum_null_draws: max_null_draws,
                },
            ),
            ProjectCommands::InhomogeneousPairCorrelation {
                project,
                cells,
                mask,
                out,
                radii_um,
                intensity_bandwidth_um,
                pair_bandwidth_um,
                grid_x,
                grid_y,
                simulations,
                seed,
                alpha,
                minimum_intensity_per_um2,
                memory_budget_mib,
                max_probes,
                max_intensity_evaluations,
                max_pair_visits,
                max_null_draws,
            } => inhomogeneous_pair_correlation::run_project(
                inhomogeneous_pair_correlation::Request {
                    project,
                    cells,
                    mask,
                    out,
                    radii_um,
                    intensity_bandwidth_um,
                    pair_bandwidth_um,
                    integration_grid: [grid_x, grid_y],
                    simulations,
                    seed,
                    alpha,
                    minimum_intensity_per_um2,
                    memory_budget_mib,
                    maximum_probes: max_probes,
                    maximum_intensity_evaluations: max_intensity_evaluations,
                    maximum_pair_visits: max_pair_visits,
                    maximum_null_draws: max_null_draws,
                },
            ),
            ProjectCommands::Classical {
                project,
                cells,
                mask,
                out,
                r_max_um,
                r_steps,
                simulations,
                seed,
                alpha,
                memory_budget_mib,
                max_pair_visits,
                max_csr_draws,
            } => classical::run_project(
                project,
                ClassicalRequest {
                    cells,
                    mask,
                    out,
                    r_max_um,
                    r_steps,
                    simulations,
                    seed,
                    alpha,
                    memory_budget_mib,
                    maximum_pair_visits: max_pair_visits,
                    maximum_csr_draws: max_csr_draws,
                },
            ),
            ProjectCommands::NearestSpace {
                project,
                cells,
                mask,
                out,
                r_max_um,
                r_steps,
                probe_grid_x,
                probe_grid_y,
                simulations,
                seed,
                alpha,
                j_denominator_epsilon,
                memory_budget_mib,
                max_nearest_queries,
                max_csr_draws,
            } => nearest_space::run_project(
                project,
                NearestSpaceRequest {
                    cells,
                    mask,
                    out,
                    r_max_um,
                    r_steps,
                    probe_grid_x,
                    probe_grid_y,
                    simulations,
                    seed,
                    alpha,
                    j_denominator_epsilon,
                    memory_budget_mib,
                    maximum_nearest_queries: max_nearest_queries,
                    maximum_csr_draws: max_csr_draws,
                },
            ),
        },
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
            SimulateCommands::GrowthFront {
                input,
                diffusion_um2_per_time,
                growth_rate_per_time,
                carrying_capacity,
                final_time,
                time_step,
                front_threshold_fraction,
                record_every_steps,
                maximum_cell_steps,
                out,
            } => simulate::growth_front::run(
                input,
                diffusion_um2_per_time,
                growth_rate_per_time,
                carrying_capacity,
                final_time,
                time_step,
                front_threshold_fraction,
                record_every_steps,
                maximum_cell_steps,
                out,
            ),
            SimulateCommands::SpatialCompetition {
                input,
                diffusion_a_um2_per_time,
                diffusion_b_um2_per_time,
                growth_a_per_time,
                growth_b_per_time,
                carrying_a,
                carrying_b,
                competition_a_from_b,
                competition_b_from_a,
                treatment_a_per_time,
                treatment_b_per_time,
                final_time,
                time_step,
                extinction_threshold_fraction,
                record_every_steps,
                maximum_cell_species_steps,
                out,
            } => simulate::spatial_competition::run(
                input,
                diffusion_a_um2_per_time,
                diffusion_b_um2_per_time,
                growth_a_per_time,
                growth_b_per_time,
                carrying_a,
                carrying_b,
                competition_a_from_b,
                competition_b_from_a,
                treatment_a_per_time,
                treatment_b_per_time,
                final_time,
                time_step,
                extinction_threshold_fraction,
                record_every_steps,
                maximum_cell_species_steps,
                out,
            ),
            SimulateCommands::AgentCompetition {
                input,
                seed,
                maximum_events,
                maximum_agents,
                maximum_pair_visits,
                retain_events,
                out,
            } => simulate::agent_competition::run(
                input,
                seed,
                maximum_events,
                maximum_agents,
                maximum_pair_visits,
                retain_events,
                out,
            ),
            SimulateCommands::ReactionDiffusion {
                input,
                final_time,
                time_step,
                record_every_steps,
                maximum_cell_steps,
                out,
            } => simulate::reaction_diffusion::run(
                input,
                final_time,
                time_step,
                record_every_steps,
                maximum_cell_steps,
                out,
            ),
            SimulateCommands::EvolveInterface {
                input,
                final_time,
                time_step,
                reinitialize_every_steps,
                record_every_steps,
                maximum_cell_steps,
                maximum_reinitialization_distance_visits,
                out,
            } => simulate::level_set::run(
                input,
                final_time,
                time_step,
                reinitialize_every_steps,
                record_every_steps,
                maximum_cell_steps,
                maximum_reinitialization_distance_visits,
                out,
            ),
            SimulateCommands::VascularTransport {
                input,
                final_time,
                time_step,
                hypoxia_threshold,
                record_every_steps,
                maximum_cell_steps,
                out,
            } => simulate::vascular_transport::run(
                input,
                final_time,
                time_step,
                hypoxia_threshold,
                record_every_steps,
                maximum_cell_steps,
                out,
            ),
            SimulateCommands::MechanisticTissue { input, out } => {
                simulate::mechanistic_tissue::run(input, out)
            }
            SimulateCommands::SummaryMatching { input, out } => {
                simulate::summary_matching::run(input, out)
            }
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
struct ClassicalRequest {
    cells: PathBuf,
    mask: PathBuf,
    out: PathBuf,
    r_max_um: f64,
    r_steps: usize,
    simulations: usize,
    seed: u64,
    alpha: f64,
    memory_budget_mib: usize,
    maximum_pair_visits: usize,
    maximum_csr_draws: usize,
}

#[derive(Debug)]
struct NearestSpaceRequest {
    cells: PathBuf,
    mask: PathBuf,
    out: PathBuf,
    r_max_um: f64,
    r_steps: usize,
    probe_grid_x: usize,
    probe_grid_y: usize,
    simulations: usize,
    seed: u64,
    alpha: f64,
    j_denominator_epsilon: f64,
    memory_budget_mib: usize,
    maximum_nearest_queries: usize,
    maximum_csr_draws: usize,
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
