# Requirements registry

Statuses are `planned`, `active`, `blocked`, `complete`, `deferred`, or `rejected`. Closure requires evidence in the linked ledger or handoff.

## Program foundations

| ID | Requirement | Prerequisites | Owner | Status | Public/result effect | Closure evidence |
|---|---|---|---|---|---|---|
| PLAT-01 | Project, artifact, and workflow operating system | WS-A baseline | Lead | active | New project/workflow surface | Pending |
| WF-01 | Typed composable workflow DAG | PLAT-01 | Lead | active | New workflow schema/API | B-04 single-node typed slice complete; general composition/resume remains |
| BACK-01 | Versioned backend registry | PLAT-01, WF-01 | Lead | planned | Backend manifests/CLI | Pending |
| DATA-01 | Identity, hierarchy, modalities, coordinates, and units | WS-B | Lead | active | New stable data contracts | C-01 identity/hierarchy, C-02 coordinates, C-03 artifact substrate, and C-04 cell-embedding substrate complete; C-05 logical multiscale tables/context are implemented, while links/physical persistence, C-06, and broader durable payload alignment remain |
| GEO-01 | Exact windows, compartments, boundaries, and geometry plans | DATA-01 | Lead | planned | New geometry/result families | Pending |
| EMB-CORE | CellViT single-cell embedding modality | DATA-01, artifact layer | Lead | complete | New artifact contract | C-04 commits `8b90141`–`55d1c8b`; bounded cell table/import/provenance/Arrow/Parquet/scale evidence complete; authorized corpus remains explicitly non-promotable until four promotion fields are supplied |
| EMB-PATCH | Patch/region/slide embeddings and links | DATA-01, artifact layer | Lead | active | New artifact contract | C-05 expected sets, patch context/footprints, and typed logical tables pass focused resource/digest/scan gates and dual review; overlap, links, records, physical profiles, graph publication, scale evidence, and closure remain; no real source profile promoted |
| BAY-01 | Bayesian model IR and prior system | PLAT-01, DATA-01 | Lead | planned | Experimental model schema | Pending |
| BAY-02 | Posterior artifacts and diagnostics | BAY-01, BACK-01 | Lead | planned | Fit-state/result contracts | Pending |
| UX-01 | Interactive computational-pathology workbench | PLAT-01, WF-01 | Lead | planned | New application | Pending |

## Scientific requirement families

