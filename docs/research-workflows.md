# Research workflows

These command families are at the **Research** tier of the
[evidence ladder](capabilities.md): each has synthetic known-answer or analytic tests and
the assumptions below, but none has an independent reference comparison on public tissue
data or a calibrated real-cohort result. Use them for method development and hypothesis
generation, not as validated measurements. `marklab <family> <command> --help` lists the
exact inputs and limits of each command.

`graph group-centrality` is the exception in its family: it is Experimental for its three
observed group scores. See [group centrality](group-centrality.md).

## Graphs (`graph`)

Signals on declared graphs or sparse physical-radius cell graphs: spectral eigenmodes,
heat diffusion (exact and Chebyshev), graph wavelets, diffusion wavelets and scattering
features, spectrum summaries against a declared null, heterogeneous-edge message passing,
hypergraphs, typed triangle motifs, Hodge decomposition of edge signals and signals on
declared cellular complexes. `graph validate` runs the mathematical oracles.

- **Measures:** how a supplied per-cell signal varies over the declared graph at different
  scales, and which typed triangles occur.
- **Assumes:** the graph construction (radius, edge types, weights) is chosen before
  looking at results; spectra and wavelet features depend on that choice.
- **Does not establish:** biological communication between cells, a learned community or
  niche, or a tissue biomarker. Feature differences between samples have no calibrated test.
- **Evidence:** `tests/graph_*_cli.rs` (matrix and graph oracles, synthetic signals and
  permutation checks) and `tests/graph_validation_suite_cli.rs`.

## Topology (`topology`)

Alpha-complex and witness-complex persistence of point patterns, landmark and bottleneck
stability of witness persistence, persistence-diagram comparison and stability under
declared perturbations, connectivity across distance thresholds, and morphology and
topology of binary raster masks. Persistence is computed by the pinned GUDHI 3.13.0
backend.

- **Measures:** the birth and death scales of connected components and loops in the
  supplied points or mask, and how sensitive those diagrams are to perturbation.
- **Assumes:** coordinates are in physical units; raster results depend on the declared
  pixel size and connectivity.
- **Does not establish:** that a persistence feature corresponds to a tissue structure or
  predicts outcome. GUDHI is the implementation, so there is no independent persistence
  comparison yet.
- **Evidence:** `tests/topology_*_cli.rs`, including known topologies, stability bounds and
  `tests/topology_validation_suite_cli.rs`.

## Three-dimensional tissue (`spatial3d`)

K functions in declared 3-D or voxel windows (including intensity-reweighted and directed
cross-type K), 3-D neighbor graphs, serial-section stack reconstruction with landmark
registration uncertainty, registered serial and longitudinal voxel K analyses, exact 3-D
alpha complexes, separation of registration deformation from declared biological change,
and clone-location models.

- **Measures:** clustering of cells in a 3-D window and how registration uncertainty
  propagates into those curves.
- **Assumes:** section registration and spacing are supplied or modeled as declared; a
  voxel window is the true observation region.
- **Does not establish:** results on real serial-section patient material. There is no
  independent 3-D K reference comparison.
