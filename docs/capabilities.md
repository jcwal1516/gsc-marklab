# What Marklab can do

Marklab measures how cells are arranged in tissue. This page lists each workflow, the
question it answers and how mature it is. `marklab --help` lists every command, and
`marklab <command> --help` shows the options.

Evidence tiers describe demonstrated scientific use, independently of software age or
file-format stability:

- **Research:** a synthetic known-answer check and documented assumptions and limitations.
- **Experimental:** Research evidence, agreement with an independent implementation or
  analytic result, and a public real-data demonstration of the particular method.
- **Core:** Experimental evidence, calibration across several tissue shapes and label
  prevalences (size and, where applicable, power), a public cohort with real patient groups,
  and a stable output format.

No workflow currently meets all Core requirements. The earlier Core label on this page
meant the oldest, most tested software; it did not establish cohort validation. A passing
fit or synthetic test alone cannot promote a method. Existing serialized `experimental`,
`research_only`, `established` and `validated` policy values use the older, caller-supplied
policy vocabulary; they are not certificates that this evidence ladder has been met.

## From tissue to a mathematical object

Marklab starts after pixels have been turned into declared objects. It can inspect or crop a
whole-slide image, and it has bounded adapters for several upstream tables and annotation formats,
but it does not segment nuclei, classify cells or infer a tissue outline from an arbitrary slide.
Those upstream decisions determine what the later mathematics means.

| Declared object | Typical fields | What it enables |
|---|---|---|
| Cell or event point pattern | Physical coordinates, observation window, patient and slide identity | K/L, pair correlation, nearest-neighbour and empty-space functions, point-process models, topology and graphs |
| Marked point pattern | Cell class, binary mark, continuous measurement or probability attached to each point | Random-labeling tests, cross-type functions, mark covariance, variograms, local maps and conditional mark models |
| Compartments and annotations | Polygons, holes, interfaces, region class and physical calibration | Border correction, distance profiles, mixing/contact measurements, invasive-front and architecture summaries |
| Raster or voxel support | Binary mask, pixel/voxel size, connectivity and declared 2-D/3-D window | Morphology, raster topology, voxel K and registered serial-section analyses |
| Feature vectors | Row-aligned cell, patch, region or slide embeddings with provenance | Spatial embedding summaries, factors, kernels, retrieval and held-out prediction fusion |
| Graph or complex | Nodes, weighted or typed edges, hyperedges or cells of a complex | Autocorrelation, centrality, graph spectra, diffusion, scattering, motifs and Hodge decomposition |
| Patient-level table | Prespecified endpoints per independent patient, with groups, blocks or sites | Permutation tests, Max-T, equivalence, bootstrap, multivariate distances and multisite inference |
| Repeated or simulated state | Time-indexed observations, state-space matrices, initial field and model parameters | Filtering and smoothing, mechanistic simulation, SBI and posterior-predictive checks |

Coordinates must be in physical units for spatial interpretation. The observation window is part
of the data: excluding a hole, clipping a region or treating a rectangle as fully observed changes
which pairs and empty locations could have been seen. A cell class supplied by a model remains a
declared observation; Marklab does not turn it into biological ground truth.

## Mathematical and mapping families

The sections below explain the major calculations and how they relate. The detailed evidence table
that follows remains the authority for exactly which variants have independent or public-data
agreement.

### Point-pattern geometry and spatial scale

For a point pattern in a declared window, Ripley's `K(r)` estimates the amount of pattern within
radius `r` of a typical point after the selected edge correction. `L(r)=sqrt(K(r)/π)` puts the
answer back on a distance scale; `L(r)-r` is zero in expectation for homogeneous complete spatial
randomness (CSR), not for every fitted or conditional null. Pair correlation `g(r)` is a local,
annular counterpart to cumulative K. Inhomogeneous forms reweight pairs by supplied or estimated
intensity, and cross-type forms direct the question from one cell class to another.

`G(r)` is the distribution of point-to-nearest-point distance, `F(r)` is empty-space-to-nearest-
point distance from declared probes, and `J(r)=(1-G(r))/(1-F(r))` where the ratio is supported.
These answer different questions: K accumulates neighbours across scales, g localizes the scale,
G emphasizes crowding around observed cells, and F includes empty tissue. Border, translation and
isotropic corrections are estimator choices rather than cosmetic options. Gaussian bandwidth
selection, piecewise-compartment intensity and polygon windows likewise change the estimand.

CSR simulations move points in the window. Random labeling holds coordinates fixed and permutes
marks. A fitted point-process posterior generates from its fitted intensity or interaction model.
These nulls are not interchangeable, and Marklab records the chosen one in the result.

### Cell classes, neighborhoods, compartments and maps

Categorical pair counts, co-occurrence and enrichment describe how declared cell classes share a
radius graph or distance band. Cross-K/L and cross-g preserve direction: tumour-to-lymphocyte need
not equal lymphocyte-to-tumour after normalization. Exact fixed-margin moments and label
permutations ask whether labels occupy a fixed graph unusually; they do not infer a communication
mechanism. Composition and diversity instead ignore most geometry and summarize proportions,
Shannon entropy or effective class count.
[SPACE cisMI](space.md) ranks every combination of up to three (by default) cell types and
markers by binned co-information over sampled discs, against censuses whose disc members were
shuffled; it finds multi-way co-patterning that pairwise statistics do not report. SPACE
transMI ranks the same combinations by how well their disc distributions separate groups of
specimens.

Pathology maps attach measurements to declared tissue geography. They provide marker/density
profiles versus signed or unsigned distance from annotations, neighborhood composition maps,
local hotspot/outlier summaries and enrichment scans. Architecture functions operate on supplied
epithelium and nucleus geometry: components, lumens, fragmentation, contour curvature, skeleton
branches, nuclear shape, distance bands from the invasive front, budding hotspots and overlap of
registered cells or transcriptomic spots. These functions map and summarize upstream regions;
they do not draw the annotations or classify tumour.

### Continuous marks, fields and local association

Scalar and vector measurements can be treated as fields over points, regions or graph nodes.
Covariance and semivariance by distance describe how quickly similar values decorrelate. Ordinary
kriging predicts from a declared variogram and reports held-out checks; the variogram and
stationarity assumption remain inputs. Moran's I and Geary's C measure global autocorrelation
under declared spatial weights, while local Moran and hotspot summaries localize association.
Kernel dependence, entropy and mutual information answer broader dependence questions; the
variogram-matching null exists because ordinary shuffling can be badly anti-conservative for two
independently autocorrelated fields.

Additional structure descriptors cover quadrat dispersion (Morisita), lacunarity and box-counting
dimension, Landy-Szalay two-point correlation, Mapper graphs, k-atic bond orientation and nematic
axial order. Nematic tensors summarize unoriented axes and screen resolved ±1/2 defects in a
declared grid; they do not measure force. Two-source partial information decomposition uses the
declared Williams–Beer `I_min` estimator to split discrete information into redundant,
source-unique and synergistic terms. Its categories must be supplied in advance, and its
decomposition is descriptive rather than causal or bias-corrected.

### Fourier, spectral and multiscale analysis

Marklab uses Fourier ideas in several distinct places:

- The marked spatial spectrum evaluates Fourier coefficients of centered marks at the observed
  cell coordinates, with label permutations as its reference. A separate tapered raster FFT
  provides a gridding diagnostic. Peaks indicate preferred wavelengths or directions under the
  declared window and wavevector convention. A spectral correlation length summarizes where power
  is concentrated; it is not a physical interaction length by itself.
