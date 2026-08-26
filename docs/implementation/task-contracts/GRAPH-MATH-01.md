# Task contract — GRAPH-MATH-01 canonical graph and higher-order mathematics

Status: complete for bounded synthetic specializations; real validation remains data-dependent

Date: 2026-08-25

Parent requirements: FND-03, FR-01A, FR-01B, GSP-01, GSP-02, GSP-03, HET-01, WS-61,
WS-62.

## Pseudocode ownership

This contract records the original prerequisite audit of Part VII. GSP-SPECTRAL-01 owns bounded
physical-radius `BuildCanonicalGraph`, combinatorial `BuildLaplacian`, `GraphFourierTransform`, and
`SummarizeGraphFrequencyBands`. Functions originally retained here were:
`BuildDiffusionWaveletTree`, `DiffusionWaveletTransform`, `GraphScattering`,
`ValidateGraphScatteringStability`,
`BuildHeterogeneousTissueGraph`, `HeterogeneousMessagePassing`, `BuildHypergraph`,
`HypergraphLaplacian`, `HypergraphSignalSmoothness`, `CountTypedMotifs`, `BuildMotifAdjacency`,
`MotifNullTest`, `BuildCliqueComplex`, `HodgeLaplacian`, `HodgeDecomposeEdgeFlow`,
`FilterKSimplicialSignal`, `BuildCellularComplex`, and `ValidateGraphMathematicsSuite`.

## Original available-but-insufficient owners

The bounded synthetic graph-signal workflows own a unique unordered positive-weight edge list,
combinatorial/symmetric-normalized Dirichlet energy, restricted complete-vector permutation, local
roughness, and a graph digest. They do not own the pseudocode `GraphOperatorSpec`, coordinate/frame
binding, edge-rule/kernel/scale provenance, isolated/component policy, sparse operator artifact,
random-walk Laplacian, or physical-scale calibration.

## Original named prerequisite and immediate-caller boundary

WS-61 must first admit the canonical graph/operator contract across node domain, coordinate frame,
edge construction, physical scale, symmetrization, uncertainty, components, and digest. The only
existing indexed radius/kNN builder belongs to the paused compatibility/root package and has no
admitted cross-package graph-artifact owner; copying it would duplicate a canonical implementation.
Heterogeneous, hypergraph, motif, clique/Hodge, cellular-complex, diffusion-wavelet, wavelet, and
scattering surfaces also lack an immediate production graph caller and admitted pathology endpoint.

The first canonical graph/Fourier workflow is now promoted with an immediate signal caller and exact
path oracle. GSP-HEAT-01 now owns `ExactHeatKernel`, the exact branch of `ApplyHeatKernel`,
`HeatKernelSignature`, and `DiffusionDistance`; GSP-WAVELET-01 owns
`SpectralGraphWaveletTransform` and `GraphWaveletEnergy`. Remaining null, diffusion-wavelet,
Chebyshev, scattering, and higher-order families retain their additional semantics; the canonical
graph owner must be reused rather than copied.

GSP-NULL-01 additionally owns `GraphSpectrumNullTest` through restricted complete-row permutations
and the shared canonical ERL primitive. It does not close the remaining approximation,
multiresolution, scattering, or higher-order declarations.

GSP-CHEB-01 now owns `ChebyshevApply` and the heat-filter specialization of
`AdaptiveChebyshevOrder`, with exact-heat differential validation and explicit non-certified error
evidence. It does not claim sparse-scale performance or general filter coefficient selection.

## Resolution

GSP-DIFFWAVE-01, GSP-SCATTER-01, HET-GRAPH-01, HET-HYPER-01, HET-MOTIF-01, HET-HODGE-01,
HET-CELLULAR-01, and GRAPH-VALIDATE-01 now own the originally retained declarations through
IC-0150–IC-0157. Each has an immediate CLI caller, bounded work, exact synthetic oracles, and an
explicit experimental/research-only claim ceiling. Broader graph rules, sparse scale, registration,
GPU parity, and pathology endpoint validation remain limitations and data prerequisites, not
unimplemented Part VII function declarations.
