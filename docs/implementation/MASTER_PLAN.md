# Marklab Frontier Spatial Pathology Operating System — Research-Backed Implementation Master Plan

**Audit target:** `https://github.com/jcwal1516/gsc-marklab`
**Pinned branch:** `main`
**Pinned commit:** `55fce12f10684a908 1ca1f744f87d6f5feedcb24`
**Pinned tree:** `5c9712d57130b89b1ae9cb3e40c7af98eaceb4c7`
**Audit date:** 2026-08-22
**Audit mode:** read-only repository audit plus research synthesis; this revision is the implementation charter for full-platform transformation

## Evidence vocabulary

Every material statement in this plan is classified implicitly or explicitly as one of:

- **V-SRC:** directly verified in source at the pinned commit.
- **V-RUN:** supported by a test or benchmark executed against the pinned commit.
- **HIST:** historical evidence from an earlier commit; context only, never proof of current HEAD.
- **LIT:** supported by primary literature, a specification, or official implementation documentation.
- **INF:** reasoned inference from verified evidence.
- **PROP:** proposed implementation or policy.
- **UNRES:** unresolved because repository, data, provenance, execution, or literature evidence is missing.

There is **no V-RUN evidence for the pinned commit**. The audit environment could not obtain a local checkout, and no completed status check was surfaced for the pinned SHA. Source and tests were inspected remotely. Historical parent-commit records report extensive green test and benchmark evidence, but those records are labeled HIST throughout.

---


# 0. Superseding product mandate

## 0.1 Mission

This document supersedes the earlier conservative interpretation of the roadmap.

Marklab is not being developed merely as a narrow Rust library of selected spatial statistics. The program objective is to transform the repository into a **unified, frontier spatial-pathology operating system** for computational pathologists and spatial-biology researchers.

The finished platform must combine, within one coherent project and provenance system:

- trusted classical spatial statistics;
- exact and approximate point-process methods;
- graph, geometric, spectral, topological, and multiscale mathematics;
- hierarchical frequentist and Bayesian inference;
- multimodal image, cell, patch, region, molecular, clonal, and clinical data;
- single-cell CellViT embeddings as a primary modality;
- multiscale patch and region embeddings;
- spatial omics and multiplex-protein measurements;
- rigid, affine, nonrigid, serial-section, 3-D, and longitudinal registration workflows;
- prediction, retrieval, representation learning, and foundation-model interoperability;
- Bayesian nonparametrics, mechanistic spatial models, simulation-based inference, and generative models;
- uncertainty propagation from acquisition through segmentation, registration, classification, model prediction, and cohort inference;
- interactive exploration, reproducible workflow composition, publication artifacts, and scalable batch execution.

A user must be able to run one method alone, run a validated standard panel, or compose complex analyses spanning multiple modalities and mathematical families. Stable and experimental methods must coexist in the same product without being presented as equally validated.

## 0.2 Product thesis

> **Marklab will be a composable computational spatial-pathology operating system: a high-performance classical-statistics engine, a hierarchical Bayesian modeling and simulation platform, and an experimental multimodal discovery laboratory that integrates cells, patches, images, tissue geometry, molecular measurements, clones, cohorts, and learned representations across 2-D, 3-D, serial, and longitudinal tissue.**

The system must be:

- scientifically explicit;
- uncertainty-aware;
- cohort-valid;
- multimodal;
- backend-pluggable;
- reproducible;
- scalable from one region to international cohorts;
- usable from CLI, Rust, Python, R, workflow files, and an interactive application;
- capable of both conservative evidence generation and high-risk/high-reward exploration.

## 0.3 All-in-one means one product, not one numerical language

All-in-one does **not** require every algorithm to be reimplemented in Rust. It requires that Marklab own the user experience and scientific contract:

- one project format;
- one identity and coordinate model;
- one workflow graph;
- one configuration and secrets boundary;
- one artifact catalog;
- one provenance ledger;
- one result browser;
- one validation and maturity system;
- one command/API surface;
- one reproducible execution record.

Numerical backends may include native Rust, CUDA, Python, R, CmdStan, PyMC, NumPyro, JAX, PyTorch, TensorFlow, INLA/inlabru, Turing, or specialized external tools. Such backends are part of Marklab when they are invoked through versioned adapters with pinned environments, checksums, typed inputs/outputs, diagnostics, and reproducible execution manifests.

## 0.4 Claim tiers

Every method and result must carry one of these maturity tiers:

| Tier | Meaning | Permitted use |
|---|---|---|
| `validated` | Independent numerical oracle, calibration, benchmark, stable schema, and appropriate real-data validation complete | Primary publication evidence and stable public API |
| `established` | Canonical established method with numerical validation and documented limitations; broader real-data validation may be ongoing | Standard analysis with explicit limitations |
| `experimental` | Implemented and testable, but validation or replication is incomplete | Exploration, hypothesis generation, methods research |
| `research_only` | Speculative or high-risk method, unstable API, or unresolved identifiability | Internal research and explicitly labelled outputs only |
| `unsupported_for_claim` | Computation may be displayed, but the requested biological, clinical, equivalence, or causal claim is not identified | No affirmative scientific claim |

Scientific restraint applies to interpretation and promotion. It must not be used to prevent ambitious methods from being implemented in the experimental laboratory.

## 0.5 Rewrite authority

The implementation lead may:

- change the public API;
- change configuration and CLI contracts;
- open new result formats;
- replace production workflows;
- create a Cargo workspace and new crates;
- add Python/R bindings and backend workers;
- remove obsolete compatibility surfaces;
- migrate or retire current engines;
- rebuild modules from first principles where characterization shows that incremental modification would preserve the wrong architecture.

However, this is a **controlled replatforming**, not an indiscriminate rewrite. Existing verified kernels, fixtures, deterministic seed logic, finite-state policies, spatial indexing, memory-budget concepts, transaction logic, and historical regression tests are assets. They are preserved, wrapped, migrated, or replaced only through explicit characterization and acceptance evidence.

## 0.6 User promise

A computational pathologist should be able to create a Marklab project and then:

1. import slides, masks, cells, patches, regions, embeddings, molecular data, clone assignments, clinical variables, and external-model outputs;
2. harmonize identities, units, coordinate frames, sections, and modalities;
3. inspect QC and uncertainty;
4. choose a single estimator or build a workflow graph;
5. run classical, Bayesian, graph, topological, predictive, and generative analyses together;
6. compare nulls, priors, models, scales, compartments, samples, and cohorts;
7. inspect maps, curves, posterior distributions, diagnostics, and uncertainty overlays;
8. save and rerun the complete analysis exactly;
9. export publication-ready figures, tables, artifacts, notebooks, and machine-readable results;
10. distinguish validated evidence from experimental discovery at every step.

# 1. Audit identity and decision brief

## 1.1 Identity, toolchain, and audit boundary

| Field | Finding | Evidence class |
|---|---|---|
| Repository | `jcwal1516/gsc-marklab` | V-SRC |
| Default/audited branch | `main` | V-SRC |
| Exact SHA | `55fce12f10684a9081ca1f744f87d6f5feedcb24` | V-SRC |
| HEAD change | Documentation-only removal of internal remediation records; parent `fad9fd081e5b8783c4a275bb392eb86037d85ed6` | V-SRC |
| Package | `marklab` 0.1.0, Rust 2021 | V-SRC |
| MSRV/toolchain | Rust 1.96 / pinned 1.96.0 | V-SRC |
| License | MIT OR Apache-2.0 | V-SRC |
| Default features | `cli`, `parallel`, `parquet`, `csv` | V-SRC |
| Optional features | `wsi`, `allocator-mimalloc`, `dhat-heap` | V-SRC |
| Core numerical/data dependencies | `rstar`, `rustfft`, `statrs`, Arrow/Parquet 56, CSV, optional WSI | V-SRC |
| Safety boundary | `#![forbid(unsafe_code)]` | V-SRC |
| Repository guidance files | No `AGENTS.md` found | V-SRC |
| Worktree cleanliness | Not observable: the audit used the remote tree and did not create a local checkout | UNRES |
| Tests executed at pinned SHA | None | UNRES |
| Benchmarks executed at pinned SHA | None | UNRES |
| Historical execution evidence | Parent-commit remediation records report 402 executable tests passing with 22 intentional skips and substantial performance/calibration work | HIST |

The `Cargo.toml` repository metadata points to `jcwal1516/marklab`, while the audited repository is `jcwal1516/gsc-marklab`. That stale metadata should be corrected in a documentation-only maintenance commit, but it is not a scientific blocker.

## 1.2 What Marklab can answer scientifically today

At the pinned commit, Marklab can answer a narrow but defensible set of questions:

1. **Single-pattern binary or probabilistic mark organization under fixed cell positions.** It computes a Fourier-domain structure-factor analysis, a centered distance-binned mark-pair covariance, anisotropy summaries, and a clearly named multiscale residual diagnostic. It can compare observed curves with fixed-position random-labeling nulls, including configured stratification, using deterministic permutation machinery and checked global envelopes.

2. **Registered serial-section MMR-IHC neighborhood description.** It fits rigid or affine landmark transforms, reports registration QC, places H&E and IHC cells in a common coordinate frame, builds one reusable spatial index and graph, estimates graph-edge enrichment, computes distance-binned cross-label pair-count curves, detects MMR-abnormal density components, and summarizes nearby cell-type fractions.

3. **Descriptive pre/post comparison of already aggregated outputs.** It aligns result axes, computes an explicitly approximate pooled-bin diagnostic, and reports descriptive margin assessments. It correctly avoids calling these operations equivalence tests.

4. **Operational reproducibility and bounded execution.** It has typed unavailable states, finite serialization checks, strict result-format 0.3 documents, deterministic seeds, configurable strict reproducibility, memory budgets, transactional output, feature-gated adapters, scheduled calibration infrastructure, and benchmarks for current hot paths.

These are meaningful capabilities. They are not a general spatial-pathology statistics platform yet.

## 1.3 What Marklab cannot answer today

Marklab cannot currently provide a scientifically valid answer to the following:

- whether apparent interaction remains after separating **cell density, inhomogeneous intensity, tissue-window geometry, compartments, and edge effects**;
- homogeneous or inhomogeneous **Ripley K/L**, classical **pair-correlation \(g(r)\)**, cross-K, cross-\(g\), F, G, or J;
- general categorical, ordinal, continuous, probabilistic, multivariate, or high-dimensional marks through a public typed mark abstraction;
- general spatial autocorrelation, variograms, cross-covariance, local indicators, or hotspot inference;
- patient-, specimen-, slide-, core-, region-, or repeated-measures inference with the correct biological replication unit;
- population-level difference, noninferiority, or genuine equivalence;
- stable analysis of the stated CellViT embedding asset, because vectors and provenance are not present in the audited import path;
- patch embeddings, cell-patch links, externally learned molecular representations, or artifact-scale matrix exchange;
- clone/CNA concordance, interface geometry, infiltration depth, compartment-relative statistics, or clonal boundaries;
- spatial regression, GP/LGCP/Gibbs/CAR/SAR/SGLMM fitting;
- nonrigid registration, probabilistic correspondence, serial-section 3-D reconstruction, or temporal spatial processes;
- causal communication, treatment spillover, or intervention effects from ordinary cross-sectional observational tissue;
- morphology-to-IHC/omics/CNA/outcome prediction with patient-held-out nested validation.

## 1.4 Fifteen highest-value program capabilities

| Rank | Requirement family | Why it is foundational or differentiating |
|---:|---|---|
| 1 | **PLAT-01 project, workflow, backend, and artifact operating system** | Without a common control plane, the repository becomes disconnected commands rather than an all-in-one program. |
| 2 | **DATA-01 typed identity, cohort hierarchy, coordinate frames, units, and modality registry** | Correct biological replication, multimodal linking, leakage prevention, and 3-D/longitudinal analysis depend on this substrate. |
| 3 | **GEO-01 exact observation windows, compartments, boundaries, graphs, and reusable geometry plans** | Every spatial estimand depends on the permitted domain and geometry. |
| 4 | **EMB-01 primary CellViT single-cell embedding support** | The existing per-cell representation is a core modality and must be analyzable statistically, predictively, and jointly with molecular data. |
| 5 | **EMB-02 multiscale patch and region embeddings with explicit links** | Tissue architecture beyond the cell is essential for computational pathology and must be represented in physical units. |
| 6 | **PP-01 classical point-process and marked-process suite** | K/L, g, cross functions, F/G/J, intensity, edge corrections, and explicit nulls provide trusted spatial foundations. |
| 7 | **BAY-01 hierarchical Bayesian inference platform** | Patient, specimen, slide, region, cell, and patch uncertainty requires partial pooling and model-based inference. |
| 8 | **BAY-PP point-process, Cox, Gibbs, and joint location-mark models** | These distinguish intensity, attraction, inhibition, compartments, and latent spatial structure while quantifying uncertainty. |
| 9 | **MM-01 multimodal latent-variable and cross-modal spatial models** | H&E, IHC, multiplex, omics, CNA, clone, patch, and clinical information must be jointly explorable. |
| 10 | **PATH-01 pathology geometry, interfaces, infiltration, fronts, glands, vessels, nerves, and necrosis** | Directly interpretable pathology endpoints create translational value beyond generic spatial metrics. |
| 11 | **COH-01 cohort-valid inference, prediction, equivalence, and external validation** | Patient-level claims require explicit designs, nested validation, multisite effects, and correct multiplicity. |
| 12 | **GSP-01 graph spectral, wavelet, scattering, hypergraph, and higher-order methods** | Irregular cell coordinates and tissue entities require non-raster multiscale mathematics. |
| 13 | **GEN-01 simulation, simulation-based inference, neural point processes, flows, and diffusion models** | Fitted tissue simulators and amortized inference provide frontier discovery and rigorous model checking. |
| 14 | **DIM-01 nonrigid, serial-section, 3-D, and longitudinal analysis** | Tumor evolution and multimodal section analysis require dimensionality-aware geometry and propagated registration uncertainty. |
| 15 | **UX-01 computational-pathologist workbench** | Interactive maps, posterior diagnostics, workflow building, comparison, retrieval, and publication export are necessary for actual use. |

## 1.5 First three implementation workstreams

The first workstreams establish a durable implementation system and a replatforming boundary. They intentionally precede broad method implementation.

1. **WS-A — Implementation control plane and baseline preservation.** Install the master plan, `AGENTS.md`, persistent ledgers, exact baseline verification, canonical-symbol ownership, worktree rules, and migration inventory.

2. **WS-B — Workspace replatforming and compatibility shell.** Convert the repository into a workspace-capable architecture without changing current numerical behavior. Move existing Marklab into a legacy/compatibility crate or module boundary, establish the new core crates, and prove current CLI/result parity through characterization tests.

3. **WS-C — Unified project, identity, coordinate, modality, artifact, and embedding substrate.** Implement the project manifest, cohort hierarchy, stable IDs, coordinate frames, units, observation entities, artifact catalog, CellViT embedding table, patch/region embedding tables, and explicit cell-patch-region links.

The first new scientific engines follow these foundations:

- exact geometry and classical point-process suite;
- Bayesian model specification and inference backends;
- multimodal embedding statistics;
- pathology geometry and cohort inference.

## 1.6 Frontier program pillars

The frontier laboratory is a primary product pillar, not a residual watch list.

1. **Hierarchical Bayesian spatial pathology.** Multilevel spatial fields, multitype point processes, latent factors, missing modalities, joint uncertainty, clone evolution, and repeated-measures models.

2. **Geometric deep spatial modeling.** Equivariant graph networks, heterogeneous graphs, hypergraphs, simplicial complexes, graph wavelets, graph scattering, multiscale cell-patch-region transformers, and learned spatial kernels.

3. **Generative and simulation-based tissue science.** Neural Cox/marked point processes, mechanistic-neural simulators, normalizing flows, diffusion models, posterior predictive simulation, and amortized Bayesian inference.

4. **Atlas, retrieval, and alignment.** Partial/unbalanced/fused transport, graph matching, reference-query mapping, analogous-region retrieval, and cross-platform tissue atlases.

5. **3-D, longitudinal, perturbational, and evolutionary modeling.** Serial-section reconstruction, anisotropic 3-D processes, temporal spatial models, treatment spillover, clone phylogeography, and active experimental design.

## 1.7 Integrated backend policy

The product may use external computational engines, but they are integrated Marklab backends rather than disconnected manual workflows.

### Native Rust responsibilities

- project and identity model;
- coordinate systems and geometry;
- exact classical statistics;
- workflow DAG and scheduler;
- artifact/provenance/result contracts;
- deterministic randomization;
- memory and performance control;
- stable numerical kernels where Rust has a concrete advantage;
- bindings and backend orchestration;
- validation harness and reference comparisons.

### Integrated Bayesian and learned backends

- CmdStan/Stan for HMC/NUTS and model reference implementations;
- PyMC, NumPyro, JAX, or PyTorch for variational, GPU, amortized, and neural inference;
- INLA/inlabru, spaMM, or related R backends for selected latent Gaussian models;
- specialized registration, foundation-model, CNA/clone, optimal-transport, and spatial-omics tools;
- containerized or environment-locked workers invoked through typed manifests.

A backend is accepted only when its version, environment, license, command/config digest, model/checkpoint checksum, input/output schema, diagnostics, and deterministic controls are recorded.

## 1.8 Ideas and claims that remain rejected

The platform is allowed to implement ambitious mathematics. The following **claims or implementation practices** remain prohibited:

- presenting predicted molecular values as measurements;
- claiming causal signaling from proximity, co-expression, attention, or predictive importance;
- calling a transport plan true cell correspondence without external evidence;
- treating cells, patches, or edges as patient-level replicates;
- claiming equivalence from non-significance or a descriptive threshold;
- hiding failed chains, divergent transitions, null failures, missing modalities, or undefined statistics;
- using a fashionable method name without implementing its accepted definition;
- silently replacing exact inferential behavior with an approximation;
- calling an unvalidated simulator a digital twin;
- presenting experimental outputs as clinically validated.

## 1.9 Recommended scientific and product thesis

> **Marklab should be the definitive composable operating system for computational spatial pathology: combining exact spatial statistics, hierarchical Bayesian modeling, multimodal and foundation-model representations, mechanistic and generative tissue models, and cohort-valid inference within one provenance-complete, scalable, interactive platform.**

Its unique advantage should not be the raw number of algorithms. It should be the ability to combine them coherently while preserving:

- the meaning of every estimand;
- the correct biological replication unit;
- explicit nulls and priors;
- full uncertainty and diagnostics;
- exact versus approximate execution status;
- multimodal identity and coordinate integrity;
- validation maturity and claim limits;
- reproducibility from raw artifact to final figure.

## 1.10 Modalities and entities in scope

The target platform must support, through native formats or integrated adapters:

### Imaging and tissue geometry

- H&E;
- brightfield IHC;
- immunofluorescence;
- multiplex immunofluorescence;
- IMC, MIBI, CODEX and related multiplex imaging;
- whole-slide images and image pyramids;
- masks, compartments, glands, vessels, nerves, necrosis, fronts, and annotations;
- serial sections and 3-D volumes.

### Cellular and regional objects

- nuclei and cells;
- phenotypes and probabilistic labels;
- cell contours;
- patches at multiple physical scales;
- glands, vessels, immune aggregates, tumor nests, stromal regions, clones, niches, and domains;
- graphs, hypergraphs, boundaries, interfaces, and complexes.

### Molecular and clinical data

- continuous and categorical IHC;
- spatial transcriptomics;
- spatial proteomics;
- mutation, CNA, clone, lineage, and phylogenetic assignments;
- bulk molecular measurements;
- treatment, response, survival, and competing-risk outcomes;
- patient, specimen, site, scanner, batch, stain, and acquisition covariates.

### Learned representations and predictions

- CellViT per-cell embeddings;
- patch, region, and slide embeddings;
- foundation-model image embeddings;
- spatial-omics embeddings;
- imported probabilistic molecular predictions;
- domain, niche, clone, and segmentation uncertainty;
- retrieved analogues and atlas mappings.

## 1.11 Missing assets and metadata

Before CellViT or patch embeddings can support stable claims, the project must identify or supply:

- the actual embedding artifact path and schema;
- stable cell-ID correspondence;
- exact model family, version, checkpoint, and weights checksum;
- encoder, extraction layer, pooling, dimension, dtype, and normalization;
- source modality and stain;
- scanner/site, section thickness, resolution, micrometres per pixel;
- physical context and receptive field;
- patch sizes, stride, overlap, and boundary policy;
- preprocessing and stain normalization;
- segmentation and classification confidence;
- missing-vector and failed-extraction policy;
- paired molecular, IHC, CNA, clone, treatment, outcome, and external-cohort availability;
- whether patch and region embeddings already exist.

The platform can implement the contracts before these assets are supplied. It cannot honestly validate model-specific scientific conclusions without them.

## 1.12 Largest program risks

1. **Architectural incoherence:** building many methods without a workflow, identity, artifact, and maturity model.
2. **Pseudoreplication and leakage:** treating cells/patches as patients or allowing adjacent/overlapping material across folds.
3. **Unidentifiable claims:** confusing association, prediction, transport, latent factors, or posterior structure with causality or correspondence.
4. **Bayesian theatre:** exposing priors and posteriors without convergence, simulation-based calibration, prior sensitivity, or posterior predictive checks.
5. **Backend drift:** external environments and checkpoints changing without reproducible manifests.
6. **Technical confounding:** scanner, stain, site, section, resolution, model version, and segmentation artifacts driving apparent biology.
7. **Model proliferation:** dozens of superficially distinct methods without benchmarked incremental value.
8. **Schema and artifact sprawl:** large matrices or unstable experimental fields contaminating stable result documents.
9. **Performance collapse:** complex models and multimodal artifacts defeating million-cell objectives.
10. **Unreviewable AI implementation:** duplicated frameworks, god workflows, placeholder code, fake tests, and repeated rewrites.
11. **Insufficient external validation:** a sophisticated system that performs only on synthetic or internal data.
12. **User failure:** a technically impressive library without an integrated workbench, explainable outputs, and reproducible recipes.

---

# 2. Evidence-backed current capability inventory

## 2.1 Inventory

| ID | Capability and owning symbols | Public/application path | Config/result contract | Static tests/bench evidence | Maturity | Principal limitation |
|---|---|---|---|---|---|---|
| CUR-01 | `data::Pattern`, `PatternMeta`, `TumorWindow`; normalized CSV/Parquet row builder | `PatternLoader` → `AnalysisEngine::analyze_pattern[_run]` | Binary `mark`; optional probability/QC/categorical strata; `WindowSummary` | API, workflow, CSV/Parquet parity tests; `pattern_load` bench | Stable narrow core | Flat section-level object; no typed CellId or cohort hierarchy; no general mark table. |
| CUR-02 | Tumor mask/QC filtering and summary geometry | Input adapters → pattern validation | Area, effective length, mean NN, retained/QC fractions | Loader and QC tests | Stable operational | The window is primarily a mask/summary contract, not a general polygon/multipolygon point-process window with edge distances. |
| CUR-03 | Fourier structure factor, shell aggregation, binary/probabilistic field execution | `AnalysisEngine` spectrum stage | `[spectrum]`; `PrimaryEndpoint`, `SpectrumSummary`, curve, null sensitivity | `engine_spectrum`, permutation and structure-factor benches | Stable project-specific endpoint | Not a substitute for K/L/g; continuous kernel exists internally but public data/config/result support is absent. |
| CUR-04 | Deterministic scalar permutation and checked ERL global envelope | Used by spectrum and cross-interaction stages | Family-wise alpha, permutation count/seed, typed global summaries | Unit/integration tests; random-labeling envelope bench | Strong reusable foundation | Randomization is cell-label based; no patient/specimen resampling design. |
| CUR-05 | `MarkPairCovariancePlan` and centered distance-binned covariance | Marked spatial stage | `mark_pair_covariance` result family | Independent centered-product and plan tests; current bench lineage | Stable descriptive/inferential mark endpoint | Explicitly not classical point-process \(g(r)\); binary/probabilistic workflow only. |
| CUR-06 | Directional anisotropy summary | Marked spatial stage | `AnisotropySummary` | Spectrum integration tests | Validated project endpoint | Scope is tied to current spectral workflow; no general directional K/g or vector-field anisotropy. |
| CUR-07 | Raster multiscale residual energy and residual neighborhoods | Marked spatial stage | `[multiscale_residual]`; scale-energy and residual-territory results | Unit/integration tests; `multiscale_residual` bench | Honest heuristic/diagnostic | Not wavelets, DoG, tissue domains, or a canonical multiscale transform. |
| CUR-08 | Pooled/separate/both component execution | `AnalysisEngine` component plan | `ComponentModeSelection`, component summaries | Engine tests | Stable | Components are supplied labels; no cohort nesting or domain discovery. |
| CUR-09 | 2-D landmark rigid and affine registration with QC | `MultimodalEngine` | `[registration]`; registration summary, residual artifacts | Registration unit/integration tests | Stable limited core | No nonrigid transform, deformation field, uncertainty distribution, serial-section reconstruction, or probabilistic correspondence. |
| CUR-10 | H&E/IHC fusion by coordinate-frame concatenation; one shared index and graph | `MultimodalEngine::analyze_run` | Fused-cell summary and internal fused table | Multimodal engine/CLI tests | Stable workflow | No same-cell matching. `same_section` is source-section status, not correspondence. |
| CUR-11 | Graph edge enrichment and null sensitivity | Multimodal engine | Neighborhood pair config, typed ratio/z unavailability, p/q values | Neighborhood/multimodal tests | Stable narrow endpoint | Raw graph semantics are unweighted union of radius and kNN; no general weights contract. |
| CUR-12 | Distance-binned cross-label pair-count curves with ERL | Multimodal engine | `CrossInteractionCurve` | Indexed/brute-force and ERL tests | Stable descriptive/inferential endpoint | Not cross-K/cross-\(g\): no intensity normalization or point-process edge correction. |
| CUR-13 | MMR-abnormal density components and circular territory profiles | Multimodal engine | Territory/profile/comparison result types | Neighborhood/profile tests | Stable MMR-specific workflow | Not morphology-defined niches, compartments, boundary-aware domains, or patient-replicated differential abundance. |
| CUR-14 | Marked and multimodal pre/post comparison | Public comparison functions and CLI | Versioned pre/post results, pooled-bin method, descriptive margins | Pre/post unit/integration/CLI tests | Honest descriptive layer | No paired-patient permutation, functional two-sample inference, noninferiority, or equivalence. |
| CUR-15 | Strict result 0.3, typed availability, finite output, transactional artifacts | `ResultDocument`, `OutputWriter` | Tagged result kinds; `deny_unknown_fields`; current provenance = program + crate version | Schema/round-trip/output tests | Strong engineering foundation | Provenance lacks Git SHA, input/config digests, model identity, coordinate frame, and artifact references. |
| CUR-16 | CLI analyze/batch/prepost/multimodal/simulate/smoke; optional WSI inspect/extract | `run_cli` | Typed commands and feature gates | CLI and WSI tests | Mature adapters | No point-process, cohort, embedding, fingerprint, or interoperability command families. |
| CUR-17 | Exact `rstar` 2-D index, reusable plans, deterministic seeds, memory accounting | Internal geometry/performance layers | `[performance]`, strict reproduction, memory budget | Performance contracts and Criterion benches | Strong substrate | No formal exact/approx result mode or quantified ANN/sampled-pair error contract. |
| CUR-18 | CI, dependency policy, fuzz build, scheduled calibration | GitHub Actions | Locked toolchain, deny/audit/machete, weekly calibration | Static workflow inspection | Strong process design | Pinned HEAD was not executed in this audit; current method coverage remains narrow. |

