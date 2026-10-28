use clap::{Parser, Subcommand};

use super::{
    backend, bayes, bayes_advanced, causal, causal_model, cohort, graph, longitudinal,
    multimodal_model, neural, numerics, pathology, policy, project, registration, space, spatial3d,
    spatial3d_model, study, topology,
};

/// Every command: the library's own commands plus the families implemented here.
#[derive(Parser)]
#[command(
    name = "marklab",
    version,
    about = "Spatial statistics for marked cell patterns in pathology"
)]
struct Cli {
    #[command(subcommand)]
    command: BinaryCommand,
}

#[derive(Subcommand)]
enum BinaryCommand {
    #[command(flatten)]
    Library(marklab::cli::Commands),
    /// Inspect the installed scientific backend environment.
    Backend {
        #[command(subcommand)]
        command: backend::BackendCommand,
    },
    /// Bayesian models with SBC, backend-agreement and prior-sensitivity checks.
    Bayes {
        #[command(subcommand)]
        command: BayesCommand,
    },
    /// Causal designs under interference, sensitivity analyses and bounds.
    Causal {
        #[command(subcommand)]
        command: CausalCommand,
    },
    /// Patient-level permutation, max-T, equivalence and distance tests.
    Cohort {
        #[command(subcommand)]
        command: cohort::CohortCommand,
    },
    /// Graph signal analysis on cell neighbour graphs.
    Graph {
        #[command(subcommand)]
        command: graph::GraphCommand,
    },
    /// Smoothing and filtering of repeated measurements over time.
    Longitudinal {
        #[command(subcommand)]
        command: longitudinal::LongitudinalCommand,
    },
    /// H&E/IHC multimodal analysis and joint factor models.
    Multimodal {
        #[command(subcommand)]
        command: MultimodalCommand,
    },
    /// Neural point-process models, generators and simulation-based inference.
    Neural {
        #[command(subcommand)]
        command: neural::NeuralCommand,
    },
    /// Numerically stable primitives and local multivariate Moran.
    Numerics {
        #[command(subcommand)]
        command: numerics::NumericsCommand,
    },
    /// H&E/IHC annotation profiles and within-slide spatial maps.
    Pathology {
        #[command(subcommand)]
        command: pathology::PathologyCommand,
    },
    /// Result maturity, execution mode and validation-ladder decisions.
    Policy {
        #[command(subcommand)]
        command: policy::PolicyCommand,
    },
    /// Analyses cached in a durable project directory.
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    /// Non-rigid and probabilistic registration between sections.
    Registration {
        #[command(subcommand)]
        command: registration::RegistrationCommand,
    },
    /// SPACE: rank multi-way co-patterning of cell types and scalars (cisMI).
    Space {
        #[command(subcommand)]
        command: space::SpaceCommand,
    },
    /// Three-dimensional K functions and serial-section analyses.
    Spatial3d {
        #[command(subcommand)]
        command: Spatial3dCommand,
    },
    /// Analyze a declared multiplex panel across slides and independent patients.
    Study {
        #[command(subcommand)]
        command: study::StudyCommand,
    },
    /// Persistent homology and raster morphology of cell patterns.
    Topology {
        #[command(subcommand)]
        command: topology::TopologyCommand,
    },
}

#[derive(Subcommand)]
enum ProjectCommand {
    #[command(flatten)]
    Library(marklab::cli::ProjectCommands),
    #[command(flatten)]
    Durable(project::ProjectCommand),
}

#[derive(Subcommand)]
enum MultimodalCommand {
    #[command(flatten)]
    Library(marklab::cli::MultimodalCommands),
    #[command(flatten)]
    Models(multimodal_model::MultimodalModelCommand),
}

#[derive(Subcommand)]
enum BayesCommand {
    #[command(flatten)]
    Models(Box<bayes::BayesCommand>),
    #[command(flatten)]
    Advanced(bayes_advanced::AdvancedBayesCommand),
}

#[derive(Subcommand)]
enum CausalCommand {
    #[command(flatten)]
    Design(causal::CausalCommand),
    #[command(flatten)]
    Model(causal_model::CausalModelCommand),
}

#[derive(Subcommand)]
enum Spatial3dCommand {
    #[command(flatten)]
    Statistics(spatial3d::Spatial3dCommand),
    #[command(flatten)]
    Model(spatial3d_model::Spatial3dModelCommand),
}

fn execute(command: BinaryCommand) -> marklab::Result<()> {
    match command {
        BinaryCommand::Library(command) => marklab::cli::run(command),
        BinaryCommand::Backend { command } => backend::execute(command),
        BinaryCommand::Bayes { command } => match command {
            BayesCommand::Models(command) => bayes::execute(*command),
            BayesCommand::Advanced(command) => bayes_advanced::execute_command(command),
        },
        BinaryCommand::Causal { command } => match command {
            CausalCommand::Design(command) => causal::execute(command),
            CausalCommand::Model(command) => causal_model::execute(command),
        },
        BinaryCommand::Cohort { command } => cohort::execute(command),
        BinaryCommand::Graph { command } => graph::execute(command),
        BinaryCommand::Longitudinal { command } => longitudinal::execute(command),
        BinaryCommand::Multimodal { command } => match command {
            MultimodalCommand::Library(command) => marklab::cli::run_multimodal(command),
            MultimodalCommand::Models(command) => multimodal_model::execute(command),
        },
        BinaryCommand::Neural { command } => neural::execute(command),
        BinaryCommand::Numerics { command } => numerics::execute(command),
        BinaryCommand::Pathology { command } => pathology::run(command),
        BinaryCommand::Policy { command } => policy::execute(command),
        BinaryCommand::Project { command } => match command {
            ProjectCommand::Library(command) => marklab::cli::run_project(command),
            ProjectCommand::Durable(command) => project::execute(command),
        },
        BinaryCommand::Registration { command } => registration::execute(command),
        BinaryCommand::Space { command } => space::run(command),
        BinaryCommand::Spatial3d { command } => match command {
            Spatial3dCommand::Statistics(command) => spatial3d::execute(command),
            Spatial3dCommand::Model(command) => spatial3d_model::execute(command),
        },
        BinaryCommand::Study { command } => study::run(command),
        BinaryCommand::Topology { command } => topology::execute(command),
    }
}

pub(crate) fn run() -> marklab::Result<()> {
    execute(Cli::parse().command)
}