| ID | Requirement | Prerequisites | Owner | Status | Claim ceiling before closure | Closure evidence |
|---|---|---|---|---|---|---|
| FND-01 | Typed identity and cohort hierarchy | DATA-01 | Lead | complete | Infrastructure only | Commit `a1260445a383c59381b2c7c7881cebb10e10c156`; `handoffs/C-01.md` |
| FND-02 | ObservationWindow2D | FND-01 | Lead | planned | Infrastructure only | Remote sampled-patch geometry exists; explicit union-window reconstruction and tissue-mask boundary pending |
| FND-03 | Reusable exact spatial geometry plan | FND-02 | Lead | planned | Infrastructure only | Pending |
| FND-04 | Typed marks and measurement provenance | FND-01 | Lead | planned | Infrastructure only | Pending |
| FND-05 | Cell/patch embedding artifacts | FND-01, FND-04 | Lead | active | No model-specific claim | C-04 bounded cell-table substrate complete and C-05 logical patch/region/slide tables exist; FND-04/C-06 measurement provenance plus C-05 overlap, links, records, and physical persistence remain prerequisites for full closure |
| FND-06 | Inference designs/randomization units | FND-01 | Lead | planned | No cohort claim | Pending |
| FND-07 | Scientific provenance and schema evolution | FND-01–06 | Lead | planned | No stable new result | Pending |
| PP-01 | Homogeneous Ripley K/L | FND-02, FND-03, FND-06 | Lead | planned | Unsupported | Pending oracle/calibration |
| PP-02 | Inhomogeneous K/L | PP-01, intensity contract | Lead | planned | Unsupported | Pending oracle/calibration |
| PP-03 | Pair correlation, cross-K, cross-g | PP-01, PP-02 | Lead | planned | Unsupported | Pending oracle/calibration |
| PP-04 | F/G/J and NN distributions | PP-01 | Lead | planned | Unsupported | Pending oracle/calibration |
| MRK-01 | General marked-process functions | FND-04, PP plans | Lead | planned | Current narrow endpoints only | Pending |
| SIG-01 | Spatial autocorrelation and variograms | FND-04, weights | Lead | planned | Unsupported | Pending |
| NIC-01 | Multiscale neighborhoods/niches | FND-03, FND-04 | Lead | planned | Current MMR-specific descriptions only | Pending |
| COH-01 | Patient-level cohort inference | FND-01, FND-06 | Lead | active | No population claim | FND-01 hierarchy/replication substrate complete; inference remains pending FND-06 |
| CMP-01 | Spatial fingerprints/two-sample tests | COH-01 | Lead | planned | Descriptive current comparison only | Pending |
| EQV-01 | Equivalence/noninferiority | COH-01, CMP-01 | Lead | planned | Descriptive margins only | Pending |
| EMB-01 | Spatial dependence of embeddings | FND-05, SIG-01 | Lead | planned | Unsupported without provenance | Pending |
| CLN-01 | Clone/CNA downstream analysis | FND-04, GEO-01, COH-01 | Lead | planned | Unsupported | Pending |
| REG-01 | Nonrigid/probabilistic registration | BACK-01, GEO-01 | Lead | planned | Rigid/affine only | Pending |
| FR-01 | Graph spectral/wavelet summaries | Stable graph/weights | Lead | planned | Research only | Pending |
| FR-02 | Joint cell/patch representation science | FND-05, COH-01 | Lead | planned | Research only | Cell vectors found; canonical patch vectors/links still absent |
| FR-03 | Partial/unbalanced/fused transport | BACK-01, data contracts | Lead | planned | Descriptive alignment only | Pending |
| TOP-01 | Topological/morphological laboratory | Geometry, cohort | Lead | planned | Research only | Pending |
| GEN-01 | Mechanistic/neural simulators | PLAT-01, validation framework | Lead | planned | Research only | Pending |
| CAU-01 | Causal/interference laboratory | Eligible study design | Lead | planned | Association only | Missing eligible data |
| DIM-01 | 3-D/serial/longitudinal contract | DATA-01, REG-01 | Lead | planned | 2-D only | Pending |

## Execution workstreams