## 2.2 Current public/internal boundary

The crate root deliberately exposes a narrow compatibility surface and keeps algorithm/orchestration modules private. Preserve that policy. New scientific functions should not be made public while their estimand, window, edge correction, null, undefined states, oracle, and calibration are still moving.

The current engines are already decomposed into private stages. Do not add point-process, cohort, embedding, or topology code by extending `api.rs` or `multimodal/engine.rs` with new branches. Add cohesive domain modules and focused application services.

## 2.3 Validation maturity

- **Directly verified from source:** broad.
- **Executed at pinned SHA:** none.
- **Historical execution evidence:** substantial, including prior remediation test matrices, scheduled negative-control calibration, scaling benchmarks, and memory measurements.
- **Independent scientific validation:** strong for selected internal kernels and naming corrections; absent for the proposed classical point-process, cohort, embedding, interface, and equivalence families.
- **Real-data external validation:** not demonstrated for the proposed roadmap.

The implementation lead must begin by reproducing the pinned baseline locally before accepting this plan’s first code change.

---

# 3. Research and comparator synthesis

## 3.1 Comparator ecosystem

| System/specification | Audited release/status and license | What it already does well | Marklab disposition |
|---|---|---|---|
| `spatstat` | 3.6-2 (2026-07-31), GPL-2-or-later | Canonical windows, K/L/g/F/G/J, marks, replicated point patterns, simulation, model fitting, diagnostics | **Numerical/semantic oracle and R interoperability.** Implement only pathology-relevant scalable summaries/inference; do not clone the full ecosystem. |
| `spatstat.model` | Active 3.7-series, GPL-2-or-later | Poisson, Cox, Gibbs and related point-process fitting and diagnostics | **Integrated R backend and numerical oracle.** Marklab owns typed inputs, invocation, diagnostics normalization, artifacts, and comparison while `spatstat.model` performs fitting. |
| Squidpy | 1.8.3 (2026), BSD-3-Clause | Python spatial-omics graphs, neighborhood enrichment, image/spatial workflows | **Interoperability and comparative validation.** Do not equate graph enrichment with edge-corrected point-process interaction. |
| SPIAT | 1.14.0, Artistic-2.0 | Spatial image analysis, cell neighborhoods, distances, tissue structure | **Interoperability/validation.** |
| spaSim | 1.14.0, Artistic-2.0 | Simulation of spatial tissue patterns | **Integrated validation backend.** Retain independent native canonical simulators while invoking spaSim through pinned workflows. |
| Giotto Suite | 4.2.3, MIT | End-to-end spatial omics, data structures, visualization, domains and analysis | **Integrated ecosystem and interchange backend.** Expose Giotto workflows from Marklab projects without duplicating its entire implementation. |
| BANKSY | Python 1.3.5 in 2026, GPL-3.0 | Spatially informed clustering/domain detection | **First-class integrated domain-discovery backend.** Import soft/hard assignments, scale, model, uncertainty, and provenance. |
| MISTy / mistyR | 1.20.0, GPL-3 | Multiview local/juxtaview/paraview predictive modeling | **First-class integrated multiview-model backend.** Marklab prepares views, executes pinned models, imports diagnostics, and tests patient-level incremental value. |
| LIANA+ | 1.8.1, BSD-3-Clause | Cell-cell communication and multimethod consensus | **First-class integrated communication-hypothesis backend.** Marklab must not relabel scores as causal signaling. |
| UTAG | Published implementation; GPL-3 reported; exact release pin unresolved | Unsupervised microenvironment-aware phenotyping | **Integrated experimental backend.** Require an exact version/license pin before enabling it. |
| CytoMAP | MATLAB ecosystem; toolbox dependencies; redistributable license not resolved here | Tissue-region and neighborhood exploration | **Comparator/integrated backend candidate** after licensing and reproducibility are resolved. |
| CytoCommunity | 1.1.0, MIT; hierarchical extension published in 2026 | Graph-neural community discovery | **Integrated graph-community backend; hierarchical extension remains research-tier.** Import assignments, uncertainty, and training provenance. |
| SpatialDE family | Bioconductor 1.16 stable / 1.19 development, MIT | Spatially variable expression | **Integrated spatial-variable-feature backend.** Marklab may invoke and consume the model while preserving its own project/provenance contract. |
| SpatialData | 0.8.0, BSD-3-Clause | Multimodal spatial coordinate systems, tables, shapes, transformations | **Primary Python interchange target.** |
| AnnData | 0.13.2, BSD-3-Clause | Annotated matrices, observations/variables/uns | **Primary table/embedding interchange target.** |
| OME-NGFF | 0.5 released; 0.6 draft/RC work in 2026 | Multiscale image and coordinate-transformation specification | **Image/coordinate metadata target.** Stable code must pin 0.5; 0.6 remains watch until final. |
| Apache Arrow | 25-series official format/implementation in 2026, Apache-2.0 | Typed columnar memory, validity bitmaps, fixed-size list arrays, zero-copy interchange | **Native artifact substrate.** |
| Apache Parquet | Active Apache columnar standard, Apache-2.0 | Durable compressed columnar artifacts | **Native durable artifact substrate.** |
| STARCH | Reference implementation, MIT | Expression/coordinate-based spatial clone inference | **Integrated clone-calling backend.** |
| SpaCNA | 2026 publication; license not resolved in this audit | HMRF CNA inference combining expression, spatial context and morphology | **Integrated CNA/clone backend.** Import probabilities, uncertainty, and provenance. |
| CalicoST | Reference implementation, BSD-3-Clause | Allele-specific spatial CNA/clone inference | **Integrated backend.** |
| PASTE / PASTE2 | Published reference implementations; PASTE2 BSD-3-Clause | Slice alignment, partial-overlap alignment, 3-D stacking | **Integrated registration/alignment backend.** |
| STalign | Published implementation, GPL-3.0 | Diffeomorphic alignment of partially matched 2-D/3-D point clouds | **Integrated backend.** Import transform/deformation/QC artifacts. |
| GPSA | Research implementation; exact release/license unresolved | Deep Gaussian-process alignment | **Integrated research backend.** |
| CellViT / CellViT++ | CellViT paper 2024; repository includes checkpoint history and restrictive Commons-Clause addition | Cell segmentation/classification and per-cell morphology representations | **First-class integrated model backend.** Marklab must require exact checkpoint checksum and extraction provenance. |
| UNI | 2024, research/non-commercial no-derivatives terms | General pathology representation learning | **Integrated model backend subject to license.** License prevents assuming unrestricted redistribution/use. |
| CONCH | 2024, research/non-commercial no-derivatives terms | Vision-language pathology representations | **Integrated model backend.** |
| Prov-GigaPath | 2024 research release | Tile- and slide-level pathology representations | **Integrated model backend.** |
| Virchow | 2024 research release | Large pathology foundation-model embeddings | **Integrated model backend.** |
| `glmmTMB` | 1.1.14, AGPL-3 | GLMMs including beta-binomial families | **Integrated statistical backend.** |
| `spaMM` | 4.6.65, CeCILL-2 | Spatial and non-spatial mixed models | **Integrated statistical backend.** |
| INLA / `inlabru` | Active 2.14-series wrapper, GPL-2-or-later | Latent Gaussian spatial models and point-process workflows | **Integrated Bayesian backend.** |

Any integration whose exact version or license is marked unresolved must remain disabled until those fields are pinned in an adapter manifest.

## 3.2 Program-wide capability classification

The prior binary distinction between “native” and “external” is replaced by a product-integration model. A capability may use an external numerical backend and still be a first-class Marklab subsystem.

### Class A — Native validated core

Implement and maintain in Rust when exact semantics, deterministic execution, scale, geometry reuse, or artifact control provide a clear advantage:

- identity, cohort, units, coordinate systems, and modality registry;
- observation windows, masks, compartments, boundaries, and 2-D/3-D geometry;
- spatial indexes, graph construction, pair plans, neighborhood plans, and streaming visitors;
- classical point-process and marked-process estimators;
- permutation, bootstrap, global-envelope, multiple-testing, and exact scalar inference;
- spatial weights, autocorrelation, covariance, and variogram summaries;
- pathology geometry and interface measurements;
- workflow graph, scheduler, project state, provenance, results, artifacts, and reports;
- compact embedding storage and model-agnostic vector statistics;
- backend manifests, validation harnesses, and cross-language oracles;
- deterministic CPU execution and explicit approximate modes.

### Class B — Native Bayesian infrastructure plus integrated inference engines

Bayesian modeling is a first-class program area. Marklab owns:

- typed model specifications;
- data and hierarchy preparation;
- prior declarations and prior provenance;
- backend-neutral parameter and latent-field schemas;
- deterministic chain/seed management;
- fit lifecycle and checkpointing;
- posterior artifact formats;
- convergence and diagnostic normalization;
- prior/posterior predictive checks;
- simulation-based calibration;
- model comparison and sensitivity workflows;
- uncertainty propagation and result interpretation.

Inference may be executed by native Rust kernels or integrated Stan, PyMC, NumPyro/JAX, INLA/inlabru, Turing, or specialized engines. The user remains inside a Marklab workflow.

### Class C — Native experimental mathematics

Implement within non-default experimental crates when Marklab can provide unique geometric integration, scale, or composability:

- graph Fourier and heat-kernel summaries;
- spectral graph wavelets, diffusion wavelets, and graph scattering;
- heterogeneous graphs, hypergraphs, simplicial/cellular complexes;
- persistence, Euler characteristic, Minkowski functionals, and percolation;
- differentiable spatial summaries;
- approximate geometry with quantified error;
- spatially aware kernel tests for high-dimensional vectors;
- mechanistic spatial simulators;
- selected neural point-process or generative components where native execution is justified.

### Class D — Integrated model and modality backends

These are first-class Marklab features accessed through pinned adapters and workflow nodes:

- pathology and spatial-omics foundation models;
- morphology-to-IHC/omics/CNA/outcome models;
- nonrigid/diffeomorphic registration;
- optimal transport and graph matching;
- CNA/clone/lineage callers;
- spatial domain and niche discovery;
- ligand-receptor and communication-hypothesis methods;
- survival, competing-risk, and high-capacity predictive models;
- GPU neural networks and large generative models.

Marklab must be able to install, invoke, validate, catalog, compare, and visualize these backends. It does not need to duplicate their internals in Rust.

### Class E — Research laboratory

Implement or integrate behind a research-only tier:

- neural marked-point and neural Cox processes;
- diffusion and flow-based cellular organization models;
- amortized posterior inference;
- neural likelihood, posterior, and ratio estimation;
- simulation-based inference;
- causal models with interference where treatment designs exist;
- 3-D and longitudinal tissue processes;
- phylogeographic and evolutionary models;
- active spatial sampling and stain/ROI selection;
- privacy-preserving federated multi-institution analysis.

### Class F — Rejected claims and anti-patterns

Reject the claim or implementation pattern, not necessarily the underlying mathematics:

- causal claims without an identified intervention/exposure design;
- clinical claims without external validation;
- generated values represented as measurements;
- transport represented as true correspondence;
- cells/patches treated as biological replicates;
- decorative algorithm names;
- posterior summaries without diagnostics;
- approximate inferential results without declared error;
- experimental methods silently entering stable reports;
- one opaque “spatial score” replacing interpretable components;
- a plugin ecosystem without schema, provenance, and security contracts.

## 3.3 Comparator role in the expanded platform

Comparators are not merely things Marklab should avoid reimplementing. They define four roles:

1. **Numerical oracle:** trusted small-case agreement and semantic reference, especially `spatstat`, Stan, PyMC, INLA, and established geometry libraries.
2. **Integrated backend:** invoked directly from Marklab projects with pinned execution manifests.
3. **Interchange target:** import/export through SpatialData, AnnData, OME-NGFF, Arrow, Parquet, model manifests, and standard image/omics formats.
4. **Benchmark competitor:** evaluated on accuracy, calibration, runtime, memory, usability, and multimodal composability.

## 3.4 Backend admission contract

Before an external backend is enabled in a stable distribution, record:

- package and repository;
- exact version/commit;
- license and redistribution constraints;
- runtime language and environment lock;
- supported platforms and hardware;
- deterministic controls;
- expected input/output schemas;
- coordinate and unit semantics;
- model/checkpoint checksums;
- failure and timeout behavior;
- diagnostics and uncertainty semantics;
- security/network requirements;
- benchmark and reference fixtures;
- upgrade and migration policy.

## 3.5 Program horizon and promotion

No method is excluded merely because it is complex or Bayesian. Promotion depends on the evidence tier:

- `research_only` may be implemented from a single compelling paper if the limitations are explicit;
- `experimental` requires executable fixtures, deterministic behavior where possible, and basic simulation checks;
- `established` requires canonical semantics and independent numerical validation;
- `validated` requires calibration, power/coverage as appropriate, real-data validation, stable schema, and external replication or benchmark evidence.

## 3.6 Scientific coherence rule

The program may contain hundreds of methods only if they are organized around explicit questions and interoperable data contracts. Every method must identify:

- the scientific question;
- estimand or prediction target;
- observation window and scale;
- null, likelihood, or generative assumptions;
- biological replication unit;
- required modalities;
- uncertainty and failure states;
- output maturity tier;
- interactions with other workflow nodes;
- benchmark and promotion criteria.

---

# 4. Scientific gap and triage matrix

## 4.1 Main triage matrix

Priority meanings: **P0** prerequisite, **P1** next stable value, **P2** advanced stable/interop, **P3** experimental, **W** watch, **R** reject.

| ID | Scientific question | Current support | Maturity | Classification | Priority | Principal reason | Primary reference/oracle | Dependencies | Major validation risk |
|---|---|---|---|---|---|---|---|---|---|
| FND-01 | What is the biological/technical unit and stable object identity? | Loose string metadata; no hierarchy or stable marked-path CellId | Foundational | Stable native | P0 | Required for replication, joins, leakage control and artifacts | Repository/domain contract | None | Breaking ingestion assumptions |
| FND-02 | What exact 2-D tissue window generated the pattern? | Mask plus area/effective-length summary | Foundational | Stable native | P0 | Estimands and edge corrections otherwise undefined | `spatstat.geom` semantics | FND-01 | Polygon validity, holes, coordinate frames |
| FND-03 | Can geometry be planned once and reused? | Strong index and endpoint-specific plans | Foundational | Stable native | P0 | Prevent duplicate distances and permutation rebuilds | Current indexed-plan architecture | FND-02 | Memory blow-up in dense radii |
| FND-04 | Is a value measured, predicted, probabilistic, ordinal or vector-valued? | Binary/probability fields only | Foundational | Stable native | P0 | Prevents measurement/prediction conflation | Typed domain contract | FND-01 | Schema sprawl |
| FND-05 | Can cell/patch embeddings be stored and traced? | CellViT class import only; no vectors/provenance | Foundational | Stable native artifact | P0 | Existing asset cannot support stable claims otherwise | Arrow/Parquet | FND-01, FND-04 | Unknown checkpoint/mapping |
| FND-06 | What null and randomization unit are permitted? | Cell-label nulls within one pattern | Foundational | Stable native | P0 | Prevents invalid exchangeability | ERL/permutation literature | FND-01 | Hidden pseudoreplication |
| FND-07 | Can results be reproduced and independently validated? | Strong 0.3/CI substrate, thin provenance | Foundational | Stable native | P0 | Required before new public scientific families | Current result contracts | FND-01–06 | Migration and artifact drift |
| PP-01 | Is a point pattern clustered or inhibited relative to homogeneous intensity? | No K/L | Established | Stable native | P1 | Canonical foundation | Ripley K; `spatstat` | FND-02/03/06 | Edge-correction mismatch |
| PP-02 | Does interaction remain after spatially varying intensity? | No inhomogeneous estimator | Established | Stable native | P1 | Separates density/compartment gradients from interaction | Baddeley et al.; `Kinhom` | PP-01, intensity | Biased intensity reuse |
| PP-03 | How does interaction vary with distance and type? | Centered mark covariance and raw cross pair counts only | Established | Stable native | P1 | Genuine \(g\), cross-K, cross-\(g\) are currently absent | `pcf`, `Kcross`, `pcfcross` | PP-01/02 | Kernel bandwidth, sparse types |
| PP-04 | How do empty-space and nearest-neighbor scales behave? | Mean NN only | Established | Stable native | P2 | Complements K/g; useful for infiltration/spacing | F/G/J literature | FND-02/03 | Border/empty-space sampling |
| MRK-01 | Are labels/values associated conditional on locations? | Binary covariance; graph enrichment | Established | Stable native | P1 | Generalizes categorical, continuous and probabilistic marks | mark connection/correlation/Kmark | FND-04, PP plans | Normalization and rare labels |
| SIG-01 | Are continuous values or embeddings spatially autocorrelated? | No public general support | Established | Stable native | P1 | Required for IHC, omics and morphology vectors | Moran, Geary, variograms | FND-04/05, weights | Weight selection, multiple testing |
| GEO-01 | How do cells relate to compartments and interfaces? | No boundary contract | Established/pathology-direct | Stable native | P1 | Clinically interpretable and distinct from generic clustering | Computational geometry | FND-02/03 | Segmentation/boundary uncertainty |
| NIC-01 | What multiscale neighborhoods exist and reproduce? | MMR density components and fixed graph profiles | Established-to-emerging | Native summaries + external discovery | P1 | High value, but clustering should not be hard-coded | SPIAT/BANKSY/UTAG | FND-01/03/04 | Scale selection and stability |
| COH-01 | Do spatial endpoints differ across patients/timepoints/sites? | No cohort inference | Established | Stable native | P1 | Cells are not biological replicates | hierarchical bootstrap/permutation | FND-01/06 | Few patients, nesting errors |
| CMP-01 | Are specimens descriptively or population-level different? | Approximate pooled-bin diagnostic | Established components | Stable native | P1 | Typed fingerprints plus patient-level tests replace ad hoc comparisons | ERL, MMD, energy | COH-01 | Endpoint selection and dimensionality |
| EQV-01 | Are endpoints genuinely equivalent within margins? | Descriptive margin only | Established | Stable native | P1 | Prevents false sameness claims | TOST/equivalence design | COH-01, CMP-01 | Unjustified margins, low power |
| EMB-01 | Are high-dimensional morphology marks spatially structured? | No vectors | Established components/emerging synthesis | Stable native summaries | P1 | Rotation-invariant, model-agnostic use of existing asset | variogram/kernel/MMD literature | FND-05, SIG-01 | Confounding and learned preprocessing |
| CLN-01 | How are imported clones/CNAs spatially organized? | No clone contract | Established downstream metrics | Stable native downstream | P2 | Native caller unnecessary; downstream geometry is valuable | STARCH/SpaCNA/CalicoST inputs | FND-04, GEO-01, COH-01 | Assignment uncertainty |
| BAY-REG | Can covariates explain spatial outcomes with hierarchical and spatial dependence? | No general regression | Established | Integrated Bayesian platform | P1 | Required for cohort, multimodal, field, and uncertainty-aware inference | Stan/PyMC/INLA/spaMM references | PLAT-01, DATA-01, BAY-01 | Identifiability, diagnostics, backend agreement |
| FR-01 | Do graph frequencies/wavelets reveal pathology architecture? | Graph exists; no transform | Established math, emerging pathology | Native experimental | P3 | Irregular-cell fit and potential uniqueness | SGWT/diffusion wavelets | FND-03, stable weights | Graph-definition sensitivity |
| FR-02 | Do patch embeddings add patient-held-out value beyond cell embeddings? | No patch data | Emerging | External encoders + native experimental stats | P3 | Potentially distinctive, but data/provenance heavy | Pathology FM literature | FND-05, COH-01 | Scanner/stain/site confounding |
| FR-03 | Can partial transport support atlas mapping/retrieval? | No OT | Established math/emerging tissue use | External solver + experimental adapter | P3 | Useful descriptive alignment; not correspondence | PASTE2/FGW literature | FND-01/04/05 | Non-identifiability |
| TOP-01 | Does topology capture clinically meaningful organization? | No | Emerging | Research watch/experimental | W | Promising only with interpretable endpoints and replication | persistent homology literature | FND-02/03, COH-01 | Feature instability/overfitting |
| REG-01 | Can sections be nonrigidly aligned with uncertainty? | Rigid/affine only | Established algorithms | Integrated backend + native uncertainty contract | P1/P2 | Essential multimodal capability; external solvers operate inside Marklab workflows | STalign/PASTE2/diffeomorphic references | PLAT-01, GEO-01, BACK-01 | False correspondence and unresolved uncertainty |
| GEN-01 | Can tissues be generated or simulated by mechanistic and neural models? | Minimal simulators only | Emerging/speculative | Native/integrated research laboratory | P2/P3 | Central frontier capability for posterior predictive science, SBI, and design | neural point-process/generative literature | PLAT-01, BAY-SBI, simulator library | Calibration, memorization, mode collapse |
| CAU-01 | What is the causal effect of a spatial exposure with interference? | None | Established theory, data-design limited | Gated research laboratory | P3 when eligible | Implement when explicit treatments/exposures and identification conditions exist; otherwise association only | Hudgens–Halloran; Aronow–Samii | DATA-01, COH-01, treatment design | Positivity/unmeasured confounding |
| DL-CORE | Should Marklab provide foundation-model execution? | No | Mature/fast-moving | First-class integrated backend; native only with proven advantage | P1/P2 | All-in-one user experience requires execution, provenance, comparison, and validation without duplicating every model in Rust | Model repositories and official checkpoints | EMB-CORE, BACK-01 | Reproducibility, license, domain shift |

## 4.2 Complete investigated-capability ledger

This ledger is intentionally broad. A “watch” or “reject” entry is a completed triage decision, not unfinished work.


### Platform, Bayesian, multimodal, and operating-system families

| ID | Capability | Current support | Classification | Priority | Decision and validation risk |
|---|---|---|---|---|---|
| PLAT-01 | MarklabProject and artifact catalog | Absent | Foundational stable platform | P0 | Required for all-in-one reproducibility and resumability. |
| WF-01 | Typed workflow DAG and scheduler | Absent | Foundational stable platform | P0 | Must support solo methods and composed analyses without untyped task execution. |
| BACK-01 | Backend/plugin registry | Absent | Foundational stable platform | P0 | Pin environments, licenses, schemas, diagnostics, and security. |
| UX-01 | Interactive computational-pathology workbench | Absent | Core product | P1 | UI must consume workflow results and never duplicate science. |
| DATA-01 | Cohort/modality/coordinate/unit registry | Partial loose metadata | Foundational stable platform | P0 | Prevent leakage, unit mismatch, and multimodal identity failure. |
| EMB-CORE | CellViT per-cell embedding table and provenance | Claimed asset, not exposed in audited importer | Core modality | P0/P1 | Recover actual vectors and model provenance before stable claims. |
| EMB-PATCH | Patch/region/slide embeddings and links | Absent/unresolved | Core modality | P1 | Physical scale, overlap, and shared-vector semantics mandatory. |
| BAY-01 | Model IR and prior system | Absent | Foundational Bayesian platform | P1 | Must remain typed, unit-aware, backend-neutral, and identifiable. |
| BAY-02 | Posterior artifacts and diagnostics | Absent | Foundational Bayesian platform | P1 | Failed/nonconverged fits cannot produce ordinary available results. |
| BAY-03 | Hierarchical cohort models | Absent | Core Bayesian evidence | P1 | Patient-level design and partial pooling are central. |
| BAY-04 | GP/Matérn/nonstationary fields | Absent | Core Bayesian spatial engine | P2 | Scale requires sparse/mesh/NN approximations with calibration. |
| BAY-05 | CAR/SAR/GMRF graph fields | Absent | Core Bayesian spatial engine | P2 | Graph/weight definitions are part of the estimand. |
| BAY-PP | Poisson/LGCP/cluster/Gibbs/marked point processes | Absent | Core Bayesian point-process engine | P2 | Require known-parameter recovery and posterior predictive spatial checks. |
| BAY-NP | Nonparametric niches/domains | Absent | Experimental Bayesian engine | P2/P3 | Label switching, scale, overlap, and reproducibility are major risks. |
| BAY-MM | Multimodal latent-variable models | Absent | Core/experimental Bayesian multimodal engine | P2 | Measured and predicted modalities must remain distinct. |
| BAY-REG | Probabilistic registration/correspondence | Deterministic rigid/affine only | Integrated Bayesian multimodal engine | P2 | Propagate transform uncertainty and avoid false identity. |
| BAY-EVO | Clone phylogeography/evolution | Absent | Experimental Bayesian engine | P3 | Cross-sectional identifiability limits must be explicit. |
| BAY-SBI | Simulation-based/amortized inference | Absent | Research-only Bayesian frontier | P3 | SBC, support/OOD, and simulator misspecification are decisive. |
| GEN-01 | Mechanistic/neural tissue generation | Minimal deterministic smoke simulators | Research laboratory | P3 | Visual realism is insufficient; calibration and memorization checks required. |
| DIM-01 | 3-D/serial/longitudinal contract | Absent | Advanced core platform | P2/P3 | Separate dimensionality and deformation uncertainty are mandatory. |
| CAU-01 | Spatial causal/interference workflows | Absent | Gated research laboratory | P3 when eligible | Treatment/exposure design and identification determine admissibility. |

