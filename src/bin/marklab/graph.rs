use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use crate::cli_error::{into_marklab_error, CliError};
use clap::Subcommand;
use marklab_graph::{
    cellular_complex_workflow, graph_chebyshev_heat_workflow, graph_diffusion_wavelet_workflow,
    graph_group_centrality_workflow, graph_heat_workflow, graph_scattering_workflow,
    graph_sparse_radius_basis_workflow, graph_sparse_radius_diffusion_wavelet_workflow,
    graph_sparse_radius_fourier_energy_workflow, graph_sparse_radius_heat_stability_workflow,
    graph_sparse_radius_heat_workflow, graph_sparse_radius_scattering_workflow,
    graph_spectral_workflow, graph_spectrum_null_test, graph_wavelet_workflow,
    heterogeneous_graph_message_workflow, hypergraph_signal_workflow, simplicial_hodge_workflow,
    typed_triangle_motif_summary_workflow, typed_triangle_motif_workflow,
    validate_graph_mathematics_suite, CellularComplexSpec, GraphChebyshevHeatSpec,
    GraphDiffusionWaveletSpec, GraphGroupCentralitySpec, GraphHeatSpec, GraphScatteringSpec,
    GraphSparseRadiusBasisSpec, GraphSparseRadiusDiffusionWaveletSpec,
    GraphSparseRadiusFourierEnergySpec, GraphSparseRadiusHeatSpec,
    GraphSparseRadiusHeatStabilitySpec, GraphSparseRadiusScatteringSpec, GraphSpectralSpec,
    GraphSpectrumNullSpec, GraphWaveletSpec, HeterogeneousMessageSpec, HypergraphSignalSpec,
    SimplicialHodgeSpec, TypedTriangleMotifSpec,
};
use serde::Serialize;