| ID | Task/workstream | Parent requirements | Owner | Status | Writable scope | Acceptance |
|---|---|---|---|---|---|---|
| A-01 | Install authoritative state/control plane | PLAT-01 | Lead | complete | `AGENTS.md`, `docs/implementation/**` | Required files, immutable-plan hash, ID coverage, and scope check recorded |
| A-02 | Reproduce current baseline | A-01 | `a02_baseline` (read-only runner) | complete | No tracked writes; generated build output only | All gates passed after clean-tree package rerun; exact evidence in validation/performance ledgers |
| A-03 | Contract and migration inventory | A-01 | Lead + read-only analysis | complete | Implementation docs only | Every current API/CLI/config/result/artifact/feature/workflow/test surface has a disposition |
| B-01 | Workspace architecture decision | A-01–A-03 | Lead | complete | Root Cargo metadata/architecture test/docs only | Commit `e8d57eed0c4b39bd651b7393e7af25b5a10a7558`; red/green contract, root/fuzz metadata, compatibility, workspace build/Clippy, package, and scope gates pass |
| B-02 | Compatibility shell | B-01 | Lead | complete | Compatibility parity test/docs only | Commit `f1bcc94d4a5f96825fae32676630304f31d332ea`; direct marked/multimodal parity, 65 default, 26 WSI/CLI, 35 CLI-only, 16 output, and no-default gates pass |
| B-03 | Workspace policy | B-01 | Lead | complete | Workspace CI/policy/architecture tests and cfg-only warning fix | Commit `ab3df430deb547a105608cad2f799fcf52fc9e66`; red/green policy/Clippy, eight-row matrix, architecture tests, YAML parse, and clean package pass |
| B-04 | Minimal project/workflow vertical slice | B-01–B-03 | Lead | complete | Focused project/workflow crates, root adapter, manifests/lock, tests/docs | Commits `bcc9450ef0a2ba89f3383904a78834925f3fcf17` and `ac7da28da082f795381da0a03e8437fc4a774262`; all focused and WS-B exit gates pass |
| C-01 | Typed identities/hierarchy | WS-B | Lead | complete | Focused core/data files | Commit `a1260445a383c59381b2c7c7881cebb10e10c156`; 14 hierarchy regressions, 430-test workspace gate, full benchmark, independent re-review |
| C-02 | Units/frames/dimensions/transforms | C-01 | Lead | complete | Focused core/data files | Commit `c676cfd732d02d6202bff8cea47fcf74b5bfd8e7`; 10 coordinate regressions, 440-test workspace gate, independent re-review; exact registry package resolution deferred by DEC-0018 |
| C-03 | Artifact catalog/immutable tables | B-04 | Lead | complete | Project/artifact files | Commits `97119cdfec7dcfe9da9375515e3d780991003888` and `488d3bd65b5a9e17c7cc1849700e13d2d8eb771f`; 471-test workspace gate, store/catalog adversarial suite, fuzz/dependency/compatibility gates, query-boundary regression, and independent final review; `handoffs/C-03.md`. DATA-01, FND-07, WS-11, WS-25, and WS-C remain open |
| C-04 | CellViT embedding table | C-01, C-03 | Lead | complete | Data/artifact/import tests | Commits `8b90141`–`55d1c8b`; exact domain/provenance/link schemas, bounded NPY/CSV and Arrow/Parquet paths, fuzz/differential/resource gates, 32-bundle reconciliation, 10k/1M scale, and independent closure review; `handoffs/C-04.md` |
| C-05 | Patch/region/slide embedding links | C-02–C-04 | Lead | active | Data/artifact tests | Frozen contract plus first logical checkpoint: expected patch/region/slide sets, exact context/footprints, typed logical tables, bounded scans/resources, 7 focused regressions, and dual read-only approval; overlap/links/records/physical profiles/graph/scale/closure remain |
| C-06 | General marks/status | C-01, C-03 | Lead | planned | Data/artifact tests | Measurement-status tests |
| SLIDE-INV | Authorized remote WSI/embedding inventory | A-01 | `remote_slide_inventory` | complete | Documentation only | 677 high-confidence WSI-compatible objects, CellViT provenance, and exact remaining gaps recorded |

Later phases WS-30 through WS-94 remain `planned` in the immutable master plan and enter this registry as task contracts when their prerequisites close. No omission from this operational registry changes the charter.

## Complete master-plan ID coverage

The tables below ensure every scientific, platform, current-capability, and workstream identifier in the charter has an operational status. Detailed task contracts replace these summary rows only when prerequisites close.

### Current-capability preservation inventory

| ID | Current capability | Status/disposition |
|---|---|---|
| CUR-01 | `Pattern`/metadata/window and CSV/Parquet pattern loading | characterized; preserve |
| CUR-02 | Tumor mask, QC filtering, summary geometry | characterized; preserve |
| CUR-03 | Fourier structure factor and shell summaries | characterized; preserve narrow semantics |
| CUR-04 | Deterministic scalar permutation and ERL envelope | characterized; preserve canonical implementation |
| CUR-05 | Centered mark-pair covariance plan | characterized; preserve; never alias to g(r) |
| CUR-06 | Directional anisotropy summary | characterized; preserve |
| CUR-07 | Multiscale residual diagnostic | characterized; preserve; never alias to wavelets/DoG |
| CUR-08 | Pooled/separate/both component execution | characterized; preserve |
| CUR-09 | Rigid/affine 2-D landmark registration | characterized; preserve |
| CUR-10 | Registered H&E/IHC fusion with one index/graph | characterized; preserve; no correspondence claim |
| CUR-11 | Graph-edge enrichment/null sensitivity | characterized; preserve |
| CUR-12 | Cross-label pair-count curves with ERL | characterized; preserve; not cross-K/g |
| CUR-13 | MMR density components and territory profiles | characterized; preserve narrow scope |
| CUR-14 | Marked/multimodal descriptive pre/post | characterized; preserve descriptive scope |
| CUR-15 | Strict result 0.3, finite/typed/transactional output | characterized; preserve |
| CUR-16 | CLI analyze/batch/prepost/multimodal/simulate/smoke/WSI | characterized; preserve |
| CUR-17 | Exact R-tree index, plans, seeds, memory accounting | characterized; preserve substrate |
| CUR-18 | CI, dependency policy, fuzz build, calibration, benchmarks | characterized; baseline execution active |