- The unmarked structure factor uses point locations without cell marks. Low-wave-
  number behavior supports descriptive hyperuniformity statistics `H` and `α` on fully observed
  rectangles. This is a different object from a fixed-label mark spectrum.
- Multiscale residuals and component modes split a pattern across declared spatial scales. Their
  gridding-sensitivity diagnostic uses a fixed 25% guide, not a formal agreement guarantee.
- Graph Fourier bases diagonalize a declared graph Laplacian. Heat kernels, Chebyshev diffusion,
  graph and diffusion wavelets, and scattering propagate node signals across graph scales. Changing
  the radius, edge weights or Laplacian changes the basis and therefore the features.
- Reaction-diffusion and phase-field solvers use spectral or finite-difference operators to evolve
  a declared dynamical model. Mathematical stability is evidence about the solver, not evidence
  that tissue follows the equations.

For a centered field `z(x)`, a schematic spatial coefficient is
`Z(k)=Σ_x z(x) exp(-i k·x)` and power is proportional to `|Z(k)|²`. Windowing, admissible
wavevectors, centering and normalization determine which numerical quantity is being reported.
Comparing a spectrum across slides therefore requires the same physical scale and conventions.

### Embeddings, multimodal factors and three kinds of fusion

Embedding inputs are row-aligned vectors produced upstream. Marklab checks identity, dimensionality
and provenance, then can compute vector semivariograms, projected-component variograms,
distance-dependent cross-covariance, kernel mark correlation, graph Dirichlet energy, graph
smoothness tests, local roughness, multiscale kernels and leakage-controlled analogous-region
retrieval. These describe the supplied representation. They do not validate the image encoder or
make an attention score a cell-level mechanism.

It is useful to separate three operations that are often all called “fusion”:

1. **Spatial/class fusion:** after registration, categorical classes or continuous marks from two
   sections are compared through cell matching, neighborhood enrichment, cross-interaction curves,
   territories and source-section-aware null models. Geometry and registration uncertainty matter.
2. **Representation fusion:** multiview factors, probabilistic CCA, MOFA-style models, tensor
   factors, spatial matrix factors and shared/private latent-factor models seek a lower-dimensional
   representation across modalities. A spatial latent factor may use a dense GP or SPDE prior.
3. **Predictive fusion:** late fusion calibrates patient-level out-of-fold modality probabilities;
   predictive stacking weights held-out log predictive densities; mixture-of-experts uses declared
   context to gate out-of-fold experts and reports OOD scores. These are held-out prediction
   comparisons, not explanations of morphology.

[Learned multimodal kernel fusion](multimodal-kernel-fusion.md) is an explicit supervised
comparison at the patient level. Each modality forms a training-standardized, centered kernel with
unit mean diagonal. For weights on the simplex, `K_w = Σ_m w_m K_m`; kernel ridge solves
`(K_w + λI)α = y - mean(y)`. A finite simplex lattice and `λ` are selected only inside inner
patient folds, then evaluated on the outer held-out patients. Equal-weight and every single-
modality kernel are tuned and reported as baselines. Linear kernels capture scaled dot-product
similarity; RBF kernels use `exp(-mean_squared_z_distance/(2 bandwidth²))`. The learned weights are
predictive tuning choices on that grid, not continuous global optima or biological importance.

### Graphs, topology and transport

A physical-radius graph turns positions into a network; a supplied graph may instead represent any
declared relation. Marklab provides degree/closeness/clustering summaries, graph spectra, heat and
wavelet features, typed message passing, hypergraphs, triangle motifs, diffusion scattering and
Hodge decomposition of edge signals or cellular complexes. None of these learns a community or
tissue niche unless a separate model supplies it.

Topology tracks connectivity and loops across a scale filtration. Alpha and witness persistence,
diagram comparison, bottleneck/coordinate stability, threshold connectivity, and binary-raster
morphology answer whether features persist under scale or perturbation. Persistence does not name
the biological structure that caused a bar in the diagram.

Optimal transport supplies couplings between source and target mass. Balanced, partial and
unbalanced Sinkhorn use a cross-cost matrix. Fused Gromov-Wasserstein combines feature cost with
the internal pairwise structure of both samples; partial and KL-relaxed unbalanced variants alter
the mass constraints. A coupling is an alignment under those costs, not proof that two rows are the
same cell or cell type.

### Bayesian models and model checking

The Bayesian family is not one monolithic tissue model. It includes:

- Gaussian, Student-t, negative-binomial, beta-binomial, hurdle, Dirichlet-multinomial and ordinal
  patient, slide and site hierarchies, crossed and nested random effects, and meta-analysis;
- Gaussian processes, multi-output and variational GPs, predictive processes, nearest-neighbour GP
  densities, CAR/SAR/GMRF, BYM/BYM2, spatially varying coefficients, resource-distance models and
  dense or SPDE spatial fields;
- inhomogeneous Poisson processes, rectangular or polygon-window LGCPs, replicated and multitype
  LGCPs, conditional multitype marks, joint location/mark/embedding models, Strauss/Geyer
  interactions and Thomas/Matérn cluster models;
- normal-mean SMC, HMC, Laplace and grid-integrated approximations, PSIS-LOO comparison, and
  simulation-based growth-front inference.

Model-checking commands have different jobs. Cross-backend agreement asks whether PyMC and
NumPyro implementations give compatible summaries. Simulation-based calibration tests rank or
coverage behavior under simulated parameters. Prior sensitivity changes declared priors.
Quadrature or kernel sensitivity changes numerical or model resolution. Posterior-predictive checks
compare simulated and observed summaries. PSIS-LOO compares predictive fit. Passing one check does
not imply that the model is biologically correct or calibrated for a patient cohort.

Sampling fits have model-specific diagnostics: inspect R-hat, effective sample size, divergences
and other required checks where the sampler supplies them. A nonconverged fit remains diagnostic.
The [Python worker environment](python-backends.md) is version-locked and isolated;
`backend doctor` checks installation, not scientific suitability. Normal-mean prior sensitivity is
native Rust ([calculation and limits](normal-mean-prior-sensitivity.md)). The advanced Thomas model,
anisotropic 3-D GP and crossed/nested variance partition have additional interpretation limits
retained later on this page.

### Population inference and prediction

The cohort family takes patients, paired patients, clusters or declared blocks as exchangeable
units. It includes patient-label permutation, paired tests, covariate-adjusted Freedman-Lane tests,
cluster permutations, repeated measures, single-step, step-down or hierarchical Max-T, functional
curve tests, MMD, energy distance, equivalence, noninferiority, hierarchical bootstrap, multisite
fixed/random-effects synthesis, fingerprint distance and nested-field reduction. Cell count does
not increase the number of independent patients.

Prediction utilities calibrate probabilities, construct grouped conformal sets, score distance
from training support and apply declared abstention thresholds. They require honest train,
calibration and test roles. Coverage or calibration on one split does not establish transport to a
new site or scanner.

### Registration, 3-D, longitudinal, causal and generative research

Registration includes nonrigid and stationary-velocity-field image alignment, diffeomorphic
landmark alignment, landmark/velocity uncertainty and multi-specimen atlases. Three-dimensional
workflows include homogeneous, cross-type and intensity-reweighted K in cuboid or voxel windows,
physical-radius graphs, exact 3-D alpha complexes, serial-section reconstruction, registered
longitudinal voxel K and clone-location models. A deformation can arise from sectioning or mounting
and is not automatically biological change.