### Point-process, null and inference families

| ID | Capability | Current support | Classification | Priority | Decision and validation risk |
|---|---|---|---|---|---|
| PP-01 | Homogeneous K/L | Absent | Stable native | P1 | Add exact, named edge corrections; oracle against `spatstat`. |
| PP-02 | Inhomogeneous K/L | Absent | Stable native | P1 | Add only with explicit intensity estimator and leave-one-out policy. |
| PP-03A | Classical pair-correlation \(g(r)\) | Absent | Stable native | P1 | Distinct result from current centered mark covariance. |
| PP-03B | Cross-K and cross-\(g\) | Absent | Stable native | P1 | Require type-specific intensity, sparse-type undefined states. |
| PP-04A | G / NN distribution | Mean NN only | Stable native | P2 | Exact point-to-point nearest-neighbor distribution. |
| PP-04B | F / empty-space distribution | Absent | Stable native | P2 | Requires deterministic or sampled window probe plan with error metadata. |
| PP-04C | J function | Absent | Stable native | P2 | Undefined when denominator approaches zero; never persist infinity. |
| PP-05 | Intensity estimation | No general public estimator | Stable native | P1 | Kernel/piecewise compartment estimators; bandwidth provenance and cross-fitting. |
| PP-06A | Border correction | Absent | Stable native | P1 | First implementation because geometry and oracle are tractable. |
| PP-06B | Translation correction | Absent | Stable native | P1/P2 | Promote after polygon-overlap fixtures and memory benchmarks. |
| PP-06C | Isotropic correction | Absent | Stable native | P2 | Requires visible-boundary arc fraction; robust geometry tests. |
| PP-06D | Toroidal null/correction | Absent | Reject default | R | Only for genuinely periodic rectangular designs, not irregular tissue. |
| NUL-01A | CSR | Minimal random-label simulation only | Stable native | P1 | Distinguish location-process null from label null. |
| NUL-01B | Random labeling | Strong current support | Stable native | Existing | Generalize to typed marks/strata and exact permitted units. |
| NUL-01C | Population independence | Absent | Stable native | P2 | Use independent-component simulations/label processes as appropriate. |
| NUL-01D | Conditional/stratified nulls | Some label strata | Stable native | P1 | Generalize with declared conditioning variables and degeneracy reports. |
| NUL-01E | Translation-based shifts | Absent | Experimental/limited | P3 | Only when window and stationarity justify; quantify lost overlap. |
| INF-01A | Scalar permutation inference | Present | Stable native | Existing | Reuse canonical implementation. |
| INF-01B | Functional global envelopes | Present | Stable native | Existing | Reuse with method-specific eligibility. |
| INF-01C | Multiple-testing correction | BH present for enrichment | Stable native | P1 | Add endpoint-family registry and hierarchical multiplicity policy. |
| INF-01D | Confidence intervals/coverage | Mostly absent | Stable native | P1 | Method-specific bootstrap/asymptotic intervals; coverage simulation. |

### Mark, signal and pathology-geometry families

| ID | Capability | Current support | Classification | Priority | Decision and validation risk |
|---|---|---|---|---|---|
| MRK-01A | Mark-connection function | Absent | Stable native | P1 | Categorical/probabilistic labels; rare-label guards. |
| MRK-01B | Mark-correlation function | Centered binary covariance only | Stable native | P1 | Exact normalization and mean policy required. |
| MRK-01C | Mark-weighted K | Absent | Stable native | P2 | Distinct from mark correlation; specify weighting/normalization. |
| MRK-02A | Multitype point patterns | Label catalog in multimodal only | Stable native | P1 | General type table keyed by CellId. |
| MRK-02B | Ordinal marks | Absent | Stable native | P2 | Preserve order; do not coerce to nominal labels. |
| MRK-02C | Probabilistic marks | Binary probability supported in spectrum | Stable native | P1 | General probability simplex and uncertainty/null semantics. |
| MRK-02D | High-dimensional continuous marks | Absent | Stable native summaries | P1 | Contiguous artifacts, f64 accumulation, rotation-invariant tests. |
| SIG-01A | Global Moran I | Absent | Stable native | P1 | Freeze weights, centering and permutation null. |
| SIG-01B | Geary C | Absent | Stable native | P1 | Same weights contract; independent oracle. |
| SIG-01C | Local Moran/LISA | Absent | Stable native with guarded release | P2 | Exploratory maps; multiplicity and patient replication mandatory. |
| SIG-01D | Bivariate Moran/LISA | Absent | Stable native with guarded release | P2 | Direction and variable roles explicit. |
| SIG-01E | Getis–Ord/hotspots | Absent | Stable or external | P2 | Avoid redundant local-map sprawl; add only with pathology use case. |
| SIG-01F | Scalar variogram | Absent | Stable native | P1 | Robust pair/bin plan and directional options. |
| SIG-01G | Cross-variogram/covariance | Absent | Stable native | P2 | Requires co-location/correspondence semantics. |
| SIG-01H | Vector embedding covariance/similarity | Absent | Stable native/experimental | P1/P3 | Omnibus kernels; training-fold transformations only. |
| GEO-01A | Compartments and signed boundary distance | Compartment labels only | Stable native | P1 | Direct pathology quantity; segmentation uncertainty must be retained. |
| GEO-01B | Interface contact fractions | Absent | Stable native | P1 | Boundary orientation and denominator explicit. |
| GEO-01C | Infiltration depth | Absent | Stable native | P1 | Signed distance, phenotype selection, window clipping. |
| GEO-01D | Fragmentation/connected components/mixing entropy | Partial DBSCAN territory summary | Stable native | P2 | Define binary mask/graph basis and scale. |
| GEO-01E | Alpha shapes/morphological operations | Absent | Stable native or external geometry | P2 | Parameter sensitivity and holes. |
| GEO-01F | Budding/satellites/invasive front | Absent | Pathology-specific stable candidate | P2 | Requires validated compartment/front input, not inferred ad hoc. |
| GEO-01G | Vessel/gland/nerve/necrosis-relative statistics | Absent | Stable downstream | P2 | Import independently segmented objects and uncertainty. |
| GEO-01H | Interface curvature/front propagation | Absent | Experimental/watch | P3/W | Cross-sectional curvature is descriptive; propagation needs longitudinal data. |

### Neighborhood, clone, molecular and comparison families

| ID | Capability | Current support | Classification | Priority | Decision and validation risk |
|---|---|---|---|---|---|
| NIC-01A | Multiscale neighborhood composition vectors | Fixed graph/profile path | Stable native | P1 | Physical scales, soft labels, one shared plan. |
| NIC-01B | Hard/soft niches | No general discovery | External discovery + native summaries | P2 | Import assignments and uncertainty. |
| NIC-01C | Topic models/community detection | Absent | External | P2 | BANKSY/UTAG/Giotto/CytoCommunity ecosystem is stronger. |
| NIC-01D | Domain boundaries/transition zones | Absent | Stable downstream | P2 | Requires imported/stable domains and geometry. |
| NIC-01E | Differential niche abundance | Absent | Stable cohort layer | P2 | Patient is unit; compositional/hierarchical uncertainty. |
| NIC-01F | Reference-query mapping | Absent | External/experimental | P3 | Held-out mapping and missing types. |
| CLN-01A | Import CNA/clone labels/probabilities | Absent | Interoperability | P2 | Require caller/version/input/probability provenance. |
| CLN-01B | Clone segregation/mixing/interfaces | Absent | Stable native downstream | P2 | Null conditioned on compartment/intensity. |
| CLN-01C | Clone-immune/stroma/IHC/morphology concordance | Absent | Stable native downstream | P2 | Same-cell vs adjacent vs long-range explicit. |
| CLN-01D | Phylogenetic vs spatial distance | Absent | Stable/experimental | P2/P3 | Imported phylogeny and uncertainty; cross-sectional limits. |
| IHC-01 | Continuous/ordinal/probabilistic IHC | Binary MMR plus probability | Stable native | P1 | Preserve continuous OD/intensity; batch/threshold/registration uncertainty. |
| MOL-01 | Spatially resolved molecular cross-association | Absent | Stable summaries + external models | P2 | Cross-covariance and bivariate autocorrelation native; regression external. |
| BULK-01 | Bulk molecular association | Absent | Cohort fingerprint + external regression | P2 | Aggregate per specimen; never use cells as replicates. |
| CMP-01A | SpatialFingerprint | Ad hoc result families | Stable native | P1 | Typed, prespecified endpoints and uncertainty. |
| CMP-01B | Functional global-envelope two-sample | Absent | Stable native | P1/P2 | Permute at patient/specimen level. |
| CMP-01C | MMD | Absent | Stable native | P2 | Kernel chosen/trained without test leakage. |
| CMP-01D | Energy distance | Absent | Stable native | P2 | Patient-level exchangeability and finite-moment assumptions. |
| CMP-01E | Graph kernels | Absent | Experimental/external | P3 | Graph construction dominates result; high overfitting risk. |
| CMP-01F | Topological comparison | Absent | Research watch | W | Require stable filtration and interpretable clinical endpoint. |
| EQV-01A | Descriptive margin | Present | Stable descriptive | Existing | Keep name and role. |
| EQV-01B | TOST equivalence | Absent | Stable native | P1 | Prespecified margins and sufficient biological replicates. |
| EQV-01C | Noninferiority | Absent | Stable native | P2 | Directional margin and clinical rationale. |

### Modeling, alignment, dimensionality and frontier families

| ID | Capability | Current support | Classification | Priority | Decision and validation risk |
|---|---|---|---|---|---|
| BAY-REG-A | Spatial regression | Absent | Integrated Bayesian/frequentist backend | P1/P2 | Marklab owns design, workflow, diagnostics normalization, provenance, and result contract; backends perform fitting. |
| BAY-FIELD-A | Gaussian processes | Absent | Core Bayesian program with integrated/native engines | P1/P2 | Exact/sparse/NN/SPDE models are first-class; approximation and prior choices are explicit workflow state. |
| BAY-PP-A | LGCP | Absent | Core Bayesian point-process program | P2 | Complex but central; fit through Stan/INLA/PyMC/native components with SBC and posterior predictive checks. |
| BAY-PP-B | Gibbs/interaction processes | Absent | Core Bayesian/likelihood point-process program | P2 | Use trusted references and integrated backends; Marklab owns model, simulation, diagnostics, and comparison. |
| BAY-GMRF-A | CAR/SAR/SGLMM | Absent | Core Bayesian program with integrated backends | P1/P2 | Required for areal/graph fields and cohort models; native model contracts plus Stan/INLA/PyMC execution. |
| BAY-HIER-A | Genuine beta-binomial hierarchical model | Fixed-prior beta posterior diagnostic only | Core hierarchical Bayesian program | P1/P2 | Fit through integrated backends with explicit overdispersion, hierarchy, priors, diagnostics, and patient-level design. |
| REG-01A | Rigid/affine registration | Present | Stable native | Existing | Keep limited and explicit. |
| REG-01B | Nonrigid/diffeomorphic registration | Absent | First-class integrated backend | P1/P2 | Marklab invokes, validates, catalogs, visualizes, and propagates transform uncertainty. |
| REG-01C | Probabilistic cell correspondence | Absent | Integrated experimental/Bayesian subsystem | P2/P3 | First-class exploratory output with uncertainty; never call plausible matches true identity. |
| DIM-01 | Separate 3-D coordinate/window/statistics contract | Absent | Foundational advanced platform | P2 | Required for serial/3-D program; never apply 2-D estimators silently to 3-D. |
| TIME-01 | Longitudinal spatial processes | Pre/post descriptive only | Advanced Bayesian/cohort program | P2/P3 | Requires repeated biological units and deformation/change separation. |
| FR-01A | Graph Fourier/heat-kernel summaries | Absent | Native experimental | P3 | Fixed Laplacian and scale. |
| FR-01B | Spectral/diffusion graph wavelets | Absent | Native experimental | P3 | Exact transform definition and topology sensitivity. |
| GSP-03 | Graph scattering | Absent | Native/integrated experimental laboratory | P3 | Implement with exact fixtures and require incremental value over simpler spectral summaries. |
| FR-02A | Patch embedding table/link | Absent | Stable artifact | P1/P2 | Physical scales, overlap, one vector per patch. |
| EMB-PRED-01 | Cell+patch late fusion probes | Absent | First-class integrated predictive workflow | P1/P2 | Patient-held-out nested validation and comparison with simpler baselines. |
| EMB-PRED-02 | Cross-attention/transformer fusion | Absent | Experimental integrated model | P3 | Implement after strong baselines; promotion requires incremental patient-held-out value. |
| FR-03A | Partial/unbalanced OT | Absent | External solver + experimental result | P3 | Descriptive compatibility only. |
| FR-03B | Fused Gromov-Wasserstein | Absent | External solver + watch | P3/W | High non-identifiability and cost. |
| TOP-01A | Persistent homology/landscapes/images | Absent | Experimental/watch | W | Tie each filtration to pathology and replicate externally. |
| TOP-01B | Euler/Minkowski/percolation | Absent | Experimental | P3 | More interpretable than generic topology if tied to masks/interfaces. |
| GEN-01A | Neural point/Cox processes | Absent | Research-only implementation track | P3 | Require calibrated inference, posterior predictive checks, and comparison with classical/LGCP models. |
| GEN-01B | Diffusion/generative tissue layouts | Absent | Research-only implementation track | P3 | Memorization, privacy, mode collapse, and simpler-model comparison are mandatory. |
| GEN-01C | “Digital twin” | Absent | Reject | R | Unsupported without prospective intervention/outcome evidence. |
| CAU-01A | Spatial interference/potential outcomes | Absent | Gated research implementation | P3 when design permits | Only with genuine treatment assignment, exposure mapping, positivity, and identification. |
| CAU-01B | Spatial propensity/DML/negative controls | Absent | Gated research implementation | P3 when design permits | Strong assumptions, positivity, negative controls, and sensitivity analysis. |
| CAU-01C | Causal discovery | Absent | Research-only hypothesis generation | P3/R | Never promoted to causal evidence without intervention/identification support. |
| CCC-01 | Spatial cell-cell communication | Absent | External hypothesis generation | P2 | Distinguish co-location, dependency, prediction and validated signaling. |
| PERT-01 | Spatial perturbation screens | Absent | Interoperability/watch | W | Data contract only until designed perturbation datasets exist. |
| ACT-01 | Active ROI/stain/FOV/sampling design | Absent | Research watch | W | Prospective design distinct from retrospective analysis. |
| WAV-01A | ODWT/MODWT/undecimated raster wavelets | Honest residual heuristic only | Reject default/research watch | R/W | Irregular cells favor graph methods; add raster transform only for raster-defined question. |
| WAV-01B | Actual DoG | Absent | Low-priority experimental | P3 | Add only for a raster/image scale-space endpoint, not cell-pattern branding. |
| SPC-01A | True Bartlett periodogram | Hann-tapered raster periodogram only | Low-priority/reject default | R/P3 | No clear advantage for current pathology questions. |
| SPC-01B | Multitaper spectral estimation | Absent | Research watch | W | Add only if variance reduction changes a prespecified endpoint. |
| SCALE-01 | ANN, streaming, out-of-core, progressive summaries | Exact index and streaming input foundations | Stable infrastructure | P1/P2 | Approximate modes must expose error and preserve exact fallback. |



## 4.3 2024–2026 frontier horizon scan and scoring

Score direction: for **scientific value, uniqueness, maturity, validation feasibility, data availability, runtime/memory feasibility and five-year durability**, 5 is favorable. For **implementation cost** and **overclaiming risk**, 5 means high cost/risk. Scores are deliberately not summed.

| Direction | 2024–2026 evidence/maturity | Reference implementation/version/license | Exact target and principal identifiability issue | Native Rust benefit | Class | SV | Unique | Mature | Validate | Data | Cost | Runtime | Overclaim | Durable |
|---|---|---|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| FR-01 graph heat/SGWT/diffusion summaries | Established mathematics; pathology application emerging | SGWT/diffusion reference toolboxes; versions/licenses must be pinned per adapter | Spectral energy/smoothness at fixed graph scales; graph definition is part of estimand | High for sparse deterministic million-node execution | Experimental bet | 4 | 5 | 3 | 4 | 4 | 3 | 4 | 3 | 5 |
| FR-02 cell+patch embedding statistics | Foundation models mature; multiscale incremental evidence still emerging | External encoders with heterogeneous/restrictive licenses | Cross-level spatial association and patient-held-out incremental value; technical signatures and overlap threaten identification | High for model-agnostic artifacts/statistics, low for model execution | Experimental/interop bet | 5 | 5 | 3 | 3 | 2 | 4 | 3 | 5 | 5 |
| FR-03 partial/unbalanced/FGW atlas comparison | OT methods established; biological correspondence remains non-identifiable | PASTE2 and external OT solvers; pin exact version/license | Descriptive transport objective/plan; multiple plausible plans and cost scaling | Moderate as typed validation/result layer, low as solver | Experimental/interop bet | 4 | 4 | 3 | 3 | 3 | 4 | 2 | 5 | 4 |
| TOP-01 persistent-homology tissue biomarkers | 2024–2026 pathology applications emerging, limited independent replication | GUDHI/Ripser-like external libraries; license/version review required | Filtration-derived topology tied to a pathology endpoint; filtration instability | Moderate for deterministic summaries, weak without a use case | Research watch | 3 | 3 | 2 | 2 | 3 | 4 | 3 | 5 | 3 |
| HET-01 heterogeneous graphs/hypergraphs/complexes | Rapidly emerging | Mainly Python deep-learning ecosystems | Higher-order architecture representation; entity extraction and invariance dominate | Low until stable nonlearned estimand exists | Watch | 3 | 3 | 2 | 2 | 2 | 5 | 2 | 5 | 3 |
| EQ-01 E(2)/SE(2)-equivariant learned models | Active 2024–2026 research | External PyTorch/JAX systems | Predictive target under chosen invariance; harmful invariance and domain shift | Low in Rust core | Interop/watch | 3 | 2 | 2 | 2 | 2 | 5 | 2 | 5 | 3 |
| GEN-01 neural point/Cox/diffusion tissue generators | Emerging/single-paper in several subfamilies | Research Python/JAX/PyTorch implementations | Conditional distribution of layouts; likelihood/calibration/memorization unresolved | Moderate as integrated research platform | Research-only build track | 4 | 5 | 1 | 3 | 2 | 5 | 2 | 5 | 4 |
| CAU-01 causal spatial interference | Established causal theory; pathology applications data-limited | Statistical research code, no single operational standard | Treatment/exposure causal effect under interference; treatment assignment/positivity often absent | Moderate as gated workflow and design checker | Research-only when eligible | 5 | 4 | 3 | 2 | 1 | 5 | 3 | 5 | 4 |
| CCC-01 spatial communication | Mature scoring ecosystems, causal meaning unvalidated | LIANA+/MISTy and related external tools | Co-expression/dependency/mechanistic plausibility; not validated signaling | Low for scoring, moderate for downstream patient tests | Interop | 3 | 2 | 3 | 2 | 3 | 4 | 3 | 5 | 3 |
| DIM-01 serial-section/3-D reconstruction | Strong technical progress, uncertainty still substantial | STalign/PASTE2/3-D reconstruction pipelines | 3-D point process after uncertain deformation/correspondence | Moderate after external reconstruction | Watch | 4 | 4 | 2 | 2 | 1 | 5 | 2 | 5 | 4 |
| ACT-01 active ROI/stain/FOV selection | 2025 active-sampling demonstrations emerging | Research pipelines | Expected information/power gain under prospective acquisition; requires laboratory action model | Moderate only with design partners | Watch | 4 | 4 | 2 | 2 | 1 | 4 | 3 | 4 | 4 |
| CLN-02 clonal phylogeography/growth direction | Spatial clone tools maturing through 2026 | External clone/phylogeny callers | Segregation and spatial–phylogenetic association are identifiable; growth direction often is not cross-sectionally | High for downstream summaries, low for evolutionary model | Stable downstream + watch for direction | 4 | 4 | 3 | 3 | 2 | 3 | 4 | 5 | 5 |

### Frontier promotion and kill framework

For every experimental task, the task contract must additionally record:

- maturity class: established, independently replicated emerging, single-paper emerging or speculative;
- primary papers and independent replications;
- reference implementation, exact version and license;
- benchmark datasets and their patient/site structure;
- exact estimand or prediction target;
- identifiability statement;
- calibration and technical-confounder requirements;
- biological-overinterpretation risk;
- computational cost and native-Rust advantage;
- promotion and kill criteria.

The selected bets are FR-01, FR-02 and FR-03. All other rows remain watch, interoperability or limited downstream scope until their promotion criteria are met.


# 5. Detailed method cards

## 5.1 Shared card conventions

Let \(n\) be cells, \(m(r)\) the number of eligible pairs within maximum distance \(r\), \(d\) embedding dimension, \(B\) null replicates, \(P\) patients, and \(S\) specimens. “Exact” means no unreported pair sampling, ANN substitution, raster approximation, or GPU-only numerical path. All persisted numbers must be finite; undefined states are typed.

Every inferential card inherits these rules:

- geometry and edge plans are built once and reused across observed/null evaluations;
- seeds are namespaced by endpoint and replicate;
- cell-label randomization is allowed only for within-pattern random-labeling questions;
- patient/specimen permutations are required for cohort claims;
- transformations learned from data—normalization, PCA, whitening, kernels, feature selection and scale selection—are fit inside training folds;
- local endpoint families require an explicit multiplicity family;
- result JSON stores summaries and artifact references, never large matrices;
- an independent oracle is generated by a pinned external implementation or a hand-computable fixture and checked in with provenance;
- stable release requires type-I calibration, power, coverage where applicable, deterministic thread/seed behavior, degenerate-state tests, and real-data face validity.

## 5.2 Foundational prerequisite cards

### FND-01 — Typed identity and cohort hierarchy

- **Scientific question/current support:** What objects are independent biological units and how are cells/patches nested? Current support is loose strings in `PatternMeta` and multimodal metadata; no typed hierarchy or stable marked-pattern `CellId`.
- **Canonical definition/input/window/estimand:** Add non-empty, validated newtypes `PatientId`, `SpecimenId`, `TimepointId`, `SlideId`, `CoreId`, `RegionId`, `CellId`, `PatchId`; `CohortHierarchy` records parent relations and design roles. Input is a normalized observation manifest. Window and estimand are not applicable.
- **Null/alternatives/edge/inhomogeneity/randomization:** Not applicable. The contract supplies permitted randomization units to downstream methods.
- **Assumptions/failure modes/status/replication:** IDs are stable within an analysis bundle; parentage is acyclic and unambiguous. Reject duplicate object IDs, missing parents, conflicting parentage, blank IDs, and cross-artifact ID drift. This is a domain prerequisite, not a scientific result. Biological replication is represented, not inferred.
- **Result/undefined states:** No cell table in result JSON. Persist `CohortDesignSummary`, counts by level, design type, and an artifact digest. Typed failures: `MissingParent`, `DuplicateId`, `ConflictingParent`, `UnsupportedNesting`, `UnresolvedBiologicalUnit`.
- **Complexity/memory/reuse/dependency:** O(number of objects) validation; compact integer encodings plus immutable lookup tables. Reuse current compact label ideas. No new dependency.
- **Oracle/validation/benchmark:** Hand fixtures for paired, multiregion, TMA, repeated measures and invalid cycles; round-trip manifest tests; real-data validation on at least one paired and one multiregion cohort; benchmark 10 million cells with 1 million regions/objects only if realistic, otherwise 1 million cells and 10,000 specimens.
- **API/schema/docs/priority:** Public only after stable. Add an artifact manifest and result-format 0.4 design summary. P0.
- **Promotion/kill:** Promote when all downstream randomization APIs consume typed units. Kill any attempt to preserve ambiguous fallback inference from `case_id` strings.

### FND-02 — `ObservationWindow2D`

- **Scientific question/current support:** What exact tissue region permits events? Current code has masks, area and effective length, but no general estimand-defining polygon/multipolygon/hole contract.
- **Canonical definition/input/window/estimand:** A finite, oriented polygon or multipolygon in micrometres, with non-self-intersecting rings, explicit holes, coordinate frame, area, perimeter, connected components, containment, signed/unsigned boundary distance and optional compartment partition. The window itself is the estimand domain.
- **Null/alternatives/edge/inhomogeneity/randomization:** Not applicable directly; it enables all point-process nulls and edge corrections.
- **Assumptions/failure modes/status/replication:** Coordinates and window share a declared frame/unit. Reject self-intersection, zero area, non-finite coordinates, cells outside the permitted window beyond tolerance, overlapping components with ambiguous semantics, and raster-only masks with unknown pixel-to-micrometre transform. Descriptive/domain prerequisite.
- **Result/undefined states:** `WindowDescriptor` stores area, perimeter, component/hole counts, frame and digest; geometry is an artifact. Typed states include `InvalidTopology`, `UnknownCoordinateFrame`, `OutsideWindow`, `BoundaryDistanceUnavailable`.
- **Complexity/memory/reuse/dependency:** O(v log v) validation for v vertices; indexed boundary queries. Evaluate adding a maintained geometry crate only after license/MSRV/build-cost review; do not implement a general polygon kernel casually.
- **Oracle/validation/benchmark:** Rectangles, concave polygons, donuts, disconnected components, near-boundary points and pathological rings; independent area/distance fixtures from GEOS/shapely or `spatstat`; real masks with holes/fragmentation; benchmark one million containment and boundary-distance queries.
- **API/schema/docs/priority:** Add under `geom`, retain `TumorWindow` as a summary compatibility view. P0.
- **Promotion/kill:** Promote after differential geometry tests across platforms. Kill or quarantine geometry that requires silent repair of invalid rings.

### FND-03 — Reusable exact spatial geometry plan

