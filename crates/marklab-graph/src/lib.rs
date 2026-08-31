#![forbid(unsafe_code)]

mod cellular;
mod chebyshev_heat;
mod diffusion_wavelet;
mod eigendecomposition;
mod filter;
mod heat;
mod heterogeneous;
mod hodge;
mod hypergraph;
mod motif;
mod scattering;
mod sparse_basis;
mod sparse_diffusion_wavelet;
mod sparse_fourier;
mod sparse_heat;
mod sparse_heat_stability;
mod sparse_radius_graph;
mod sparse_scattering;
mod spectral;
mod spectrum_null;
mod validation;
mod wavelet;

pub use cellular::{
    cellular_complex_workflow, CellularComplexArtifact, CellularComplexResult, CellularComplexSpec,
    CellularDomainInput, CellularInterfaceInput, CellularJunctionInput, CellularPerturbationResult,
    CellularSegmentationInput, CellularSegmentationPerturbation, OrientedCellularInterfaceInput,
};
pub use chebyshev_heat::{
    graph_chebyshev_heat_workflow, GraphChebyshevHeatResult, GraphChebyshevHeatSpec,
};
pub use diffusion_wavelet::{
    graph_diffusion_wavelet_workflow, DiffusionWaveletLevel, DiffusionWaveletTransformResult,
    DiffusionWaveletTree, GraphDiffusionWaveletResult, GraphDiffusionWaveletSpec,
};
pub use filter::chebyshev_apply;
pub use heat::{
    graph_heat_workflow, DiffusionDistanceResult, DiffusionPairSpec, GraphHeatAtTime,
    GraphHeatResult, GraphHeatSpec,
};
pub use heterogeneous::{
    heterogeneous_graph_message_workflow, HeterogeneousEdge, HeterogeneousMessageResult,
    HeterogeneousMessageSpec, HeterogeneousNodeInput, MessageAggregation, SpatialNearRelationSpec,
};
pub use hodge::{
    simplicial_hodge_workflow, CanonicalHodgeEdge, HodgeDecompositionResult, HodgeEdgeInput,
    SimplicialHodgeResult, SimplicialHodgeSpec, SimplicialInventory,
};
pub use hypergraph::{
    hypergraph_signal_workflow, HyperedgeInput, HypergraphMemberInput, HypergraphNodeInput,
    HypergraphSignalResult, HypergraphSignalSpec, IncidenceEntry,
};
pub use motif::{
    typed_triangle_motif_summary_workflow, typed_triangle_motif_workflow, MotifEdgeInput,
    MotifNodeInput, TypedTriangleMotifResult, TypedTriangleMotifSpec,
    TypedTriangleMotifSummaryResult,
};
pub use scattering::{
    graph_scattering_workflow, GraphScatteringFeature, GraphScatteringResult, GraphScatteringSpec,
    GraphScatteringStability, GraphSignalPerturbation,
};
pub use sparse_basis::{
    graph_sparse_radius_basis_workflow, GraphSparseRadiusBasisResult, GraphSparseRadiusBasisSpec,
    SparseRadiusBasisComponent, SparseRadiusBasisMode,
};
pub use sparse_diffusion_wavelet::{
    graph_sparse_radius_diffusion_wavelet_workflow, GraphSparseRadiusDiffusionWaveletResult,
    GraphSparseRadiusDiffusionWaveletSpec, SparseRadiusDiffusionWaveletScaleResult,
};
pub use sparse_fourier::{
    graph_sparse_radius_fourier_energy_workflow, GraphSparseRadiusFourierEnergyResult,
    GraphSparseRadiusFourierEnergySpec, SparseRadiusFourierModeEnergy,
};
pub use sparse_heat::{
    graph_sparse_radius_heat_workflow, GraphSparseRadiusHeatResult, GraphSparseRadiusHeatSpec,
};
pub use sparse_heat_stability::{
    graph_sparse_radius_heat_stability_workflow, GraphSparseRadiusHeatStabilityResult,
    GraphSparseRadiusHeatStabilitySpec, SparseRadiusHeatPerturbationResult,
};
pub use sparse_scattering::{
    graph_sparse_radius_scattering_workflow, GraphSparseRadiusScatteringResult,
    GraphSparseRadiusScatteringSpec, SparseRadiusFirstOrderScatteringResult,
    SparseRadiusSecondOrderScatteringResult,
};
pub use spectral::{
    graph_spectral_workflow, CanonicalGraphEdge, CanonicalGraphNode, FrequencyBandSpec,
    FrequencyBandSummary, GraphError, GraphNodeInput, GraphSpectralResult, GraphSpectralSpec,
    GraphSpectralWork, GraphSpectrum, GraphWeightSpec, LaplacianSpec,
};
pub use spectrum_null::{
    graph_spectrum_null_test, GraphNullNodeStratum, GraphSpectrumNullBand, GraphSpectrumNullResult,
    GraphSpectrumNullSpec,
};
pub use validation::{
    validate_graph_mathematics_suite, GraphMathematicsValidationResult, GraphValidationEntry,
};
pub use wavelet::{
    graph_wavelet_workflow, GraphWaveletResult, GraphWaveletScaleResult, GraphWaveletSpec,
};

pub(crate) use eigendecomposition::symmetric_eigendecomposition_with_limit;
pub(crate) use filter::{heat_chebyshev_coefficients, heat_grid_error};