### Detailed classical, inference, mark, and geometry IDs

| IDs | Status | Notes |
|---|---|---|
| PP-03A, PP-03B | planned | Classical g, cross-K, and cross-g remain distinct from current endpoints |
| PP-04A, PP-04B, PP-04C | planned | G, F, and J with explicit probe/error/undefined contracts |
| PP-05 | planned | Intensity estimation with bandwidth/cross-fit provenance |
| PP-06A, PP-06B, PP-06C | planned | Border, translation, and isotropic corrections |
| PP-06D | rejected default | Toroidal behavior only for genuinely periodic designs |
| NUL-01A, NUL-01C, NUL-01D | planned | CSR, population independence, and conditional/stratified nulls |
| NUL-01B | existing/extend later | Preserve current random-labeling owner |
| NUL-01E | experimental planned | Shift null only when window/stationarity justify it |
| INF-01A, INF-01B | existing | Preserve scalar permutation and ERL owners |
| INF-01C, INF-01D | planned | Multiplicity families and method-specific intervals/coverage |
| MRK-01A, MRK-01B, MRK-01C | planned | Mark connection, correlation, and weighted K remain separate |
| MRK-02A, MRK-02B, MRK-02C, MRK-02D | planned | Multitype, ordinal, probabilistic, and vector marks |
| SIG-01A, SIG-01B, SIG-01F | planned P1 | Global Moran, Geary, scalar variogram |
| SIG-01C, SIG-01D, SIG-01E, SIG-01G | planned P2 | Local/bivariate/hotspot/cross summaries with multiplicity and semantics |
| SIG-01H | planned stable/experimental boundary | Vector similarity/covariance with rotation and leakage controls |
| GEO-01A, GEO-01B, GEO-01C | planned P1 | Compartments, contact, infiltration |
| GEO-01D, GEO-01E, GEO-01F, GEO-01G | planned P2 | Fragmentation, morphology, fronts, object-relative statistics |
| GEO-01H | experimental/watch | Interface curvature; propagation requires longitudinal data |
| NIC-01A | planned P1 | Multiscale soft composition |
| NIC-01B, NIC-01C, NIC-01D, NIC-01E | planned P2 | Imported discovery, transitions, patient-level abundance |
| NIC-01F | experimental planned | Reference-query mapping |
| IHC-01, MOL-01, BULK-01 | planned | Continuous IHC, spatial molecular, and bulk association contracts |
| CMP-01A, CMP-01B | planned P1/P2 | Versioned fingerprint and functional patient-level comparison |
| CMP-01C, CMP-01D | planned P2 | MMD and energy distance at biological-unit level |
| CMP-01E | experimental planned | Graph kernels after construction stability |
| CMP-01F | watch | Topological comparison requires stable filtration/use case |
| EQV-01A | existing | Descriptive margin only |
| EQV-01B, EQV-01C | planned | Genuine TOST equivalence and noninferiority |
| CLN-01A, CLN-01B, CLN-01C | planned P2 | Clone import and downstream geometry/concordance |
| CLN-01D, CLN-02 | planned experimental/stable boundary | Phylogenetic association; no unsupported growth direction |