- **Scientific question/current support:** Can all endpoints reuse deterministic spatial queries, pair ordering, distance bins and edge metadata? Current code has a strong `SpatialIndex2D` and several endpoint-specific plans.
- **Canonical definition/input/window/estimand:** `SpatialGeometryPlan2D` owns point coordinates, stable row-to-CellId mapping, one exact index, canonical unordered/directed pair iteration, distance-bin assignment, boundary distances and optional weight caches. It does not own a statistic.
- **Null/alternatives/edge/inhomogeneity/randomization:** No null itself; observed and null evaluators must reuse it. Edge correction data are typed by correction kind. Inhomogeneity values remain external inputs.
- **Assumptions/failure modes/status/replication:** Finite 2-D coordinates; duplicate-coordinate policy explicit. Reject over-budget plans before partial construction. Domain infrastructure.
- **Result/undefined states:** Telemetry includes exact/approx mode, maximum radius, pair count, storage bytes and digest; no raw pairs in JSON. States: `BudgetExceeded`, `DuplicateCoordinatePolicyRequired`, `RadiusExceedsWindowSupport`, `ApproximationNotPermitted`.
- **Complexity/memory/reuse/dependency:** Index O(n log n); pair plan O(n log n + m(r)); memory O(n + m(r)) when pairs retained, with streaming visitor alternative. Reuse `rstar`, current seed/performance systems.
- **Oracle/validation/benchmark:** Brute-force differential tests for radius/kNN/pairs/ties/bins; adversarial duplicates, clusters, sparse grids and boundary-heavy masks; benchmarks to 1 million cells at fixed density and increasing radius.
- **API/schema/docs/priority:** Internal stable owner first; public only as opaque plan metadata. P0.
- **Promotion/kill:** Promote when K/g/variogram/mark functions share one canonical pair traversal. Kill any endpoint-specific all-pairs rebuild or unbounded retained-pair matrix.

### FND-04 — Typed marks and measurement provenance

- **Scientific question/current support:** What exactly is attached to each location, and is it measured or predicted? Current public pattern supports a binary mark and optional probability.
- **Canonical definition/input/window/estimand:** `MarkTable` keyed by `CellId` with columns typed as `Binary`, `Categorical`, `Ordinal`, `Continuous`, `Probability`, `ProbabilitySimplex`, or `VectorArtifactRef`. Each column has `MeasurementStatus::{Measured, ImportedPrediction, MorphologyPrediction, DerivedSummary}`, modality, unit, assay/model provenance and missingness policy.
- **Null/alternatives/edge/inhomogeneity/randomization:** Not a statistic. It constrains valid downstream randomization—for example, probability-simplex rows can be permuted as rows, not coordinate-wise.
- **Assumptions/failure modes/status/replication:** One semantic definition per column; no mixed measured/predicted values without explicit strata. Reject non-finite values, invalid probabilities, inconsistent ordinal levels, unit conflicts and undocumented thresholding.
- **Result/undefined states:** Results cite mark IDs and provenance digests. Typed states include `MissingNotPermitted`, `InvalidProbability`, `UnitMismatch`, `MixedMeasurementStatus`, `ThresholdProvenanceMissing`.
- **Complexity/memory/reuse/dependency:** O(n × columns); dense columns contiguous, sparse categorical encodings compact. Reuse finite validation and Arrow/Parquet.
- **Oracle/validation/benchmark:** Round-trip all types, missingness, thresholds and provenance; real IHC continuous/ordinal/binary examples; benchmark one million rows with 50 scalar marks.
- **API/schema/docs/priority:** New artifact/schema family; current binary `Pattern` remains supported through an adapter. P0.
- **Promotion/kill:** Promote when every public result names its input mark and measurement status. Kill generic untyped `value: f64` APIs.

### FND-05 — Cell/patch embedding artifact contracts

- **Scientific question/current support:** Can high-dimensional morphology assets be used without duplication or provenance loss? Current CellViT adapter imports class labels only.
- **Canonical definition/input/window/estimand:** `CellEmbeddingTable` and `PatchEmbeddingTable` contain stable IDs plus checked row-major contiguous `f32` matrices, fixed dimension, validity/missing bitmap and `EmbeddingProvenance`. `CellPatchLink` records cell, patch, physical scale, containment/interpolation, overlap semantics and optional weight. No scientific estimand.
- **Null/alternatives/edge/inhomogeneity/randomization:** Not applicable. Downstream tests define nulls. Overlapping patches and shared patch assignments are explicit clusters, not independent replicates.
- **Assumptions/failure modes/status/replication:** Stable IDs, one model/checkpoint/layer/pooling contract per table. Reject unknown dimension, non-finite values, duplicate IDs, checksum mismatch, pixels-only scale, vector repetition per cell and unknown missingness.
- **Result/undefined states:** JSON stores `ArtifactRef`, shape, dtype, provenance digest and QC summary. Matrix remains Arrow/Parquet. States: `ProvenanceIncomplete`, `ChecksumMismatch`, `DimensionMismatch`, `MissingVector`, `AmbiguousCellPatchLink`.
- **Complexity/memory/reuse/dependency:** O(nd) validation, O(nd) storage; 1 million × 256 f32 values = 1.024 GB before metadata. Accumulate statistics in f64. Use existing Arrow/Parquet features; evaluate memory mapping separately.
- **Oracle/validation/benchmark:** Bitwise/digest round trips, sliced/streamed reads, missing rows, 1 million × 256 scheduled load/scan benchmark, and a CellViT asset reconciliation report.
- **API/schema/docs/priority:** Stable artifact API after provenance is available; no model inference. P0.
- **Promotion/kill:** Promote after the real asset’s exact provenance and CellId mapping are supplied. Kill stable scientific claims if checkpoint/layer/preprocessing remain unknown.

### FND-06 — Inference design and permitted randomization units

- **Scientific question/current support:** Which units are exchangeable under which null? Current support covers fixed-position label permutations within one pattern and strata.
- **Canonical definition/input/window/estimand:** `InferenceDesign` declares analysis level, null family, conditioning variables, permutation unit, blocking/pairing, cluster hierarchy, number of replicates, seed and multiplicity family.
- **Null/alternatives/edge/inhomogeneity/randomization:** Enumerate `CSR`, `RandomLabeling`, `StratifiedRandomLabeling`, `PopulationIndependence`, `PatientLabelPermutation`, `PairedSignFlip`, `HierarchicalBootstrap`, and explicitly limited shift nulls. Every method declares allowed designs.
- **Assumptions/failure modes/status/replication:** Exchangeability is an input assertion checked against hierarchy. Reject cell-level permutation for patient-level exposure/outcome, broken pairs, one patient per group, homogeneous strata with no permutations, and post-hoc unit switching.
- **Result/undefined states:** Persist the full design summary, eligible units, rejected replicates and exact seed namespace. States: `NoExchangeableUnits`, `InsufficientBiologicalReplicates`, `BrokenPair`, `DegenerateNull`, `UnsupportedDesign`.
- **Complexity/memory/reuse/dependency:** O(B × statistic evaluation), with geometry reuse; hierarchy index O(number of objects). Reuse current scalar p-values, ERL and deterministic shuffle.
- **Oracle/validation/benchmark:** Hand-enumerated small permutations, thread determinism, blocked/paired fixtures, type-I simulation under null and failure under deliberately invalid pseudoreplication.
- **API/schema/docs/priority:** Stable cross-cutting API. P0.
- **Promotion/kill:** Promote when no inferential endpoint internally chooses a randomization unit. Kill any automatic fallback from patient to cell permutation.

### FND-07 — Scientific provenance, schema evolution and validation ledger

- **Scientific question/current support:** Can a result be independently reproduced and its claim audited? Result 0.3 is strict but provenance is only program/crate version.
- **Canonical definition/input/window/estimand:** `RunProvenance` records crate version, Git SHA, toolchain, feature set, config digest, input/artifact digests, coordinate frame, exact/approx mode, seed/thread policy, external tool/model provenance and result-schema version.
- **Null/alternatives/edge/inhomogeneity/randomization:** Not applicable; records each chosen contract.
- **Assumptions/failure modes/status/replication:** Inputs are immutable/digestible. Reject stable output if required provenance is absent. Infrastructure, not analysis.
- **Result/undefined states:** Add `ArtifactRef` and typed `ProvenanceAvailability`; preserve 0.3 reader. Large artifacts remain separate.
- **Complexity/memory/reuse/dependency:** O(total artifact bytes) for initial cryptographic hashing, streamable; use a reviewed SHA-256 crate only after dependency assessment.
- **Oracle/validation/benchmark:** Golden digests, schema round trips, unknown-field rejection, migration rejection when semantics are unrecoverable, cross-platform deterministic metadata; benchmark hashing and manifest construction on multi-GB artifacts.
- **API/schema/docs/priority:** Result-format 0.4 prerequisite. P0.
- **Promotion/kill:** Stable release blocked without complete provenance. Kill any migration that invents unavailable scientific fields.

## 5.3 Twelve highest-priority method cards

### PP-01 — Homogeneous Ripley K and L

- **Question/current support/canonical definition:** Does an unmarked or type-filtered pattern show excess or deficit of neighbors within radius \(r\) relative to homogeneous Poisson intensity? Absent. Estimate \(K(r)=\lambda^{-1} E[N_o(r)]\); report \(L(r)=\sqrt{K(r)/\pi}\) and optionally \(L(r)-r\).
- **Input/window/estimand:** `CellId`, coordinates, `ObservationWindow2D`, optional type subset, radius axis. Estimand is the window-process second-order cumulative interaction function.
- **Null/alternatives:** CSR for location-process inference; attraction \(K>\pi r^2\), inhibition \(K<\pi r^2\). For marks at fixed locations, use mark-specific functions instead.
- **Edge/inhomogeneity/randomization:** First stable correction = border/reduced-sample. Translation/isotropic are distinct named modes added later. Homogeneous only; reject/flag strong intensity gradients. CSR simulations preserve the exact window; within-cohort tests resample patients, not cells.
- **Assumptions/failure modes/status/replication:** Stationary homogeneous interpretation within window; fail on missing window, unsupported radius, too few interior points, zero area/intensity, duplicate policy unresolved. Descriptive per specimen; within-pattern inference under CSR; population claims require patients.
- **Result fields/undefined states:** Correction, radius, K, L, L-minus-r, eligible point/pair counts, CSR expectation, global envelope, p-global, null/design, window/provenance. Typed unavailable bins/reasons.
- **Complexity/memory/reuse/dependency:** O(n log n + m(rmax)); streaming cumulative histogram O(k) memory after index, or retained pair plan under budget. Reuse FND-03 and ERL.
- **Oracle/simulation/real data/benchmark:** Pinned `spatstat::Kest` fixtures for rectangles, holes and disconnected windows; Poisson, Thomas, Matérn and Strauss simulations; pathology masks with known cell classes; 10k–1M cells at fixed density and increasing r.
- **API/schema/docs/priority:** New `PointProcessResult` in 0.4; config names exact correction. P1.
- **Promotion/kill:** Promote after ≤ documented tolerance to oracle, calibrated CSR error and coverage. Kill any “K” implementation lacking a real window or correction.

### PP-02 — Inhomogeneous K and L

- **Question/current support/canonical definition:** Does second-order interaction remain after varying intensity? Absent. Estimate intensity-reweighted K using pair weights \(1/(\hat\lambda(x_i)\hat\lambda(x_j))\) with a named edge correction.
- **Input/window/estimand:** Point pattern, window, intensity field or estimator artifact, radius axis. Estimand is second-order intensity-reweighted stationarity.
- **Null/alternatives:** Inhomogeneous Poisson null conditional on estimated/prespecified intensity; excess/deficit after reweighting.
- **Edge/inhomogeneity/randomization:** Border first, then translation/isotropic. Intensity estimator, bandwidth, boundary correction, leave-one-out/cross-fit policy and compartments must be persisted. Simulations draw from fixed intensity artifact; patient-level group inference remains patient-level.
- **Assumptions/failure modes/status/replication:** Positive finite intensity, estimator not circularly tuned to final signal, adequate support. Fail on near-zero intensity, extrapolation, bandwidth selected on test data, and intensity estimated from the same rare type without bias handling.
- **Result/undefined states:** Kinhom/Linhom curves, intensity provenance/QC, min/max/quantiles, effective pair count, correction/null/envelope. States: `IntensityUnavailable`, `NearZeroIntensity`, `ExtrapolatedIntensity`, `BandwidthUnspecified`.
- **Complexity/memory/reuse/dependency:** Same pair complexity as PP-01 plus intensity evaluation; contiguous f64 weights. Reuse geometry.
- **Oracle/simulation/real/benchmark:** `spatstat::Kinhom/Linhom`; inhomogeneous Poisson, compartment-confounded and boundary-heavy simulations; real compartment-gradient tissue; benchmark intensity grid/kd evaluation plus pair pass.
- **API/schema/docs/priority:** Stable only after PP-01. P1.
- **Promotion/kill:** Promote on null calibration across multiple intensity estimators. Kill a one-click “corrected for density” mode with hidden bandwidth/defaults.

### PP-03 — Classical pair-correlation, cross-K and cross-pair-correlation

- **Question/current support/canonical definition:** At which distances do points or types attract/repel? Current mark covariance/raw pair counts are not equivalent. Implement kernel-smoothed \(g(r)\), cumulative cross-K and derivative cross-\(g\) with explicit ordered/unordered type semantics.
- **Input/window/estimand:** Windowed unmarked or multitype pattern, type columns, radius grid, kernel/bandwidth. Estimands are density-normalized second-order functions.
- **Null/alternatives:** CSR, random labeling, or population independence depending question. Two-sided distance-varying alternatives.
- **Edge/inhomogeneity/randomization:** Named border/translation/isotropic correction; inhomogeneous variants only after PP-02. Label permutations preserve locations and type counts; population independence simulates components.
- **Assumptions/failure modes/status/replication:** Adequate type counts/pairs and bandwidth support. Fail/suppress sparse type pairs, zero intensity, unsupported near-zero radii, empty bins and ill-defined bandwidth.
- **Result/undefined states:** g/K curves, pair counts/effective weights, bandwidth/kernel, type order, correction, eligible radii, ERL envelope and p-global. Empty geometry is unavailable, not zero.
- **Complexity/memory/reuse/dependency:** O(n log n + m(rmax)); kernel accumulation O(m × nearby grid support), not O(mk) if compact support is used.
- **Oracle/simulation/real/benchmark:** `spatstat::pcf`, `Kcross`, `pcfcross`; multitype attraction/repulsion, rare phenotypes, inhomogeneous confounding; immune–tumor examples; benchmark type-pair explosion and sparse labels.
- **API/schema/docs/priority:** Separate result family from current `CrossInteractionCurve`. P1.
- **Promotion/kill:** Promote after normalization/edge behavior matches oracle. Kill any aliasing of current pair counts or covariance to g.

### PP-04 — F, G, J and nearest-neighbor distributions

- **Question/current support/canonical definition:** Are events regularly spaced, clustered, or leaving unusual empty space? Current mean NN is insufficient. G is event-to-nearest-event CDF; F is random-location-to-nearest-event CDF; \(J=(1-G)/(1-F)\) where defined.
- **Input/window/estimand:** Window, points, event types, deterministic probe plan for F. Estimands are empty-space and nearest-neighbor distributions.
- **Null/alternatives:** CSR or type-specific/random-labeling analogues; clustering tends to lower J, inhibition raises it, subject to interpretation limits.
- **Edge/inhomogeneity/randomization:** Kaplan–Meier/border-style corrections must be named. Inhomogeneous variants are deferred. Probe locations are generated deterministically or with recorded Monte Carlo error.
- **Assumptions/failure modes/status/replication:** Valid window and sufficient probes/events. J unavailable when \(1-F\) is too small; duplicate points affect G and require policy.
- **Result/undefined states:** F/G/J curves, probe count/method, censoring/edge correction, Monte Carlo SE for sampled F, eligible radii, envelopes.
- **Complexity/memory/reuse/dependency:** G O(n log n); F O(q log n) for q probes; J O(k). Reuse index/window.
- **Oracle/simulation/real/benchmark:** `spatstat::Fest/Gest/Jest`; Poisson/cluster/inhibition, holes/boundaries; gland/immune spacing examples; benchmark exact grid versus sampled probes.
- **API/schema/docs/priority:** P2 after K/L/g.
- **Promotion/kill:** Promote when probe error and undefined J states are explicit. Kill deterministic-looking F curves generated from an undocumented random sample.

### MRK-01 — Mark connection, mark correlation and mark-weighted K

- **Question/current support/canonical definition:** Conditional on event locations, are labels or values associated between points at distance r? Current binary centered covariance is one narrow member. Implement separately: categorical mark-connection probabilities; normalized continuous mark-correlation; mark-weighted K with specified weight function.
- **Input/window/estimand:** Windowed points plus typed mark column(s), radius/kernel axis. Estimand depends on named family and normalization.
- **Null/alternatives:** Random labeling, compartment-stratified random labeling, or covariate-conditional labeling. Alternatives are distance-dependent positive/negative association or label pair enrichment/depletion.
- **Edge/inhomogeneity/randomization:** Pair edge correction as PP-03. Locations fixed for label null. Probabilistic labels are evaluated by expected contributions or sampled in a separately named uncertainty mode.
- **Assumptions/failure modes/status/replication:** Mark exchangeability under declared strata, finite moments, sufficient label counts. Fail on mixed measured/predicted semantics or rare labels.
- **Result/undefined states:** Family/canonical formula, normalization, label/value summaries, curve, counts/effective sample size, edge correction, null/envelope, measurement status.
- **Complexity/memory/reuse/dependency:** Pair-plan evaluation O(m); B nulls O(Bm) with no geometry rebuild; vectorized compact mark encodings.
- **Oracle/simulation/real/benchmark:** `spatstat` mark-connection/mark-correlation/`Kmark`; random labeling, attraction/repulsion, continuous correlated marks, probabilities; IHC intensity and cell-type examples.
- **API/schema/docs/priority:** P1.
- **Promotion/kill:** Promote each family independently. Kill a generic “mark interaction” endpoint that merges formulas or normalizations.

### SIG-01 — Spatial autocorrelation and variogram suite

- **Question/current support/canonical definition:** Are scalar values spatially smooth, contrasting or locally clustered? Absent publicly. Implement global Moran I, Geary C and semivariance \(\gamma(r)=\frac12 E[(Z(x)-Z(x+r))^2]\); local/bivariate endpoints are later subfamilies.
- **Input/window/estimand:** Typed continuous/ordinal/probabilistic mark, a frozen `SpatialWeights` or distance-bin plan, optional covariates/residual artifact.
- **Null/alternatives:** Random labeling/permutation of values within allowed strata; high/low/global two-sided alternatives. Cohort comparisons operate on specimen summaries.
- **Edge/inhomogeneity/randomization:** Weight normalization, self-neighbor, symmetry, disconnected nodes and physical scale are explicit. Spatial trend can be residualized only with externally fitted/cross-fitted artifacts. Pair variograms use window edge corrections where required.
- **Assumptions/failure modes/status/replication:** Nonzero variance, finite values, fixed weights. Fail on constant marks, isolated-node policy missing, weights selected on test outcomes, or local maps without multiplicity family.
- **Result/undefined states:** Statistic/curve, expectation/variance if used, weights digest, lag bins, pair counts, null/envelope/p/q, local artifact ref. Typed `ZeroVariance`, `DisconnectedWeights`, `InsufficientPairs`.
- **Complexity/memory/reuse/dependency:** O(edges) for graph statistics; O(m) for variogram; B nulls reuse edges/pairs.
- **Oracle/simulation/real/benchmark:** Hand matrices plus PySAL/R oracle; Gaussian fields, gradients, compartment confounding, continuous IHC; benchmark 1M nodes with bounded-degree graphs and pair bins.
- **API/schema/docs/priority:** Global Moran/Geary and scalar variogram P1; local/bivariate P2.
- **Promotion/kill:** Local stable release requires family-wise/FDR policy and patient-level validation. Kill implicit row-standardization or hidden weights.

### EMB-01 — Rotation-invariant spatial dependence of embeddings

- **Question/current support/canonical definition:** Are morphology vectors more similar at nearby locations than expected, independent of embedding coordinate rotation? No current vectors. Primary endpoints: distance-dependent squared Euclidean/cosine/kernel similarity, vector semivariogram, and a kernel omnibus global test.
- **Input/window/estimand:** `CellEmbeddingTable`, geometry plan, optional training-fold transformation artifact. Estimand is expected pairwise similarity or dispersion by physical distance; omnibus statistic aggregates the curve.
- **Null/alternatives:** Random spatial labeling of complete vectors within declared compartments/batches; paired/group differences permute patients. Alternative is distance-dependent spatial structure.
- **Edge/inhomogeneity/randomization:** Edge-correct pair contributions; no coordinate-wise testing as primary analysis. Center/whiten/PCA only from training folds. Technical covariates handled by prespecified strata or external residualization.
- **Assumptions/failure modes/status/replication:** Provenance complete; vectors finite; similarity metric prespecified. Fail on unknown checkpoint, mixed model versions, duplicated vectors per shared patch, cell-level patient prediction split, or post-hoc component selection.
- **Result/undefined states:** Metric/kernel, dimension, transformation provenance, curve/counts, global envelope, p-global, technical-confounder diagnostics, artifact digest.
- **Complexity/memory/reuse/dependency:** Exact O(m d); blocked SIMD-friendly accumulation, f64 sums; optional approximate pair sampling is experimental and reports CI/error against exact subsets.
- **Oracle/simulation/real/benchmark:** Hand low-dimensional rotations; correlated Gaussian vectors, technical batch shifts and null rotations; real CellViT asset plus scanner/site controls; 1M × 256 benchmark with bounded pair radius.
- **API/schema/docs/priority:** P1 after FND-05; experimental until real provenance exists.
- **Promotion/kill:** Promote on rotation invariance, null calibration and cross-site sensitivity report. Kill per-dimension multiplicity explosion as the default endpoint.

### GEO-01 — Compartments, interfaces and infiltration

- **Question/current support/canonical definition:** How do phenotypes/marks relate to tumor–stroma, tumor–immune or other pathology boundaries? Absent. Define oriented compartment boundaries, signed distance, infiltration depth distributions, contact fractions and boundary-local density.
- **Input/window/estimand:** Observation window, non-overlapping or explicitly overlapping compartment polygons/masks, cell points/marks, boundary orientation and uncertainty artifact. Estimands are direct geometric quantities.
- **Null/alternatives:** Descriptive by default. Random labeling tests phenotype enrichment at fixed positions; conditional location nulls preserve compartment intensity; patient-level comparisons use patients.
- **Edge/inhomogeneity/randomization:** Window and compartment edges distinct. Inhomogeneity policy conditions on compartments or models intensity externally; do not treat the boundary itself as nuisance.
- **Assumptions/failure modes/status/replication:** Boundary has biological meaning and common coordinate frame. Fail on unknown orientation, invalid overlaps, registration uncertainty larger than analysis band, or cells outside all compartments.
- **Result/undefined states:** Signed-distance curves/quantiles, penetration depth, contact length/fraction, band densities, uncertainty sensitivity, null/p/q and geometry digest.
- **Complexity/memory/reuse/dependency:** Boundary index O(v log v), cell queries O(n log v); band plans reused.
- **Oracle/simulation/real/benchmark:** Rectangles/circles/concave boundaries, known infiltration gradients, registration noise; annotated tumor–stroma cohorts; one million cells and million-segment boundaries.
- **API/schema/docs/priority:** P1.
- **Promotion/kill:** Promote direct quantities first. Kill “invasive front” labels without independently supplied front geometry.

### NIC-01 — Multiscale neighborhood composition and reproducible niches

- **Question/current support/canonical definition:** What cell-type/mark composition surrounds each cell across physical scales, and are derived niches reproducible? Current profiles are territory-centered and MMR-specific. Define per-cell soft composition vectors at prespecified radii or kernels.
- **Input/window/estimand:** Cell types/probabilities, coordinates, scales in micrometres, optional compartment restrictions. Estimand is local composition, not a discovered biological domain.
- **Null/alternatives:** Random labeling/stratified labeling for composition association; cohort niche abundance comparisons at patient level. Discovery algorithms are external or experimental.
- **Edge/inhomogeneity/randomization:** Denominators account for available window/compartment support. Never assume one radius. Scale selection is prespecified or nested within training.
- **Assumptions/failure modes/status/replication:** Reliable cell typing/probabilities, enough neighbors, stable physical scale. Fail on zero support, dominant boundary truncation, model/site shifts or unstable clustering.
- **Result/undefined states:** Composition artifact, scale/support metadata, entropy/diversity summaries, imported niche assignment provenance, stability metrics and patient-level abundance results.
- **Complexity/memory/reuse/dependency:** O(n log n + returned memberships); store dense composition only when category count is bounded, otherwise Arrow sparse/struct artifact.
- **Oracle/simulation/real/benchmark:** Hand neighborhoods, mixtures, overlapping niches, scale shifts, rare types; external BANKSY/UTAG/SPIAT comparison; cross-patient mapping; million-cell multiscale benchmark.
- **API/schema/docs/priority:** Composition P1; discovery P2 external.
- **Promotion/kill:** Promote niche labels only after cross-patient stability and held-out mapping. Kill a single universal-radius default.

### COH-01 — Patient-level permutation and hierarchical bootstrap

- **Question/current support/canonical definition:** Do spatial endpoints differ across groups/timepoints while respecting nesting? Absent. Provide design-aware patient permutation, paired sign-flip/permutation, cluster bootstrap and hierarchical bootstrap over declared levels.
- **Input/window/estimand:** Per-specimen endpoint/fingerprint plus `CohortHierarchy`, group/time covariates, pairing and prespecified summary. Estimand is population-level difference/change.
- **Null/alternatives:** Exchangeable patient labels, paired zero change, or bootstrap sampling distribution. Cells/patches never cross patient units.
- **Edge/inhomogeneity/randomization:** Spatial edge handling occurs inside specimen endpoint; cohort layer resamples complete units/artifacts. Technical/clinical covariates require restricted permutation or external models.
- **Assumptions/failure modes/status/replication:** Correct hierarchy and exchangeability; enough clusters. Fail on one patient/group, broken pairs, site perfectly confounded with group, or patient labels reconstructed from filenames.
- **Result/undefined states:** Design, unit counts, statistic/effect, CI, p-value, rejected replicates, hierarchy digest and endpoint provenance. `InsufficientPatients`, `ConfoundedDesign`, `BrokenPair`.
- **Complexity/memory/reuse/dependency:** O(B × number of endpoint vectors), not O(B × cells) when specimen summaries are fixed; optional nested recomputation only when required.
- **Oracle/simulation/real/benchmark:** Enumerated small designs, null cohorts with variable cells/specimen, paired/multiregion/multisite simulations; real paired cohort and independent cohort; 100k endpoints/10k patients stress.
- **API/schema/docs/priority:** P1, foundational for every population claim.
- **Promotion/kill:** Promote after type-I error under unequal cluster sizes. Kill automatic cell-level fallback or unreported dropped patients.