Longitudinal commands provide linear-Gaussian Kalman filtering and RTS smoothing, nonlinear
Gaussian filtering, particle smoothing and phylogenetic-spatial distance association. Causal
commands implement randomized interference and exposure mapping, Gaussian expected information
gain, Manski and Rosenbaum bounds, confounder sensitivity, cross-fitted observational estimators,
randomized perturbation mediation/spillover analysis and budgeted active design. Their causal
meaning depends on the declared design and identification assumptions.

Simulation includes random labeling, Fisher-KPP fronts, Lotka-Volterra and stochastic agents,
reaction-diffusion, level-set interfaces, vascular transport, a coupled mechanistic model,
Cahn-Hilliard phase separation and summary matching. Neural point-process and point-set generators,
ABC, SMC, synthetic likelihood, neural SBI, simulation-support checks, SBC and posterior-predictive
laboratories support method development. Matching summaries or recovering parameters in a
simulator does not identify a mechanism in tissue. These families remain Research tools without an
independent reference comparison and public real-data demonstration except where a narrower row in
the evidence table says otherwise; [research workflows](research-workflows.md) gives their model-
specific assumptions.

## Method coverage

Four additional Research workflows were implemented on 23 October 2026:

| Workflow | Implemented behavior | Evidence and boundary |
|---|---|---|
| [Multimodal kernel fusion](multimodal-kernel-fusion.md) | Learn nonnegative linear/RBF kernel weights and ridge penalties inside nested patient folds; compare equal weights and single modalities | Synthetic linear oracle and held-out-label leakage checks; finite weight grid, continuous target, no cohort-performance or causal-weight claim |
| [Nematic orientation and defects](nematic-orientation.md) | Weighted global/local axial order, regular-grid director field, resolved ±1/2 defects | Known tensors, rotated positive/negative defect fields and hole exclusions; static shape measurements, no force inference |
| [Partial information decomposition](partial-information-decomposition.md) | Two-source discrete Williams–Beer I_min redundancy, unique information and synergy | Copy, XOR, independent, unique and joint-target controls; declared categorical bins, no bias correction or causal interpretation |
| [Cahn–Hilliard phase field](phase-field.md) | Periodic 2-D conserved phase separation with stepwise mass and discrete-energy checks | Constant and spectral-mode oracles, conservation and energy decay; declared hypothetical dynamics, no biological parameter fitting |

This inventory was checked against the command tree, workspace crates and linked upstream
documentation on 10 October 2026. **Implemented** means the stated calculation exists,
not that a named package has been reproduced. **Interoperable** requires a tested data
path. **Planned** identifies missing work. **Out of scope** states a deliberate boundary.
Reference names identify intended comparisons, not unexecuted agreement claims.

Upstream model choice is independent of this inventory. CellViT is one supported
example, and its normalized bundle is an optional pipeline-specific adapter. The
shared cell-table and spatial-vector inputs accept outputs mapped from other models;
they do not require that CellViT bundle or its fixed calibration. This does not imply
automatic parsing of every model's native export files.