### Bayesian, multimodal, dimensional, and frontier IDs

| ID | Status | Claim/result boundary |
|---|---|---|
| BAY-03 | planned | Hierarchical cohort models; patient is default population unit |
| BAY-04 | planned | GP/Matérn/nonstationary fields with calibrated approximations |
| BAY-05 | planned | CAR/SAR/GMRF graph fields with graph contract |
| BAY-REG-A | planned | Integrated Bayesian/frequentist spatial regression |
| BAY-FIELD-A | planned | Exact/sparse/NN/SPDE field strategies |
| BAY-PP-A, BAY-PP-B | planned | LGCP and Gibbs/interaction processes |
| BAY-GMRF-A | planned | CAR/SAR/SGLMM backend integration |
| BAY-HIER-A | planned | Genuine hierarchical beta-binomial, not current beta diagnostic |
| BAY-PP | planned | Bayesian point-process/joint location-mark family |
| BAY-NP | planned experimental | Nonparametric niches with label/scale uncertainty |
| BAY-MM, MM-01 | planned | Multimodal latent models; measured/predicted remain distinct |
| BAY-REG | planned | Registration/correspondence uncertainty; no true-identity claim |
| BAY-EVO | planned experimental | Clone phylogeography with cross-sectional limits |
| BAY-SBI | planned research-only | SBI/amortized inference with SBC and OOD support checks |
| DL-CORE | planned integrated backend | Foundation-model execution/provenance, not native duplication by default |
| REG-01A | existing | Preserve rigid/affine contract |
| REG-01B, REG-01C | planned | Nonrigid backend and probabilistic correspondence |
| TIME-01 | planned advanced | Repeated biological units and deformation/change separation |
| EMB-02, FR-02A | planned | Patch/region tables and links before modeling |
| EMB-PRED-01 | planned integrated | Patient-held-out late fusion first |
| EMB-PRED-02 | experimental planned | Cross-attention only after simpler baselines |
| FR-01A, FR-01B, GSP-01 | experimental planned | Graph heat/Fourier/wavelet transforms with exact small oracles |
| GSP-02, HET-01 | watch/experimental | Heterogeneous graphs/hypergraphs/higher-order tissue |
| GSP-03 | experimental planned | Graph scattering with incremental-value gate |
| FR-03A | experimental planned | Partial/unbalanced transport, descriptive alignment only |
| FR-03B | experimental/watch | Fused Gromov-Wasserstein with non-identifiability sensitivity |
| TOP-01A | watch | Persistent homology requires pathology endpoint/replication |
| TOP-01B | experimental planned | Euler/Minkowski/percolation with explicit basis/scale |
| GEN-01A, GEN-01B | research-only planned | Neural point/Cox and diffusion/generative layouts |
| GEN-01C | rejected claim | “Digital twin” unsupported without prospective evidence |
| CAU-01A, CAU-01B | gated research-only | Only for eligible treatment/exposure designs |
| CAU-01C | research-only/rejected causal claim | Hypothesis generation only absent identification |
| CCC-01 | planned interoperability | Communication hypotheses, never causal signaling by default |
| PERT-01, ACT-01 | watch | Require designed perturbation/prospective acquisition data |
| EQ-01 | watch/interoperability | Equivariant learned models require patient/site validation |
| WAV-01A | rejected default/watch | Raster wavelets only for raster-defined questions |
| WAV-01B | low-priority experimental | Genuine DoG only for an explicit raster scale-space endpoint |
| SPC-01A | rejected default/low priority | Current Hann periodogram is not Bartlett |
| SPC-01B | watch | Multitaper only if variance reduction changes a prespecified endpoint |
| SCALE-01 | planned infrastructure | Exact fallback and reported approximation error required |
| PATH-01 | planned | Pathology geometry umbrella requirement |

### Dependency-ordered roadmap workstreams