### CMP-01 — SpatialFingerprint and patient-level two-sample testing

- **Question/current support/canonical definition:** How do specimens differ descriptively, and do groups differ in a population? Current comparison is ad hoc pooled-bin shuffling. `SpatialFingerprint` is a versioned vector/curve bundle of prespecified endpoints and uncertainty; tests include functional envelopes, MMD and energy distance at the patient/specimen level.
- **Input/window/estimand:** Compatible per-specimen results with identical endpoint definitions/axes or explicit harmonization. Estimand may be descriptive distance or population distribution difference; these are separate result kinds.
- **Null/alternatives:** Patient-label permutation for group difference; paired permutation for matched samples. Similarity ranking/retrieval has no p-value by default.
- **Edge/inhomogeneity/randomization:** Inherited from each component. No permutation of curve bins. Kernel/distance and feature scaling are prespecified or fitted in training folds.
- **Assumptions/failure modes/status/replication:** Comparable acquisition/endpoint contracts and enough patients. Fail on incompatible axes, hidden missing endpoints, group/site confounding or test-set feature selection.
- **Result/undefined states:** Fingerprint schema/digest, components, uncertainty, descriptive distances, test method/statistic/effect/p, permutation units and multiplicity. Typed compatibility failures.
- **Complexity/memory/reuse/dependency:** O(Sq) fingerprint build for q summaries; pairwise comparison O(S²q), with retrieval index experimental.
- **Oracle/simulation/real/benchmark:** Hand fingerprints, MMD/energy reference libraries, null/shifted patient cohorts, varying cell counts; real external cohort; benchmark 10k specimens × 1k features.
- **API/schema/docs/priority:** P1.
- **Promotion/kill:** Promote only prespecified fingerprint versions. Kill a single opaque “similarity score” or cell/bin permutations.

### EQV-01 — Genuine equivalence and noninferiority

- **Question/current support/canonical definition:** Are prespecified population endpoints equivalent within scientifically justified margins? Current margins are descriptive only. Implement TOST or an equivalent confidence-interval decision: reject both non-equivalence nulls for lower and upper margins.
- **Input/window/estimand:** Patient-level scalar or low-dimensional prespecified endpoint, paired/unpaired design, lower/upper margins and confidence level. Functional equivalence remains later research.
- **Null/alternatives:** \(H_{01}:\Delta\le -\delta_L\), \(H_{02}:\Delta\ge\delta_U\); alternative lies inside margins. Noninferiority is one-sided.
- **Edge/inhomogeneity/randomization:** Specimen spatial estimator already fixed. Inference unit is patient; margins cannot be estimated from the final test cohort.
- **Assumptions/failure modes/status/replication:** Margin has clinical/scientific rationale, design supports effect estimate/SE, adequate patients. Fail on no margin provenance, underpowered design, only one specimen/group, post-hoc endpoint selection or incompatible measurement.
- **Result/undefined states:** Effect/CI, margins/rationale reference, two one-sided p-values, decision, design and power/sensitivity. “Not demonstrated” is not “different.”
- **Complexity/memory/reuse/dependency:** O(P) for scalar endpoints; bootstrap/permutation O(BP). Reuse cohort design.
- **Oracle/simulation/real/benchmark:** Standard TOST fixtures, paired/unpaired simulations for type-I/power/coverage, external statistical package comparison; real repeated-assay or method-comparison cohort.
- **API/schema/docs/priority:** P1 after COH/CMP.
- **Promotion/kill:** Promote only with prespecified margins and power guidance. Kill “equivalent” output when a difference test is merely non-significant.

## 5.4 Three frontier method cards

### FR-01 — Graph Fourier, heat-kernel and spectral graph-wavelet summaries

- **Question/current support/canonical definition:** Which graph scales carry mark/embedding variation on irregular cellular geometry? Current graph is unweighted and has no transform. Define one graph construction, symmetric weight matrix, Laplacian variant, eigen/approximation policy, heat kernel and spectral graph-wavelet filters.
- **Input/window/estimand:** Stable `SpatialWeights`, scalar/vector signal, physical graph scale. Estimands are graph spectral energy, heat diffusion summaries or wavelet coefficient distributions—not generic “multiscale biology.”
- **Null/alternatives:** Random labeling/stratified labeling; group differences at patient level. Alternatives are excess energy/smoothness at named graph scales.
- **Edge/inhomogeneity/randomization:** Boundary enters graph construction and support; graph/weights fixed across nulls. No learned graph on final test labels.
- **Assumptions/failure modes/status/replication:** Graph connectedness/components, degree distribution and normalization explicit. Fail on graph instability, eigen multiplicity ambiguity for directional claims, scale without physical interpretation, or sensitivity to minor topology perturbation.
- **Result/undefined states:** Graph contract digest, Laplacian, filter family/parameters, approximation error, energy/curve/envelope and stability sensitivity.
- **Complexity/memory/reuse/dependency:** Exact eigendecomposition O(n³) is prohibited at pathology scale. Use sparse polynomial/Chebyshev operators O(K|E|) with quantified approximation; small exact oracle.
- **Oracle/simulation/real/benchmark:** SGWT/diffusion-wavelet hand graphs and reference toolbox; lattices, rings, disconnected components, clustered tissues; real interface/niche examples; 1M-node bounded-degree benchmark.
- **API/schema/docs/priority:** Experimental only. P3.
- **Promotion/kill:** Promote a specific summary only after exact small-graph agreement, approximation bounds and replicated pathology value. Kill a broad transform framework without an immediate endpoint.

### FR-02 — Joint cell and multiscale patch embedding statistics

- **Question/current support/canonical definition:** Do patch representations add spatial and predictive information beyond CellViT cell vectors? No patch data. Use separate patch tables and links; start with late-fusion linear probes and distance-dependent cross-level covariance, not cross-attention.
- **Input/window/estimand:** Cell/patch embeddings, physical patch scales, overlap/link weights, patient hierarchy, optional measured IHC/molecular targets. Estimands are incremental predictive performance and cross-level spatial association.
- **Null/alternatives:** Cell-to-patch link permutation respecting overlap/containment for correspondence questions; patient-label permutation for group effects; nested patient-held-out model comparison M0–M5 for prediction.
- **Edge/inhomogeneity/randomization:** Shared patches/overlap define clusters and effective sample size. All preprocessing/tuning inside training folds. Window/compartment effects controlled explicitly.
- **Assumptions/failure modes/status/replication:** Complete provenance, no adjacent-section/patient/site leakage, measured targets distinguished from predictions. Fail on duplicated patch vectors per cell, within-slide primary splits, mixed scanners with no test, or no incremental value over cell-only/covariates.
- **Result/undefined states:** Model comparison, patient-level metrics/calibration/OOD, spatial cross-covariance, patch-scale provenance, effective independent patch count and abstention.
- **Complexity/memory/reuse/dependency:** External training; native artifacts O(nd + qd_p + links). Statistical passes block/stream matrices.
- **Oracle/simulation/real/benchmark:** Synthetic shared-patch dependence, null/incremental signals, external-site datasets; compare M0–M5; memory benchmark for one million cells and overlapping multiscale patches.
- **API/schema/docs/priority:** Artifact pieces stable; joint models experimental/external. P3.
- **Promotion/kill:** Promote only if patch level adds externally validated patient-held-out value. Kill complex fusion if linear/late fusion is equal or provenance is incomplete.

### FR-03 — Partial/unbalanced/fused transport for atlas mapping and retrieval

- **Question/current support/canonical definition:** Can tissues with missing types or altered anatomy be aligned descriptively or queried for analogous regions? Absent. Use external partial/unbalanced OT or fused Gromov-Wasserstein with explicit feature/geometric costs and mass penalties.
- **Input/window/estimand:** Two typed fingerprints or cell/region feature sets, geometry, masses, cost definitions and external solver manifest. Estimand is minimum transport objective/plan under that model—not true correspondence.
- **Null/alternatives:** Descriptive by default. Group inference permutes patients and recomputes prespecified specimen-level transport summaries. Paired change uses paired units.
- **Edge/inhomogeneity/randomization:** Physical registration and biological similarity are separate inputs. Compartments and missing mass explicit.
- **Assumptions/failure modes/status/replication:** Cost scales and regularization prespecified; plan non-identifiability assessed. Fail on unscaled heterogeneous features, numerical non-convergence, one plausible plan presented as truth or hidden mass deletion.
- **Result/undefined states:** Objective components, solver/version/license, convergence, regularization, marginal errors, uncertainty/sensitivity, plan artifact reference and interpretation class `descriptive_alignment`.
- **Complexity/memory/reuse/dependency:** External solver; dense O(nm) cost prohibited for large cell sets without region aggregation/sparsity. Marklab validates artifacts and can run small exact fixtures.
- **Oracle/simulation/real/benchmark:** Small analytically solvable transports, partial-overlap and missing-type simulations, PASTE2/STalign-style comparisons, cross-platform atlas data; benchmark region-level and sparse-cell plans.
- **API/schema/docs/priority:** Experimental interoperability. P3.
- **Promotion/kill:** Promote only descriptive summaries with stability across cost/regularization and external cohorts. Kill any field named `cell_correspondence` absent ground truth/probability calibration.




## 5.5 Platform prerequisite cards

### PLAT-01 — Marklab Project and workflow operating system

- **Scientific question:** How can an analysis involving multiple modalities, methods, backends, scales, and cohorts remain reproducible and scientifically interpretable?
- **Current support:** Independent CLI commands and result directories; no unified project or workflow DAG.
- **Canonical definition:** A `MarklabProject` owns immutable input references, identities, coordinate frames, modality declarations, workflow graph, backend manifests, artifacts, results, validation state, and provenance.
- **Input:** Project manifest plus local/remote artifacts.
- **Estimand/status:** Infrastructure, not a scientific estimand.
- **Failure states:** Missing artifact, digest mismatch, cyclic workflow, incompatible coordinate frames, unresolved backend, stale result, invalid maturity transition.
- **Complexity:** Metadata operations linear in nodes/artifacts; scheduler complexity depends on workflow.
- **Persistence:** Human-readable manifest plus append-only execution ledger and content-addressed artifacts.
- **Validation:** Deterministic replay, interrupted-run recovery, artifact tamper detection, backend failure recovery, DAG cycle/invalid-edge tests.
- **API effects:** New top-level project API and CLI; existing one-shot commands become convenience workflow constructors.
- **Promotion gate:** A complete current marked and multimodal workflow runs through the project engine with numerically identical outputs.
- **Kill criterion:** A project format that duplicates scientific data instead of referencing immutable artifacts, or a scheduler that hides backend/config changes.

### WF-01 — Typed composable analysis graph

- **Scientific question:** How can users run one method alone or combine many methods without bespoke orchestration code?
- **Canonical definition:** A typed DAG whose nodes declare input/output schemas, maturity tier, resources, deterministic keys, scientific contract, and artifact/result ownership.
- **Node families:** ingest, QC, transform, geometry, feature, estimator, null, Bayesian fit, posterior diagnostic, cohort aggregation, comparison, prediction, simulation, visualization, export.
- **Failure states:** Type mismatch, missing dependency, invalid maturity composition, cyclic graph, resource limit, non-reproducible node without explicit permission.
- **Caching:** Content-addressed by code version, node spec, backend manifest, input digests, and seed.
- **API effects:** Rust builder, YAML/TOML/JSON workflow schema, Python/R bindings, CLI `marklab run`.
- **Promotion gate:** Current workflows, K/L, and one Bayesian model compose without workflow-specific branches.
- **Kill criterion:** A generic untyped “task runner” that cannot enforce scientific contracts.

### BACK-01 — Backend and plugin registry

- **Canonical definition:** Versioned, capability-scoped backend descriptors with environment, command/container, schema, resources, license, security, diagnostics, and deterministic controls.
- **Backend types:** native library, local process, Python worker, R worker, CmdStan, container, remote scheduler, GPU service.
- **Security:** Default-deny filesystem/network access, explicit mounts, resource quotas, signed manifests where feasible.
- **Failure states:** Unpinned environment, license conflict, schema mismatch, backend drift, timeout, missing diagnostics, unsupported hardware.
- **Promotion gate:** A Stan model and a Python embedding model run reproducibly and produce validated typed artifacts.
- **Kill criterion:** Plugins that return arbitrary JSON or execute unpinned code without provenance.

### UX-01 — Computational pathologist workbench

- **Canonical definition:** Interactive project browser, spatial viewer, workflow builder, method catalog, result/diagnostic explorer, cohort comparison interface, and publication export.
- **Modes:** local desktop, browser/server, headless CLI, notebook/API.
- **Core views:** slide and cells; compartments/interfaces; embeddings; graphs; curves; posterior maps; chain diagnostics; uncertainty; cohort hierarchy; provenance; maturity badges.
- **Non-goal:** The UI must not calculate scientific results independently of the workflow engine.
- **Promotion gate:** A user can create, run, inspect, compare, and export a complete multimodal project without editing configuration by hand.

## 5.6 Bayesian foundation cards

### BAY-01 — Bayesian model specification and prior system

- **Scientific question:** How can Marklab express hierarchical spatial models reproducibly across several inference backends?
- **Canonical definition:** A backend-neutral typed model specification identifies observations, hierarchy, likelihood, linear/nonlinear predictors, spatial fields, point-process components, latent factors, priors, constraints, generated quantities, and diagnostics.
- **Priors:** Named, parameterized, unit-aware, versioned, and linked to rationale. Default priors are conservative and visible, never implicit.
- **Inputs:** Project entities, typed marks, covariates, windows, graphs, fields, and external predictions.
- **Failure states:** Improper prior, unidentified model, incompatible units, singular design, unsupported backend, hidden default, invalid constraint.
- **Validation:** Prior predictive simulation, hand-conjugate fixtures, cross-backend agreement for small models, prior sensitivity.
- **Artifacts:** Model spec, compiled backend representation, prior draws, fit checkpoints, posterior samples, diagnostics.
- **Promotion gate:** The same simple hierarchical model fits in at least two backends with agreement within Monte Carlo error.
- **Kill criterion:** A free-form model string with no typed hierarchy, units, or prior provenance.

### BAY-02 — Bayesian fit lifecycle and diagnostics

- **Canonical definition:** One fit lifecycle normalizes HMC/NUTS, SMC, VI, Laplace, INLA-style, and amortized inference outputs.
- **Required diagnostics:** chain count; warmup; effective sample size; rank-normalized split R-hat; divergences; treedepth; energy/BFMI where available; ELBO trace for VI; Pareto-k for LOO; convergence/failure reasons; runtime/memory.
- **Required checks:** prior predictive, posterior predictive, simulation-based calibration where generative simulation is possible, prior sensitivity, chain/seed reproducibility, held-out predictive checks.
- **Typed fit states:** complete, incomplete, nonconverged, divergent, numerically_failed, diagnostics_unavailable, approximate_only.
- **Promotion gate:** No stable posterior summary can be `available` when mandatory diagnostics fail.
- **Kill criterion:** Returning posterior means from a failed or nonconverged fit as ordinary results.

### BAY-03 — Hierarchical cohort models

- **Scientific question:** How do spatial endpoints vary across patients, specimens, slides, regions, sites, treatments, and timepoints?
- **Canonical models:** generalized linear mixed models and Bayesian multilevel models with nested/crossed random effects, paired effects, site effects, varying slopes, and partial pooling.
- **Inputs:** Prespecified patient-level or region-level endpoints; optional measurement uncertainty from lower-level analyses.
- **Likelihoods:** Gaussian, Student-t, Bernoulli, binomial, beta-binomial, Poisson, negative binomial, hurdle/zero-inflated, ordinal, survival extensions through integrated backends.
- **Replication:** Patient is the default population unit; lower levels enter explicitly through hierarchy.
- **Failure states:** One patient per group for population claim, complete separation, unidentified random effects, insufficient clusters, prior-dominated fit.
- **Validation:** Simulated nested designs, coverage, SBC, comparison to known mixed-model implementations, paired/multisite real cohorts.
- **Result:** Population effects, patient effects, heterogeneity, posterior probabilities, intervals, predictive distributions, shrinkage diagnostics.
- **Kill criterion:** Fitting cell-level rows against a patient-level outcome without a valid aggregation or hierarchical observation model.

### BAY-04 — Gaussian-process and latent spatial-field models

- **Scientific question:** How does a continuous, binary, count, or vector-derived signal vary smoothly or nonstationarily over tissue?
- **Models:** Gaussian processes, Matérn fields, sparse/inducing-point GPs, nearest-neighbor GPs, SPDE approximations, spatially varying coefficients, multivariate GPs, anisotropic and nonstationary kernels.
- **Inputs:** Coordinates/window, typed observations, covariates, measurement error, hierarchy.
- **Outputs:** Posterior fields, gradients, exceedance probabilities, uncertainty maps, covariance parameters, predictions at requested locations.
- **Scale:** Exact GP limited to small data; sparse/mesh/NN methods required for pathology scale.
- **Validation:** Known-field simulation, coverage, kernel recovery, mesh/inducing sensitivity, cross-backend small oracle.
- **Kill criterion:** Rasterizing irregular data without recording interpolation/approximation or claiming cell-scale resolution unsupported by the field model.

### BAY-05 — CAR, SAR, GMRF, and graph spatial models

- **Scientific question:** How do signals depend on a declared spatial graph or areal adjacency structure?
- **Models:** intrinsic/proper CAR, BYM/BYM2, SAR, graph GMRF, multivariate and spatiotemporal extensions.
- **Required contract:** graph construction, weights, normalization, disconnected components, identifiability constraints, scale, and coordinate frame.
- **Validation:** Exact small precision-matrix fixtures, posterior recovery, graph sensitivity, disconnected-component tests.
- **Kill criterion:** A graph prior whose result is interpreted without reporting the graph/weight definition.

### BAY-PP — Bayesian point-process and joint location-mark models

- **Scientific question:** Which latent intensity, clustering, inhibition, interaction, compartment, and mark processes plausibly generated the observed spatial pattern?
- **Models:** Poisson, inhomogeneous Poisson, LGCP, Thomas/Matérn cluster, Strauss/Gibbs, multitype interaction, marked processes, joint location-mark, compartment/interface-conditioned, covariate-driven intensity, hierarchical replicated patterns.
- **Window:** Exact observation domain mandatory.
- **Inference:** HMC/SMC/Laplace/INLA/SBI depending model; approximation method recorded.
- **Outputs:** Posterior intensity, interaction parameters, latent fields, replicated-pattern effects, posterior predictive K/g/mark summaries.
- **Validation:** Known-parameter simulation, SBC, posterior predictive envelopes, type-I/power of derived decisions, comparison with `spatstat`/INLA/Stan small cases.
- **Kill criterion:** Treating a fitted latent intensity field as evidence of attraction without conditioning on the model’s interaction component.

### BAY-NP — Bayesian nonparametric niches and domains

- **Scientific question:** How many latent cellular/morphological niches exist, how do they overlap, and how reproducible are they across patients?
- **Models:** finite mixtures, Dirichlet-process mixtures, Pitman–Yor processes, hierarchical DPs, spatial Potts/HMRF mixtures, spatial topic models, mixed-membership/overlapping niches.
- **Inputs:** Multiscale neighborhood composition, cell/patch embeddings, compartments, molecular marks, hierarchy.
- **Outputs:** Posterior niche probabilities, number-of-niches uncertainty, patient-level prevalence, reference-query mapping, stability across scales.
- **Validation:** Synthetic overlapping domains, label-switching handling, posterior predictive checks, cross-patient recovery, comparison to imported BANKSY/UTAG/CytoCommunity results.
- **Kill criterion:** Selecting one hard clustering as truth without posterior uncertainty or sensitivity to priors/scale.

### BAY-MM — Bayesian multimodal latent-variable models

- **Scientific question:** What shared and modality-specific latent structures connect morphology, spatial context, IHC, omics, CNA/clones, and clinical variables?
- **Models:** probabilistic CCA, multiview factor analysis, Bayesian matrix/tensor factorization, hierarchical latent factors, missing-modality models, supervised latent factors, spatially structured factors.
- **Inputs:** Cell, patch, region, slide embeddings; measured and predicted modalities kept distinct; hierarchy and coordinate links.
- **Outputs:** Shared factors, modality loadings, uncertainty, missing-modality posterior predictions, spatial factor maps, patient-level summaries.
- **Validation:** Synthetic factor recovery, missingness mechanisms, posterior calibration, patient-held-out prediction, site/stain sensitivity.
- **Kill criterion:** Reporting morphology-derived molecular posterior predictions as measured molecular maps.

### BAY-REG — Joint registration and biological uncertainty

- **Scientific question:** How do registration uncertainty and biological spatial relationships interact across serial or multimodal sections?
- **Models:** probabilistic landmark transforms, deformation fields, latent correspondence probabilities, joint downstream models integrating transform uncertainty.
- **Outputs:** transform posterior, uncertainty field, correspondence probabilities, downstream effect sensitivity.
- **Validation:** Known deformations, missing/distorted sections, landmark noise, coverage and downstream bias.
- **Kill criterion:** One deterministic transform treated as exact in analyses whose scale is below registration resolution.

### BAY-EVO — Clonal phylogeography and tumor evolution

- **Scientific question:** How are imported clones/lineages spatially organized and how might they have expanded or competed?
- **Models:** spatial phylogenetic random effects, clone-specific point processes, growth-front models, competition models, state-space longitudinal models.
- **Inputs:** Imported clone probabilities, phylogeny/evolutionary distances, compartments, timepoints, treatment.
- **Claim limit:** Cross-sectional tissue supports association and model plausibility, not uniquely identified evolutionary history.
- **Validation:** Known simulated clone geometries and lineage histories, assignment uncertainty, 3-D/longitudinal data where available.
- **Kill criterion:** Inferring direction or ancestry from spatial proximity alone.

### BAY-SBI — Simulation-based and amortized inference

- **Scientific question:** Can complex mechanistic or neural tissue models be fitted when the likelihood is unavailable or intractable?
- **Methods:** ABC, synthetic likelihood, neural posterior estimation, neural likelihood estimation, neural ratio estimation, sequential neural methods, amortized inference.
- **Inputs:** Simulator, prior, summary/raw representation, observation, calibration suite.
- **Required validation:** simulation-based calibration, coverage, posterior predictive checks, out-of-distribution detection, sensitivity to simulator misspecification, comparison with tractable special cases.
- **Artifacts:** simulator version, training simulations, network/checkpoint, calibration, posterior samples.
- **Kill criterion:** A learned posterior used outside its training support or without calibration.

## 5.7 Embedding, graph, topology, and generative frontier cards

### EMB-CORE — CellViT single-cell embeddings as a primary modality

- **Canonical contract:** `CellEmbeddingTable` keyed by stable `CellId`, contiguous `f32` storage, `f64` accumulation, complete model/checkpoint/layer/context provenance.
- **Analyses:** spatial covariance, vector variograms, kernel mark correlation, graph smoothness, graph spectra, local diversity, morphological boundaries, cross-sample comparison, retrieval, uncertainty, predictive models.
- **Joint models:** combine with patch/region embeddings, neighborhood composition, measured IHC/omics, clones, compartments, and clinical covariates.
- **Primary evaluation:** patient-held-out nested validation; within-slide splits are diagnostics only.
- **Kill criterion:** per-cell JSON vectors, unknown checkpoint, or final-test feature selection.

### EMB-PATCH — Multiscale patch and region representations

- **Contract:** `PatchEmbeddingTable`, `RegionEmbeddingTable`, `CellPatchLink`, `PatchRegionLink`, physical scale, stride, overlap, effective receptive field, and shared-vector semantics.
- **Scales:** one-cell, local multicellular, gland/niche, compartment, broad architecture.
- **Analyses:** incremental-value M0–M5 program, cross-level covariance, multiscale kernels, late fusion, hierarchical latent models, transformers only after simpler baselines.
- **Failure states:** overlap leakage, duplicated vectors, unknown scale, inconsistent coordinate frame.

### GSP-01 — Graph spectral and multiscale transforms

- **Methods:** graph Fourier, heat kernels, diffusion maps, spectral graph wavelets, diffusion wavelets, graph scattering, graph filter banks.
- **Contract:** graph/weights, Laplacian, normalization, boundary/component policy, filters, scales, approximation error.
- **Scale:** sparse polynomial/Chebyshev execution; exact eigendecomposition for small oracles only.
- **Validation:** known graphs, exact eigen fixtures, perturbation stability, physical-scale interpretation, patient-level endpoint validation.

### GSP-02 — Heterogeneous graphs, hypergraphs, and higher-order tissue structure

- **Entities:** cells, glands, vessels, nerves, tumor regions, immune aggregates, compartments, boundaries, patches.
- **Methods:** typed edges, hyperedges, motifs, simplicial/cellular complexes, message passing, equivariant models, hierarchical pooling.
- **Claim limit:** learned representations require held-out patient/site validation and interpretation auditing.

### TOP-01 — Topological and morphological tissue summaries

- **Methods:** persistent homology, persistence images/landscapes, Euler characteristic curves, alpha/witness complexes, Minkowski functionals, percolation and connectivity transitions.
- **Requirement:** every quantity maps to a pathology question and has a stability/scale contract.
- **Validation:** analytic shapes, simulated tissues, segmentation perturbation, patient-level reproducibility.

### GEN-01 — Mechanistic and neural tissue simulators

- **Models:** cluster/inhibition processes, reaction–diffusion, ecological competition, front propagation, vascular/resource models, neural Cox/marked processes, flows, diffusion models, hybrid simulators.
- **Use:** posterior predictive checking, power analysis, SBI, augmentation research, counterfactual exploration only where identified.
- **Required tests:** held-out summary checks, mode-collapse, memorization, privacy/leakage, parameter recovery, comparison with simpler models.
- **Kill criterion:** simulator quality judged only by visual realism.

### CAU-01 — Spatial causal inference and interference laboratory

- **Methods:** exposure mappings, potential outcomes with interference, continuous spatial treatments, spatial propensity scores, doubly robust/DML estimators, negative controls, sensitivity and partial identification.
- **Admission requirement:** explicit treatment/exposure, temporal ordering, unit, interference structure, positivity, and confounders.
- **Default claim:** association only when requirements are not met.
- **Kill criterion:** causal communication inferred from observational proximity or ligand-receptor scoring.

### DIM-01 — 3-D, serial-section, and longitudinal tissue