| Method or data path | Marklab status | Reference implementation | Evidence and remaining boundary |
|---|---|---|---|
| Homogeneous K/L, border correction | Implemented | [spatstat](https://spatstat.org/resources/spatstatQuickref.pdf), [pointpats](https://pysal.org/pointpats/api.html) | Public amacrine/betacells agreement: scaled K error ≤4.05e-16 and L error ≤2.85e-14 µm; both limits 1e-10. [Estimator and grid conventions](reference-point-patterns.md). Fixed-count CSR differs from label shuffling. |
| Translation and isotropic K/L | Implemented | spatstat `Kest` | Public amacrine/betacells agreement: scaled K error ≤8.51e-15 and L error ≤8.53e-14 µm; both limits 1e-10. [Earlier failures and rectangle correction](reference-point-patterns.md) are retained. |
| Inhomogeneous K/L | Implemented | spatstat `Kinhom` | Two published retinal recordings (all cells, then each of two cell types: six cases), 96 radii: matched-intensity error ≤2.59e-11 against 1e-10 tolerance. [Scope](pathology-studies.md#reference-comparisons-on-published-cells). |
| Homogeneous, inhomogeneous and cross-type g | Implemented; reference agreement for homogeneous g and directed cross-g only | spatstat `pcf`; independent pair sums using public edge weights | [Homogeneous translation/isotropic g](reference-point-patterns.md#pair-correlation-agreement) agrees with analytic `pcf.ppp` within 3.40e-15 scaled error. Directed homogeneous/inhomogeneous cross-g pair sums agree within 1.91e-15 (limits 1e-10); the inhomogeneous check supplies Marklab's own intensities. Discretized `pcfcross` package curves fail exact equality: up to 1.16e-3 on the package grid and 1.07e-6 with 1,024 subdivisions, shrinking with refinement. No single-type inhomogeneous g comparison exists. Standard-border variants retain Python known-answer scope. |
| Directed homogeneous cross-type K/L | Implemented | spatstat `Kcross` | Four public amacrine/betacells direction checks: scaled K error ≤4.21e-16 and L error ≤4.27e-14 µm, both within 1e-10. Sparse-grid failure and package defaults retained; [conventions](categorical-pairs.md). Inhomogeneous cross-K is out of scope here: its type-specific intensity model and normalization need separate reference agreement; the homogeneous contract cannot be reused unchanged. |
| G, F and J | Implemented | spatstat `Fest` / `Gest`; matched finite J ratio | [Public amacrine/betacells](reference-point-patterns.md#empty-space-nearest-neighbour-and-j-agreement): exact reduced-sample F/G agreement on a declared 64×64 probe grid and polygon border distances, and exact J ratio at supported radii. This is not a direct `Jest` or arbitrary-probe claim. |
| ERL global envelopes | Implemented | [GET](https://stat.ethz.ch/CRAN/web/packages/GET/GET.pdf) | Executed GET 1.0-9 tied-curve oracle: exact bounds, depths and p=0.8. `pathology spatial-study` exports its within-slide null curves; GET reproduces the plug-in L-r p-values (error ≤2.08e-17) and bounds (≤5.08e-15) on six public retinal cases, all p = 1/40, and one synthetic case with p = 0.25 ([details](pathology-studies.md#reference-comparisons-on-published-cells)). Refit, g, cross-g and compartment null curves are exported but not compared. |
| Gibbs: Strauss/Geyer; Cox: Thomas/Matérn/LGCP | Implemented (Research) | spatstat, PyMC/NumPyro | Model-specific synthetic/backend checks. The [Bayesian Thomas cluster model](bayesian-thomas-cluster.md) adds known-answer, recovery and 200-replicate SBC checks and a descriptive spatstat `kppm` comparison on public data, where real-data fits mixed slowly. Binary Gibbs heuristics and pseudolikelihood are not general continuous-space Gibbs fits. |
| Neighborhood enrichment / interaction permutation | Categorical radius workflow implemented; [imcRtools comparison](categorical-enrichment.md#comparison-with-imcrtools-and-spiat) run | [Squidpy](https://squidpy.readthedocs.io/en/latest/api.html); exact fixed-count moments checked against small-graph answers and Cliff–Ord | [1,107 CODEX cells](categorical-enrichment.md): exact observed counts, analytic scaled error ≤2.85e-16 (limit 1e-12). Monte Carlo z-score agreement is unassessed. imcRtools 1.16.0 `testInteractions` (classic, histoCAT-style) on 3 public spots: identical graphs, per-cell neighbor statistic within 3.6e-15 for 1,163 ordered pairs, permutation p-values within 0.0158 (independent 9,999-draw streams). Its `histocat` and `patch` variants were not compared. Registered multimodal/stratified nulls retain separate Research scope. |
| Distance-dependent cell-type co-occurrence | Implemented | Squidpy `gr.co_occurrence` | [Four-radius CODEX comparison](categorical-cooccurrence.md): exact directed counts and native formula values; Squidpy error ≤3.56e-15 under its fixed floating-point bound. Edge-incidence normalization; no inferential or scimap-pipeline parity claim. |
| Multi-way ensemble co-patterning (cisMI, transMI) | Implemented for cell tables (Research) | R [SPACE](https://github.com/eschrom/SPACE) `census_table` / `measure_cisMI` / `measure_transMI` at commit 94f0a0f | [SPACE tutorial table, 9,757 cells](space.md#validation): census identical; observed co-information on reported grids within 1.4e-15 for 2,700 ensembles; BH and normal P within 1.3e-13. Randomized results agree as closely as two R runs do (10 µm Spearman of Z 0.9954 vs 0.9956); rare classes can change grid between runs in both implementations. transMI on six tiles of the same table: modularity within 6.0e-16 of R (two R defects fixed) for 1,350 ensembles and two factors; Spearman of Z 0.9980 vs 0.9979 between R runs. Patterns, diversity and image census not yet ported. |
| Global Moran/Geary and local Moran | Implemented | Squidpy, PySAL/ESDA | Global binary-radius Moran/Geary have exact PySAL isolate checks and [70-field CODEX agreement](validation-schurch.md) ≤2.50e-15. Local Moran remains Research: unconditional shuffling and population variance differ from default conditional LISA. |
| Graph centrality | Group degree, group closeness and average clustering implemented; individual betweenness out of scope | Squidpy / NetworkX | [19 CODEX cell groups](group-centrality.md), exact graph counts and native score error ≤3.34e-16 under 512 eps64 × (1 + absolute reference). These group summaries address cell-class graph position; individual betweenness is a separate path-flow statistic. |
| scimap spatial distance/count/interaction/aggregate | Native scimap pipeline out of scope | [scimap](https://scimap-doc.readthedocs.io/en/latest/tools/) | The listed native point-pattern and neighborhood primitives answer related questions. scimap's graph construction, aggregation and interaction-test conventions form a separate pipeline; normalized AnnData import does not establish that pipeline's parity. |
| Giotto spatial networks, enrichment and domains | Native Giotto objects and domain discovery out of scope | [Giotto Suite](https://giottosuite.com/reference/) | Its spatial-expression/network object model and domain discovery are separate from Marklab's declared cell-table statistics. Ligand–receptor mechanism inference additionally requires expression and a declared interaction database. |
| Patient-group comparison of slide summaries | Implemented; spicyR model out of scope | SciPy/NumPy; adjacent [spicyR](https://www.bioconductor.org/packages/devel/bioc/vignettes/spicyR/inst/doc/spicyR.html) | Two-group Max-T agrees on [35 CODEX](validation-schurch.md) patients (nine neighborhood fractions and four Moran/Geary endpoints) and [100 IMC](validation-jackson.md) patients (13 metacluster fractions: composition, not spatial arrangement). spicyR's cell-count weighting and response model target a different estimator; substituting Max-T is not a reproduction of that model. |
| Immune/tumor distance, mixing and neighborhoods | Declared compartment metrics implemented; [SPIAT mixing-score comparison](categorical-enrichment.md#comparison-with-imcrtools-and-spiat) run | Independent Python/GEOS and analytic geometry; adjacent [SPIAT](https://www.bioconductor.org/packages/release/bioc/html/SPIAT.html) | Radius-mixing counts use a Python oracle; contact/fragmentation use GEOS fixtures; signed interface distances have exact rectangle answers. These synthetic checks do not establish public-cohort performance. SPIAT 1.12.1 mixing scores equal the categorical radius counts' ratios exactly for 33 public cell-type pairs (3 more are undefined, where SPIAT reports 0); SPIAT's distance metrics have not been compared. |
| Epithelial architecture and invasive front | Front derivation, distance bands, fragmentation, lumen containment, contour curvature, skeleton branching, ITBCC-style budding hotspot, nuclear shape and spot/cell attachment implemented (Research) | Analytic geometry; adjacent [Bertillot et al. preprint](https://doi.org/10.1101/2025.08.11.669722) and [ICCR/ITBCC budding definition](https://pmc.ncbi.nlm.nih.gov/articles/PMC8820778/) | A square gland, its lumen, fragments and front offsets have exact areas, distances and turning angles; bar, plus, T and ring skeletons have exact branch and end counts; hotspot counts match hand-computed chords; spot covers are exact for inside, split and off-tissue disks. Regions and classes are declared upstream; Marklab does not segment or classify. No public real-data demonstration or agreement with a pathologist-drawn front yet. |
| Multiview contextual effects | MISTy model fitting out of scope | [MISTy / mistyR](https://master.bioconductor.org/packages/release/bioc/manuals/mistyR/man/mistyR.pdf) | Fitting and evaluating learned view contributions requires a declared multiview training/held-out design. Existing factors/regressions do not implement this decomposition. |
| Learned niches | Native niche-model fitting out of scope | [CellCharter](https://cellcharter.readthedocs.io/en/stable/graph.html), [UTAG](https://www.nature.com/articles/s41592-022-01657-2), [BANKSY](https://prabhakarlab.github.io/Banksy/) | Their feature aggregation, cross-image clustering and preprocessing are distinct models. Marklab consumes declared categorical observations; this does not reproduce the models or validate a predicted niche as biological ground truth. |
| Recurring cellular neighborhoods | Interoperable through published labels; clustering out of scope | [Schürch et al.](https://pmc.ncbi.nlm.nih.gov/articles/PMC7479520/) | The original nine neighborhood labels flow through the [Schürch patient analysis](validation-schurch.md). Native radius composition is not the published kNN-composition clustering. |
| QuPath GeoJSON and measurements | Interoperable for classified PUMA nucleus annotations; general measurements out of scope for this profile | [QuPath](https://qupath.readthedocs.io/en/stable/docs/tutorials/exporting_measurements.html) | [Biological PUMA ROI](qupath-import.md): 168 exact IDs/classes, centroid error ≤2.85e-14 µm against an independent shoelace calculation (limit 1e-9). Explicit calibration; no generic detection/cell measurement claim. |
| CellViT cells and embeddings | Optional normalized NPY and bundle adapters implemented; native `cells.json`/graph outputs out of scope for the current profile | [CellViT-Inference](https://github.com/TIO-IKIM/CellViT-Inference), [STHELAR](cellvit-import.md) | 128 real CellViT-SAM-H vectors pass exact NPY/columnar checks. Full normalized v1 CSV/manifest admission has fixed calibration and synthetic fixtures only; this is an adapter-specific evidence limit, not a prerequisite for other models. Native outputs have different layouts and need separately bound source image calibration and metadata. |
| HoVer-Net nucleus output | Native JSON adapter planned | [HoVer-Net](https://github.com/vqdang/hover_net) | Native `mag` can be null or nominal objective power, and does not bind physical MPP. An adapter needs a small reviewed native biological output with independent calibration; none has been found yet. The PUMA QuPath file is a corrected re-export, not a native HoVer-Net fixture. A search on 10 October 2026 found native output on TCGA slides (Waldron lab bucket, slide MPP independently available from GDC), but with no stated licence and no recorded HoVer-Net version or checkpoint, so it is not used. In whole-slide mode HoVer-Net's `bbox` is offset with swapped axes (`infer/wsi.py`), so an adapter must use `centroid` and `contour`. |
| HistomicsTK, StarDist, Cellpose, Mesmer, InstanSeg | Native project/model formats and segmentation out of scope | Upstream object/measurement exports | These tools produce distinct masks, objects and measurement exports. Use explicit calibrated cell-table mapping after upstream processing; it is not an automatic adapter for each tool, and Marklab does not run segmentation or train image models. |
| MCMICRO and Steinbock tables | Steinbock and MCMICRO quantification interoperable | [MCMICRO](https://mcmicro.org/parameters/specs.html), [Steinbock](https://bodenmillergroup.github.io/steinbock/latest/cli/export/) | [Steinbock paired-CSV adapter](steinbock-import.md) has exact Object joins, explicit axes/calibration, malformed-file tests and an upstream mock-fixture file check. The [MCMICRO adapter](mcmicro-import.md) reads MCQuant tables with explicit calibration; on nf-core's public tonsil test output (1,621 cells) its centroids and areas match the segmentation mask exactly and the OME-XML scale is applied. That output came from MCMICRO's component tools with an unrecorded MCQuant version; legacy column names are not admitted. |
| TIAToolbox and WSI viewers | Native viewer/project state out of scope | TIAToolbox / QuPath | Pixel reading is tested separately. Viewer annotations, slide databases and framework objects are distinct stateful formats, not implicit extensions of the region reader. |
| UNI, CONCH, Virchow, Prov-GigaPath, H-optimus embeddings | Generic vector/NPY inputs and declared-model cell provenance implemented; native model-export profiles out of scope | Upstream feature matrices/coordinates | NPY CLI and verified Arrow/Parquet checks cover non-CellViT dimensions. Tensor layout, pooling, normalization, coordinates and weights are producer declarations. CellViT file evidence does not validate other encoders or imply their native exports load directly. |
| ABMIL, CLAM, TransMIL slide models | Architectures and native model artifacts out of scope | Upstream scores/embeddings | Patient evaluation accepts explicitly declared endpoint values. That is not a native model-output adapter or evidence that MIL attention identifies a cell-level mechanism. |
| AnnData `.h5ad` | Interoperable: bounded in-memory profile | [AnnData](https://anndata.readthedocs.io/en/stable/fileformat-prose.html) 0.12.4 | Python client and sparse round trips, including [1,107 observed Schürch cells](validation-schurch.md) in a derived H5AD with exact identities/values/coordinates. Physical window/patient/group declarations required; no backed/lazy arrays or universal AnnData schema claim. |
| SpatialData / OME-Zarr | Bounded STHELAR table extraction example; general ingestion out of scope | [SpatialData](https://spatialdata.scverse.org/en/latest/tutorials/notebooks/notebooks/examples/models1.html), [OME-NGFF](https://ngff.openmicroscopy.org/specifications/index.html) | The [STHELAR script](cellvit-import.md) preserves selected published Zarr arrays and joins exact cell IDs. It is not a general image/label/point transform adapter; physical units require a separately justified source contract. |
| OME-TIFF metadata and WSI pixels | Pixel reader implemented; OME-XML mapping out of scope | [`wsi-rs` / OME-TIFF](https://ome-model.readthedocs.io/en/stable/ome-tiff/) | [Public Aperio SVS pixels](formats.md#public-image-check) agree with OpenSlide within the existing lossy-codec limits. A codec does not establish OME axes, plane or physical-calibration semantics; no OME-XML adapter or WSI writer is provided. |
| CSV and Parquet cell tables | Read/write implemented | CSV / Arrow-Parquet | Synthetic round trips/parser tests; the calibrated [Schürch CSV](validation-schurch.md) feeds all 70 multiplex fields. The native CSV/Parquet round trip preserves 1,107 observed cells exactly; [format scope](formats.md). |
| Arrow IPC / Parquet embedding artifacts | Internal storage implemented | Arrow / Parquet | Exact actual-file transport of 163,840 published CellViT values and 128 IDs, with explicitly synthetic test provenance. Strict physical profile deliberately rejects arbitrary pyarrow artifacts; not a universal interchange format or verified checkpoint claim. |
| Structure factor and hyperuniformity | Mark spectra implemented (Research: synthetic checks only); [unmarked structure factor with `H` and `α`](structure-descriptors.md#structure-factor-and-hyperuniformity-numerics-hyperuniformity) implemented (Research) | [structure-factor](https://for-a-few-dpps-more.github.io/structure-factor/) | Scattering intensity at the allowed wavevectors of a fully observed rectangle. Five public CODEX squares agree with structure-factor 2.2.1 (`S` ≤3.5e-13, `H` ≤2.2e-8, `α` ≤3.7e-6) after a first run failed on a last-bin boundary convention. No uncertainty for `H` or `α`; irregular tissue windows are not supported. Fixed-label spectra do not establish unmarked hyperuniformity. |
| Minkowski functionals | Implemented | scikit-image | Area/perimeter/Euler-characteristic worker/oracle tests; pixel connectivity and resolution must be declared. |
| Lacunarity and fractal dimension | [Box-counting dimension and gliding-box lacunarity](structure-descriptors.md#box-counting-dimension-and-lacunarity-numerics-lacunarity-fractal-dimension) implemented (Research) | [`lacunaritycovariance::gblemp`](https://cran.r-project.org/package=lacunaritycovariance); box-counting definitions | Declared mask, pixel size, grid origin and box sizes. 40 public CODEX cell masks agree with gblemp 1.1-9 (lacunarity error ≤8.3e-13) and an independent R box count (regression error ≤1.1e-15). The dimension is a descriptive slope over declared scales without uncertainty, not evidence that tissue is fractal. |
| Landy–Szalay two-point estimator | [Landy–Szalay ξ against a declared random catalog](structure-descriptors.md#landyszalay-two-point-correlation-numerics-landy-szalay) implemented (Research) | [TreeCorr](https://rmjarvis.github.io/TreeCorr/) (Corrfunc did not build here) | The caller supplies the random catalog that samples the tissue. Pair counts on 15 public CODEX patterns are identical to TreeCorr 5.1.4 and ξ matches the 1993 normalization exactly; TreeCorr's own `n²/2` normalization differs by up to 0.19 for 38 cells. No uncertainty is reported. |
| Diversity indices | Shannon entropy/effective class count implemented; other ecological indices out of scope | Independent Python composition formula; adjacent [vegan](https://search.r-project.org/CRAN/refmans/vegan/html/diversity.html) | Soft-class composition compares against the checked-in Python oracle, including effective class count 2.8387205126507515. This synthetic profile does not establish native vegan parity or validate biological diversity estimates. |
| Morisita dispersion, ecological co-occurrence/niche models | [Morisita dispersion](structure-descriptors.md#morisita-quadrat-dispersion-numerics-morisita-dispersion) implemented (Research); niche models planned | `vegan::dispindmorisita` / ecological count models | Declared quadrat width and origin; only whole-tissue quadrats count. 30 public CODEX cases agree with vegan 2.7-6: identical counts, index error ≤2.0e-16, p-value ≤5.8e-15. Ecological association/niche models need a specified abundance/environment model; categorical composition alone supplies neither. |
| Kulldorff-style Bernoulli scan | Stratified circular scan implemented; general SaTScan parity out of scope | Exact enumeration and [Kulldorff's likelihood ratio](https://www.tandfonline.com/doi/abs/10.1080/03610929708831995) | The six-cell test enumerates all 20 fixed-count allocations, checks the score within 1e-12 and the Monte Carlo p-value against exact 2/20 within 0.025. Public-data validation remains absent; this is Research evidence. |
| Variograms | Implemented | [SciKit-GStat](https://mmaelicke.github.io/scikit-gstat/) | [Public two-marker empirical variogram](variograms.md): 1,107 CODEX cells, exact bin counts, scaled error ≤3.98e-15 (limit 1e-12). Weighted/projected/scalar-permutation variants retain separate Research scope. |
| Kriging | [Ordinary kriging](structure-descriptors.md#ordinary-kriging-numerics-ordinary-kriging) with a declared variogram and leave-one-block-out checks implemented (Research) | [SciKit-GStat](https://mmaelicke.github.io/scikit-gstat/reference/kriging.html) | 600 public CODEX cells: predictions agree with SciKit-GStat 1.0.23 within 4.2e-14 and variances within 4.9e-15 (limit 1e-8). The variogram fit and the constant-mean/stationarity assumptions remain the user's; the held-out summary checks them. |
| Persistent homology | Implemented via pinned backend (Research) | GUDHI 3.13.0 | Alpha/witness persistence, diagrams and stability tests; no cohort biomarker claim. GUDHI is the implementation, so no independent persistence comparison exists yet. |
| Mapper | [Mapper graph of cell features](structure-descriptors.md#mapper-graph-of-cell-features-numerics-mapper-graph) implemented (Research) | [KeplerMapper](https://kepler-mapper.scikit-tda.org/en/latest/) | One-dimensional lens, uniform interval cover, single-linkage clusters at a declared threshold. 12 graphs on public CODEX cells are identical to KeplerMapper 2.1.0 with DBSCAN (`min_samples=1`). No stability or significance assessment; multi-dimensional lenses are not implemented. |
| Optimal transport between patterns | Implemented | Python Optimal Transport | Balanced/unbalanced Sinkhorn, partial/full fused Gromov–Wasserstein and [unbalanced fused Gromov–Wasserstein with KL-relaxed marginals](research-workflows.md#transport-alignment-bayes--fused-gromov-wasserstein); analytic and independent-optimizer known answers for the unbalanced form; biological correspondence unvalidated. |
| Graph motifs and communities | Motifs implemented; native community fitting out of scope | NetworkX / Leiden | Typed triangles have exhaustive known-answer checks; spectra/Hodge have analytic checks. Community fitting adds an objective, resolution and cross-sample label-alignment problem; no learned-community or niche claim is made. |
| Bond-orientational order | [k-atic order ψ_k](structure-descriptors.md#bond-orientational-order-numerics-bond-orientational-order) implemented (Research); Steinhardt 3-D order out of scope | [freud](https://freud.readthedocs.io/en/latest/modules/order.html) | Declared nearest-count or radius neighbors and a border rule. On 734 public tumor cells, double-precision ψ₆ on freud 3.6.1's neighbor lists agrees within 2.2e-16; freud's own single-precision values differ by up to 7.0e-4, failing the prespecified single-precision bound in 5 of 6 cases. Crystal ordering and tissue-cell anisotropy answer different questions. |
| Information-theoretic dependence | Entropy implemented; [mutual information between two marks](structure-descriptors.md#mutual-information-between-two-marks-numerics-mutual-information) implemented (Research) | [scikit-learn MI](https://scikit-learn.org/stable/modules/generated/sklearn.feature_selection.mutual_info_regression.html) | KSG estimates agree with scikit-learn 1.9.1 in 59 of 60 public cases (≤3.6e-14); one failed by 1.07e-5 where scikit-learn's tie-breaking noise splits a tied value. Under autocorrelated independent fields the variogram-matching null held size (0.040–0.065 at ranges 10–200 µm) while ordinary permutation rejected up to 0.89. Calibration covers one spot's geometry and Gaussian fields; entropy/covariance/kernel dependence are not claimed as MI. |
| 3-D, causal, longitudinal, simulation and SBI | Implemented research families | GUDHI, SciPy, PyMC/NumPyro, analytic examples | Research tier; see [research workflows](research-workflows.md). No independent reference comparison or public real-data demonstration yet. |

**Planned** rows are missing work, not deliberate boundaries: ecological niche models,
a native HoVer-Net adapter, and comparison with SPIAT's distance metrics.

## Command maturity

Each row applies to the named command **and every subcommand**, unless a more specific row
overrides it. Cached `project` routes inherit their method's evidence. Research is a ceiling
where an individual model still lacks a complete public demonstration; a command named
`validate` or `agreement` does not certify every model or configuration it can run.

| Command family | Tier | Evidence and limit |
|---|---|---|
| `analyze`, `batch` | Research | Known-answer smoke tests and [shuffle calibration](validation-methodology.md#scheduled-calibration) of the shipped primary endpoint: no evidence of size above 5% (observed 3.9%–6.2%) across five layouts and three prevalences, power ≥ 95% for small foci. One clone covering a quarter of a small window was detected in 0.6% of replicates. No independent implementation agreement or public patient cohort. Batch adds no scientific evidence. |
| `classical` | Experimental for observed rectangular border, translation and isotropic K/L; Research for CSR inference | Hand-counted K/L and public spatstat agreement; the observed-curve comparison does not calibrate the CSR test or its null distribution. |
| `nearest-space` | Experimental for the demonstrated rectangular observed F/G/J curves; Research for inference | SciPy known answers and exact public amacrine/betacells reduced-sample F/G and matched-J agreement; no patient cohort or multi-shape calibration claim. |
| `prepost` | Research | Axis/descriptive-change tests; not a paired-patient causal or equivalence claim. |
| `multimodal` | Research | Synthetic registration/neighborhood/factor tests and [shuffle calibration](validation-methodology.md#scheduled-calibration) of the lymphocyte/MMR enrichment under all four source-section null models: conservative size (1.2%–4.2%) on two layouts and two prevalences, full power on the immune-association scenario. No validated paired H&E/IHC cohort. |
| `cohort` | Experimental for two-group `max-t`; Research for other designs | Patient-unit exact/synthetic checks plus independent real-group Max-T agreement on Schürch and Jackson. Other designs are not promoted by these two-group examples. |
| `study` | Experimental for `binary_symmetric_including_isolates` Moran/Geary with two-group Max-T; Research for other weight policies | Synthetic/oracle, durable-resume and [Schürch multiplex cohort](validation-schurch.md) checks, which used only that weight policy; no multi-shape calibration or clinical validation. |
| `pathology maps`, `pathology composition`, `pathology scan` | Research | Hand/exhaustive examples and synthetic calibration; public demonstration pending. |
| `pathology architecture` | Research | Exact square, offset, lumen, turning-angle, skeleton and hotspot checks; no real-data, pathologist-front or pathologist budding comparison. |
| `pathology spatial-study` | Research overall; Experimental for fixed-bandwidth rectangular L-minus-r | Published retinal K/L agreement and analytic intensity check. Patient groups, g/cross-g, irregular windows and fitted-null inference not covered. |
| `bayes` | Research | Model-specific analytic, worker, SBC and backend checks. Nonconverged fits remain diagnostic only. No blanket model calibration claim. |
| `bayes vector-semivariogram` | Experimental for the unweighted observed curve | Hand-computed/rotation tests and [public SciKit-GStat agreement](variograms.md); this descriptive command does not fit a Bayesian model. Weighted and inferential extensions remain Research. |
| `project` | Inherits the underlying method; Research unless specifically promoted | Durable transaction/identity/resume and method tests; caching does not promote scientific maturity. |
| `project categorical-pair` | Experimental for directed homogeneous cross-K/L with standard border correction; Research otherwise | [Public spatstat `Kcross` agreement](categorical-pairs.md) in four directions; translation/isotropic variants and inhomogeneous cross-K are not covered. |
| `graph` | Research | Matrix/graph oracles and synthetic signal/permutation tests; no tissue-cohort validation. [Research workflows](research-workflows.md#graphs-graph). |
| `graph group-centrality` | Experimental for the three observed group scores | Hand/disconnected graph tests and [public Squidpy/NetworkX agreement](group-centrality.md); no inferential or cohort-biomarker claim. |
| `topology` | Research | GUDHI/scikit-image and known topology/stability tests; tissue biomarker interpretation unvalidated. |
| `spatial3d` | Research | Cuboid/voxel/registration and known 3-D answers; real serial-section patient claims unvalidated. |
| `simulate` | Research | Solver/conservation/bound/seeded-process tests; no fitted mechanistic tissue validation. |
| `microbio preprocess` | Experimental for linear-mode Bruker flex reading and the five steps and binning; Research for reflector mode | [Agreement with readBrukerFlexData, MALDIquant and flexAnalysis](maldi-preprocessing.md#evidence) on public example spectra and 11,055 RKI acquisitions. Not checked on a current Biotyper software version. |
| `microbio cross-validate-ast` | Research | [scikit-learn agreement](maldi-ast-prediction.md#evidence). No patient-grouped, external-site or clinical validation; no model for new isolates. |
| `neural` | Research | Synthetic point-process/generator/SBI checks; no image-model training or real-cohort performance claim. |
| `causal` | Research | Analytic/synthetic identification and sensitivity controls; observational tissue association is not causation. |
| `longitudinal` | Research | Analytic/synthetic filter/smoother tests; no repeated-patient clinical validation. |
| `registration` | Research | Synthetic deformation/landmark checks; deformation alone does not establish biological change. |
| `numerics` | Research | Known numerical answers and finite/resource checks; local Moran has its own null conventions. |
| `numerics categorical-co-occurrence` | Experimental for observed radius-dependent scores | Hand-counted duplicate/boundary cases and [public Squidpy agreement](categorical-cooccurrence.md); no calibrated null or patient test. |
| `numerics categorical-enrichment` | Experimental for observed counts/exact null descriptors; Research for Monte Carlo inference | Hand-counted fixed-margin cases and [public count/moment agreement](categorical-enrichment.md). Monte Carlo z-score package agreement and multi-shape calibration remain unassessed. |
| `space census`, `space cis`, `space trans` | Research | Hand-computed census, smoothing and modularity; synthetic striped/random controls; [R SPACE agreement](space.md#validation) on its public tutorial table. Exploratory ranking; the transMI null shuffles only pairwise divergences, so it is not calibrated patient-level inference. |
| `inspect-slide`, `extract-region` | Experimental for the tested Aperio region; Research ceiling for other codecs | Public-file/OpenSlide pixel agreement plus codec fixtures and bounded reads; no statistical inference or universal scanner/OME guarantee. |
| `smoke` | Research evidence generator | Production synthetic controls; smoke success is not calibration. |
| `backend doctor` | Research ceiling; utility | Locked-package/runtime checks; not model suitability or numerical accuracy. |
| `policy` | Research ceiling; utility | Evaluates declared flags; does not independently verify evidence. |
| `profile-plan`, `help` | Research ceiling; utility | Profiling instructions/help only; no scientific claim. |

## Work priorities

1. **Calibration:** `analyze` and `multimodal` now have size and power across several layouts,
   prevalences and null models. Next are stratified permutation, the default 999
   permutations, and the pathology workflows. Unsupported cases remain support tests, not
   size or power evidence. Fitted-null and irregular-window inference stay scoped until
   checked.
2. **Planned methods:** the Planned rows above, each with a concrete tissue question, a
   reference comparison and documented failure modes before promotion beyond Research.
3. **Research families:** independent reference comparisons and public real-data
   demonstrations for the [research workflows](research-workflows.md).

## How the capabilities combine

Marklab does not impose one universal pipeline. The same admitted cell table can support different
questions, but each branch must keep its own observation window, mark definition, null model and
unit of inference.

### One section

`marklab analyze` takes cells with a binary mark or probability and asks whether those marks are
arranged differently than fixed-count label shuffling on the same coordinates. It combines
multiscale clustering/dispersion, directional summaries, a mark spectrum, component modes and
local over-representation maps in one versioned result. It is a useful broad screen, but its
outputs are related views of one marked pattern rather than independent confirmatory tests.

`classical` and `nearest-space` ignore the mark and ask whether all retained cells depart from CSR
through K/L, G, F and J. Directed categorical pairs, cross-g, co-occurrence and enrichment then ask
which declared classes contribute to the pattern. Scalar and vector marks can branch into
variograms, covariance, mutual information, autocorrelation, kriging, embedding kernels or graph
signals. The [classical workflow](classical-spatial-workflow.md) and [categorical-pair
conventions](categorical-pairs.md) define the demonstrated estimators.

Annotations create another branch. `pathology maps` measures marker or density change with distance
from supplied structures and maps neighborhoods and hotspots. `pathology architecture` derives
front-relative, gland, lumen, fragmentation, skeleton, budding and nuclear-shape summaries from
declared epithelium and nuclei, and can attach spots and registered cells. `pathology composition`,
`spatial-study` and `scan` move those quantities toward patient comparison, spatial curves and
enrichment scans. See [pathology maps](pathology-maps.md), [architecture](pathology-architecture.md)
and [pathology studies](pathology-studies.md). The pathology study routes have 23 synthetic
scenarios but no real-study calibration; sparse cell types require the support rule to be declared
before analysis.

### Two sections, stains or timepoints

`prepost` compares two completed section analyses axis by axis. Because the sections contain
different physical cells, it describes change between samples; it is not a paired-cell treatment
effect. `multimodal` registers an H&E/IHC pair, matches declared observations, computes cross-stain
neighborhood and territory summaries, and supports joint factor models. Its spatial factor can use
a dense GP or mesh SPDE prior; the synthetic evidence shows recovery of a common smooth factor,
not equality of the priors or validation on tissue
([SPDE details](python-backends.md#spde-spatial-factor)).

Registration uncertainty can feed serial-section and 3-D workflows. Embedding or class features
from modalities may instead enter factor models, learned kernels or patient-level predictive
fusion. These routes answer different questions and should not be treated as interchangeable
“multimodal scores.”

### Slides, sites and patients

`study` runs a declared multiplex panel from fields and slides to patient-level endpoints, supports
resume, and has a thin AnnData client. `cohort` then applies paired, blocked, clustered,
covariate-adjusted, multivariate, Max-T, bootstrap or margin-based inference. `pathology` study
routes provide specialized composition, spatial-curve and scan endpoints. A patient with many cells
still counts as one patient. The [multiplex study](multiplex-study.md) and public [Jackson](validation-jackson.md)
examples show the supported evidence paths and their scope.

`project` provides durable, identity-checked caching for selected native and Python-backed methods.
Caching avoids repeating a completed fit; it does not upgrade that fit's scientific maturity.

### Model-specific interpretation retained

`bayes advanced-cluster` fits a [Bayesian Thomas cluster model](bayesian-thomas-cluster.md) with
latent parents sampled by birth-death MCMC and pooled cluster parameters. Dense public B-cell fits
mixed slowly, so nonconverged outputs are diagnostic. `bayes anisotropic-gp-3d` requires pairwise
squared displacements that identify all three axis scales. Its
`posterior_mean_anisotropy_metric = diag(1 / mean(length_scale_axis)^2)` is a plug-in metric in
inverse square micrometres, not an average of per-draw inverse-square lengths and not the
dimensionless anisotropy matrix used by physical-radius K.

The crossed/nested Gaussian hierarchy's `variance_partition` averages posterior shares of random-
effect and residual variance over supplied rows, including sum-to-zero centering and standardized
random slopes. It excludes fixed effects and is neither R-squared nor the sample variance of fitted
effects. `bayes normal-mean-sbc` tests discrete posterior ranks; its coverage diagnostic uses
analytic 95% intervals regardless of the requested posterior draw count.

`spatial3d clone-models` fits patient intercepts with the clone A-minus-B contrast and residual
scale. Contrast uncertainty includes the free nuisance parameters; at the residual-SD floor it is
conditional on that floor. Unidentifiable or bound-hitting contrasts fail. `patient_feature_means`
contains observed patient means, not fitted intercepts.

## Recommended exploratory slide workflow

This is a decision path, not a single turn-key command. It deliberately starts with inexpensive
descriptions and QC, then adds models only when a specific biological question requires them. That
ordering is also used in the official [Squidpy spatial analysis example](https://squidpy.readthedocs.io/en/stable/notebooks/tutorials/tutorial_vizgen.html),
which moves from QC and spatial visualization to graphs, co-occurrence and autocorrelation; the
reference supports the sequencing, not a claim of pipeline parity.

1. **Freeze the observational contract.** Record the specimen and patient identity, physical
   calibration, coordinate axes, tissue window and holes, upstream segmentation/classification
   provenance, mark definitions and missing-data rules. Decide whether the intended null is CSR,
   random labeling, a fitted process, within-block permutation or patient-label permutation.
2. **Inspect support before statistics.** Plot cell positions, classes, probabilities, annotations
   and density against the tissue outline. Check duplicates, non-finite rows, class counts, sparse
   regions, edge proximity, excluded holes and whether coordinates agree with the image. Use WSI
   inspection or extraction only where its codec/calibration profile applies.
3. **Build a descriptive baseline.** Report cell counts, area, density, class composition and
   support by region. If crowding or voids is the question, inspect unmarked K/L or G/F/J over
   biologically meaningful radii using the appropriate input eligibility and edge support.
   For marked cells, use `analyze` or directed categorical functions to see clustering,
   dispersion, cross-type direction and local enrichment. Treat this stage as exploration.
4. **Ask one focused spatial question at a time.** Use g when the local distance band matters, K/L
   for cumulative scale, signed profiles for interfaces, Moran/variograms for a continuous field,
   spectra or structure factor for wavelengths and direction, nematic order for axial alignment,
   graph methods for a declared network, and topology for persistence across scale. Choose radii,
   graph construction, bandwidths and edge correction before inspecting group differences.
5. **Check sensitivity to representation and null.** Compare justified edge corrections, intensity
   or bandwidth choices, grid resolution, coordinate perturbation and registration uncertainty.
   Inspect unavailable statuses instead of dropping unsupported radii or sparse classes. A stable
   descriptive curve under these checks is more useful than a large list of fragile p-values.
6. **Escalate to a model for a named gap.** Use an LGCP or Gibbs/cluster model for a point-process
   hypothesis, a GP/CAR/SPDE model for a field, factors or kernels for multimodal structure, and a
   longitudinal, causal or simulation model only when its assumptions match the design. Run the
   corresponding convergence, sensitivity, SBC, agreement or posterior-predictive checks; none is
   a substitute for the others.
7. **Keep tuning outside the reported test data.** For embeddings, kernel weights, bandwidths,
   ridge penalties, fusion or other supervised choices, fit every preprocessing and tuning step on
   training patients and evaluate on outer held-out patients. Reusing the same finite data for
   selection and evaluation biases the performance estimate; nested evaluation addresses that
   specific risk ([Cawley and Talbot, 2010](https://jmlr.org/papers/v11/cawley10a.html)).
8. **Move to population inference only after endpoint definition.** Reduce fields, slides and
   repeated specimens to prespecified patient-level endpoints, preserve pairing, blocks, clusters
   and sites, and use Max-T or another declared multiplicity strategy when several endpoints remain.
   Reserve a public or external cohort for validation when a predictive or biomarker claim is the
   goal.
9. **Report boundaries with the result.** State which findings are descriptive, exploratory or
   inferential; name the unit, null, window, scale and unavailable cases; show uncertainty and model
   diagnostics. A significant pattern shows incompatibility with its null, not its cause. A null
   result is not equivalence without a prespecified margin.

For an exploratory single-slide analysis, the most defensible default is therefore: verify the
window and cell table; map density and classes; run one marked, cross-type or unmarked analysis
tied to the biological question; inspect direction/spectral or interface
structure only when relevant; and repeat the result under the few estimator/null choices that
could materially change it. Bayesian, graph, topology and simulation families are powerful second
steps when the descriptive result identifies the model they need to answer.

## Execution contracts

- **Reproducible**: fixed input, settings, seed, implementation and numerical environment
  support replay. Cross-platform bitwise identity is not established for every backend.
- **No silent zeros**: a measurement that cannot be computed (too few cells, region too
  small) is reported as unavailable with the reason.
- **Bounded**: commands have limits on work and memory and stop with an error instead of
  running out of memory.
- **Transactional directories**: study/analysis publication exposes a complete output
  directory only after the run succeeds. File-producing utilities have their own contracts.

## Before relying on a result

- A significant result shows incompatibility with its declared null. It does not explain why.
- A non-significant difference does not show that two groups are the same. Use an
  equivalence test with a margin chosen in advance.
- When comparing patients, count patients, not cells.
- The spread of patient scores describes how patients vary. It is not a confidence
  interval for the average.
- `marklab backend doctor` checks that the Python environment is installed correctly. It
  does not check whether a model suits your data.

## Known limitations

- **`bayes advanced-cluster`** mixes slowly on dense cell aggregates. On three public B-cell
  spots, four chains of 8,000 draws converged for one of four fits under the default policy,
  and the single-scale Thomas model under-predicted close neighbours. Non-converged fits are
  published as such and are diagnostic only. See the
  [Bayesian Thomas cluster model](bayesian-thomas-cluster.md).
- The gridding-sensitivity check for FFT-based statistics uses a fixed 25% tolerance as a
  rough guide. It is not a formal agreement test.
- A real-data pilot of the joint location–embedding model did not converge. Treat its
  output as diagnostic only.
- Linux is the CI reference platform for the Python backends. Changes that have not yet
  run in CI have macOS evidence only; macOS results cannot substitute for Linux.

Windows binaries are built for each release but are not yet tested end to end.
