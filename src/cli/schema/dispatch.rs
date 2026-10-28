use super::*;

pub(crate) fn run(command: Commands) -> Result<()> {
    match command {
        #[cfg(feature = "wsi")]
        Commands::InspectSlide { slide, output } => slide::inspect(slide, output),
        #[cfg(feature = "wsi")]
        Commands::ExtractRegion(request) => slide::extract(request),
        Commands::Analyze(request) => analyze::run(request),
        Commands::Classical(request) => classical::run(request),
        Commands::NearestSpace(request) => nearest_space::run(request),

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
            SimulateCommands::GrowthFront(args) => simulate::growth_front::run(args),
            SimulateCommands::SpatialCompetition(args) => simulate::spatial_competition::run(args),
            SimulateCommands::AgentCompetition(args) => simulate::agent_competition::run(args),
            SimulateCommands::ReactionDiffusion(args) => simulate::reaction_diffusion::run(args),
            SimulateCommands::PhaseField(args) => simulate::phase_field::run(args),
            SimulateCommands::EvolveInterface(args) => simulate::level_set::run(args),
            SimulateCommands::VascularTransport(args) => simulate::vascular_transport::run(args),
            SimulateCommands::MechanisticTissue { input, out } => {
                simulate::mechanistic_tissue::run(input, out)
            }
            SimulateCommands::SummaryMatching { input, out } => {
                simulate::summary_matching::run(input, out)
            }
        },
        Commands::Microbio { command } => match command {
            MicrobioCommands::Preprocess(args) => microbio::run_preprocess(args),
            MicrobioCommands::FeatureTable(args) => microbio::run_feature_table(args),
            MicrobioCommands::FitAst(args) => microbio::run_fit_ast(args),
            MicrobioCommands::PredictAst(args) => microbio::run_predict_ast(args),
        },

        Commands::Smoke {
            suite,
            replicates,
            out,
        } => smoke::run(&suite, replicates, out),
    }
}

pub(crate) fn run_project(command: ProjectCommands) -> Result<()> {
    match command {
        ProjectCommands::CategoricalPair(request) => categorical_pair::run_project(request),
        ProjectCommands::CategoricalCrossPairCorrelation(request) => {
            categorical_cross_pair_correlation::run_project(request)
        }
        ProjectCommands::TranslationCategoricalCrossPairCorrelation(request) => {
            categorical_cross_pair_correlation::run_translation_project(request)
        }
        ProjectCommands::IsotropicCategoricalCrossPairCorrelation(request) => {
            categorical_cross_pair_correlation::run_isotropic_project(request)
        }
        ProjectCommands::InhomogeneousSpatial(request) => {
            inhomogeneous_spatial::run_project(request)
        }
        ProjectCommands::TranslationSpatial(request) => translation_spatial::run_project(request),
        ProjectCommands::TranslationPairCorrelation(request) => {
            translation_pair_correlation::run_project(request)
        }
        ProjectCommands::IsotropicSpatial(request) => isotropic_spatial::run_project(request),
        ProjectCommands::IsotropicPairCorrelation(request) => {
            isotropic_pair_correlation::run_project(request)
        }
        ProjectCommands::GaussianBandwidthSelectedSpatial(request) => {
            inhomogeneous_bandwidth_selection::run_project(request)
        }
        ProjectCommands::PiecewiseCompartmentSpatial(request) => {
            piecewise_compartment_spatial::run_project(request)
        }
        ProjectCommands::PiecewiseCompartmentPairCorrelation(request) => {
            piecewise_compartment_pair_correlation::run_project(request)
        }
        ProjectCommands::InhomogeneousPairCorrelation(request) => {
            inhomogeneous_pair_correlation::run_project(request)
        }
        ProjectCommands::InhomogeneousCategoricalCrossPairCorrelation(request) => {
            inhomogeneous_categorical_cross_pair_correlation::run_project(request)
        }
        ProjectCommands::ScalarVariogram(request) => scalar_variogram::run_project(request),
        ProjectCommands::Classical(args) => classical::run_project(args.project, args.request),
        ProjectCommands::NearestSpace(args) => {
            nearest_space::run_project(args.project, args.request)
        }
    }
}

pub(crate) fn run_multimodal(command: MultimodalCommands) -> Result<()> {
    match command {
        MultimodalCommands::Analyze(request) => multimodal::analyze::run(request),
        MultimodalCommands::Prepost { pre, post, out } => {
            multimodal::commands::prepost(pre, post, out)
        }
        MultimodalCommands::Batch { manifest, out } => multimodal::commands::batch(manifest, out),
    }
}