- **Contract:** dimension-aware coordinates, anisotropic units, section order, missing/distorted section states, transform/deformation uncertainty, 3-D windows.
- **Methods:** 3-D point processes, graphs, topology, territories, latent fields, longitudinal processes, deformation-versus-change models.
- **Kill criterion:** silently applying 2-D estimators to 3-D data.

---
# 6. Target architecture for the frontier platform

## 6.1 Architectural decision

The repository should evolve into a **multi-crate, multi-backend spatial-pathology platform** through controlled replatforming.

The current crate remains a characterization source and compatibility shell during migration. Existing workflows must continue to pass until their replacements reach parity. The implementation lead may retire old APIs after a documented migration and version boundary.

Dependency direction:

```text
project/data/identity/units/coordinates
        ↓
geometry/index/graph/artifact primitives
        ↓
classical statistics + Bayesian model IR + simulation primitives
        ↓
multimodal/embedding/pathology/cohort scientific engines
        ↓
workflow orchestration + backend workers
        ↓
results/artifacts/provenance/reporting
        ↓
CLI + Python/R bindings + server/UI
```

No UI, CLI, report, or backend adapter may own a scientific formula. No scientific engine may depend on the UI or CLI.

## 6.2 Proposed workspace

Names may be refined during the architecture phase, but ownership boundaries should approximate:

```text
crates/
  marklab-core/             # errors, finite values, IDs, units, deterministic seeds
  marklab-project/          # project manifest, artifact catalog, workflow state
  marklab-data/             # cohort, modality, observations, marks, embeddings
  marklab-geometry/         # 2-D/3-D windows, boundaries, transforms, spatial plans
  marklab-graph/            # graphs, hypergraphs, weights, graph storage
  marklab-inference/        # permutation, bootstrap, envelopes, multiplicity
  marklab-point-process/    # K/L/g/F/G/J, marked/multitype methods
  marklab-spatial-signal/   # autocorrelation, covariance, variograms, fields
  marklab-bayes/            # model IR, priors, posterior and diagnostic contracts
  marklab-bayes-native/     # selected native conjugate/Laplace/SMC kernels
  marklab-multimodal/       # registration/fusion/cross-modal spatial analysis
  marklab-embeddings/       # cell/patch/region vector statistics and links
  marklab-pathology/        # compartments, interfaces, infiltration, glands, vessels
  marklab-cohort/           # hierarchical designs, population inference, prediction
  marklab-simulation/       # canonical simulators and validation generators
  marklab-experimental/     # graph spectral, topology, generative, causal, 3-D labs
  marklab-workflow/         # typed DAG, planner, cache, scheduler
  marklab-backends/         # native/process/container/Python/R/Stan registries
  marklab-artifacts/        # Arrow/Parquet/Zarr/OME/result/report persistence
  marklab-interop/          # SpatialData, AnnData, OME-NGFF, external tools
  marklab-report/           # publication tables/figures and claim-aware narratives
  marklab-cli/              # command surface
  marklab-server/           # API and job service
  marklab-ui/               # interactive computational-pathology workbench
  marklab-python/           # Python bindings/client
  marklab-r/                # R bindings/client
legacy/
  marklab-legacy/           # characterized current API during migration
workers/
  python/                   # locked Python backend worker
  r/                        # locked R backend worker
  stan/                     # CmdStan model/build worker
schemas/
  project/
  workflow/
  artifact/
  result/
validation/
  reference/
  simulation/
  real_data/
  calibration/
```

A workspace split is justified only when it clarifies ownership, feature/dependency isolation, compilation, or backend boundaries. Do not create ceremonial crates.

## 6.3 Core project model

### `MarklabProject`

Owns:

- project UUID and schema version;
- patient/specimen/slide/core/region/cell/patch identities;
- modality registry;
- coordinate frames, transforms, units, and dimensionality;
- immutable input artifact references and digests;
- observation windows, masks, compartments, and annotations;
- embeddings and external model outputs;
- workflow definitions;
- backend manifests and environments;
- execution ledger;
- result and artifact catalog;
- validation maturity and claim policies.

### Artifact addressing

Artifacts should be content-addressed where practical:

```text
ArtifactId = hash(schema_version, content_digest, semantic_manifest)
```

The project manifest references artifacts rather than duplicating large data.

## 6.4 Typed workflow engine

Each node declares:

```text
node_id
type/version
maturity tier
scientific contract ID
input schemas
output schemas
resource request
backend requirement
determinism/seed policy
cache key
failure semantics
result/artifact ownership
```

### Node families

- ingestion and normalization;
- QC and filtering;
- coordinate transformation and registration;
- segmentation/label/embedding import;
- geometry and graph planning;
- classical estimator;
- null/randomization;
- Bayesian model compile/fit;
- posterior diagnostic/check;
- simulation and SBI;
- cohort aggregation and comparison;
- predictive training/evaluation;
- visualization/report/export.

### Solo versus combined analysis

Every scientific method is a node that can be invoked alone. Complex recipes compose nodes, for example:

```text
CellViT embeddings
  → technical-confounder QC
  → multiscale graph construction
  → vector variogram
  → Bayesian spatial factor model
  → patient-level group comparison
  → posterior map/report
```

or:

```text
H&E + IHC + clone probabilities + tumor/stroma boundary
  → nonrigid registration backend
  → uncertainty propagation
  → clone-specific infiltration
  → multitype LGCP
  → paired treatment model
```

## 6.5 Backend architecture

### Backend classes

```text
NativeRustBackend
LocalProcessBackend
PythonWorkerBackend
RWorkerBackend
CmdStanBackend
ContainerBackend
RemoteSchedulerBackend
GpuServiceBackend
```

### Environment reproducibility

Every backend run records:

- executable/container digest;
- package lockfile;
- OS/architecture/GPU/CUDA;
- environment variables permitted;
- command and configuration;
- input/output digests;
- random seeds and chain IDs;
- runtime and peak memory;
- logs and diagnostics;
- backend-specific version metadata.

### Bayesian backend strategy

- Native Rust: conjugate models, deterministic summaries, selected Laplace/SMC kernels, model preparation, diagnostics, artifact handling.
- CmdStan: canonical HMC/NUTS reference and complex user-defined models.
- PyMC/NumPyro/JAX: GPU/VI/amortized and flexible latent models.
- INLA/inlabru: selected latent Gaussian and point-process models.
- Turing/other: optional adapters where a unique model or ecosystem benefit exists.

## 6.6 Data and modality model

### Identity hierarchy

```text
Cohort
  └─ Site
      └─ Patient
          └─ Specimen/Timepoint
              └─ Block
                  └─ Slide/Section
                      └─ Core/Region
                          ├─ Cell
                          ├─ Patch
                          ├─ Region entity
                          └─ Molecular observation
```

Cross-cutting identities include modality, acquisition batch, scanner, stain, model, transform, clone, compartment, and intervention.

### Coordinate model

- 2-D and 3-D coordinate frames;
- physical units;
- image pixel/index coordinates;
- transformations and uncertainty;
- serial-section z order and thickness;
- containment/link relationships;
- explicit missing/distorted section states.

### Mark model

Support:

- binary;
- categorical;
- ordinal;
- count;
- continuous;
- probability;
- probability simplex;
- censored/interval;
- vector artifact;
- distribution-valued or posterior artifact;
- measured, imported prediction, morphology-derived prediction, and derived summary status.

## 6.7 Cell, patch, region, and slide embeddings

Store separately:

```text
CellEmbeddingTable
PatchEmbeddingTable
RegionEmbeddingTable
SlideEmbeddingTable
CellPatchLink
PatchRegionLink
EmbeddingProvenance
```

Requirements:

- stable entity IDs;
- contiguous `f32`/`f16` storage where appropriate;
- `f64` accumulation for statistics;
- model/checkpoint/layer/pooling/context provenance;
- physical scale and receptive field;
- overlap and shared-assignment semantics;
- missingness and QC;
- Arrow/Parquet or Zarr artifacts, not result JSON;
- optional memory mapping, chunking, compression, and GPU transfer plans.

## 6.8 Scientific engine boundaries

### Classical evidence engine

- point processes;
- marked processes;
- spatial signal/geostatistics;
- graph statistics;
- pathology geometry;
- resampling and global inference.

### Bayesian engine

- model IR and priors;
- hierarchical and spatial fields;
- point processes;
- multimodal latent factors;
- nonparametric niches;
- evolution/longitudinal models;
- diagnostics, SBC, posterior predictive checks.

### Experimental discovery engine

- graph wavelets/scattering;
- hypergraphs/complexes/topology;
- neural and generative models;
- SBI/amortized inference;
- causal/interference laboratory;
- active design;
- 3-D/temporal research.

## 6.9 API surfaces

### Rust

Low-level domain APIs, estimator plans, workflow engine, artifact readers/writers, and backend registry.

### CLI

Proposed command families:

```text
marklab project init|validate|inspect|migrate
marklab ingest cells|patches|images|embeddings|omics|clones|clinical
marklab workflow validate|plan|run|resume|graph|export
marklab analyze classical|bayes|embedding|graph|topology|pathology|cohort
marklab simulate generate|fit|posterior-predictive|calibrate
marklab compare samples|cohorts|models|nulls|priors
marklab backend list|install|validate|doctor
marklab report build|export
marklab serve
```

Existing commands become compatibility aliases or workflow recipe shortcuts.

### Python and R

- project and artifact clients;
- workflow construction;
- zero-copy/Arrow access;
- model/backend adapters;
- result exploration;
- no duplicate scientific implementation unless serving as a reference oracle.

### Server/UI

A service API owns jobs, projects, artifacts, authentication, and collaboration. The UI never recomputes science.

## 6.10 Configuration and recipe system

Separate:

1. project manifest;
2. workflow recipe;
3. scientific method configuration;
4. backend/environment configuration;
5. resource/scheduler policy;
6. report/visualization configuration;
7. claim/maturity policy.

Recipes must be versioned and importable. Preset panels should include:

- classical point-process panel;
- multimodal MMR/IHC panel;
- embedding spatial-dependence panel;
- pathology interface/infiltration panel;
- Bayesian cohort panel;
- clone/immune/morphology panel;
- exploratory graph/topology panel;
- 3-D/longitudinal panel.

## 6.11 Results, posterior artifacts, and reports

### Stable JSON result

Contains bounded summaries, scientific contract, maturity, estimand, assumptions, design, diagnostics status, intervals/p-values/posterior probabilities, artifact references, and provenance digests.

### Posterior artifact

Use Arrow/Parquet/Zarr/NetCDF-compatible representations for:

- draws;
- latent fields;
- per-entity probabilities;
- chain diagnostics;
- generated quantities;
- posterior predictive simulations;
- variational parameters/checkpoints.

### Experimental result

Must be tagged with unstable schema and claim ceiling. It cannot silently deserialize as a stable result.

### Reports

Report generation is claim-aware. It must display:

- maturity tier;
- measured versus predicted status;
- biological replication unit;
- null/likelihood/prior;
- convergence/calibration warnings;
- approximation mode;
- technical-confounder diagnostics;
- unsupported interpretations.

## 6.12 Provenance and reproducibility

Every execution records:

- repository/version/SHA;
- project/workflow/schema versions;
- input and artifact digests;
- coordinate and unit metadata;
- backend/environment/container digest;
- model/checkpoint checksum;
- code/config/command digest;
- seed namespace, chain IDs, and replicates;
- exact versus approximate mode;
- runtime/hardware/memory;
- logs, warnings, failed replicates/chains;
- validation maturity and claim policy.

## 6.13 Performance ownership

Performance is owned at several layers:

- geometry and pair plans;
- graph/hypergraph storage;
- embedding matrix and link storage;
- posterior sample storage;
- backend staging/serialization;
- workflow cache;
- out-of-core/streaming;
- GPU transfer and kernels;
- distributed cohort execution.

Every scientific node declares a memory/time model and exact/approximate strategy.

## 6.14 Exact, approximate, and learned modes

Every result records:

```text
execution_mode: exact | approximate | learned
algorithm/backend/version
error_bound_or_calibration
sampling_fraction_or_rank
seed/checkpoint
exact_fallback_available
training_support_or_domain
```

No approximate or learned mode may masquerade as exact inference.

---

# 7. Dependency-ordered implementation roadmap

## 7.1 Program strategy

The target is a full transformation, but implementation remains dependency-ordered and evidence-gated. The program runs three tracks in parallel after the foundation is stable:

- **Evidence track:** trusted classical and cohort-valid methods.
- **Bayesian/model track:** hierarchical spatial and multimodal probabilistic models.
- **Frontier track:** graph/topology/generative/causal/3-D research.

All tracks share the same project, workflow, data, artifact, provenance, validation, and UI layers.

## Phase 0 — Bootstrap, characterize, and preserve

### WS-00 — Implementation control plane

- Install this master plan in the repository.
- Create root `AGENTS.md` and persistent state/ledger files.
- Record exact baseline SHA, worktree, features, toolchain, and dirty state.
- Run all existing CI, calibration-smoke, fuzz-build, packaging, and benchmark gates available.
- Capture current public API/CLI/config/result/artifact behavior.
- Open a dedicated replatform branch/worktree.
- Prohibit broad source changes until baseline evidence is recorded.

### WS-01 — Legacy characterization suite

- Freeze numerical and serialized behavior of current marked, multimodal, pre/post, WSI, output, and performance workflows.
- Add golden fixtures only where current behavior is supported and scientifically intended.
- Identify behaviors to preserve, migrate, deprecate, or intentionally break.
- Produce a compatibility matrix.

**Exit gate:** Baseline is reproducible; existing failures are documented; current behavior can be compared after replatforming.

## Phase 1 — Workspace and operating-system foundation

### WS-10 — Cargo workspace and compatibility shell

- Establish new crate boundaries.
- Place current implementation behind a compatibility boundary.
- Preserve current binary and library behavior through adapters.
- Add workspace-wide CI, dependency, feature, and documentation rules.

### WS-11 — Project, artifact, and provenance layer

- Implement `MarklabProject`, artifact catalog, content digests, execution ledger, schema registry, and migration framework.
- Add interrupted-run recovery and artifact integrity checks.

### WS-12 — Typed workflow graph and scheduler

- Implement typed DAG, node registry, resource plan, cache keys, resumability, and local scheduler.
- Run one current marked workflow end-to-end through the DAG.

### WS-13 — Backend registry

- Implement native, process, Python, R, Stan, and container backend contracts.
- Add environment lock and doctor/validation commands.

**Exit gate:** Current analyses run through the new project/workflow system with parity; no scientific formula is duplicated.

## Phase 2 — Unified data, modalities, geometry, and embeddings

### WS-20 — Identity and cohort hierarchy

- Patient/site/specimen/timepoint/block/slide/section/core/region/cell/patch IDs.
- Biological versus technical replication roles.
- Paired, repeated, multiregion, multicore, and multisite designs.

### WS-21 — Coordinate, unit, dimensionality, transform, and uncertainty model

- 2-D/3-D frames;
- image/pixel and physical coordinates;
- serial-section z structure;
- transform chains and uncertainty;
- containment and correspondence relationships.

### WS-22 — Observation windows, compartments, and exact geometry

- Polygon/multipolygon/hole/volume windows;
- boundary distance;
- shared exact index and pair/edge plans;
- memory-budgeted streaming visitors.

### WS-23 — General marks and measurement provenance

- Binary, categorical, ordinal, continuous, count, probability, simplex, vector, and posterior-valued observations.
- Measured versus predicted status.

### WS-24 — CellViT and multiscale embedding artifacts

- Recover/import actual per-cell embeddings and provenance.
- Implement cell/patch/region/slide tables and links.
- Add technical-confounder QC and storage benchmarks.

### WS-25 — Multimodal interchange

- SpatialData, AnnData, OME-NGFF, Arrow/Parquet/Zarr, external-model manifests.

**Exit gate:** A project can represent and validate all target modalities, identities, embeddings, windows, and transforms without running a scientific model.

## Phase 3 — Classical spatial evidence engine

### WS-30 — Point-process core

- K/L with border, translation, and isotropic corrections;
- inhomogeneous K/L;
- g, cross-K, cross-g;
- F/G/J and nearest-neighbor distributions;
- multitype and marked functions;
- intensity estimation;
- directional and anisotropic extensions.

### WS-31 — Null and global inference

- CSR, random labeling, population independence, stratified/conditional nulls, appropriate shifts/translation nulls;
- global envelopes;
- functional and scalar tests;
- multiple-testing families.

### WS-32 — Spatial signal and geostatistics

- Moran, Geary, bivariate/local indicators, Getis-Ord with multiplicity;
- covariance, cross-covariance, variograms, spatially varying summaries.

### WS-33 — Pathology geometry

- interfaces, signed distance, infiltration, fronts, budding, satellites, fragmentation, mixing, glands, vessels, nerves, necrosis, resources, curvature.

### WS-34 — Cohort-level resampling and comparison

- patient-level permutation;
- hierarchical bootstrap;
- paired/repeated designs;
- spatial fingerprints;
- MMD, energy, graph/topological/functional comparisons;
- genuine equivalence/noninferiority.

**Exit gate:** Trusted methods match independent oracles, calibrate under simulations, and run through project/workflow/UI surfaces.

## Phase 4 — Bayesian foundation and established probabilistic models

### WS-40 — Model IR, priors, posterior artifacts, and diagnostics

- Implement backend-neutral model specification.
- Integrate CmdStan and one GPU-capable Python backend.
- Add R-hat/ESS/divergence/LOO/VI diagnostics, prior/posterior predictive checks, SBC, sensitivity.

### WS-41 — Hierarchical cohort models

- Gaussian/Student-t, binomial/beta-binomial, Poisson/negative-binomial, ordinal, hurdle, repeated and multisite models.

### WS-42 — GP/GMRF/CAR/SAR spatial fields

- Continuous, binary, count, multivariate, nonstationary, anisotropic, and spatially varying coefficient models.

### WS-43 — Bayesian point processes

- Poisson, LGCP, cluster, Gibbs/Strauss, multitype, marked, joint location-mark, replicated-pattern models.

### WS-44 — Bayesian model comparison and calibration

- LOO/WAIC where valid;
- predictive comparison;
- prior sensitivity;
- posterior predictive spatial summaries;
- benchmark against trusted backends.

**Exit gate:** At least one hierarchical, one field, and one point-process model have cross-backend agreement, SBC, posterior predictive checks, and stable experimental or established result contracts.

## Phase 5 — Multimodal cell, patch, molecular, and clone intelligence

### WS-50 — Single-cell embedding statistics

- Vector variograms, covariance, kernel mark correlation, graph smoothness, local diversity, boundaries, cross-sample tests, retrieval.

### WS-51 — Patch/region multiscale representations

- Physical-scale patch links, complementarity analysis, overlap-aware inference, multiscale kernels and fingerprints.

### WS-52 — Predictive M0–M5 framework

- Technical baseline;
- cell embedding;
- neighborhood context;
- cell+context;
- patch context;
- independent molecular measurement;
- patient-held-out nested validation, calibration, OOD, abstention.

### WS-53 — Multimodal Bayesian latent models

- Morphology, IHC, omics, CNA/clone, compartments, and clinical variables;
- missing modalities and spatial factors;
- measured/predicted separation.

### WS-54 — Clone/CNA and evolutionary downstream analysis

- Imported assignment uncertainty;
- segregation/mixing/boundaries/interfaces/niches;
- clone-specific immune/stromal context;
- phylogenetic-spatial comparison.

### WS-55 — Registration and uncertainty

- Nonrigid backend integration;
- probabilistic correspondence;
- joint downstream sensitivity.

**Exit gate:** Cell and patch embeddings are first-class modalities; at least one multimodal Bayesian model and one patient-level predictive workflow are externally validated.

## Phase 6 — Neighborhoods, domains, graph mathematics, and topology

### WS-60 — Multiscale neighborhoods and niches

- Continuous fingerprints;
- hard/soft/overlapping niches;
- imported discovery methods;
- Bayesian nonparametric models;
- cross-patient reference/query mapping.

### WS-61 — Heterogeneous graphs and higher-order tissue

- Cells, glands, vessels, nerves, boundaries, patches, regions;
- typed graphs, hypergraphs, motifs, complexes.

### WS-62 — Spectral graph and scattering laboratory

- Graph Fourier, heat kernels, diffusion, wavelets, scattering, equivariant representations.

### WS-63 — Topological and morphological laboratory

- Persistence, Euler/Minkowski, percolation, connectivity, mapper, scale stability.

**Exit gate:** Experimental methods have exact mathematical fixtures, perturbation analyses, patient-level reproducibility studies, and clear claim tiers.

## Phase 7 — Generative, mechanistic, and simulation-based tissue science

### WS-70 — Canonical simulator library

- Poisson, inhomogeneous, Thomas, Matérn, Strauss, Gibbs, LGCP, multitype attraction/repulsion, compartment, clone, field, registration, segmentation, and domain-shift simulators.

### WS-71 — Mechanistic tissue models

- Reaction-diffusion, competition, resource/vessel, interface/front, clone-growth, treatment-response models.

### WS-72 — Neural point processes, flows, and diffusion

- Neural Cox and marked processes;
- conditional cellular layout generation;
- graph/tissue diffusion and normalizing flows.

### WS-73 — Simulation-based and amortized Bayesian inference

- ABC, synthetic likelihood, NPE/NLE/NRE, sequential methods, calibration.

### WS-74 — Posterior predictive digital tissue laboratory

- Clearly labelled simulated tissue, uncertainty, failure tests, memorization/privacy checks.

**Exit gate:** Generative models outperform simpler baselines on prespecified held-out summaries and pass calibration/memorization checks. “Digital twin” language remains prohibited without prospective evidence.

## Phase 8 — 3-D, longitudinal, causal, perturbational, and active design

### WS-80 — 3-D and serial-section platform

- Anisotropic windows, section reconstruction, missing/distorted sections, 3-D point processes, graphs, topology, fields, territories.

### WS-81 — Longitudinal and evolutionary models

- Temporal spatial processes, paired biopsies, deformation versus biological change, clone trajectories.

### WS-82 — Causal/interference laboratory

- Treatment/exposure designs, interference mappings, negative controls, sensitivity, partial identification.

### WS-83 — Perturbation and treatment-response workflows

- Spatial CRISPR/FISH, spillover, dose response, matched controls, niche/clone response.

### WS-84 — Active experimental design

- ROI, stain, landmark, FOV, cell sequencing, replicate allocation, and prospective power under budget.

**Exit gate:** Each causal or intervention workflow has an explicit identification contract and data-design gate.

## Phase 9 — Workbench, collaboration, distribution, and publication-grade release

### WS-90 — Interactive workbench

- Spatial viewer;
- workflow builder;
- model/diagnostic explorer;
- posterior and uncertainty maps;
- cohort dashboards;
- method catalog and maturity badges.

### WS-91 — Server, collaboration, and remote execution

- multi-user projects;
- authentication/authorization;
- remote schedulers and GPU workers;
- resumable jobs;
- audit logs.

### WS-92 — Python/R/Notebook ecosystem

- bindings, clients, zero-copy artifacts, notebooks, recipe library.

### WS-93 — Publication and benchmark program

- canonical benchmark datasets;
- methods benchmark paper;
- external-cohort demonstrations;
- reproducible capsules;
- tutorials and protocol papers.

### WS-94 — Stable 1.0 release

- stable project/workflow/result/artifact schemas;
- migration tools;
- long-term support policy;
- security and dependency review;
- documented claim tiers.

---

# 8. First three implementation workstreams

These workstreams are deliberately bounded. They start the full transformation without asking one agent to rewrite the repository in one pass.

## WS-A — Implementation control plane and baseline preservation

### A-01 — Install authoritative implementation state

- **Parent requirements:** PLAT-01, implementation protocol.
- **Allowed write scope:** `AGENTS.md`, `docs/implementation/**` only.
- **Owned symbols:** none in production.
- **Read-only context:** entire repository and this plan.
- **Behavior first:** state files exist, identify exact SHA/branch/worktree, and contain no unsupported passing claims.
- **Implementation:** copy the immutable plan; create status, requirements, decisions, repository map, interface contracts, canonical-symbol registry, validation/performance/claims ledgers, and handoff directory.
- **Verification:** link/path checks, markdown lint if available, Git scope audit.
- **Commit:** one documentation/bootstrap commit.

### A-02 — Reproduce current repository baseline

- **Allowed write scope:** ledgers and generated external logs/artifacts; no production changes.
- **Commands:** formatting, Clippy, all-feature tests, documentation tests, no-default build, WSI integration, dependency policy, package, fuzz build, current benchmarks and smoke/calibration subsets available.
- **Required output:** exact commands, pass/fail counts, skipped tests, runtime, failures, unavailable tools, current performance baseline.
- **Stop condition:** a failing baseline is documented and triaged before architecture changes.

### A-03 — Current contract and migration inventory

- **Allowed write scope:** `docs/implementation/REPOSITORY_MAP.md`, `INTERFACE_CONTRACTS.md`, `REQUIREMENTS.md`, `DECISIONS.md`.
- **Deliverable:** map every public API, CLI command, config section, result family, artifact, feature, workflow, and test to preserve/migrate/deprecate/remove.
- **Red behavior:** no production symbol may be moved in WS-B without a recorded disposition.

### WS-A completion

- Repository-local state survives compaction.
- Baseline is executed or failures are explicit.
- Migration inventory is complete.
- No production code changed.

## WS-B — Workspace replatforming and compatibility shell

### B-01 — Workspace architecture decision record

- **Parent requirements:** PLAT-01, WF-01, BACK-01.
- **Allowed write scope:** root Cargo files, architecture docs, new empty/skeleton crates, CI configuration.
- **Owned symbols:** workspace packages and dependency rules.
- **Non-goals:** no scientific algorithm migration yet.
- **Behavior first:** current `marklab` build/test commands still work from workspace root.
- **Decision:** finalize crate names and ownership using immediate callers; reject ceremonial crates.

### B-02 — Compatibility crate/shell

- **Allowed write scope:** workspace manifests, compatibility facade, current binary/lib entry wiring, tests.
- **Behavior first:** public API and CLI characterization fixtures remain identical.
- **Implementation boundary:** do not copy algorithms; current source is referenced or moved once with history-preserving commits.
- **Expected commits:** mechanical workspace move separated from semantic changes.

### B-03 — Workspace-wide policy

- Add `cargo xtask` or equivalent only if it centralizes real repeated commands.
- Add workspace dependency policy, feature matrix, documentation, test categories, and benchmark categories.
- Add architecture tests preventing upward dependencies and CLI-gated science.

### B-04 — Project/workflow crate skeletons with one vertical slice

