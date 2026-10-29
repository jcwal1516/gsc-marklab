use super::{
    graph_sparse_radius_basis_workflow, graph_sparse_radius_diffusion_wavelet_workflow,
    graph_sparse_radius_fourier_energy_workflow, graph_sparse_radius_heat_stability_workflow,
    graph_sparse_radius_heat_workflow, graph_sparse_radius_scattering_workflow, run_native_spec,
    CliError, Codec, GraphSparseRadiusBasisResult, GraphSparseRadiusBasisSpec,
    GraphSparseRadiusDiffusionWaveletResult, GraphSparseRadiusDiffusionWaveletSpec,
    GraphSparseRadiusFourierEnergyResult, GraphSparseRadiusFourierEnergySpec,
    GraphSparseRadiusHeatResult, GraphSparseRadiusHeatSpec, GraphSparseRadiusHeatStabilityResult,
    GraphSparseRadiusHeatStabilitySpec, GraphSparseRadiusScatteringResult,
    GraphSparseRadiusScatteringSpec, NativeSpecCommand, NodeError, SparseRadiusBasisProjectArgs,
    SparseRadiusDiffusionWaveletProjectArgs, SparseRadiusFourierEnergyProjectArgs,
    SparseRadiusHeatProjectArgs, SparseRadiusHeatStabilityProjectArgs,
    SparseRadiusScatteringProjectArgs,
};

const SPARSE_RADIUS_HEAT: NativeSpecCommand = NativeSpecCommand {
    name: "sparse-radius-heat",
    label: "sparse radius heat input",
    input_kind: "application/vnd.marklab.source.graph-sparse-radius-heat+json;version=1",
    node_id: "sparse-radius-heat",
    node_kind: "graph_signal_transform",
    node_version: 1,
    implementation_identity: "marklab-project-sparse-radius-heat-node-v1",
    execution_policy: b"native-safe-rust-sparse-radius-heat-v1",
    output_kind: "application/vnd.marklab.graph-sparse-radius-heat-result+json;version=1",
    result_schema: ("marklab.graph_sparse_radius_heat_result", 1),
    codec: Codec::ExactFloatJson,
};

pub(super) fn run_sparse_radius_heat(args: SparseRadiusHeatProjectArgs) -> Result<(), CliError> {
    let SparseRadiusHeatProjectArgs {
        project: project_path,
        input: input_path,
        out: output_path,
    } = args;
    run_native_spec(
        &SPARSE_RADIUS_HEAT,
        &project_path,
        input_path,
        &output_path,
        |spec: &GraphSparseRadiusHeatSpec| {
            graph_sparse_radius_heat_workflow(spec.clone()).map_err(NodeError::execution)
        },
        |spec, output: &GraphSparseRadiusHeatResult| {
            output.validate_for_spec(spec).map_err(NodeError::decode)
        },
    )
}

const SPARSE_RADIUS_BASIS: NativeSpecCommand = NativeSpecCommand {
    name: "sparse-radius-basis",
    label: "sparse radius basis input",
    input_kind: "application/vnd.marklab.source.graph-sparse-radius-basis+json;version=2",
    node_id: "sparse-radius-basis",
    node_kind: "graph_sparse_low_frequency_basis",
    node_version: 2,
    implementation_identity: "marklab-project-sparse-radius-basis-node-v3",
    execution_policy: b"native-safe-rust-sparse-radius-basis-v3-complement-checked",
    output_kind: "application/vnd.marklab.graph-sparse-radius-basis-result+json;version=2",
    result_schema: ("marklab.graph_sparse_radius_basis_result", 2),
    codec: Codec::ExactFloatJson,
};

pub(super) fn run_sparse_radius_basis(args: SparseRadiusBasisProjectArgs) -> Result<(), CliError> {
    let SparseRadiusBasisProjectArgs {
        project: project_path,
        input: input_path,
        out: output_path,
    } = args;
    run_native_spec(
        &SPARSE_RADIUS_BASIS,
        &project_path,
        input_path,
        &output_path,
        |spec: &GraphSparseRadiusBasisSpec| {
            graph_sparse_radius_basis_workflow(spec.clone()).map_err(NodeError::execution)
        },
        |spec, output: &GraphSparseRadiusBasisResult| {
            output.validate_for_spec(spec).map_err(NodeError::decode)
        },
    )
}

const SPARSE_RADIUS_FOURIER_ENERGY: NativeSpecCommand = NativeSpecCommand {
    name: "sparse-radius-fourier-energy",
    label: "sparse radius Fourier energy input",
    input_kind: "application/vnd.marklab.source.graph-sparse-radius-fourier-energy+json;version=2",
    node_id: "sparse-radius-fourier-energy",
    node_kind: "graph_sparse_fourier_energy",
    node_version: 2,
    implementation_identity: "marklab-project-sparse-radius-fourier-energy-node-v3",
    execution_policy: b"native-safe-rust-sparse-radius-fourier-energy-v3-complement-checked",
    output_kind: "application/vnd.marklab.graph-sparse-radius-fourier-energy-result+json;version=2",
    result_schema: ("marklab.graph_sparse_radius_fourier_energy_result", 2),
    codec: Codec::ExactFloatJson,
};