- **Evidence:** `tests/spatial3d_*_cli.rs` and `tests/durable_spatial3d_*_project.rs`
  (cuboid and voxel answers, registration and clone-model checks). The clone-model
  contract is described under [Research workflows in capabilities](capabilities.md#research-workflows).

## Simulation (`simulate`)

Random labeling of a cell row, one-dimensional Fisher–KPP growth fronts and Lotka–Volterra
competition, an exact stochastic two-species agent process, periodic 2-D reaction–diffusion,
level-set interface evolution, transport from vessels through a static flow field, a
coupled mechanistic tissue model, and scoring of simulated against observed summaries.

- **Measures:** model trajectories under declared parameters; `summary-matching` scores
  how close simulated summaries are to observed ones.
- **Assumes:** the model equations and parameters are the user's hypothesis.
- **Does not establish:** that a model has been fitted to, or explains, real tissue. A good
  summary match is not identification of a mechanism.
- **Evidence:** `tests/simulation_*_cli.rs` (solver accuracy, conservation, bounds and seeded
  stochastic processes).

## Neural models (`neural`)

Neural point-process models of intensity and marks, declared neural point-set generators,
neural simulation-based inference on declared simulations, and comparison of generated with
reference point sets.

- **Measures:** fitted intensity or generated patterns for the supplied data and simulator.
- **Assumes:** the training data and simulator are representative; fits are seeded and
  bounded.
- **Does not establish:** image-model training, real-cohort predictive performance or
  calibrated posterior coverage on tissue data.
- **Evidence:** `tests/neural_*_cli.rs` (synthetic point-process, generator and SBI checks).

## Causal designs (`causal`)

Randomized effects under declared spatial interference and exposure mapping, expected
information gain for Gaussian designs, sensitivity to unmeasured confounding (bias
sensitivity, Manski bounds, Rosenbaum sign-test bounds), cross-fitted observational
estimators on synthetic data, direct/spillover/mediation analysis of randomized
perturbations, and budgeted active design.

- **Measures:** effects and bounds under the declared design and interference graph.
- **Assumes:** the identification assumptions stated by each command (randomization,
  declared interference, bounded outcomes).
- **Does not establish:** causation from observational tissue associations. Spatial
  association in a section is not a treatment effect.
- **Evidence:** `tests/causal_*_cli.rs` and `tests/durable_causal_interference_project.rs`
  (analytic identification cases and synthetic sensitivity controls).

## Longitudinal data (`longitudinal`)

Linear-Gaussian Kalman smoothing, nonlinear filtering and particle smoothing of repeated
observations, and association between phylogenetic and spatial distances.

- **Measures:** smoothed latent trajectories under the declared state-space model.
- **Assumes:** the state-space model and noise levels are correctly specified.
- **Does not establish:** clinical change in repeated patient samples.
- **Evidence:** `tests/longitudinal_*_cli.rs` and `tests/durable_longitudinal_kalman_project.rs`
  (analytic and synthetic filter and smoother tests).

## Registration (`registration`)

Nonrigid and stationary-velocity-field image registration, diffeomorphic landmark
registration, uncertainty for velocity-field registration and propagated landmark
uncertainty, and multi-specimen atlases.

- **Measures:** the deformation that aligns the supplied images or landmarks, with
  declared uncertainty.
- **Assumes:** the images or landmarks correspond; deformation smoothness is set by the
  declared model.
- **Does not establish:** biological change. A deformation alone cannot separate tissue
  change from sectioning or mounting artefact.
- **Evidence:** `tests/registration_*_cli.rs` (synthetic deformations and landmark checks).

## Transport alignment (`bayes … fused-gromov-wasserstein`)

Fused Gromov–Wasserstein (FGW) alignment of two weighted point sets, each with feature
vectors and a pairwise structure matrix, through the pinned POT 0.9.7.post1 backend.
`fused-gromov-wasserstein` transports all mass; `partial-fused-gromov-wasserstein` transports
a declared share of it, with marginals bounded by the supplied masses;
`unbalanced-fused-gromov-wasserstein` instead holds each marginal to its masses with a
tensorized KL penalty of strength `--marginal-relaxation`, so the two sets may carry different
totals and mass may be created or destroyed where that lowers the cost.

- **Measures:** which source points are matched to which target points when both their
  features and their internal geometry are taken into account.
- **Assumes:** the feature and structure scales, the relative weights and, for the
  unbalanced form, the relaxation strength are chosen before looking at the alignment. The
  unbalanced objective is POT's lower bound of fused unbalanced GW (Thual et al. 2022;
  Séjourné, Vialard and Peyré 2021), optimized over two couplings of equal mass; `ε = 0`
  uses POT's KL majorization–minimization solver and `ε > 0` log-domain Sinkhorn. The worker
  recomputes every objective term and requires it to equal POT's reported cost within 1e-8
  relative. Each command fits from three starting plans and reports the spread.
- **Does not establish:** cell-to-cell correspondence, a biological identity between the two
  sets, or a calibrated test of their similarity. The objective is non-convex. When its optimum
  transports no mass, POT's KL solver divides a vanished plan by itself; Marklab reports that
  collapse as an error rather than a plan.
- **Evidence:** `tests/bayes_fused_gromov_wasserstein_cli.rs` and
  `tests/bayes_partial_fgw_cli.rs`. For the unbalanced form,
  `tests/python/test_unbalanced_fgw_known_answers.py` checks, with tolerances fixed before the
  comparison: identical spaces give the one-to-one plan with the supplied masses (error
  1.9e-16; limit 1e-6) and zero objective; for single points, the transported mass equals the
  global minimizer of the analytic scalar objective (errors ≤5.9e-14; limit 1e-6), and the
  zero-mass case is reported as a collapse; on a 2×2 problem without structure, the objective
  equals an independent SciPy SLSQP optimum over both couplings (difference 1.7e-16; limit
  1e-8 relative) and the plan agrees within 6.2e-9 (limit 1e-4).
  `tests/bayes_unbalanced_fgw_cli.rs` repeats the identity and single-point answers through
  the CLI.

## Promotion

A command in these families moves to Experimental only with agreement against an
independent implementation or analytic result and a public real-data demonstration of
that method, as for the [validated workflows](capabilities.md#method-coverage).
