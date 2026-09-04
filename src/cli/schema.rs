use super::*;

#[path = "schema/dispatch.rs"]
mod dispatch;

pub(super) fn run_cli() -> Result<()> {
    dispatch::run_cli()
}

pub(super) fn command() -> clap::Command {
    <Cli as clap::CommandFactory>::command()
}

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
    TranslationCategoricalCrossPairCorrelation {
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
        #[arg(long)]
        max_overlap_evaluations: usize,
        #[arg(long)]
        max_overlap_candidate_work: usize,
        #[arg(long)]
        max_overlap_output_vertices: usize,
    },
    IsotropicCategoricalCrossPairCorrelation {
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
        #[arg(long)]
        max_visible_arc_evaluations: usize,
        #[arg(long)]
        max_arc_segment_tests: usize,
        #[arg(long)]
        max_arc_membership_queries: usize,
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
        cross_fit_folds: Option<usize>,
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
    InhomogeneousCategoricalCrossPairCorrelation {
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
        intensity_bandwidth_um: f64,
        #[arg(long)]
        cross_fit_folds: Option<usize>,
        #[arg(long, value_enum, default_value_t = CliInhomogeneousCrossEdgeCorrection::StandardBorder)]
        edge_correction: CliInhomogeneousCrossEdgeCorrection,
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
        #[arg(long)]
        max_overlap_evaluations: Option<usize>,
        #[arg(long)]
        max_overlap_candidate_work: Option<usize>,
        #[arg(long)]
        max_overlap_output_vertices: Option<usize>,
        #[arg(long)]
        max_visible_arc_evaluations: Option<usize>,
        #[arg(long)]
        max_arc_segment_tests: Option<usize>,
        #[arg(long)]
        max_arc_membership_queries: Option<usize>,
    },
    ScalarVariogram {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        cells: PathBuf,
        #[arg(long)]
        mask: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, value_delimiter = ',')]
        lag_edges_um: Vec<f64>,
        #[arg(long)]
        condition_by_histologic_compartment: bool,
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
        max_permutation_pair_evaluations: usize,
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
pub(super) enum HeInputFormat {
    HeCsv,
    CellvitCsv,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(super) enum CliInhomogeneousCrossEdgeCorrection {
    StandardBorder,
    Translation,
    Isotropic,
}