pub(super) fn run_sparse_radius_fourier_energy(
    args: SparseRadiusFourierEnergyProjectArgs,
) -> Result<(), CliError> {
    let SparseRadiusFourierEnergyProjectArgs {
        project: project_path,
        input: input_path,
        out: output_path,
    } = args;
    run_native_spec(
        &SPARSE_RADIUS_FOURIER_ENERGY,
        &project_path,
        input_path,
        &output_path,
        |spec: &GraphSparseRadiusFourierEnergySpec| {
            graph_sparse_radius_fourier_energy_workflow(spec.clone()).map_err(NodeError::execution)
        },
        |spec, output: &GraphSparseRadiusFourierEnergyResult| {
            output.validate_for_spec(spec).map_err(NodeError::decode)
        },
    )
}

const SPARSE_RADIUS_HEAT_STABILITY: NativeSpecCommand = NativeSpecCommand {
    name: "sparse-radius-heat-stability",
    label: "sparse radius heat stability input",
    input_kind: "application/vnd.marklab.source.graph-sparse-radius-heat-stability+json;version=1",
    node_id: "sparse-radius-heat-stability",
    node_kind: "graph_coordinate_perturbation_stability",
    node_version: 1,
    implementation_identity: "marklab-project-sparse-radius-heat-stability-node-v1",
    execution_policy: b"native-safe-rust-sparse-radius-heat-stability-v1",
    output_kind: "application/vnd.marklab.graph-sparse-radius-heat-stability-result+json;version=1",
    result_schema: ("marklab.graph_sparse_radius_heat_stability_result", 1),
    codec: Codec::ExactFloatJson,
};

pub(super) fn run_sparse_radius_heat_stability(
    args: SparseRadiusHeatStabilityProjectArgs,
) -> Result<(), CliError> {
    let SparseRadiusHeatStabilityProjectArgs {
        project: project_path,
        input: input_path,
        out: output_path,
    } = args;
    run_native_spec(
        &SPARSE_RADIUS_HEAT_STABILITY,
        &project_path,
        input_path,
        &output_path,
        |spec: &GraphSparseRadiusHeatStabilitySpec| {
            graph_sparse_radius_heat_stability_workflow(spec.clone()).map_err(NodeError::execution)
        },
        |spec, output: &GraphSparseRadiusHeatStabilityResult| {
            output.validate_for_spec(spec).map_err(NodeError::decode)
        },
    )
}

const SPARSE_RADIUS_DIFFUSION_WAVELET: NativeSpecCommand = NativeSpecCommand {
    name: "sparse-radius-diffusion-wavelet",
    label: "sparse radius diffusion wavelet input",
    input_kind:
        "application/vnd.marklab.source.graph-sparse-radius-diffusion-wavelet+json;version=1",
    node_id: "sparse-radius-diffusion-wavelet",
    node_kind: "graph_sparse_diffusion_wavelet",
    node_version: 1,
    implementation_identity: "marklab-project-sparse-radius-diffusion-wavelet-node-v1",
    execution_policy: b"native-safe-rust-sparse-radius-diffusion-wavelet-v1",
    output_kind:
        "application/vnd.marklab.graph-sparse-radius-diffusion-wavelet-result+json;version=1",
    result_schema: ("marklab.graph_sparse_radius_diffusion_wavelet_result", 1),
    codec: Codec::ExactFloatJson,
};

pub(super) fn run_sparse_radius_diffusion_wavelet(
    args: SparseRadiusDiffusionWaveletProjectArgs,
) -> Result<(), CliError> {
    let SparseRadiusDiffusionWaveletProjectArgs {
        project: project_path,
        input: input_path,
        out: output_path,
    } = args;
    run_native_spec(
        &SPARSE_RADIUS_DIFFUSION_WAVELET,
        &project_path,
        input_path,
        &output_path,
        |spec: &GraphSparseRadiusDiffusionWaveletSpec| {
            graph_sparse_radius_diffusion_wavelet_workflow(spec.clone())
                .map_err(NodeError::execution)
        },
        |spec, output: &GraphSparseRadiusDiffusionWaveletResult| {
            output.validate_for_spec(spec).map_err(NodeError::decode)
        },
    )
}

const SPARSE_RADIUS_SCATTERING: NativeSpecCommand = NativeSpecCommand {
    name: "sparse-radius-scattering",
    label: "sparse radius scattering input",
    input_kind: "application/vnd.marklab.source.graph-sparse-radius-scattering+json;version=1",
    node_id: "sparse-radius-scattering",
    node_kind: "graph_sparse_scattering",
    node_version: 1,
    implementation_identity: "marklab-project-sparse-radius-scattering-node-v1",
    execution_policy: b"native-safe-rust-sparse-radius-scattering-v1",
    output_kind: "application/vnd.marklab.graph-sparse-radius-scattering-result+json;version=1",
    result_schema: ("marklab.graph_sparse_radius_scattering_result", 1),
    codec: Codec::ExactFloatJson,
};

pub(super) fn run_sparse_radius_scattering(
    args: SparseRadiusScatteringProjectArgs,
) -> Result<(), CliError> {
    let SparseRadiusScatteringProjectArgs {
        project: project_path,
        input: input_path,
        out: output_path,
    } = args;
    run_native_spec(
        &SPARSE_RADIUS_SCATTERING,
        &project_path,
        input_path,
        &output_path,
        |spec: &GraphSparseRadiusScatteringSpec| {
            graph_sparse_radius_scattering_workflow(spec.clone()).map_err(NodeError::execution)
        },
        |spec, output: &GraphSparseRadiusScatteringResult| {
            output.validate_for_spec(spec).map_err(NodeError::decode)
        },
    )
}