const MAXIMUM_INPUT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Subcommand)]
pub(crate) enum GraphCommand {
    /// Compute graph eigenmodes and spectral summaries of a signal.
    Spectral {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Diffuse a signal over a graph with the heat operator.
    Heat {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Analyze graph signals across spectral wavelet scales.
    Wavelet {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Compare graph-spectrum summaries with a declared null model.
    SpectrumNull {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Approximate graph heat diffusion with Chebyshev polynomials.
    ChebyshevHeat {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Diffuse a signal on a sparse radius-neighbor graph.
    SparseRadiusHeat {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Construct a spectral basis on a sparse radius-neighbor graph.
    SparseRadiusBasis {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Measure signal energy in a sparse radius-graph spectral basis.
    SparseRadiusFourierEnergy {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Assess heat-diffusion sensitivity to graph perturbations.
    SparseRadiusHeatStability {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Compute diffusion-wavelet features on a sparse radius graph.
    SparseRadiusDiffusionWavelet {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Compute multiscale scattering features on a sparse radius graph.
    SparseRadiusScattering {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Compute categorical group centralities on a sparse physical-radius graph.
    GroupCentrality {
        /// JSON specification containing grouped nodes and resource limits.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the compact centrality result.
        #[arg(long)]
        out: PathBuf,
    },
    /// Compute multiscale graph features with diffusion wavelets.
    DiffusionWavelet {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Compute graph scattering features from cascaded wavelet transforms.
    Scattering {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Propagate signals across a graph with heterogeneous edge types.
    HeterogeneousMessage {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Analyze signals using multi-node hypergraph relations.
    Hypergraph {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Analyze typed triangle motifs in a graph.
    MotifTriangle {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Summarize typed triangle motifs in a graph.
    MotifTriangleSummary {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Decompose simplicial edge signals into gradient, curl and harmonic parts.
    Hodge {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Analyze signals on a declared cellular complex.
    CellularComplex {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Run graph mathematical oracles and report the available validation evidence.
    Validate {
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
}

pub(crate) fn run_cli(command: GraphCommand) -> Result<(), CliError> {
    match command {
        GraphCommand::Spectral { input, out } => run(input, out),
        GraphCommand::Heat { input, out } => run_heat(input, out),
        GraphCommand::Wavelet { input, out } => run_wavelet(input, out),
        GraphCommand::SpectrumNull { input, out } => run_spectrum_null(input, out),
        GraphCommand::ChebyshevHeat { input, out } => run_chebyshev_heat(input, out),
        GraphCommand::SparseRadiusHeat { input, out } => run_sparse_radius_heat(input, out),
        GraphCommand::SparseRadiusBasis { input, out } => run_sparse_radius_basis(input, out),
        GraphCommand::SparseRadiusFourierEnergy { input, out } => {
            run_sparse_radius_fourier_energy(input, out)
        }
        GraphCommand::SparseRadiusHeatStability { input, out } => {
            run_sparse_radius_heat_stability(input, out)
        }
        GraphCommand::SparseRadiusDiffusionWavelet { input, out } => {
            run_sparse_radius_diffusion_wavelet(input, out)
        }
        GraphCommand::SparseRadiusScattering { input, out } => {
            run_sparse_radius_scattering(input, out)
        }
        GraphCommand::GroupCentrality { input, out } => run_group_centrality(input, out),
        GraphCommand::DiffusionWavelet { input, out } => run_diffusion_wavelet(input, out),
        GraphCommand::Scattering { input, out } => run_scattering(input, out),
        GraphCommand::HeterogeneousMessage { input, out } => run_heterogeneous(input, out),
        GraphCommand::Hypergraph { input, out } => run_hypergraph(input, out),
        GraphCommand::MotifTriangle { input, out } => run_motif(input, out),
        GraphCommand::MotifTriangleSummary { input, out } => run_motif_summary(input, out),
        GraphCommand::Hodge { input, out } => run_hodge(input, out),
        GraphCommand::CellularComplex { input, out } => run_cellular_complex(input, out),
        GraphCommand::Validate { out } => run_validation(out),
    }
}

fn run_validation(out: PathBuf) -> Result<(), CliError> {
    let result =
        validate_graph_mathematics_suite().map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_cellular_complex(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(&input)?;
    let spec: CellularComplexSpec = serde_json::from_slice(&bytes)?;
    let result =
        cellular_complex_workflow(spec).map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_hodge(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(&input)?;
    let spec: SimplicialHodgeSpec = serde_json::from_slice(&bytes)?;
    let result =
        simplicial_hodge_workflow(spec).map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_motif(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(&input)?;
    let spec: TypedTriangleMotifSpec = serde_json::from_slice(&bytes)?;
    let result =
        typed_triangle_motif_workflow(spec).map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_motif_summary(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(&input)?;
    let spec: TypedTriangleMotifSpec = serde_json::from_slice(&bytes)?;
    let result = typed_triangle_motif_summary_workflow(spec)
        .map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_hypergraph(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(&input)?;
    let spec: HypergraphSignalSpec = serde_json::from_slice(&bytes)?;
    let result =
        hypergraph_signal_workflow(spec).map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_heterogeneous(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(&input)?;
    let spec: HeterogeneousMessageSpec = serde_json::from_slice(&bytes)?;
    let result = heterogeneous_graph_message_workflow(spec)
        .map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_scattering(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(&input)?;
    let spec: GraphScatteringSpec = serde_json::from_slice(&bytes)?;
    let result =
        graph_scattering_workflow(spec).map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_diffusion_wavelet(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(&input)?;
    let spec: GraphDiffusionWaveletSpec = serde_json::from_slice(&bytes)?;
    let result = graph_diffusion_wavelet_workflow(spec)
        .map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_chebyshev_heat(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(&input)?;
    let spec: GraphChebyshevHeatSpec = serde_json::from_slice(&bytes)?;
    let result =
        graph_chebyshev_heat_workflow(spec).map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_sparse_radius_heat(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(&input)?;
    let spec: GraphSparseRadiusHeatSpec = serde_json::from_slice(&bytes)?;
    let result = graph_sparse_radius_heat_workflow(spec)
        .map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_sparse_radius_basis(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(&input)?;
    let spec: GraphSparseRadiusBasisSpec = serde_json::from_slice(&bytes)?;
    let result = graph_sparse_radius_basis_workflow(spec)
        .map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_sparse_radius_fourier_energy(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(&input)?;
    let spec: GraphSparseRadiusFourierEnergySpec = serde_json::from_slice(&bytes)?;
    let result = graph_sparse_radius_fourier_energy_workflow(spec)
        .map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_sparse_radius_heat_stability(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(&input)?;
    let spec: GraphSparseRadiusHeatStabilitySpec = serde_json::from_slice(&bytes)?;
    let result = graph_sparse_radius_heat_stability_workflow(spec)
        .map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_sparse_radius_diffusion_wavelet(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(&input)?;
    let spec: GraphSparseRadiusDiffusionWaveletSpec = serde_json::from_slice(&bytes)?;
    let result = graph_sparse_radius_diffusion_wavelet_workflow(spec)
        .map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_sparse_radius_scattering(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(&input)?;
    let spec: GraphSparseRadiusScatteringSpec = serde_json::from_slice(&bytes)?;
    let result = graph_sparse_radius_scattering_workflow(spec)
        .map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_group_centrality(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(&input)?;
    let spec: GraphGroupCentralitySpec = serde_json::from_slice(&bytes)?;
    let result = graph_group_centrality_workflow(spec)
        .map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_spectrum_null(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(&input)?;
    let spec: GraphSpectrumNullSpec = serde_json::from_slice(&bytes)?;
    let result =
        graph_spectrum_null_test(spec).map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_wavelet(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(&input)?;
    let spec: GraphWaveletSpec = serde_json::from_slice(&bytes)?;
    let result =
        graph_wavelet_workflow(spec).map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_heat(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(&input)?;
    let spec: GraphHeatSpec = serde_json::from_slice(&bytes)?;
    let result = graph_heat_workflow(spec).map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(&input)?;
    let spec: GraphSpectralSpec = serde_json::from_slice(&bytes)?;
    let result =
        graph_spectral_workflow(spec).map_err(|error| CliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn read_input(input: &Path) -> Result<Vec<u8>, CliError> {
    crate::input_file::read_regular_file(
        input,
        MAXIMUM_INPUT_BYTES,
        "input must be a regular file within 16 MiB",
    )
}

fn publish_json(path: &Path, result: &impl Serialize) -> Result<(), CliError> {
    if fs::symlink_metadata(path).is_ok() {
        return Err(CliError::Input(format!(
            "output already exists: {}",
            path.display()
        )));
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|source| CliError::Io {
        path: parent.to_owned(),
        source,
    })?;
    let file_name = path
        .file_name()
        .ok_or_else(|| CliError::Input("output must name a file".into()))?;
    let mut staging_name = OsString::from(".");
    staging_name.push(file_name);
    staging_name.push(format!(".marklab-graph-{}.tmp", std::process::id()));
    let staging = parent.join(staging_name);
    let bytes = serde_json::to_vec_pretty(result)?;
    let publication = (|| -> Result<(), CliError> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)
            .map_err(|source| CliError::Io {
                path: staging.clone(),
                source,
            })?;
        file.write_all(&bytes)
            .and_then(|()| file.write_all(b"\n"))
            .and_then(|()| file.sync_all())
            .map_err(|source| CliError::Io {
                path: staging.clone(),
                source,
            })?;
        fs::rename(&staging, path).map_err(|source| CliError::Io {
            path: path.to_owned(),
            source,
        })
    })();
    if publication.is_err() {
        let _ = fs::remove_file(&staging);
    }
    publication
}

pub(crate) fn execute(command: GraphCommand) -> marklab::Result<()> {
    run_cli(command).map_err(into_marklab_error)
}
