# Marklab program tracker

Last updated: 2026-08-25

Authority: MASTER_PLAN.md remains the immutable, authoritative total program scope. This tracker is a derived execution control. A conflict is resolved in favor of the master plan, and no tracker state removes, weakens, permanently defers, or declares a master-plan item unnecessary.

Master-plan identity: SHA-256 1cdb619edc39d1d3d8c72bdf15930de3a928bf4b90651a05dc481c1238e5f064.

## State contract

Every canonical row uses exactly one of these states:

- complete — the bounded requirement represented by the ID has closure evidence;
- active — implementation is currently advancing the ID;
- ready — prerequisites are satisfied and the ID may be promoted into the active roadmap;
- blocked with named prerequisite — in-repository prerequisite work is not complete;
- data-dependent with named missing data — required external scientific data or metadata are unavailable;
- gated pending explicit user decision — the master plan itself gates or prohibits the direction and promotion requires an explicit user decision in addition to scientific admission evidence;
- planned — retained in total scope but not yet promoted or otherwise blocked.

Complete means the exact bounded capability described in the evidence column, not completion of a broader parent family. Planned is neither deferred nor optional. Data-dependent and gated rows remain in total program scope. A major stabilization checkpoint must update this file from executed evidence, reconsider every blocked or data-dependent row affected by the checkpoint, and promote the next dependency-ordered outcomes into ACTIVE_ROADMAP.md.

The ID column is the canonical coverage key. Each of the 233 master-plan requirement, capability, task, alias, and workstream IDs appears in that column exactly once. Prerequisite references in the evidence column do not create additional tracker rows.

## Requirement and capability coverage

