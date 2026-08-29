use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};
use marklab_graph::{
    cellular_complex_workflow, graph_chebyshev_heat_workflow, graph_diffusion_wavelet_workflow,
    graph_heat_workflow, graph_scattering_workflow, graph_sparse_radius_basis_workflow,
    graph_sparse_radius_diffusion_wavelet_workflow, graph_sparse_radius_heat_stability_workflow,
    graph_sparse_radius_heat_workflow, graph_sparse_radius_scattering_workflow,
    graph_spectral_workflow, graph_spectrum_null_test, graph_wavelet_workflow,
    heterogeneous_graph_message_workflow, hypergraph_signal_workflow, simplicial_hodge_workflow,
    typed_triangle_motif_workflow, validate_graph_mathematics_suite, CellularComplexSpec,
    GraphChebyshevHeatSpec, GraphDiffusionWaveletSpec, GraphHeatSpec, GraphScatteringSpec,
    GraphSparseRadiusBasisSpec, GraphSparseRadiusDiffusionWaveletSpec, GraphSparseRadiusHeatSpec,
    GraphSparseRadiusHeatStabilitySpec, GraphSparseRadiusScatteringSpec, GraphSpectralSpec,
    GraphSpectrumNullSpec, GraphWaveletSpec, HeterogeneousMessageSpec, HypergraphSignalSpec,
    SimplicialHodgeSpec, TypedTriangleMotifSpec,
};
use serde::Serialize;
use thiserror::Error;

const MAXIMUM_INPUT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct GraphCli {
    #[command(subcommand)]
    command: GraphTopLevel,
}

#[derive(Debug, Subcommand)]
enum GraphTopLevel {
    Graph {
        #[command(subcommand)]
        command: GraphCommand,
    },
}