| Workstream | Status | Exit/role summary |
|---|---|---|
| WS-00 | complete | Control plane, state, branch, baseline |
| WS-01 | complete | Legacy characterization and compatibility matrix |
| WS-10 | complete | Workspace boundary, compatibility shell, policy matrix, and architecture gates complete |
| WS-11 | active | B-04 reference/digest/cache and C-03 immutable catalog/local-store slices are implemented and green; mutable project heads, durable execution ledgers, and resume remain |
| WS-12 | active | B-04 typed DAG/key/failure-atomic single-node scheduler implemented and green; general scheduling/resume follows |
| WS-13 | planned | Backend registry and doctor/validation commands |
| WS-20 | complete | C-01 typed identity and hierarchy slice; commit `a1260445a383c59381b2c7c7881cebb10e10c156` |
| WS-21 | complete | C-02 coordinates, units, dimensions, transforms, uncertainty; commit `c676cfd732d02d6202bff8cea47fcf74b5bfd8e7` |
| WS-22 | planned | Windows, compartments, exact geometry |
| WS-23 | planned | General marks and measurement provenance |
| WS-24 | active/data available with blockers | C-04 cell-vector substrate is complete. C-05 now has synthetic/domain-level expected sets, context/footprints, and typed logical tables, but overlap, links, provenance/support, physical profiles, and graph validation remain. Authorized candidates still lack promotable canonical identity/context/provenance/linkage; region/slide profiles are inadmissible. General measurement status follows in C-06 |
| WS-25 | planned | SpatialData/AnnData/OME-NGFF/Arrow/Parquet/Zarr interchange |
| WS-30 | planned | Classical point-process core |
| WS-31 | planned | Nulls, global inference, multiplicity |
| WS-32 | planned | Spatial signal/geostatistics |
| WS-33 | planned | Pathology geometry |
| WS-34 | planned | Cohort resampling, comparison, equivalence |
| WS-40 | planned | Bayesian IR, priors, artifacts, diagnostics/backends |
| WS-41 | planned | Hierarchical cohort models |
| WS-42 | planned | GP/GMRF/CAR/SAR fields |
| WS-43 | planned | Bayesian point processes |
| WS-44 | planned | Bayesian comparison/calibration |
| WS-50 | planned/data-dependent | Single-cell embedding statistics |
| WS-51 | planned/data-dependent | Patch/region multiscale representations |
| WS-52 | planned/data-dependent | Predictive M0–M5 framework |
| WS-53 | planned/data-dependent | Multimodal Bayesian latent models |
| WS-54 | planned/data-dependent | Clone/CNA/evolutionary downstream analysis |
| WS-55 | planned/data-dependent | Nonrigid registration and uncertainty |
| WS-60 | planned | Multiscale neighborhoods/niches |
| WS-61 | planned | Heterogeneous/higher-order tissue graphs |
| WS-62 | planned experimental | Spectral graph/scattering lab |
| WS-63 | planned experimental | Topological/morphological lab |
| WS-70 | planned | Canonical simulator library |
| WS-71 | planned research | Mechanistic tissue models |
| WS-72 | planned research | Neural point processes/flows/diffusion |
| WS-73 | planned research | SBI/amortized inference |
| WS-74 | planned research | Posterior-predictive digital tissue lab |
| WS-80 | planned/data-dependent | 3-D and serial-section platform |
| WS-81 | planned/data-dependent | Longitudinal/evolutionary models |
| WS-82 | gated/data-dependent | Causal/interference lab |
| WS-83 | gated/data-dependent | Perturbation/treatment-response workflows |
| WS-84 | planned/data-dependent | Active experimental design |
| WS-90 | planned | Interactive workbench |
| WS-91 | planned | Server/collaboration/remote execution |
| WS-92 | planned | Python/R/notebook ecosystem |
| WS-93 | planned/data-dependent | Publication and benchmark program |
| WS-94 | planned | Stable 1.0 release/migrations/LTS/security review |

The aliases `WS-A`, `WS-B`, and `WS-C` are the bounded execution groupings for WS-00/01, WS-10–12's initial slice, and WS-20–24's initial substrate respectively.