| ID | State | Evidence, named prerequisite, missing data, or gate |
|---|---|---|
| A-01 | complete | Authoritative repository state and control plane installed; implementation ledgers carry closure evidence. |
| A-02 | complete | Baseline reproduced and exact unavailable or failing gates recorded. |
| A-03 | complete | Current API, CLI, config, result, artifact, feature, workflow, and test dispositions inventoried. |
| ACT-01 | active | IC-0132 and IC-0194 provide analytic EIG plus bounded synthetic sequential ROI/stain/landmark/allocation/power workflows; operational actions, calibrated real utility, and prospective outcomes remain promotion gaps. |
| B-01 | complete | Workspace architecture decision and compatibility boundary closed. |
| B-02 | complete | Compatibility shell parity closed. |
| B-03 | complete | Workspace policy and feature-matrix milestone closed. |
| B-04 | complete | Minimal project/workflow vertical slice closed. |
| BACK-01 | active | Static typed PyMC 6.3.0 and POT 0.9.7.post1 workflows now share one closed descriptor and run durably with exact Python-3.12 lock/worker/license/schema/control identity and strict typed replay without Python restart; broader backend classes, capability/security manifests, doctor/admission validation, and the promotion gate remain. |
| BAY-01 | active | One explicit backend-neutral Normal prior/known-sigma likelihood model IR is runnable through PyMC; hierarchy, units beyond one scalar observation, broader priors/likelihoods, identifiability, and cross-backend agreement remain. |
| BAY-02 | active | NUTS, audited SMC, differentiated/nested Laplace, PSIS-LOO/comparison, conjugate SBC, and conjugate posterior/predictive/decision prior sensitivity are runnable; broader sampler/model calibration and inference families remain. |
| BAY-03 | active | Gaussian patient varying intercepts and site/cohort random-effects meta-regression with explicit units, partial pooling, heterogeneity, and predictive checks are runnable; repeated, crossed, non-Gaussian, varying-slope, and spatial hierarchies remain. |
| BAY-04 | active | Exact/multi-output/VFE GP, predictive process, NNGP, and IC-0200 bounded rectangular SPDE are runnable; holed/adaptive SPDE, arbitrary multi-output, anisotropic, nonstationary, and broader fields remain. |
| BAY-05 | active | Stable weights, CAR/GMRF, fixed/fitted SAR, exact-constraint BYM, and scaled-ICAR BYM2 are runnable; broader likelihoods, scale, and calibration remain. |
| BAY-EVO | data-dependent with named missing data | Missing provenance-complete clone probabilities, phylogeny, and replicated longitudinal or 3-D evidence. |
| BAY-FIELD-A | active | Exact 1-D physical Matérn, bounded two-output, VFE, predictive-process, and NNGP foundations are runnable; SPDE is blocked on a promoted exact 2-D window/mesh contract, while general multivariate/anisotropic/nonstationary/calibration work remains. |
| BAY-GMRF-A | active | IC-0047 through IC-0053 cover weights, CAR/GMRF, SAR, exact ICAR, BYM, and scaled BYM2; broader SGLMM and calibration remain. |
| BAY-HIER-A | active | Gaussian known-sigma patient varying intercepts, exact-conjugate nonlinear resource-distance response, and known-SE site meta-regression are runnable; unknown dispersion, Student-t, binomial/count/ordinal/hurdle, crossed/nested levels, random slopes, and broader calibration remain. |
| BAY-MM | active | IC-0165–IC-0177 and IC-0200 cover paired/multiview, hierarchy, matrix/tensor, GP/graph/SPDE spatial, dropout, joint, comparison, and synthetic validation; matched external patient validation remains missing. |
| BAY-NP | blocked with named prerequisite | Requires Bayesian fit infrastructure and reproducible multiscale niche inputs from WS-60. |
| BAY-PP | active | Exact rectangular IPP/grid/SPDE LGCP, cluster/Strauss/Geyer, exact finite exchange/multitype, latent-parent/replicated cluster, marked/embedding constructors, replicated LGCP, and PPC are runnable; broader continuous exchange, spatial PPC, fitted marks, arbitrary windows, and real calibration remain. |
| BAY-PP-A | active | Exact rectangle-grid LGCP plus IC-0200 rectangular SPDE LGCP are runnable with conserved counts; spatial-summary checks, adaptive refinement, calibration, arbitrary windows, and inferred SPDE hyperparameters remain. |
| BAY-PP-B | blocked with named prerequisite | Requires BAY-PP foundation, interaction-process simulation, and backend admission. |
| BAY-REG | blocked with named prerequisite | Requires BACK-01, GEO-01, BAY-01/BAY-02, and uncertainty-bearing transforms. |
| BAY-REG-A | active | One centered exact 1-D Matérn varying-coefficient Gaussian workflow is runnable; GMRF, multiple-coefficient, non-Gaussian, 2-D, and calibration extensions remain. |
| BAY-SBI | active | IC-0117–IC-0121 supply classical SBI, prior-predictive SBC, and reference-fit KNN/conformal simulation OOD; learned SBI remains blocked by `NEURAL-SBI-01`, while posterior-predictive lab and real calibration remain. |
| BULK-01 | data-dependent with named missing data | Missing specimen-linked bulk molecular measurements, outcomes, and patient/site covariates. |
| C-01 | complete | Typed identities and cohort hierarchy closed in the C-01 handoff. |
| C-02 | complete | Units, coordinate frames, dimensions, transforms, and uncertainty references closed. |
| C-03 | complete | Immutable artifact catalog and table/store boundary closed. |
| C-04 | complete | Bounded CellViT cell-embedding table, provenance, physical formats, and scale evidence closed. |
| C-05 | complete | Bounded patch/region/slide tables, links, graphs, receipts, formats, and finalizers closed. |
| C-06 | planned | Thirteen bounded slices are green; broader general marks remain in scope but are not the current increment. |
| CAU-01 | active | IC-0130–IC-0135 and IC-0192–IC-0195 provide exact randomized mechanics, sensitivity, bounded synthetic observational/perturbation estimators, active design, and partial validation; admitted real treatment data and identification remain promotion gaps. |
| CAU-01A | data-dependent with named missing data | Missing genuine treatment assignment, exposure mapping, interference graph, positivity, and identification evidence. |
| CAU-01B | data-dependent with named missing data | Missing exposure/outcome cohorts with measured confounders, overlap, negative controls, and sensitivity variables. |
| CAU-01C | gated pending explicit user decision | The master plan permits hypothesis generation only; causal-evidence promotion is gated and also requires intervention or identification support. |
| CCC-01 | data-dependent with named missing data | Missing matched spatial molecular/protein data and external signaling validation; any output remains hypothesis-generating. |
| CLN-01 | data-dependent with named missing data | Missing provenance-complete clone/CNA assignments, uncertainty, and patient-linked validation cohorts. |
| CLN-01A | data-dependent with named missing data | Missing caller/version/input-provenance-complete clone or CNA probabilities. |
| CLN-01B | data-dependent with named missing data | Missing clone assignments plus compartments/intensity inputs for conditioned spatial nulls. |
| CLN-01C | data-dependent with named missing data | Missing linked clone, immune/stroma/IHC/morphology observations with explicit spatial relation semantics. |
| CLN-01D | active | IC-0129 provides a provenance-labelled exact imported-tree/clone-centroid association specialization with patient/specimen-restricted nulls; real imported phylogenies/evolutionary distances, assignment/topology uncertainty, and replicated validation remain missing. |
| CLN-02 | data-dependent with named missing data | Missing phylogeny and longitudinal/3-D evidence needed to separate association from growth direction. |
| CMP-01 | blocked with named prerequisite | Requires completion of FND-06 and cohort-valid COH-01 execution. |
| CMP-01A | blocked with named prerequisite | Requires stable endpoint families, FND-07 provenance, and COH-01 replication semantics. |
| CMP-01B | blocked with named prerequisite | Requires FND-06 patient/specimen permutation and functional endpoint contracts. |
| CMP-01C | active | Exact complete-fingerprint linear/fixed-RBF MMD with patient permutations is runnable; spatial fingerprint kernels, leakage-safe trained kernel selection, broader validation, and promotion evidence remain. |
| CMP-01D | active | Exact complete-fingerprint Euclidean energy distance with patient permutations is runnable; additional prespecified negative-type metrics, broader validation, and promotion evidence remain. |
| CMP-01E | blocked with named prerequisite | Requires stable graph construction plus cohort-valid evaluation. |
| CMP-01F | active | Whole-patient exact bottleneck/energy permutation mechanics are runnable through IC-0162; a stable pathology filtration, endpoint, and replicated cohort remain missing. |
| COH-01 | active | Scalar, paired, functional, Max-T, MMD/energy, fingerprint/region compatibility, equivalence, noninferiority, and hierarchical bootstrap patient workflows are runnable; repeated-measures and multisite extensions remain. |
| CUR-01 | complete | Pattern metadata/window and CSV/Parquet loading behavior characterized and preserved. |
| CUR-02 | complete | Tumor mask, QC filtering, and summary geometry characterized and preserved. |
| CUR-03 | complete | Fourier structure-factor and shell-summary behavior characterized and preserved at its narrow claim. |
| CUR-04 | complete | Deterministic scalar permutation and ERL behavior characterized and preserved. |
| CUR-05 | complete | Centered mark-pair covariance behavior characterized and preserved without relabeling it as pair correlation. |
| CUR-06 | complete | Directional anisotropy summary characterized and preserved. |
| CUR-07 | complete | Multiscale residual diagnostic characterized and preserved without wavelet/DoG claims. |
| CUR-08 | complete | Pooled, separate, and both-component execution characterized and preserved. |
| CUR-09 | complete | Rigid/affine 2-D landmark registration characterized and preserved. |
| CUR-10 | complete | Registered H&E/IHC fusion characterized and preserved without a correspondence claim. |
| CUR-11 | complete | Graph-edge enrichment/null-sensitivity behavior characterized and preserved. |
| CUR-12 | complete | Cross-label pair-count curves with ERL characterized and preserved without cross-K/g claims. |
| CUR-13 | complete | MMR density components and territory profiles characterized and preserved at their narrow scope. |
| CUR-14 | complete | Marked/multimodal descriptive pre/post behavior characterized and preserved. |
| CUR-15 | complete | Strict result 0.3 finite, typed, and transactional behavior characterized and preserved. |
| CUR-16 | complete | Existing CLI command families characterized and preserved through the compatibility boundary. |
| CUR-17 | complete | Exact R-tree plans, deterministic seeds, and memory-accounting substrate characterized and preserved. |
| CUR-18 | complete | CI, dependency policy, fuzz-build, calibration, and benchmark baseline characterized; later gates remain checkpoint-scoped. |
| DATA-01 | active | Identity, coordinates, artifacts, embeddings, and borrowed scalar identity are implemented; general modalities/marks and durable payload alignment remain. |
| DIM-01 | active | IC-0126 provides strict physical-unit/spacing dimensionality, cuboid-window, anisotropic-metric, and homogeneous 3-D K/L specialization; general 3-D windows/geometry and provenance-complete serial/longitudinal cohorts with section order, thickness, deformation, and missing-section states remain missing. |
| DL-CORE | blocked with named prerequisite | Requires BACK-01 plus licensed, checksum-pinned model manifests and patient/site validation contracts. |
| EMB-01 | blocked with named prerequisite | Exact synthetic vector/projected/kernel curves, within/cross-modal covariance, and complete-vector ERL inference are runnable; stable promotion still requires FND-05 real row-bound provenance, admitted shared geometry/edge correction, technical-confounder calibration, and patient-level validation. |
| EMB-02 | data-dependent with named missing data | Missing promotable real patch/region source identity, scale, provenance, and cell/patch correspondence; synthetic multiscale infrastructure already exists. |
| EMB-CORE | complete | Bounded CellViT single-cell artifact contract and import/format/resource evidence closed by C-04. |
| EMB-PATCH | complete | Bounded synthetic patch/region/slide artifact and explicit-link contract closed by C-05. |
| EMB-PRED-01 | data-dependent with named missing data | Synthetic nested M0–M5, probability calibration, split conformal, shrinkage-Mahalanobis OOD, and prespecified abstention are runnable; real incremental value/transportability require a patient-held-out labeled cohort with matched cell/patch embeddings and technical covariates. |
| EMB-PRED-02 | data-dependent with named missing data | A synthetic calibrated patient-OOF late-fusion baseline with missingness/ablations is runnable; a real matched cohort and demonstrated baseline gap are still required before cross-attention work. |
| EQV-01 | active | Patient-effect TOST equivalence and directional noninferiority with explicit rationale, Student-t interval/bound agreement, and R 4.5.2 oracles are runnable; unpaired/paired-from-raw, functional/bootstrap variants, power guidance, and real clinical validation remain. |
| EQV-01A | complete | Existing descriptive margin is characterized and retained strictly as descriptive. |
| EQV-01B | data-dependent with named missing data | Missing prespecified equivalence margins and sufficient independent biological replicates. |
| EQV-01C | data-dependent with named missing data | Missing directional noninferiority margin, clinical rationale, and sufficient biological replicates. |
| FND-01 | complete | Typed identities, hierarchy, parentage, and design roles closed by C-01. |
| FND-02 | ready | Exact bounded ObservationWindow2D, canonical topology, closed membership, and indexed unsigned boundary distance are complete for the classical caller; signed distance, typed frame binding, compartments, external differential evidence, and broader promotion remain ready work. |
| FND-03 | blocked with named prerequisite | Exact streaming ordered-pair traversal is complete for K/L; full promotion requires completed FND-02 semantics plus an immediate second canonical caller such as g, variograms, or mark functions. |
| FND-04 | planned | Measurement-status vocabulary and concrete consumers exist; general typed marks remain in total scope after the classical increment. |
| FND-05 | data-dependent with named missing data | Missing promotable real-source correspondence and provenance; synthetic cell/multiscale artifacts and bounded consumers exist. |
| FND-06 | active | Whole-pattern CSR plus independent-patient, exact-block, complete-pair sign-flip, and unblocked functional patient designs are implemented; general hierarchy-backed design objects, repeated/multisite/cluster/interference schemes, and multiplicity ownership remain. |
| FND-07 | complete | Native Git/toolchain/feature/executable/input identity, canonical project head, append-only ledger, verified replay, pending recovery, and typed unified invocation are implemented for dependency-free native nodes. |
| FR-01 | active | IC-0145 supplies one canonical physical-radius binary graph, combinatorial Laplacian, exact Fourier transform, and band summaries; broader graph rules/operators and endpoint calibration remain. |
| FR-01A | active | Exact small-graph Fourier, heat, ERL spectrum null, and differentially checked Chebyshev heat are runnable through IC-0145–0149. |
| FR-01B | active | Exact fixed-kernel spectral wavelets, diffusion-wavelet compression, and order-two scattering with declared signal-perturbation stability are runnable; broader kernels, graph perturbation bounds, sparse scale, and physical calibration remain. |
| FR-02 | data-dependent with named missing data | Links/context/dependency plus synthetic nested M0–M5 complementarity are runnable; promotable matched embeddings, patient-held-out outcomes, and external validation remain missing. |
| FR-02A | complete | Bounded synthetic patch/region/slide tables and explicit links were delivered by C-05. |
| FR-03 | active | Balanced, KL-unbalanced, fixed-mass partial, dustbin, balanced FGW, and fixed-mass partial FGW plans are runnable with strict descriptive semantics and sensitivity; real atlas inputs, external comparison, and KL-unbalanced FGW remain missing. |
| FR-03A | active | Native balanced/unbalanced, pinned-SciPy partial, and pinned-POT fixed-mass partial FGW expose plans, objectives, residuals, unmatched mass, and non-correspondence semantics; broader solver agreement and atlas integration remain. |
| FR-03B | active | Pinned-POT entropic FGW now reports three initialization plans, cost scaling, non-identifiability, and Rust-replayed objectives; fixed-mass partial sensitivity is runnable, while KL-unbalanced FGW lacks an established pinned backend and real atlas validation remains missing. |
| GEN-01 | active | Bounded mechanistic/resource/coupled simulators plus differentiable Gaussian pair-summary matching are runnable; vector/general/reciprocal PDE, neural, SBI inference, validation, and real calibration remain. |
| GEN-01A | blocked with named prerequisite | `NEURAL-GEN-01`: requires a reviewed pinned neural backend/serialization/runtime plus independent-patient patterns/windows/contexts and prespecified classical/LGCP count/K/g calibration evidence. |
| GEN-01B | blocked with named prerequisite | `NEURAL-GEN-01`: requires the pinned learned set-generator backend plus admitted held-out data, privacy/memorization, mode-collapse, cardinality, boundary-support, and simpler-model gates. |
| GEN-01C | gated pending explicit user decision | The master plan prohibits a digital-twin claim without prospective intervention/outcome evidence. |
| GEO-01 | blocked with named prerequisite | The exact window/boundary subset is complete for K/L; compartments, signed interfaces, objects, and morphology require the remaining FND-02 contract and immediate annotated-pathology callers. |
| GEO-01A | blocked with named prerequisite | Requires WS-22 exact compartments, interfaces, and signed boundary distance. |
| GEO-01B | blocked with named prerequisite | Requires GEO-01A boundary orientation and denominator contracts. |
| GEO-01C | blocked with named prerequisite | Requires GEO-01A signed distance, phenotype selection, and clipping semantics. |
| GEO-01D | blocked with named prerequisite | Requires WS-22 and an explicit mask/graph basis and scale. |
| GEO-01E | blocked with named prerequisite | Requires WS-22 validated polygon/morphology operations and parameter-sensitivity contracts. |
| GEO-01F | data-dependent with named missing data | Missing validated compartment/front annotations and pathology endpoint labels. |
| GEO-01G | data-dependent with named missing data | Missing independently segmented vessel, gland, nerve, and necrosis objects with uncertainty. |
| GEO-01H | data-dependent with named missing data | Missing validated interfaces for curvature and longitudinal observations for propagation claims. |
| GSP-01 | active | IC-0145–IC-0152 own bounded canonical graph, exact/approximated spectral, diffusion-wavelet, and scattering workflows; broader rules, sparse scale, graph-perturbation theory, and real calibration remain. |
| GSP-02 | active | IC-0153–IC-0156 provide synthetic typed heterogeneous messages, normalized hypergraphs, typed motifs, clique/Hodge mathematics, and research-only cellular complexes; pathology endpoint validation remains data-dependent. |
| GSP-03 | blocked with named prerequisite | Requires GSP-01 plus an incremental-value gate over simpler spectral summaries. |
| HET-01 | active | Bounded synthetic heterogeneous, hypergraph, motif, simplicial/Hodge, and cellular-complex specializations are runnable; clinical/pathology utility and robustness beyond declared fixtures remain unvalidated. |
| IHC-01 | data-dependent with named missing data | Missing provenance-complete continuous/ordinal IHC with batch, threshold, registration, and measurement uncertainty. |
| INF-01A | complete | Canonical scalar permutation implementation exists and is preserved. |
| INF-01B | complete | Canonical functional global-envelope implementation exists and is preserved. |
| INF-01C | active | Single-step Max-T over one complete prespecified patient endpoint family is runnable; step-down, broader multiplicity families, blocked/paired families, and calibration remain. |
| INF-01D | blocked with named prerequisite | Requires FND-06 and method-specific interval/coverage designs. |
| MM-01 | data-dependent with named missing data | Explicit-pair synthetic cross-modal covariance/random-label inference is runnable; matched measured multimodal cohort data, validated correspondence, missing-modality policy, and technical/site covariates remain missing. |
| MOL-01 | data-dependent with named missing data | Missing spatial molecular observations linked to cells/regions with correspondence uncertainty. |
| MRK-01 | blocked with named prerequisite | Requires completion of FND-04 and the relevant point/pair/weight plans. |
| MRK-01A | blocked with named prerequisite | Requires typed categorical/probabilistic marks and PP pair-plan semantics. |
| MRK-01B | blocked with named prerequisite | Requires typed continuous marks and explicit normalization/mean policy. |
| MRK-01C | blocked with named prerequisite | Requires PP-01 plus mark-weight and normalization contracts. |
| MRK-02A | blocked with named prerequisite | Requires a general CellId-keyed type table and an immediate multitype production caller. |
| MRK-02B | blocked with named prerequisite | Requires completion of the general mark artifact and an immediate order-sensitive caller. |
| MRK-02C | planned | Existing dense probability declarations remain; general simplex/null semantics await a later immediate caller. |
| MRK-02D | blocked with named prerequisite | Three immediate formal vector-statistic callers are runnable on strict dynamic inputs; stable mark admission still requires FND-05 canonical row-bound vector artifacts and provenance. |
| NIC-01 | blocked with named prerequisite | Requires FND-03 geometry, FND-04 marks, and cohort-valid execution. |
| NIC-01A | blocked with named prerequisite | Requires shared physical-scale geometry/graph plans and general mark inputs. |
| NIC-01B | blocked with named prerequisite | Requires NIC-01A plus imported assignment uncertainty contracts. |
| NIC-01C | blocked with named prerequisite | Requires BACK-01 and reproducible imported discovery results. |
| NIC-01D | blocked with named prerequisite | Requires stable/imported domains and GEO-01 boundary geometry. |
| NIC-01E | blocked with named prerequisite | Requires NIC-01 domains plus COH-01 compositional/hierarchical inference. |
| NIC-01F | data-dependent with named missing data | Missing independent reference/query cohorts with held-out mapping and missing-cell-type cases. |
| NUL-01A | complete | Conditional homogeneous CSR fixes the exact validated window and observed point count, resamples the whole location pattern with deterministic namespaced streams, and exposes exact simulation/draw-budget evidence. |
| NUL-01B | active | Existing scalar random labeling plus complete embedding-row ERL, cross-modal B-row max-T, and graph-smoothness low-tail nulls are runnable; broader typed mark/strata calibration and stable promotion remain. |
| NUL-01C | blocked with named prerequisite | Requires FND-06 population-independence design and component simulators. |
| NUL-01D | blocked with named prerequisite | Requires FND-06 declared conditioning variables, units, and degeneracy reporting. |
| NUL-01E | blocked with named prerequisite | Requires exact overlap-aware windows and a justified stationarity contract. |
| PATH-01 | blocked with named prerequisite | Requires WS-22/GEO-01 compartments, boundaries, and object geometry. |
| PERT-01 | active | IC-0193 provides a bounded randomized synthetic perturbation/control specialization with research-only mediation; real designed perturbations with dose/time and spatial readouts remain missing. |
| PLAT-01 | active | Immutable artifacts, catalog/store/cache, exact source/runtime identities, durable heads/ledger/recovery/replay, and typed unified dependency-free native/PyMC/POT execution exist; dependency-bearing multi-node orchestration remains. |
| PP-01 | blocked with named prerequisite | The complete unmarked standard-border K/L workflow is delivered; stable PP-01 promotion still requires remaining FND-02/FND-03/FND-06 contracts, pinned external-oracle fixtures, null calibration, intensity-gradient policy, and scale evidence. |
| PP-02 | blocked with named prerequisite | Requires PP-01 and a cross-fit, provenance-bearing intensity estimator. |
| PP-03 | blocked with named prerequisite | Requires PP-01/PP-02 and typed multitype/mark inputs. |
| PP-03A | blocked with named prerequisite | Requires PP-01/PP-02, bandwidth policy, and exact pair-plan semantics. |
| PP-03B | blocked with named prerequisite | Requires PP-01/PP-02 plus type-specific intensity and sparse-type states. |
| PP-04 | blocked with named prerequisite | Requires FND-02/FND-03 and completed nearest/empty-space contracts. |
| PP-04A | blocked with named prerequisite | Requires FND-03 exact nearest-neighbor plan and boundary policy. |
| PP-04B | blocked with named prerequisite | Requires FND-02 exact window and deterministic/sampled probe plan. |
| PP-04C | blocked with named prerequisite | Requires PP-04A/PP-04B plus finite denominator/undefined semantics. |
| PP-05 | blocked with named prerequisite | Requires FND-02/FND-03 and explicit bandwidth/cross-fitting provenance. |
| PP-06A | complete | Standard reduced-sample border K/L is implemented as A*q(r)/(n*m(r)) with exact eligible-center/ordered-pair counts, hand and brute-force oracles, typed unavailable states, and deterministic CSR inference. |
| PP-06B | blocked with named prerequisite | Requires validated polygon-overlap geometry and PP-01 baseline. |
| PP-06C | blocked with named prerequisite | Requires visible-boundary arc geometry and PP-01 baseline. |
| PP-06D | gated pending explicit user decision | The master plan rejects toroidal behavior by default; admission requires a genuinely periodic rectangular design and explicit user approval. |
| REG-01 | blocked with named prerequisite | Requires BACK-01 and GEO-01 uncertainty-bearing geometry. |
| REG-01A | complete | Existing rigid/affine 2-D registration is characterized and retained at its limited claim. |
| REG-01B | blocked with named prerequisite | Requires BACK-01, GEO-01, and an admitted nonrigid backend with transform-uncertainty output. |
| REG-01C | data-dependent with named missing data | Missing paired multimodal/serial sections with landmarks or correspondence ground truth and uncertainty evidence. |
| SCALE-01 | active | VFE inducing-point and low-rank predictive-process workflows expose explicit approximation state/error bounds; nearest-neighbor, out-of-core, progressive, calibrated-error, and broader scale infrastructure remain. |
| SIG-01 | blocked with named prerequisite | Requires FND-04 completion plus stable geometry/weight contracts. |
| SIG-01A | blocked with named prerequisite | Requires declared weights, centering, FND-06 permutation design, and typed scalar marks. |
| SIG-01B | blocked with named prerequisite | Requires the same stable weights contract as SIG-01A and an independent oracle. |
| SIG-01C | blocked with named prerequisite | Requires SIG-01A, multiplicity ownership, and cohort-valid exploratory-map policy. |
| SIG-01D | blocked with named prerequisite | Requires typed paired variables, direction semantics, weights, and multiplicity. |
| SIG-01E | data-dependent with named missing data | Missing a prespecified pathology hotspot use case and replicated endpoint data. |
| SIG-01F | blocked with named prerequisite | Requires FND-03 pair/bin plans and typed scalar marks. |
| SIG-01G | data-dependent with named missing data | Missing co-located or correspondence-qualified bivariate observations. |
| SIG-01H | blocked with named prerequisite | Synthetic distance and graph vector/covariance, leakage-safe projected/kernel, and complete-vector null workflows are runnable; stable promotion requires shared geometry/graph provenance/scale plus canonical vector-input calibration. |
| SPC-01A | gated pending explicit user decision | The master plan rejects Bartlett branding/default use absent a prespecified endpoint and demonstrated advantage. |
| SPC-01B | planned | Multitaper spectral estimation remains a watch item pending a prespecified variance-reduction endpoint. |
| TIME-01 | data-dependent with named missing data | Missing repeated biological units with registered timepoints and deformation-versus-change evidence. |
| TOP-01 | active | All Part VIII functions have bounded experimental workflows through IC-0158–IC-0164; pathology endpoint and replicated validation remain missing. |
| TOP-01A | active | Pinned exact alpha/witness persistence, transforms, patient comparison, and validation controls are runnable; stable pathology-linked filtration and replication remain missing. |
| TOP-01B | active | Supplied-mask Crofton/Euler/morphology, connectivity, and perturbation laboratory are runnable; validated mask/interface basis, endpoint, and cohort remain missing. |
| UX-01 | blocked with named prerequisite | Requires stable PLAT-01/WF-01 project and result contracts for a UI that does not duplicate science. |
| WAV-01A | gated pending explicit user decision | The master plan rejects raster wavelets by default; admission requires a raster-defined question and explicit approval. |
| WAV-01B | planned | Genuine DoG remains in scope for a future explicit raster/image scale-space endpoint. |
| WF-01 | active | Typed keys, single-node scheduling, cache, and one scientific node exist; general composition, schema, and resume remain. |