- Implement minimal `MarklabProject`, artifact reference, workflow node trait/enum, and local scheduler sufficient to run the existing marked analysis through one node.
- **Red tests:** content digest changes invalidate cache; cyclic workflow rejected; failed node is not committed as successful; current result matches compatibility path.
- No broad generic framework beyond the demonstrated vertical slice.

### WS-B completion

- Workspace builds all current features.
- Current tests pass.
- One existing analysis runs through project/workflow infrastructure.
- No numerical implementation was duplicated.

## WS-C — Unified data, artifact, and embedding substrate

### C-01 — Typed identities and hierarchy

- **Parent requirements:** DATA-01, COH-01.
- **Allowed write scope:** new data/core crates and focused ingestion adapters/tests.
- **Owned symbols:** ID newtypes, hierarchy, parent links, biological/technical roles.
- **Red tests:** blank/duplicate IDs; missing parent; cycle; conflicting parent; paired and multisite designs; no filename inference.

### C-02 — Units, coordinate frames, dimensions, and transforms

- Implement 2-D/3-D physical units, image coordinates, coordinate-frame IDs, transform chains, uncertainty references, serial-section metadata.
- **Red tests:** unit mismatch, transform cycle, missing frame, invalid z spacing, 2-D method requested on 3-D data.

### C-03 — Artifact catalog and immutable tables

- Implement content-addressed artifact references, schema/version/digest, local path/object-store abstraction, Arrow/Parquet table manifests.
- **Red tests:** digest mismatch, schema mismatch, partial write, stale cache, path traversal/symlink boundary.

### C-04 — CellViT embedding table

- Implement contiguous per-cell vectors keyed by `CellId` with model/checkpoint/layer/pooling/context provenance.
- Add importers for the actual available artifact once its location is identified.
- **Red tests:** duplicate/missing cell, non-finite vector, dimension mismatch, unknown checkpoint, row-order mismatch, deterministic digest, memory-map/chunk parity.
- **Benchmark:** 10k, 1M, and planned 10M rows at representative dimensions; load, scan, covariance/kernel blocks, peak memory.

### C-05 — Patch/region/slide embeddings and links

- Implement separate tables and cell-patch/patch-region links with physical scale, containment/interpolation, overlap, and shared-vector semantics.
- **Red tests:** duplicated patch vector prohibited, inconsistent scale/frame, overlap metadata missing, one patch shared by multiple cells preserved without replication.

### C-06 — General marks and measured/predicted status

- Add binary/categorical/ordinal/count/continuous/probability/simplex/vector/posterior mark declarations.
- Require measurement status and provenance.
- **Red tests:** probability not normalized, unit conflict, measured/predicted conflation, missing threshold provenance.

### WS-C completion

- A project can represent patient hierarchy, cells, patches, regions, CellViT vectors, patch vectors, modalities, units, coordinates, transforms, and immutable artifacts.
- Current workflows can be adapted without changing current numerical results.
- No large vector enters result JSON.

## Exact acceptance commands for the first three workstreams

At minimum, preserving current repository gates:

```bash
cargo +1.96.0 fmt --all --check
cargo +1.96.0 clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo +1.96.0 nextest run --locked --workspace --all-features
cargo +1.96.0 test --locked --workspace --doc --all-features
cargo +1.96.0 check --locked --workspace --no-default-features
cargo +1.96.0 test --locked --features wsi,cli --test wsi_integration
cargo audit
cargo deny check advisories licenses bans sources
cargo machete
cargo package --locked --workspace
cargo +nightly fuzz check
```

If workspace migration changes exact syntax, the implementation lead records the replacement commands in `AGENTS.md` and the validation ledger rather than silently dropping a gate.

---

# 9. Implementation-agent operating protocol

## 9.1 Lead and subagent authority

The implementation lead owns:

- architecture and dependency direction;
- requirement IDs and ordering;
- public interface and result-schema freeze;
- canonical symbol ownership;
- integration and migration;
- acceptance and final validation;
- scientific claim wording;
- release/promotion decisions.

Subagents may own only bounded tasks with explicit writable files and symbols. They may not independently broaden scope, introduce public abstractions, alter schema, add dependencies, or reinterpret scientific requirements.

No two concurrent writing agents may own the same file or canonical symbol. Read-only overlap is permitted.

## 9.2 Persistent repository-local state

Create on the implementation branch:

```text
docs/implementation/
    MASTER_PLAN.md
    STATUS.md
    REQUIREMENTS.md
    DECISIONS.md
    REPOSITORY_MAP.md
    INTERFACE_CONTRACTS.md
    CANONICAL_SYMBOLS.md
    VALIDATION_LEDGER.md
    PERFORMANCE_LEDGER.md
    CLAIMS_LEDGER.md
    handoffs/
        <TASK_ID>.md
```

Required semantics:

- **MASTER_PLAN.md:** immutable copy of this plan plus audited base SHA. Amendments are append-only and reference a decision ID.
- **STATUS.md:** current SHA/branch/phase/task, dirty files, passing/failing commands, blockers, next exact action.
- **REQUIREMENTS.md:** every requirement ID, owner, prerequisites, status, result/API effects and closure evidence.
- **DECISIONS.md:** append-only ADR-style decisions, alternatives and consequences.
- **REPOSITORY_MAP.md:** actual module/file/symbol owners and dependency direction.
- **INTERFACE_CONTRACTS.md:** frozen input/output semantics, nulls, edge correction, units, undefined states and version.
- **CANONICAL_SYMBOLS.md:** one row per statistic/null/parser/schema conversion/geometry plan/output projection with owner, location, contract, callers, tests and active task.
- **VALIDATION_LEDGER.md:** numerical oracle, fixture provenance, simulations, calibration, power, coverage, real-data validation and exact results.
- **PERFORMANCE_LEDGER.md:** hardware/toolchain, workload shape, timings, RSS/allocations, output sizes and before/after assessment.
- **CLAIMS_LEDGER.md:** every public scientific claim, evidence class, limitations and approved wording.
- **handoffs:** one immutable task handoff per completed/paused task.

## 9.3 Initial bootstrap

Before source edits:

1. Confirm branch, exact SHA and clean state.
2. Read repository guidance, README, SPEC, public API, result-format, migration, validation and current workflow files.
3. Read this master plan and create persistent state files.
4. Run baseline commands.
5. Record all pre-existing failures and skipped tests.
6. Populate repository map and canonical-symbol registry for files the first phase will touch.
7. Freeze the first workstream’s interfaces in `INTERFACE_CONTRACTS.md`.
8. Create the first task contract.
9. Only then edit.

Suggested commands:

```bash
git status --short
git branch --show-current
git rev-parse HEAD
git log --oneline -10

cargo +1.96.0 fmt --all --check
cargo +1.96.0 clippy --locked --all-targets --all-features -- -D warnings
cargo +1.96.0 check --locked --no-default-features
cargo +1.96.0 nextest run --locked --all-features
cargo +1.96.0 test --locked --doc --all-features
```

Run repository dependency/fuzz/workflow commands exactly as defined at the audited implementation SHA and record unavailable tools rather than substituting weaker claims.

## 9.4 Checkpoint cadence

Update `STATUS.md`, requirements and relevant ledgers:

- after each red test is committed or handed off;
- after each production behavior becomes green;
- after an interface, schema or dependency decision;
- after any full-suite failure;
- before and after benchmark/calibration runs;
- before handoff;
- at least once per focused task session.

A checkpoint records the exact current SHA and `git status --short`, not “clean” or “green” without evidence.

## 9.5 Pre-compaction procedure

Immediately before context compaction:

1. Update `STATUS.md`.
2. Record exact SHA, branch and `git status --short`.
3. Record `git diff --stat` and every dirty file with reason.
4. Record current phase/task/requirement IDs.
5. Record commands actually run and exact results.
6. Record known failures and whether pre-existing.
7. Update canonical-symbol, validation, performance and claims ledgers.
8. Record decisions not yet committed.
9. Record the next exact command and next exact edit.
10. Commit completed verified work when appropriate; do not create a knowingly broken “checkpoint commit.”

## 9.6 Post-compaction recovery

After compaction, before editing:

```bash
git status --short
git log --oneline -10
git diff --stat
git diff
```

Then:

1. Read `MASTER_PLAN.md`, `STATUS.md`, `DECISIONS.md`, `REQUIREMENTS.md`, relevant interface contracts and the current task handoff.
2. Reconcile persisted state with actual Git; Git wins if they conflict.
3. Inspect the real diff, not a remembered summary.
4. Rerun the narrowest relevant test.
5. State the current requirement, current failure/green behavior and next exact action.
6. Resume only after confirming owned files/symbols remain unchanged by another task.

## 9.7 Phase transition

At phase entry:

- verify prerequisite requirement IDs are closed;
- freeze interfaces needed by the phase;
- allocate file/symbol ownership;
- record baseline performance and scientific oracles.

At phase closure:

- run all phase gates;
- audit result/API/migration changes;
- audit every new public scientific name;
- update claims and canonical-symbol registries;
- close or explicitly defer each requirement with residual risk;
- create a phase-closure record;
- merge only after integration review.

## 9.8 Final closure

A final implementation report must:

- pin base and final SHA;
- show clean status;
- list every completed/deferred/rejected requirement;
- give exact command results;
- report tests not run;
- report calibration/power/coverage and benchmark results;
- list schema/API/migration changes;
- audit public claims against evidence;
- audit canonical-symbol uniqueness and no-AI-slop rules;
- list remaining risks and next release action.

## 9.9 Subagent contract

Every subagent receives:

- task ID and parent requirement IDs;
- exact base SHA and branch/worktree;
- writable files;
- owned canonical symbols;
- read-only context;
- explicit non-goals;
- scientific invariants;
- first red behavior;
- required oracle fixtures;
- tests/benchmarks and exact acceptance commands;
- stop/escalation conditions;
- handoff template.

Stop and escalate when:

- a required file/symbol is owned by another agent;
- the task needs an unapproved dependency, public API or schema change;
- the source contradicts the frozen contract;
- the numerical oracle disagrees beyond tolerance;
- calibration fails;
- the implementation requires a second full rewrite;
- memory/performance cannot meet the agreed bound;
- required provenance/data are absent;
- scope cannot remain inside the writable set.


# 10. Mandatory NO-AI-SLOP rules

These are acceptance rules, not style preferences.

## 10.1 Canonical ownership

1. One canonical implementation per scientific statistic, null model, parser state machine, schema conversion, geometry plan and output projection.
2. Before adding a function in one of those categories, search `CANONICAL_SYMBOLS.md` and the source.
3. A commit changing a canonical symbol must update its registry row.
4. A second complete rewrite of a canonical function requires a recorded decision describing why characterization and targeted correction were insufficient.

**Gate:**

```bash
rg -n '(^|_)(new|v2|v3|final|legacy2|replacement)(_|$)' src tests
```

Every match is reviewed; replacement-style production names are rejected unless they describe a true versioned external format.

## 10.2 No god files or distributed god workflows

- Do not expand `src/api.rs`, `src/multimodal/engine.rs`, `src/cli.rs`, config facades or output writers with unrelated scientific responsibilities.
- Split by data ownership and dependency direction, not arbitrary line count.
- A new stage must have explicit typed input/output and must not import the entire parent workflow.
- Moving a god workflow across several files through wildcard imports does not close the problem.

**Gate:** phase closure includes a repository-map diff and review of every touched file’s responsibilities and callers.

## 10.3 No speculative abstractions

- No trait with one implementation unless it enforces an immediate external boundary.
- No plugin registry, universal engine, generic method factory or extension point without a current caller.
- No `utils`, `helpers`, `manager` or `service` dumping ground.
- No thin wrapper that merely renames a call and enforces no invariant.
- No one-function files created solely to reduce line count.

**Gate:** every new public/private abstraction is named in the task contract with its immediate callers and invariant.

## 10.4 No duplicate scientific or policy logic

- Statistical normalization, edge correction, permutation p-values, finite checks, missingness, serialization and measurement-status policy each have one owner.
- Endpoint modules may compose shared primitives but may not copy formulas.
- External-oracle fixture generators never share implementation code with the Rust estimator.

**Gate:** task handoff includes a source search showing no duplicate formula/field policy was introduced.

## 10.5 No fake completeness

Reject:

- placeholder public fields;
- `todo!`, `unimplemented!` or unconditional success in reachable stable code;
- empty vectors or zero values representing unavailable science;
- NaN/infinity persistence;
- silent fallbacks;
- broad error swallowing;
- “best effort” parsing that guesses units, IDs, hierarchy, transforms or provenance;
- a result family made public before its oracle/calibration path exists.

**Gate:**

```bash
rg -n 'todo!\(|unimplemented!\(|unwrap_or\(0(\.0)?\)|f64::(NAN|INFINITY|NEG_INFINITY)' src
```

Matches are reviewed against typed-state policy. The existing finite-boundary tests remain mandatory.

## 10.6 No fake tests

A valid scientific test must check behavior, not construction.

Reject tests that:

- assert only that a type can be instantiated;
- reproduce the implementation formula in test code;
- snapshot unsupported or unstable behavior;
- use the same library/kernel for expected and observed values;
- report success without running the public production path;
- ignore failed simulation replicates;
- use one convenient random seed as “calibration.”

Every stable method requires:

- hand-computable edge cases;
- independent oracle fixtures;
- degenerate-state tests;
- deterministic seed/thread tests;
- simulation calibration;
- real-data validation;
- a performance workload.

## 10.7 No scientific vocabulary without a contract

A public scientific name is prohibited until the registry records:

- canonical definition and citation;
- estimand;
- normalization;
- window;
- edge correction;
- null/alternative;
- permitted randomization unit;
- undefined states;
- independent oracle;
- validation and benchmark path;
- limitations;
- result schema.

This rule specifically covers K, L, g, Moran, Geary, LISA, variogram, wavelet, diffusion, topology, equivalence, noninferiority, beta-binomial, causal, clone, domain and correspondence terminology.

## 10.8 No unsupported biological claims

- No equivalence from distance or non-significance.
- No causation from association, proximity, co-expression, prediction, attention or importance.
- No measurement label for predictions.
- No biological replication from cells/patches/edges.
- No same-cell claim from registered-section proximity.
- No true correspondence from a transport plan.
- No clonal evolution direction from one cross-sectional section.
- No “generalization” from within-slide splits.

**Gate:** every new result/documentation phrase is entered in `CLAIMS_LEDGER.md` with evidence and approved wording.

## 10.9 No hidden performance or format changes

- No approximate algorithm silently replaces exact behavior.
- No unbounded cache or all-pairs matrix.
- No result field rename/removal/addition without version/migration documentation.
- No unrelated formatting sweep, dependency upgrade or cleanup in a scientific feature commit.
- No benchmark claim without workload shape, output size, hardware/toolchain and exact command.

## 10.10 Completion language

Forbidden completion phrases include:

- “mostly done”;
- “should pass”;
- “appears correct”;
- “works in principle”;
- “validated” without naming the validation;
- “equivalent” without the inferential contract.

A task is **complete**, **blocked**, **failed**, **deferred**, or **rejected**, with evidence.


# 11. Handoff and completion templates

## 11.1 Task contract template

```markdown
# Task Contract — <TASK_ID>: <TITLE>

Base SHA:
Branch/worktree:
Parent requirement IDs:
Owner:
Status: planned | active | blocked | complete | rejected

## Scientific objective
<one testable objective>

## Frozen contract
- Canonical definition:
- Input:
- Window:
- Estimand:
- Null/alternative:
- Edge correction:
- Inhomogeneity policy:
- Permitted randomization unit:
- Undefined states:
- Exact/approx mode:
- Public claim ceiling:

## Scope
Writable files:
Owned canonical symbols:
Read-only context:
Explicit non-goals:

## Invariants
1.
2.
3.

## Red behavior first
Test name:
Input fixture:
Expected behavior:
Independent oracle and provenance:

## Required implementation boundary
<what this task owns and what it must call rather than duplicate>

## Required verification
- Unit:
- Integration:
- Schema/API:
- Determinism:
- Calibration:
- Benchmark:
- Migration:
- Exact commands:

## Stop/escalation conditions
1.
2.
3.

## Expected commit shape
<files, conceptual change, no unrelated work>
```

## 11.2 Task handoff template

```markdown
# Task Handoff — <TASK_ID>

Status: complete | blocked | failed | rejected
Base SHA:
Final SHA:
Branch/worktree:
Owner:

## Scope delivered
Parent requirement IDs:
Scientific behavior:
Explicit non-goals preserved:

## Changed files
- <path>: <reason>

## Changed canonical symbols
- <symbol>: <old contract -> new contract>
Registry updated: yes/no

## Commands actually run
| Command | Exit/result | Tests run/passed/skipped | Notes |
|---|---|---|---|

## Tests not run
- <test/command>: <reason and risk>

## Numerical/scientific evidence
Oracle:
Tolerance:
Simulation/calibration:
Real-data validation:
Determinism:
Undefined-state coverage:

## Performance evidence
Hardware/toolchain:
Workload:
Before:
After:
Peak memory/allocations:
Assessment:

## API/config/CLI/schema/artifact changes
<exact fields, versions and migration behavior>

## Assumptions
1.

## Decisions
- <DEC-ID>: <summary>

## Known risks
1.

## Remaining work
1.

## Next exact action
<one command or edit>

## Scope audit
- Unrelated files changed: no / list
- Dependency changes: no / exact decision
- Formatting sweep: no / explain
- Hidden result-format change: no
- Duplicate canonical implementation introduced: no
- Unsupported claim introduced: no
- Worktree status:
```

## 11.3 Phase-closure template

```markdown
# Phase Closure — <PHASE_ID>: <TITLE>

Entry SHA:
Exit SHA:
Date:
Lead:

## Requirements
| ID | Status | Closure evidence | Residual risk |
|---|---|---|---|

## Interface and architecture audit
- Frozen interfaces:
- Canonical symbols added/changed:
- Dependency direction:
- Files requiring follow-up split:
- Rejected scope:

## Verification
| Gate | Exact command | Result |
|---|---|---|

## Scientific validation
- Independent oracles:
- Calibration:
- Power:
- Coverage:
- Real data:
- External replication:
- Claims approved/rejected:

## Performance
- Workloads:
- Time:
- Memory:
- Exact/approx:
- Regressions and dispositions:

## Schema/API/migration
- Version changes:
- Compatibility:
- Rejected conversions:
- Documentation:

## Deferred/rejected items
- ID:
- Reason:
- Remaining risk:
- Re-entry criterion:

## Next phase prerequisites
1.
```

## 11.4 Compaction-checkpoint template

```markdown
# Compaction Checkpoint

Timestamp:
Current SHA:
Branch/worktree:
Current phase:
Current task/requirement IDs:

## Git state
git status --short:
git diff --stat:
Dirty files and reasons:

## Current contract
Owned files:
Owned symbols:
Scientific invariant:
Current red/green behavior:

## Commands actually run
| Command | Result |
|---|---|

Known failures:
Pre-existing failures:
Tests not run:

## Decisions and ledgers updated
- STATUS:
- REQUIREMENTS:
- DECISIONS:
- CANONICAL_SYMBOLS:
- VALIDATION_LEDGER:
- PERFORMANCE_LEDGER:
- CLAIMS_LEDGER:

## Next exact action
Command:
Then edit:
Expected result:
Stop condition:
```

## 11.5 Final implementation report template

```markdown
# Marklab Implementation Report

Master-plan version:
Audited base SHA:
Implementation base SHA:
Final SHA:
Branch:
Release/result-schema target:
Worktree clean: yes/no

## Executive decision
<what is now scientifically supported; what remains unsupported>

## Requirements
| ID | Final status | Evidence | Public effect | Residual risk |
|---|---|---|---|---|

## Changed architecture
<module ownership and dependency direction>

## Public API/config/CLI/result/artifact changes
<exact list>

## Migration
<accepted, rejected and manual rerun paths>

## Exact verification results
| Command | Result |
|---|---|

## Tests not run
<exact list and consequences>

## Numerical validation
<oracles, tolerances, calibration, power, coverage, deterministic behavior>

## Real-data and external validation
<datasets, hierarchy, endpoints, limitations>

## Performance and memory
<hardware, workloads, exact/approx, regression assessment>

## Scientific claim audit
| Claim | Evidence class | Approved wording | Prohibited wording |
|---|---|---|---|

## NO-AI-SLOP audit
- Canonical registry complete:
- No duplicate implementations:
- No god workflow expansion:
- No placeholders/sentinels:
- No hidden fallbacks:
- No unsupported names/claims:
- Scope audit:

## Remaining risks
1.

## Next exact release action
<one action>
```

A handoff missing base/final SHA, changed files/symbols, exact command results, tests not run, assumptions, decisions, risks, remaining work, next action or scope audit is invalid and must be rejected.


# 12. Final validation and definition of done

## 12.1 Gate matrix

| Gate | Every task | Phase boundary | Nightly/scheduled | Before stable public result family |
|---|---:|---:|---:|---:|
| `cargo fmt --check` | Yes | Yes | — | Yes |
| Warnings-denied Clippy for affected/all features | Affected feature set | All targets/all features | — | Yes |
| Narrow unit/integration tests | Yes | Yes | — | Yes |
| Full all-feature Nextest | When risk warrants | Yes | — | Yes |
| Documentation tests | If docs/API change | Yes | — | Yes |
| No-default build | If domain/public code changes | Yes | — | Yes |
| Feature-combination checks | Affected combinations | Required matrix | — | Yes |
| WSI integration | If WSI/geometry/units touched | Yes under current CI | Scheduled external fixture | If affected |
| Schema round trips/unknown-field rejection | If result/artifact changes | Yes | — | Yes |
| 0.3 compatibility and migration rejection | If schema changes | Yes | — | Yes |
| Dependency audit/deny/machete/package | If dependency changes | Yes | Scheduled advisories | Yes |
| Fuzz build | If parser/geometry/schema changes | Yes | Fuzz execution campaign | Yes |
| Seed/thread determinism | If stochastic/parallel | Yes | Multi-platform repeat | Yes |
| Independent numerical oracle | Scientific task | Yes | — | Mandatory |
| Smoke simulation | Scientific task | Yes | — | Mandatory |
| Formal type-I/power/coverage | — | Selected phase exit | Yes | Mandatory |
| Real-data face validity | — | Selected phase exit | — | Mandatory |
| External cohort/replication | — | — | As datasets permit | Required for broad biological claims |
| Criterion smoke benchmark | Hot path task | Yes | — | Yes |
| Large scaling and memory | — | Phase where method stabilizes | Yes | Mandatory |
| Public API/semver audit | If public | Yes | — | Mandatory |
| Scientific claims ledger | If wording/result changes | Yes | — | Mandatory |
| NO-AI-SLOP/scope audit | Yes | Yes | — | Mandatory |

## 12.2 Exact current repository gates

Run the repository’s current gates, using the pinned toolchain:

```bash
cargo +1.96.0 fmt --all --check
cargo +1.96.0 clippy --locked --all-targets --all-features -- -D warnings
cargo +1.96.0 nextest run --locked --all-features
cargo +1.96.0 test --locked --doc --all-features
cargo +1.96.0 check --locked --no-default-features
cargo +1.96.0 test --locked --features wsi,cli --test wsi_integration

cargo audit
cargo deny check advisories licenses bans sources
cargo machete
cargo package --locked

cargo +nightly fuzz check

env MARKLAB_BENCH_PROFILE=smoke \
  cargo +1.96.0 bench --locked --all-features -- --quick

cargo +1.96.0 test --locked --no-default-features \
  --features dhat-heap --lib dhat_ -- --test-threads=1
```

The implementation lead must refresh these commands from the actual implementation-base CI before use; this plan does not authorize deleting or weakening a gate.

## 12.3 Formatting, compilation and feature combinations

At phase boundaries, compile at least:

- no default features;
- default features;
- all features;
- `csv` without `parquet`;
- `parquet` without `csv` where supported;
- `wsi,cli`;
- `dhat-heap` in its supported no-default combination;
- new `experimental` feature separately and never as a stable-core dependency.

A feature may gate an adapter or heavy optional dependency. It may not gate the existence of a stable scientific definition needed by library users.

## 12.4 Unit and integration tests

Every new stable statistic requires:

1. a hand-computable fixture;
2. an independent external oracle;
3. degenerate and undefined states;
4. exact serialization behavior;
5. public application-path integration;
6. CLI/library parity if exposed in CLI;
7. deterministic seed/thread behavior;
8. no hidden allocation or geometry rebuild in declared hot loops where material.

Current integration suites remain regression gates. New services add focused tests rather than expanding unrelated current engine fixtures.

## 12.5 Schema and artifact round trips

For each new result/artifact:

- strict JSON result round trip;
- unknown result kind/version/field rejection;
- Arrow/Parquet schema and metadata validation;
- identity and row-order preservation;
- digest validation;
- missingness preservation;
- no NaN/infinity;
- no matrix embedded in JSON;
- transactional writer failure leaves no final partial directory;
- result and manifest reference exactly the written artifacts;
- migration tests state what is rejected and why.

## 12.6 Determinism

For each stochastic/parallel endpoint:

- exact equality across repeated runs with the same seed and thread count;
- exact equality across supported thread counts where the algorithm promises it;
- fixed endpoint seed namespace;
- stable ordering of IDs, pairs, bins, labels and artifacts;
- deterministic tie policy;
- explicit nondeterminism declaration for external tools that cannot guarantee it.

Approximate methods still require deterministic seeded behavior.

## 12.7 Calibration, power and coverage

### Minimum formal design

For each inferential stable method:

- at least 1,000 null replicates for scheduled screening; use more when CI width is inadequate;
- report attempted, completed and failed replicates;
- Wilson or exact interval for type-I error;
- power curves across effect size, n, density, window shape and biological replicate count;
- interval coverage for estimators with CIs;
- multiplicity-family calibration for local/multiscale endpoints;
- separate smoke and formal calibration artifacts.

Required generators, as applicable:

- homogeneous and inhomogeneous Poisson;
- random labeling;
- Thomas and Matérn clusters;
- Strauss/Gibbs inhibition;
- LGCP reference data from an external generator;
- multitype attraction/repulsion;
- compartment-confounded patterns;
- irregular/holed/boundary-heavy windows;
- duplicate coordinates and sparse/rare labels;
- continuous/probabilistic marks;
- correlated high-dimensional embeddings;
- domain/scanner shift;
- registration/segmentation error;
- known clone geometries;
- paired/multiregion/multisite cohort structures.