#[derive(Debug, Subcommand)]
enum GraphCommand {
    Spectral {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Heat {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Wavelet {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    SpectrumNull {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    ChebyshevHeat {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    SparseRadiusHeat {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    SparseRadiusBasis {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    SparseRadiusHeatStability {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    SparseRadiusDiffusionWavelet {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    SparseRadiusScattering {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    DiffusionWavelet {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Scattering {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    HeterogeneousMessage {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Hypergraph {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    MotifTriangle {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Hodge {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    CellularComplex {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Validate {
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Debug, Error)]
pub(crate) enum GraphCliError {
    #[error("invalid graph input: {0}")]
    Input(String),
    #[error("failed to access {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid graph JSON: {0}")]
    Json(#[from] serde_json::Error),
}

pub(crate) fn run_cli() -> Result<(), GraphCliError> {
    match GraphCli::parse().command {
        GraphTopLevel::Graph {
            command: GraphCommand::Spectral { input, out },
        } => run(input, out),
        GraphTopLevel::Graph {
            command: GraphCommand::Heat { input, out },
        } => run_heat(input, out),
        GraphTopLevel::Graph {
            command: GraphCommand::Wavelet { input, out },
        } => run_wavelet(input, out),
        GraphTopLevel::Graph {
            command: GraphCommand::SpectrumNull { input, out },
        } => run_spectrum_null(input, out),
        GraphTopLevel::Graph {
            command: GraphCommand::ChebyshevHeat { input, out },
        } => run_chebyshev_heat(input, out),
        GraphTopLevel::Graph {
            command: GraphCommand::SparseRadiusHeat { input, out },
        } => run_sparse_radius_heat(input, out),
        GraphTopLevel::Graph {
            command: GraphCommand::SparseRadiusBasis { input, out },
        } => run_sparse_radius_basis(input, out),
        GraphTopLevel::Graph {
            command: GraphCommand::SparseRadiusHeatStability { input, out },
        } => run_sparse_radius_heat_stability(input, out),
        GraphTopLevel::Graph {
            command: GraphCommand::SparseRadiusDiffusionWavelet { input, out },
        } => run_sparse_radius_diffusion_wavelet(input, out),
        GraphTopLevel::Graph {
            command: GraphCommand::SparseRadiusScattering { input, out },
        } => run_sparse_radius_scattering(input, out),
        GraphTopLevel::Graph {
            command: GraphCommand::DiffusionWavelet { input, out },
        } => run_diffusion_wavelet(input, out),
        GraphTopLevel::Graph {
            command: GraphCommand::Scattering { input, out },
        } => run_scattering(input, out),
        GraphTopLevel::Graph {
            command: GraphCommand::HeterogeneousMessage { input, out },
        } => run_heterogeneous(input, out),
        GraphTopLevel::Graph {
            command: GraphCommand::Hypergraph { input, out },
        } => run_hypergraph(input, out),
        GraphTopLevel::Graph {
            command: GraphCommand::MotifTriangle { input, out },
        } => run_motif(input, out),
        GraphTopLevel::Graph {
            command: GraphCommand::Hodge { input, out },
        } => run_hodge(input, out),
        GraphTopLevel::Graph {
            command: GraphCommand::CellularComplex { input, out },
        } => run_cellular_complex(input, out),
        GraphTopLevel::Graph {
            command: GraphCommand::Validate { out },
        } => run_validation(out),
    }
}

fn run_validation(out: PathBuf) -> Result<(), GraphCliError> {
    let result = validate_graph_mathematics_suite()
        .map_err(|error| GraphCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_cellular_complex(input: PathBuf, out: PathBuf) -> Result<(), GraphCliError> {
    let bytes = read_input(&input)?;
    let spec: CellularComplexSpec = serde_json::from_slice(&bytes)?;
    let result =
        cellular_complex_workflow(spec).map_err(|error| GraphCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_hodge(input: PathBuf, out: PathBuf) -> Result<(), GraphCliError> {
    let bytes = read_input(&input)?;
    let spec: SimplicialHodgeSpec = serde_json::from_slice(&bytes)?;
    let result =
        simplicial_hodge_workflow(spec).map_err(|error| GraphCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_motif(input: PathBuf, out: PathBuf) -> Result<(), GraphCliError> {
    let bytes = read_input(&input)?;
    let spec: TypedTriangleMotifSpec = serde_json::from_slice(&bytes)?;
    let result = typed_triangle_motif_workflow(spec)
        .map_err(|error| GraphCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_hypergraph(input: PathBuf, out: PathBuf) -> Result<(), GraphCliError> {
    let bytes = read_input(&input)?;
    let spec: HypergraphSignalSpec = serde_json::from_slice(&bytes)?;
    let result = hypergraph_signal_workflow(spec)
        .map_err(|error| GraphCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_heterogeneous(input: PathBuf, out: PathBuf) -> Result<(), GraphCliError> {
    let bytes = read_input(&input)?;
    let spec: HeterogeneousMessageSpec = serde_json::from_slice(&bytes)?;
    let result = heterogeneous_graph_message_workflow(spec)
        .map_err(|error| GraphCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_scattering(input: PathBuf, out: PathBuf) -> Result<(), GraphCliError> {
    let bytes = read_input(&input)?;
    let spec: GraphScatteringSpec = serde_json::from_slice(&bytes)?;
    let result =
        graph_scattering_workflow(spec).map_err(|error| GraphCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_diffusion_wavelet(input: PathBuf, out: PathBuf) -> Result<(), GraphCliError> {
    let bytes = read_input(&input)?;
    let spec: GraphDiffusionWaveletSpec = serde_json::from_slice(&bytes)?;
    let result = graph_diffusion_wavelet_workflow(spec)
        .map_err(|error| GraphCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_chebyshev_heat(input: PathBuf, out: PathBuf) -> Result<(), GraphCliError> {
    let bytes = read_input(&input)?;
    let spec: GraphChebyshevHeatSpec = serde_json::from_slice(&bytes)?;
    let result = graph_chebyshev_heat_workflow(spec)
        .map_err(|error| GraphCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_sparse_radius_heat(input: PathBuf, out: PathBuf) -> Result<(), GraphCliError> {
    let bytes = read_input(&input)?;
    let spec: GraphSparseRadiusHeatSpec = serde_json::from_slice(&bytes)?;
    let result = graph_sparse_radius_heat_workflow(spec)
        .map_err(|error| GraphCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_sparse_radius_basis(input: PathBuf, out: PathBuf) -> Result<(), GraphCliError> {
    let bytes = read_input(&input)?;
    let spec: GraphSparseRadiusBasisSpec = serde_json::from_slice(&bytes)?;
    let result = graph_sparse_radius_basis_workflow(spec)
        .map_err(|error| GraphCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_sparse_radius_heat_stability(input: PathBuf, out: PathBuf) -> Result<(), GraphCliError> {
    let bytes = read_input(&input)?;
    let spec: GraphSparseRadiusHeatStabilitySpec = serde_json::from_slice(&bytes)?;
    let result = graph_sparse_radius_heat_stability_workflow(spec)
        .map_err(|error| GraphCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_sparse_radius_diffusion_wavelet(input: PathBuf, out: PathBuf) -> Result<(), GraphCliError> {
    let bytes = read_input(&input)?;
    let spec: GraphSparseRadiusDiffusionWaveletSpec = serde_json::from_slice(&bytes)?;
    let result = graph_sparse_radius_diffusion_wavelet_workflow(spec)
        .map_err(|error| GraphCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_sparse_radius_scattering(input: PathBuf, out: PathBuf) -> Result<(), GraphCliError> {
    let bytes = read_input(&input)?;
    let spec: GraphSparseRadiusScatteringSpec = serde_json::from_slice(&bytes)?;
    let result = graph_sparse_radius_scattering_workflow(spec)
        .map_err(|error| GraphCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_spectrum_null(input: PathBuf, out: PathBuf) -> Result<(), GraphCliError> {
    let bytes = read_input(&input)?;
    let spec: GraphSpectrumNullSpec = serde_json::from_slice(&bytes)?;
    let result =
        graph_spectrum_null_test(spec).map_err(|error| GraphCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_wavelet(input: PathBuf, out: PathBuf) -> Result<(), GraphCliError> {
    let bytes = read_input(&input)?;
    let spec: GraphWaveletSpec = serde_json::from_slice(&bytes)?;
    let result =
        graph_wavelet_workflow(spec).map_err(|error| GraphCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run_heat(input: PathBuf, out: PathBuf) -> Result<(), GraphCliError> {
    let bytes = read_input(&input)?;
    let spec: GraphHeatSpec = serde_json::from_slice(&bytes)?;
    let result =
        graph_heat_workflow(spec).map_err(|error| GraphCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn run(input: PathBuf, out: PathBuf) -> Result<(), GraphCliError> {
    let bytes = read_input(&input)?;
    let spec: GraphSpectralSpec = serde_json::from_slice(&bytes)?;
    let result =
        graph_spectral_workflow(spec).map_err(|error| GraphCliError::Input(error.to_string()))?;
    publish_json(&out, &result)
}

fn read_input(input: &Path) -> Result<Vec<u8>, GraphCliError> {
    let metadata = fs::metadata(input).map_err(|source| GraphCliError::Io {
        path: input.to_owned(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(GraphCliError::Input(
            "input must be a regular file within 16 MiB".into(),
        ));
    }
    fs::read(input).map_err(|source| GraphCliError::Io {
        path: input.to_owned(),
        source,
    })
}

fn publish_json(path: &Path, result: &impl Serialize) -> Result<(), GraphCliError> {
    if fs::symlink_metadata(path).is_ok() {
        return Err(GraphCliError::Input(format!(
            "output already exists: {}",
            path.display()
        )));
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|source| GraphCliError::Io {
        path: parent.to_owned(),
        source,
    })?;
    let file_name = path
        .file_name()
        .ok_or_else(|| GraphCliError::Input("output must name a file".into()))?;
    let mut staging_name = OsString::from(".");
    staging_name.push(file_name);
    staging_name.push(format!(".marklab-graph-{}.tmp", std::process::id()));
    let staging = parent.join(staging_name);
    let bytes = serde_json::to_vec_pretty(result)?;
    let publication = (|| -> Result<(), GraphCliError> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)
            .map_err(|source| GraphCliError::Io {
                path: staging.clone(),
                source,
            })?;
        file.write_all(&bytes)
            .and_then(|()| file.write_all(b"\n"))
            .and_then(|()| file.sync_all())
            .map_err(|source| GraphCliError::Io {
                path: staging.clone(),
                source,
            })?;
        fs::rename(&staging, path).map_err(|source| GraphCliError::Io {
            path: path.to_owned(),
            source,
        })
    })();
    if publication.is_err() {
        let _ = fs::remove_file(&staging);
    }
    publication
}

pub(crate) fn into_marklab_error(error: GraphCliError) -> marklab::MarklabError {
    marklab::MarklabError::Validation(error.to_string())
}
