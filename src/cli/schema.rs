use super::*;

mod dispatch;

pub(crate) use dispatch::{run, run_multimodal, run_project};

/// Top-level library commands. `project` and `multimodal` subcommands are separate enums
/// so the executable can combine them with its own subcommands of the same name.
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Print a whole-slide image's metadata as JSON.
    #[cfg(feature = "wsi")]
    InspectSlide {
        /// Whole-slide image to inspect or extract from.
        slide: PathBuf,
        /// Destination for the generated output.
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Extract one region of a whole-slide image as RGBA pixels.
    #[cfg(feature = "wsi")]
    ExtractRegion(slide::ExtractRequest),
    /// Run the marked-pattern analysis on one tissue section.
    Analyze(AnalyzeRequest),
    /// Test whether cells are clustered or regularly spaced (Ripley's K and L against CSR simulations).
    Classical(ClassicalRequest),
    /// Nearest-neighbour (G), empty-space (F) and J functions against CSR simulations.
    NearestSpace(NearestSpaceRequest),
    /// Run analyze for every row of a manifest.
    Batch {
        /// Manifest with one row per analysis.
        #[arg(long)]
        manifest: PathBuf,
        /// Analysis configuration (TOML).
        #[arg(long)]
        config: PathBuf,
        /// Output directory.
        #[arg(long)]
        out: PathBuf,
        /// Number of worker threads (default: all available cores).
        #[arg(long)]
        threads: Option<usize>,
    },
    /// Compare two finished analyses, such as before and after treatment.
    Prepost {
        /// Output directory of the earlier analysis.
        #[arg(long)]
        pre: PathBuf,
        /// Output directory of the later analysis.
        #[arg(long)]
        post: PathBuf,
        /// Output directory.
        #[arg(long)]
        out: PathBuf,
    },
    /// Write the profiling commands for a benchmark workload.
    ProfilePlan {
        /// Benchmark workload whose profiling commands should be generated.
        #[arg(long)]
        workload: String,
        /// File to write the profiling plan to.
        #[arg(long)]
        out: PathBuf,
    },
    /// Simulate synthetic patterns and tissue models.
    Simulate {
        #[command(subcommand)]
        command: SimulateCommands,
    },
    /// Run the synthetic or multimodal smoke checks.
    Smoke {
        /// Synthetic validation suite to execute.
        #[arg(long)]
        suite: String,
        /// Number of simulated or resampled datasets used by the analysis.
        #[arg(long)]
        replicates: usize,
        /// Output directory.
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub enum SimulateCommands {
    /// Write a row of cells with a fixed fraction marked at random.
    RandomLabeling {
        /// Number of cells in the synthetic pattern.
        #[arg(long)]
        n: usize,
        /// Probability that a synthetic cell receives the positive mark.
        #[arg(long)]
        p: f64,
        /// Random seed; the same seed gives the same result.
        #[arg(long, default_value_t = 123_456_789)]
        seed: u64,
        /// CSV or Parquet file to write.
        #[arg(long)]
        out: PathBuf,
    },
    /// Simulate a one-dimensional Fisher-KPP growth front.
    GrowthFront(simulate::growth_front::GrowthFrontArgs),
    /// Simulate two-species Lotka-Volterra competition in one dimension.
    SpatialCompetition(simulate::spatial_competition::SpatialCompetitionArgs),
    /// Simulate two competing cell species as an exact stochastic agent process.
    AgentCompetition(simulate::agent_competition::AgentCompetitionArgs),
    /// Simulate a periodic two-dimensional reaction-diffusion field.
    ReactionDiffusion(simulate::reaction_diffusion::ReactionDiffusionArgs),
    /// Evolve a tissue interface with a two-dimensional level-set method.
    EvolveInterface(simulate::level_set::EvolveInterfaceArgs),
    /// Simulate transport from vessels through a static two-dimensional flow field.
    VascularTransport(simulate::vascular_transport::VascularTransportArgs),
    /// Simulate the coupled mechanistic tissue model.
    MechanisticTissue {
        /// JSON grid, initial density and phi fields, plus vascular, agents, coupling and numerics.
        #[arg(long)]
        input: PathBuf,
        /// File to write the result to.
        #[arg(long)]
        out: PathBuf,
    },
    /// Score simulated against observed summary statistics.
    SummaryMatching {
        /// JSON SummaryMatchingSpec: window, observed and generated points, radii and weights.
        #[arg(long)]
        input: PathBuf,
        /// File to write the result to.
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub enum ProjectCommands {
    /// Count cell pairs between two compartment levels against label permutations.
    CategoricalPair(categorical_pair::Request),
    /// Directed cross-type pair correlation between two compartment levels.
    CategoricalCrossPairCorrelation(categorical_cross_pair_correlation::Request),
    /// Cross-type pair correlation with translation edge correction.
    TranslationCategoricalCrossPairCorrelation(
        categorical_cross_pair_correlation::TranslationRequest,
    ),
    /// Cross-type pair correlation with isotropic edge correction.
    IsotropicCategoricalCrossPairCorrelation(categorical_cross_pair_correlation::IsotropicRequest),
    /// Intensity-reweighted (inhomogeneous) K and L functions.
    InhomogeneousSpatial(inhomogeneous_spatial::Request),
    /// K and L functions with translation edge correction.
    TranslationSpatial(translation_spatial::Request),
    /// Pair correlation with translation edge correction.
    TranslationPairCorrelation(translation_pair_correlation::Request),
    /// K and L functions with isotropic edge correction.
    IsotropicSpatial(isotropic_spatial::Request),
    /// Pair correlation with isotropic edge correction.
    IsotropicPairCorrelation(isotropic_pair_correlation::Request),
    /// Inhomogeneous K and L after choosing the Gaussian intensity bandwidth from prespecified candidates.
    GaussianBandwidthSelectedSpatial(inhomogeneous_bandwidth_selection::Request),
    /// Inhomogeneous K and L with a separate intensity in each compartment.
    PiecewiseCompartmentSpatial(piecewise_compartment_spatial::Request),
    /// Inhomogeneous pair correlation with a separate intensity in each compartment.
    PiecewiseCompartmentPairCorrelation(piecewise_compartment_pair_correlation::Request),
    /// Intensity-reweighted (inhomogeneous) pair correlation.
    InhomogeneousPairCorrelation(inhomogeneous_pair_correlation::Request),
    /// Intensity-reweighted cross-type pair correlation between two compartment levels.
    InhomogeneousCategoricalCrossPairCorrelation(
        inhomogeneous_categorical_cross_pair_correlation::Request,
    ),
    /// Semivariogram of a scalar mark with a random-labeling test.
    ScalarVariogram(scalar_variogram::Request),
    /// Classical K and L workflow, cached in a durable project.
    Classical(ProjectClassicalArgs),
    /// Nearest-neighbour, empty-space and J workflow, cached in a durable project.
    NearestSpace(ProjectNearestSpaceArgs),
}

/// `--project` plus the arguments of the uncached command.
#[derive(clap::Args, Debug)]
pub struct ProjectNearestSpaceArgs {
    /// Durable project directory that caches results.
    #[arg(long)]
    pub(super) project: PathBuf,
    #[command(flatten)]
    pub(super) request: NearestSpaceRequest,
}

/// `--project` plus the arguments of the uncached command.
#[derive(clap::Args, Debug)]
pub struct ProjectClassicalArgs {
    /// Durable project directory that caches results.
    #[arg(long)]
    pub(super) project: PathBuf,
    #[command(flatten)]
    pub(super) request: ClassicalRequest,
}

#[derive(Debug, Subcommand)]
pub enum MultimodalCommands {
    /// Align H&E and IHC sections, match their cells and run the multimodal analysis.
    Analyze(MultimodalAnalyzeRequest),
    /// Compare two finished multimodal analyses.
    Prepost {
        /// Output directory of the earlier analysis.
        #[arg(long)]
        pre: PathBuf,
        /// Output directory of the later analysis.
        #[arg(long)]
        post: PathBuf,
        /// Output directory.
        #[arg(long)]
        out: PathBuf,
    },
    /// Run multimodal analyze for every row of a manifest.
    Batch {
        /// Manifest with one row per analysis.
        #[arg(long)]
        manifest: PathBuf,
        /// Output directory.
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum HeInputFormat {
    HeCsv,
    CellvitCsv,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum CliInhomogeneousCrossEdgeCorrection {
    StandardBorder,
    Translation,
    Isotropic,
}