## Dependency-ordered workstream coverage

| ID | State | Evidence, named prerequisite, missing data, or gate |
|---|---|---|
| WS-00 | complete | Implementation control plane and repository state installed. |
| WS-01 | complete | Legacy characterization and compatibility matrix closed. |
| WS-10 | complete | Workspace and compatibility-shell phase slice closed. |
| WS-11 | complete | Catalog/store/cache plus canonical durable project heads, append-only execution ledger, verified replay, and interrupted-run recovery are implemented. |
| WS-12 | active | Typed durable single-node execution exists for native and two static external workflows; dependency-bearing DAG composition, resource planning, workflow schema, and resume remain. |
| WS-13 | active | Durable PyMC Normal-mean and POT FGW share one closed static descriptor with exact environment/adapter/license/schema/control identity and typed replay; generalized discovery, broader backend classes, doctor/admission validation, and capability/security manifests remain. |
| WS-20 | complete | Typed identity and cohort hierarchy closed by C-01. |
| WS-21 | complete | Coordinate, unit, dimensionality, transform, and uncertainty slice closed by C-02. |
| WS-22 | blocked with named prerequisite | Exact bounded 2-D windows, canonical boundary distance, and one streaming K/L pair plan are complete; compartments, volumes, signed distance, and shared multi-endpoint plans require promoted immediate callers. |
| WS-23 | planned | Thirteen concrete slices exist; broader marks remain in scope after the classical workflow. |
| WS-24 | data-dependent with named missing data | Missing promotable canonical source identity, context, provenance, and linkage; synthetic artifacts remain complete evidence. |
| WS-25 | planned | SpatialData, AnnData, OME-NGFF, Arrow/Parquet/Zarr interchange remains in Phase 2 scope. |
| WS-30 | blocked with named prerequisite | The first complete homogeneous standard-border K/L workflow is complete; translation/isotropic corrections, inhomogeneous K/L, g, F/G/J, cross/marked functions, and intensity estimation require their named geometry/mark/intensity prerequisites. |
| WS-31 | active | Conditional CSR, reused ERL, scalar/paired/functional patient permutation, single-step Max-T, and complete-vector embedding ERL/cross-modal max-T nulls are runnable; remaining null families, calibration, and general multiplicity require further FND-06 slices. |
| WS-32 | blocked with named prerequisite | Synthetic vector variogram/projected/covariance workflows are runnable; stable spatial-signal promotion still requires WS-22 shared geometry/weights/edge correction and canonical typed marks. |
| WS-33 | blocked with named prerequisite | Requires WS-22 compartments, boundaries, and object geometry. |
| WS-34 | active | Scalar/paired/functional permutation, Max-T, MMD, energy, structured fingerprint distance, equivalence, noninferiority, and patient-first hierarchical bootstrap are runnable; repeated/multisite designs and broader validation remain. |
| WS-40 | active | Typed PyMC NUTS/SMC, differentiated/nested Laplace, and pinned ArviZ PSIS-LOO lifecycles with normalized diagnostics are runnable; broader families, SBC, sensitivity, second-backend agreement, and generalized artifacts remain. |
| WS-41 | active | Patient Gaussian partial pooling and one-covariate site meta-regression are runnable through PyMC; broader likelihoods, repeated/crossed models, multiple covariates, and validation remain. |
| WS-42 | active | Exact/multi-output/VFE/predictive-process/NNGP 1-D workflows plus CAR/GMRF, SAR, BYM, and scaled BYM2 are runnable; SPDE/2-D requires promoted exact window/mesh ownership, while varying coefficients are next. |
| WS-43 | active | Fixed/fitted rectangular IPP, Berman–Turner refinement, exact-cell fixed-hyperparameter LGCP construction/fitting/pattern replicas, bounded Thomas/Matérn cluster simulation, Thomas minimum contrast, and Strauss statistic/Papangelou/birth-death/pseudolikelihood are runnable; latent-parent fitting is backend-blocked while exchange inference, spatial PPC envelopes, arbitrary windows, multitype/marked/replicated processes remain. |
| WS-44 | active | PSIS-LOO/comparison, conjugate SBC, and conjugate posterior/predictive/decision prior sensitivity are runnable; exact refits/K-fold, stacking, and hierarchical/field/point-process calibration/sensitivity remain. |
| WS-50 | blocked with named prerequisite | Distance/kernel/covariance/envelope plus global/local graph-smoothness workflows are runnable on bounded synthetic inputs; promotion still requires admitted geometry/graph scale, promotable real provenance, technical calibration, and patient-held-out validation. |
| WS-51 | data-dependent with named missing data | Exact links, weighted context, overlap-component effective count, and prior synthetic consumers exist; promotable matched source correspondence and patient-held-out overlap-aware cohort data remain missing. |
| WS-52 | data-dependent with named missing data | Synthetic nested M0–M5, calibration, split conformal, exact retrieval, held-out-domain shrinkage OOD, abstention, and calibrated late fusion are runnable; matched patient-held-out labeled cohorts with technical/site/multimodal variables remain missing. |
| WS-53 | active with data-dependent promotion | Synthetic pCCA, Bayesian pCCA, multiview/matrix/graph-spatial/tensor factors, hierarchy compilation, missing-view prediction, calibrated late fusion, stacking, and context-gated experts are runnable; matched measured cohorts, validated correspondence, calibrated missingness, hierarchy, and patient/site generalization remain missing. |
| WS-54 | data-dependent with named missing data | Missing provenance-complete clone/CNA probabilities, phylogenies, compartments, and cohort outcomes. |
| WS-55 | data-dependent with named missing data | Dustbin, FGW, and fixed-mass partial-FGW descriptive plans are runnable, but paired serial/multimodal sections with landmarks, deformation/correspondence uncertainty, atlas provenance, and downstream validation endpoints remain missing. |
| WS-60 | blocked with named prerequisite | Requires WS-22 geometry, WS-23 marks, and FND-06 cohort-valid designs. |
| WS-61 | active | Canonical homogeneous radius graphs plus bounded typed heterogeneous, hypergraph, motif, simplicial/Hodge, and perturbation-checked cellular structures are live; broader construction rules and real pathology validation remain. |
| WS-62 | active | Exact Fourier/bands, restricted ERL nulls, heat/signatures/distances, spectral/diffusion wavelets, Chebyshev heat, and scattering are runnable; sparse scale, GPU parity, broader perturbations, and real calibration remain. |
| WS-63 | active | Bounded synthetic/supplied-input topology and morphology laboratory is live; pathology-linked endpoints, representative sparse scale, and replicated cohorts remain missing. |
| WS-70 | active | `marklab-simulation` now owns seven consumed simulators, including one-way composition, with units, bounds, diagnostics, deterministic replay where stochastic, and resource limits; richer observation/noise models, validation generators, and artifact integration remain. |
| WS-71 | active | Fisher–KPP, scalar reaction-pattern, competition, level-set, vascular/resource response, and one-way vascular→density→interface→agent coupling are runnable with claim ceilings; vector/general PDE, signed/network response, reciprocal coupling, and biological calibration remain. |
| WS-72 | blocked with named prerequisite | `NEURAL-GEN-01` names the absent reviewed PyTorch/JAX-class backend contract and provenance-complete independent-patient calibration/memorization corpus; native simulators do not satisfy learned-generator admission. |
| WS-73 | active | `marklab-sbi` now provides rejection ABC, weighted SMC-ABC, and Gaussian synthetic-likelihood MCMC growth-front specializations alongside IC-0116 summaries; adaptive/neural SBI, calibration, OOD, and real lifecycle integration remain. |
| WS-74 | blocked with named prerequisite | IC-0122 supplies one synthetic two-summary growth-front posterior-predictive laboratory, but full digital-tissue validation still requires validated WS-71–WS-73 models, admitted held-out data, broader catalogs, and privacy/memorization gates. |
| WS-80 | active | IC-0126 provides a bounded synthetic physical-cuboid 3-D K/L workflow with exact correction oracles; serial reconstruction, general windows/graphs/topology/fields, and real 3-D cohorts with section geometry, deformation uncertainty, and validation landmarks remain missing. |
| WS-81 | active | IC-0123 provides bounded time-varying linear-Gaussian filtering/RTS smoothing with exact synthetic and missing-observation oracles; nonlinear/particle/spatial-field/evolutionary workflows and repeated registered biological units, treatment/time metadata, and clone trajectories remain missing. |
| WS-82 | active | IC-0130/IC-0192 supply randomized interference plus synthetic propensity/dose/AIPW/exposure-AIPW/DML/negative-control mechanics; eligible real treatment data and identification remain missing. |
| WS-83 | active | IC-0193 supplies a randomized synthetic perturbation and research-only mediation specialization; real designed cohorts remain missing. |
| WS-84 | active | IC-0132/IC-0194/IC-0195 supply analytic EIG, synthetic sequential/constrained selection, allocation/power, and partial validation; prospective outcomes remain missing. |
| WS-90 | planned | Interactive workbench remains in Phase 9 scope after stable project/workflow/result contracts. |
| WS-91 | planned | Server, collaboration, authorization, audit, and remote execution remain in Phase 9 scope. |
| WS-92 | planned | Python/R/notebook clients, bindings, zero-copy artifacts, and recipes remain in Phase 9 scope. |
| WS-93 | data-dependent with named missing data | Missing admitted benchmark datasets, external cohorts, and publication-grade validation partners. |
| WS-94 | planned | Stable 1.0 schemas, migrations, LTS, security review, and claim tiers remain the terminal release workstream. |
| WS-A | complete | Alias grouping for implementation control and baseline preservation closed. |
| WS-B | complete | Alias grouping for workspace replatforming and compatibility shell closed. |
| WS-C | planned | Initial substrate slices through C-06 exist; broader marks/interchange/source promotion remain in total scope after the active classical workflow. |