A method failing calibration is not “experimental stable.” It remains blocked until the cause is understood or is rejected.

## 12.8 Real-data validation

Stable release needs:

- one dataset for numerical/operational face validity;
- one independent dataset or cohort for any broad biological claim;
- documented patient/specimen/slide/core hierarchy;
- acquisition/stain/scanner/site metadata;
- prespecified endpoints and scales;
- positive and negative controls;
- failed/undefined specimens shown, not discarded;
- comparison with at least one established external implementation where applicable.

Embedding methods additionally need scanner/site/stain/model-version confounder tests.

## 12.9 Benchmark and memory gates

Every performance report records:

- exact SHA, hardware, OS, compiler, features and threads;
- cell/patch/patient counts;
- spatial density, window, maximum radius and returned pairs/edges;
- embedding dimension;
- permutations and null family;
- exact/approx mode and approximation parameters;
- wall time, peak RSS and allocations where material;
- before/after equivalent-work assessment.

Proposed acceptance:

- no equivalent default workload regresses >20% without an accepted decision;
- fixed-density bounded-radius exact methods must not show unexplained quadratic growth before output size requires it;
- geometry/null plans rebuild zero times per permutation;
- approximate mode reports exact-subset error;
- 1M-cell stable workloads complete within the configured memory budget on the documented reference machine;
- a 100M-cell claim is prohibited until an actual out-of-core benchmark exists.

## 12.10 Documentation and public API

Before public release:

- canonical definition/formula and references;
- scientific question and claim ceiling;
- window, edge correction, intensity and null;
- permitted randomization unit;
- assumptions and failure modes;
- typed unavailable states;
- complexity/memory;
- exact/approx behavior;
- example with real units;
- result fields and artifact schemas;
- migration and compatibility;
- validation and benchmark evidence;
- measured versus predicted semantics;
- unsupported causal/equivalence language audited.

## 12.11 Scientific claim audit

The release lead reviews:

- every result type/field name;
- README/SPEC/public API docs;
- CLI help;
- reports and example interpretations;
- migration notes;
- benchmark/calibration summaries.

The claim audit answers:

1. Is this measurement, description, within-pattern inference, cohort inference, equivalence, prediction or causation?
2. What is the biological replicate?
3. What null/estimand supports the wording?
4. Is the value measured or predicted?
5. What uncertainty and undefined states exist?
6. What cannot be concluded?

## 12.12 Definition of done

A stable public result family is done only when:

- its requirement and interface contract are frozen;
- one canonical implementation exists;
- independent oracle agreement passes;
- simulation calibration/power/coverage gates pass;
- real-data validation is documented;
- exact/approx and memory behavior are bounded;
- result/artifact schema and migration are strict;
- public API, config and CLI agree;
- reproducibility/provenance is complete;
- no unsupported claim remains;
- every exact command/result and every test not run are in the closure report;
- the final diff passes a scope and NO-AI-SLOP audit.


# 13. Validation and performance program

## 13.1 Validation asset layout

Proposed repository layout:

```text
validation/
    README.md
    fixtures/
        point_process/
        autocorrelation/
        embeddings/
        geometry/
        cohort/
    generators/
        r_spatstat/
        python_reference/
    simulations/
        manifests/
    real_data/
        manifests/
        LICENSES.md
```

Checked-in fixtures contain small inputs and expected outputs. Large datasets remain external with immutable digests and retrieval instructions. Every generator records environment, versions, seed, command and license.

## 13.2 Smoke versus formal validation

- **Smoke:** tens of replicates, fast, runs on PR/main, catches wiring and gross sign/normalization errors.
- **Formal calibration:** scheduled, at least 1,000 replicates and enough to bound error; reports confidence intervals and failures.
- **Power/coverage:** scheduled or release-candidate; not merged into a generic “validation passed” flag.
- **External replication:** release evidence for biological claims, not a CI job.


## 13.3 Predictive embedding protocol

Predictive work remains external, but Marklab must define the evaluation contract and ingest its results.

| Model | Inputs | Incremental comparison | What a gain can establish | What it cannot establish |
|---|---|---|---|---|
| M0 | Technical, clinical, compartment and acquisition covariates | Baseline | Predictability from known non-morphology covariates | Morphological or spatial value |
| M1 | Index-cell CellViT embedding | M1 vs M0 | Incremental morphology representation value | Spatial-context value, causation or molecular measurement |
| M2 | Spatial neighborhood context only | M2 vs M0 | Incremental local-composition/geometry value | Cell-intrinsic morphology value |
| M3 | Cell embedding + neighborhood | M3 vs max(M1,M2) | Complementarity of intrinsic morphology and neighborhood | Patch-level architecture value |
| M4 | M3 + patch embedding at prespecified physical scales | M4 vs M3 | Incremental broader context value | Mechanism or generalization outside evaluated domains |
| M5 | M4 + independently measured IHC/molecular values | M5 vs M4 | Incremental measured multimodal value | That morphology predicted the measured modality, unless a separate prediction target is evaluated |

A parallel cell/patch ablation may name M1=cell, M2=patch, M3=cell+patch, M4=cell+patch+neighbors and M5=plus independently measured data. The contract must state which naming is used; results from the two sequences are not merged by label alone.

Primary evidence requires patient-held-out nested evaluation. Report separately:

- within-slide interpolation: technical diagnostic only;
- cross-slide generalization;
- cross-patient generalization: minimum primary claim;
- cross-site and cross-institution generalization;
- cross-scanner and resolution generalization;
- stain/preprocessing generalization;
- cross-tumor-type transfer.

All normalization, whitening, PCA, clustering, scale selection, feature selection and tuning occur inside training folds. For specimen-level molecular/outcome targets, spatial information is aggregated to a prespecified specimen fingerprint and inference is at patient level.


## 13.4 Input-size tiers

| Tier | Cells/patches | Purpose |
|---|---:|---|
| Tiny | 0–100 | Exact/hand oracle and degenerate behavior |
| Small | 1,000–10,000 | PR integration and quick Criterion |
| Medium | 100,000 | routine performance and memory regression |
| Large | 1,000,000 | stable pathology-scale acceptance |
| Very large | 10,000,000 | scheduled out-of-core/streaming experiments |
| Frontier | 100,000,000 | no claim until a real workload is executed |

Cohort benchmarks separately vary patients, specimens per patient, regions per specimen and endpoints per fingerprint. A million cells in one patient is not a cohort benchmark.

## 13.5 Fixed-density and output-sensitive scaling

For spatial queries, hold density and maximum physical radius constant while increasing window area. Report:

- n;
- window area;
- density;
- radius;
- pairs/edges returned;
- build/evaluate/null timings separately;
- memory for index, retained plan, scratch and output.

A method may be O(n²) in the pathological case when the output contains O(n²) eligible pairs. The performance claim must therefore be output-sensitive, not merely “subquadratic.”

## 13.6 Embedding storage and computation

Raw f32 matrix sizes:

| Shape | Raw matrix |
|---|---:|
| 1M × 128 | 0.512 GB |
| 1M × 256 | 1.024 GB |
| 1M × 768 | 3.072 GB |
| 10M × 256 | 10.24 GB |

These exclude IDs, validity, Arrow buffers, indexes and scratch. Therefore:

- block iteration is mandatory;
- pairwise full distance matrices are prohibited;
- f64 accumulation uses bounded block scratch;
- dimensionality reduction is an external/training-fold artifact, not an automatic load-time transformation;
- local maps stream to artifacts;
- memory mapping is evaluated after a concrete caller and portability test.

## 13.7 Graph and patch memory

Report:

- graph nodes/edges and weight dtype;
- symmetric versus directed storage;
- isolated nodes/components;
- patch count/dimension;
- links per cell and cells per patch;
- overlap and effective independent patch count;
- retained versus streamed neighbor plans.

A shared patch is stored once. A cell-patch link is a row in a relation, not a repeated vector.

## 13.8 Deterministic parallelism

- Partition by stable index ranges or endpoint seed namespaces.
- Reduce floating-point sums in a documented deterministic order where exact cross-thread equality is promised.
- If deterministic parallel reduction is too costly, strict-repro mode uses one thread and normal mode declares the bounded numerical tolerance. Stable p-values/decisions must not change within that tolerance.
- External GPU results record framework, hardware, deterministic flags and residual nondeterminism.

## 13.9 Streaming and out-of-core

Priority order:

1. stream input decoding;
2. block embedding scans;
3. stream local result artifacts;
4. stream pair contributions when retained pair plans exceed budget;
5. partition by window component/region when the estimand permits;
6. distributed multi-slide cohort aggregation only after deterministic merge semantics exist.

Do not distribute a single inferential permutation workflow before the exact merge and seed order are defined.

## 13.10 Approximate modes

Possible approximate research modes:

- sampled eligible pairs with confidence/error estimates;
- ANN neighborhoods with recall measured against exact subsets;
- polynomial graph filters with spectral approximation bounds;
- region-level aggregation before OT;
- progressive descriptive curves.

They are not allowed for stable inference until simulation shows calibrated type-I error and an exact fallback exists. Result names and schema must expose approximation.

---



## 13.11 Bayesian validation program

Every Bayesian model family must include:

- prior predictive simulation;
- simulation-based calibration where simulation is possible;
- parameter and latent-field recovery under known generative models;
- rank-normalized R-hat and effective sample size;
- divergence/treedepth/energy diagnostics for HMC;
- ELBO and importance/held-out checks for VI;
- Pareto-smoothed LOO diagnostics when used;
- posterior predictive spatial summaries;
- prior sensitivity;
- cross-backend agreement on tractable small cases;
- failure-state fixtures;
- reproducibility across chains, seeds, and supported hardware;
- explicit approximate-inference status.

A Bayesian fit is not successful merely because a backend returned samples.

## 13.12 Multimodal and predictive validation

Require:

- patient-held-out nested validation;
- external-site cohorts;
- scanner, stain, resolution, institution, platform, and model-version stress tests;
- adjacent-section, overlapping-patch, patient, and institution leakage checks;
- calibration and uncertainty decomposition;
- OOD detection and abstention;
- M0–M5 incremental-value comparisons;
- measured versus predicted modality separation;
- subgroup and missing-modality robustness;
- survival/competing-risk evaluation when applicable.

## 13.13 Generative and SBI validation

Require:

- known-simulator calibration;
- held-out posterior predictive summaries;
- coverage and SBC;
- mode-collapse and diversity metrics;
- nearest-neighbor/memorization tests against training data;
- privacy and leakage analysis;
- sensitivity to simulator misspecification;
- comparison with simpler point-process and mechanistic models;
- clearly bounded training support and OOD behavior.

## 13.14 Nature-grade evidence program

The platform reaches publication-grade maturity through evidence, not feature count. The implementation program must create:

1. **Numerical validation paper assets:** independent reference fixtures, cross-language agreement, edge cases, and exact/approximate error.
2. **Calibration benchmark assets:** type-I error, power, coverage, SBC, posterior predictive behavior, and technical-confounder simulations.
3. **Scalability benchmark assets:** one-million and larger cell/patch workflows, fixed-density scaling, GPU/CPU comparisons, and memory profiles.
4. **Multimodal demonstration cohorts:** H&E + IHC, multiplex, spatial omics, clone/CNA, and clinical examples with patient-level replication.
5. **External validation:** multiple institutions/scanners/stains/platforms and independent investigators.
6. **Ablation studies:** classical versus Bayesian versus graph/learned components and cell versus patch versus combined models.
7. **Reproducible capsules:** exact data manifests, environments, workflows, results, and figures.
8. **Usability evidence:** computational pathologist workflows from import to publication output.

No document can guarantee a Nature publication. The architecture and validation program should make the platform capable of producing work at that standard.

# 14. Result-format evolution

## 14.1 Version policy

- Format 0.3 remains the stable contract for current marked, multimodal and pre/post results.
- Format 0.4 begins with the first stable new result family, likely `point_process`.
- Adding only a new artifact schema does not automatically require a result-version bump if the stable result envelope already supports a versioned `ArtifactRef`; introducing that support itself does.
- Experimental outputs use their own stability marker and may be intentionally incompatible between minor releases.

## 14.2 Proposed result families

| Family | Stable contents | Large artifacts |
|---|---|---|
| `point_process` | K/L/g/F/G/J summaries, corrections, nulls, global inference | full curves if large, intensity fields |
| `marked_spatial` | mark-connection/correlation/weighted-K summaries | local/type-pair maps |
| `spatial_signal` | Moran/Geary/variogram/covariance summaries | local LISA/maps, residual fields |
| `pathology_geometry` | interface/infiltration/contact summaries | boundaries, signed-distance/local maps |
| `cohort_comparison` | design, effect, CI, p, fingerprint compatibility | fingerprints and patient-level endpoint table |
| `equivalence` | endpoint, margins, CI, two one-sided tests, decision | protocol/margin evidence |
| `embedding_spatial` | metric/kernel, transformation/provenance, omnibus curves/tests | embedding table and local diversity/maps |

## 14.3 Stable versus experimental outputs

Stable results:

- have fixed semantics and migration policy;
- use closed enums;
- reject unknown fields;
- include full provenance and claim ceiling;
- have independent oracle/calibration evidence.

Experimental outputs:

- name the exact method/version;
- include `stability = experimental`;
- do not appear in stable aggregate interpretation;
- may require rerun after schema change;
- cannot be silently promoted by renaming a field.

## 14.4 Typed availability

Required unavailable reasons include, as applicable:

- insufficient points/pairs/types/patients;
- zero variance/intensity;
- invalid/unsupported window;
- radius outside support;
- missing or incomplete provenance;
- incompatible measurement status;
- degenerate null/no exchangeable units;
- budget exceeded;
- approximation not permitted;
- external solver non-convergence;
- registration uncertainty exceeds scale.

An unavailable result is not numeric zero, NaN, infinity, an empty curve or a success status.

## 14.5 Compatibility limits

Do not convert:

- current mark-pair covariance to g;
- current cross pair-count curves to cross-K/cross-g;
- current residual energy to wavelet coefficients;
- current density territories to general niches/domains;
- current pooled-bin comparison to patient-level difference;
- current margins to equivalence;
- imported predictions to measurements;
- registered proximity to same-cell correspondence.

When semantics cannot be recovered, reject and require reanalysis.

---

# 15. Methods, claims, and product directions that are prohibited or gated

The frontier program is intentionally broad. This section rejects incoherent implementation and unsupported claims rather than rejecting complexity itself.

| Item | Decision | Reason |
|---|---|---|
| Predicted molecular maps represented as measurements | Prohibited | Prediction is a separate modality with uncertainty and provenance. |
| Cell/patch/edge rows used as patient-level replicates | Prohibited | Violates biological replication and inflates evidence. |
| Causal communication from proximity, co-expression, attention, or importance | Prohibited | No identified intervention or counterfactual estimand. |
| Non-significance called equivalence | Prohibited | Requires prespecified margins and two-sided equivalence design. |
| Transport plan called cell correspondence | Prohibited without external evidence | Optimal plans are generally non-identifiable biological mappings. |
| Bayesian posterior reported without diagnostics | Prohibited | Samples may be nonconverged, divergent, or prior-dominated. |
| Approximate inference silently replacing exact | Prohibited | Execution mode and error/calibration must be visible. |
| Experimental output included in a validated report without a maturity badge | Prohibited | Prevents users from distinguishing evidence from exploration. |
| Native reimplementation solely for branding | Gated | Requires unique performance, integration, validation, or scientific advantage. |
| Deep model before simple baseline | Gated | Must demonstrate incremental patient-held-out value over linear/generalized models. |
| “Digital twin” claim | Research-only until prospective evidence | Visual realism or posterior simulation is insufficient. |
| One opaque spatial score | Reject | Obscures estimands, scales, uncertainty, and failure. |
| Generic arbitrary plugin returning JSON | Reject | Violates type, provenance, security, and schema contracts. |
| Big-bang uncharacterized rewrite | Reject | Full transformation is allowed only through controlled replatforming and parity gates. |
| Decorative wavelet/DoG/Bartlett/beta-binomial terminology | Reject | Genuine implementations may be added as distinct validated endpoints. |
| Clinical decision support without regulatory/clinical validation | Unsupported for claim | Research platform outputs are not automatically clinical tools. |

# 16. Five-year frontier strategy

## 16.1 What Marklab should become uniquely best at

Marklab should be best at the **combination** of:

1. exact and trusted spatial statistics;
2. hierarchical Bayesian spatial inference;
3. cell/patch/region foundation-model representation analysis;
4. multimodal pathology and molecular integration;
5. graph, topology, geometry, and tissue-interface mathematics;
6. simulation, SBI, and generative tissue models;
7. cohort-valid, multisite, uncertainty-aware evidence;
8. one composable and reproducible computational-pathology workbench;
9. million-cell and larger execution with explicit exact/approximate modes;
10. clear separation of validated evidence, experimental discovery, prediction, and unsupported causal claims.

## 16.2 Frontier bets

### Bet 1 — Bayesian multimodal spatial tissue models

Jointly model morphology, CellViT vectors, patch context, measured IHC/omics, clones, compartments, registration uncertainty, and patient hierarchy. This is a durable scientific differentiator if posterior calibration and external validation are rigorous.

### Bet 2 — Irregular-geometry graph and higher-order tissue mathematics

Graph spectral methods, diffusion/wavelets/scattering, heterogeneous graphs, hypergraphs, complexes, and topology are better matched to cells and tissue entities than generic raster transforms. The advantage must be demonstrated against classical baselines.

### Bet 3 — Generative mechanistic spatial pathology and SBI

Fit mechanistic and neural tissue simulators through simulation-based inference, use them for posterior predictive analysis, power, hypothesis testing, and prospective design, and maintain strict limits on digital-twin or causal language.

### Bet 4 — Multiscale CellViT and patch representation science

Use single-cell and multiscale patch embeddings as spatial marks, not merely classifier inputs. Build robust patient-held-out representation statistics, retrieval, cross-modal latent models, and uncertainty-aware predictions.

### Bet 5 — Cross-sample atlas, clone, and evolutionary mapping

Integrate partial/unbalanced transport, graph matching, reference-query mapping, clone phylogeography, and 3-D/longitudinal models with explicit uncertainty and non-identifiability.

## 16.3 Year-by-year roadmap

### Year 0–1 — Replatform and establish scientific foundations

- workspace/project/workflow/backend architecture;
- identity/cohort/coordinate/artifact/modalities;
- CellViT and patch embedding recovery;
- exact windows/geometry;
- classical point-process suite;
- spatial signal and pathology geometry;
- initial UI and Python/R access;
- model IR, Stan/Python backend, diagnostics, first hierarchical Bayesian models.

### Year 1–2 — Bayesian and multimodal core

- GP/GMRF/CAR/SAR and Bayesian point processes;
- multimodal latent factors;
- continuous IHC and spatial omics;
- clone/CNA downstream analysis;
- nonrigid registration integration;
- patient-level cohort inference, fingerprints, equivalence;
- external-site validation and benchmark release.

### Year 2–3 — Embedding and tissue-architecture intelligence

- complete CellViT/patch M0–M5 program;
- multiscale niches and domains;
- heterogeneous graphs and graph spectral methods;
- topology and morphology endpoints;
- retrieval, atlas mapping, and multimodal prediction;
- workbench maturity and collaborative server execution.

### Year 3–4 — Generative, SBI, 3-D, and longitudinal platform

- canonical simulators and power tools;
- mechanistic-neural hybrids;
- neural point processes, flows, diffusion;
- amortized inference and SBC;
- 3-D serial-section and longitudinal models;
- clone evolution and treatment-response modeling.

### Year 4–5 — Frontier integration and stable 1.0 ecosystem

- causal/interference and perturbation laboratory for appropriate designs;
- active experimental design;
- federated/multisite analysis;
- distributed 10M–100M cell/patch workflows where scientifically necessary;
- mature plugin/backend ecosystem;
- benchmark and methods publications;
- stable project/workflow/result/artifact schemas;
- long-term support and governance.

## 16.4 Promotion gates into the stable public schema

A method moves from research to experimental, established, or validated only after its tier-specific evidence is complete. Stable promotion requires, as applicable:

- canonical definition and claim ceiling;
- trusted independent numerical oracle;
- deterministic behavior or documented stochastic variation;
- type-I error/power/coverage or SBC;
- posterior/optimization diagnostics;
- exact/approximate error characterization;
- real-data face validity;
- patient-level and external-site validation;
- technical-confounder stress tests;
- runtime/memory acceptance;
- stable API/result/artifact semantics;
- migration documentation;
- security/license/backend review;
- claims-ledger approval.

## 16.5 Final scientific thesis

> **Marklab will uniquely unite classical spatial evidence, Bayesian generative modeling, multimodal foundation-model representations, pathology geometry, cohort inference, and frontier mathematical exploration in one provenance-complete computational-pathology operating system—capable of running a single trusted statistic or an integrated multimodal research program without sacrificing scientific meaning.**


# 17. Evidence index, unresolved items and audit limitations

## 17.1 Repository evidence map

The audit was pinned to `55fce12f10684a9081ca1f744f87d6f5feedcb24`.

Principal current files inspected:

- `Cargo.toml`, `rust-toolchain.toml`, `deny.toml`;
- `.github/workflows/ci.yml`, `calibration.yml`, benchmark/release/WSI workflows;
- `README.md`, `SPEC.md`, `docs/public-api.md`, `docs/result-format-0.3.md`, `docs/migration-0.2-to-0.3.md`, `docs/validation-methodology.md`;
- `src/lib.rs`, `src/api.rs` and its stages;
- `src/config/*`, `src/data/*`, `src/geom/*`, `src/inference/*`, `src/permutation/*`;
- `src/spectra/*`, `src/periodogram/*`, `src/multiscale_residual/*`;
- `src/multimodal/*`, `src/neighborhood/*`, `src/registration/*`;
- `src/prepost/*`, `src/comparison/*`;
- `src/io/*`, `src/output/*`, `src/cli*`, optional WSI;
- integration tests, fixtures and Criterion benches;
- examples and feature wiring.

Historical parent-commit records inspected as context only:

- `docs/refactor/MASTER_PLAN.md`
- `STATUS.md`
- `DECISIONS.md`
- `FINDINGS_MATRIX.md`
- `COMPLETION_AUDIT.md`
- `PERFORMANCE_BASELINE.md`
- `PERFORMANCE_FINAL.md`
- `CLOSURE_REPORT.md`

Those records are not proof that the pinned HEAD was executed.

## 17.2 Selected primary and official evidence

### Point processes and inference

- Ripley, B.D. (1976/1977), second-order analysis of stationary point processes.
- Baddeley, Møller and Waagepetersen, inhomogeneous pair-correlation/K methodology.
- `spatstat` 3.6-2 and current `spatstat.model` official manuals.
- Myllymäki et al. (2017), global envelope tests, *JRSS B*, DOI `10.1111/rssb.12172`.
- Moran (1950), spatial autocorrelation.
- Geary (1954), contiguity ratio.
- Anselin (1995), local indicators of spatial association.
- Getis and Ord (1992), local spatial statistics.
- Gretton et al. (2012), kernel two-sample test/MMD, *JMLR*.
- Székely and Rizzo (2013), energy statistics.
- Lakens (2017), equivalence testing/TOST.
- Saravanan, Berman and Sober (2020), hierarchical bootstrap.
- Møller, Syversveen and Waagepetersen (1998), log-Gaussian Cox processes.

### Spatial-pathology and spatial-omics ecosystems

- Squidpy official 1.8.3 release/documentation.
- SPIAT and spaSim Bioconductor 1.14.0 documentation.
- Giotto Suite 4.2.3 and its 2025 methods paper.
- BANKSY 2024 paper and 2026 reference releases.
- MISTy/mistyR 1.20.0.
- LIANA+ 1.8.1 and its 2024 framework paper.
- UTAG, CytoMAP and CytoCommunity original papers; CytoCommunity hierarchical extension (2026) treated as emerging.
- SpatialDE official Bioconductor release.
- SpatialData 0.8.0, AnnData 0.13.2, OME-NGFF 0.5, Apache Arrow and Parquet official specifications.

### Alignment, clones and representations

- PASTE (2022), PASTE2 partial-overlap alignment.
- STalign (2023), diffeomorphic alignment.
- GPSA research implementation.
- STARCH, SpaCNA (2026) and CalicoST reference implementations.
- CellViT (2024) and CellViT++ emerging work; repository checkpoint history.
- UNI, CONCH, Prov-GigaPath and Virchow 2024 foundation-model papers/releases.
- Scanner/site/stain sensitivity studies from 2025–2026, including preprints explicitly treated as unreplicated where applicable.

### Graph/frontier methods

- Hammond, Vandergheynst and Gribonval (2011), spectral graph wavelets.
- Coifman and Maggioni (2006), diffusion wavelets.
- Graph scattering literature.
- 2024–2026 topology, generative spatial-layout, 3-D reconstruction and active-sampling papers, classified as experimental/watch rather than stable evidence.
- Hudgens and Halloran (2008) and Aronow and Samii (2017) for interference/exposure mappings.

## 17.3 Unresolved evidence

- No local worktree or test execution at the pinned SHA.
- Worktree cleanliness cannot be established from the remote audit.
- Exact release/license pin remains unresolved for selected external projects, including GPSA, some UTAG/CytoMAP packaging and SpaCNA’s repository license. Do not enable adapters until resolved.
- Actual CellViT embedding matrix and provenance were not found in the audited repository.
- Patch-embedding existence is unresolved.
- No cohort manifests, external validation cohorts or paired molecular datasets were supplied.
- No general observation-window polygon artifact was found.
- No benchmark proves current or proposed 100M-cell operation.
- No evidence supports causal claims, same-cell serial-section matching, or genuine equivalence in current Marklab.

## 17.4 Audit conclusion

The repository is not a broken prototype. It is a disciplined, narrow marked-pattern/MMR-IHC engine with unusually strong attention to naming, finite states, reproducibility, memory and output contracts. Its scientific limitation is breadth of estimands and study design, not basic engineering quality.

The correct next move is to begin a controlled replatforming that preserves verified assets while replacing the narrow product architecture. The platform must build the project/workflow/data foundation first, then develop trusted classical and Bayesian engines, make CellViT and patch embeddings primary modalities, integrate external model backends as first-class workflow nodes, and expand into graph, topological, generative, 3-D, longitudinal, causal, and active-design research under explicit maturity tiers. The goal is one coherent computational-pathology operating system, not a loose algorithm catalogue.
