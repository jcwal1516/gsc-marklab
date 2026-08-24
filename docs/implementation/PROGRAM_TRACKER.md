# Marklab program tracker

Last updated: 2026-08-24

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
| ACT-01 | data-dependent with named missing data | Missing prospective acquisition action model, budget, and design-partner data for ROI, stain, field, or sampling decisions. |
| B-01 | complete | Workspace architecture decision and compatibility boundary closed. |
| B-02 | complete | Compatibility shell parity closed. |
| B-03 | complete | Workspace policy and feature-matrix milestone closed. |
| B-04 | complete | Minimal project/workflow vertical slice closed. |
| BACK-01 | blocked with named prerequisite | Requires the promoted durable PLAT-01/WF-01 execution-ledger and replay outcome; multi-backend admission will then use pinned native/process/Python/R/Stan/container adapters rather than assume Rust ports. |
| BAY-01 | blocked with named prerequisite | Requires stable PLAT-01, DATA-01, FND-04, and backend-neutral workflow inputs. |
| BAY-02 | blocked with named prerequisite | Requires BAY-01 model IR and BACK-01 backend admission. |
| BAY-03 | blocked with named prerequisite | Requires BAY-01, BAY-02, and FND-06 cohort-valid inference design. |
| BAY-04 | blocked with named prerequisite | Requires BAY-01/BAY-02 and exact window/field geometry from WS-22. |
| BAY-05 | blocked with named prerequisite | Requires BAY-01/BAY-02 and a stable graph/weight contract. |
| BAY-EVO | data-dependent with named missing data | Missing provenance-complete clone probabilities, phylogeny, and replicated longitudinal or 3-D evidence. |
| BAY-FIELD-A | blocked with named prerequisite | Requires BAY-01/BAY-02 and WS-22 exact window/field contracts. |
| BAY-GMRF-A | blocked with named prerequisite | Requires BAY-01/BAY-02 and stable declared adjacency/weight graphs. |
| BAY-HIER-A | blocked with named prerequisite | Requires BAY-01/BAY-02 and FND-06 patient-level hierarchy/inference contracts. |
| BAY-MM | data-dependent with named missing data | Missing matched measured multimodal cohort data with modality, patient, site, and missingness provenance. |
| BAY-NP | blocked with named prerequisite | Requires Bayesian fit infrastructure and reproducible multiscale niche inputs from WS-60. |
| BAY-PP | blocked with named prerequisite | Requires BAY-01/BAY-02 plus validated WS-22, WS-30, and WS-31 foundations. |
| BAY-PP-A | blocked with named prerequisite | Requires BAY-PP foundation, exact windows, and calibrated simulator/oracle support. |
| BAY-PP-B | blocked with named prerequisite | Requires BAY-PP foundation, interaction-process simulation, and backend admission. |
| BAY-REG | blocked with named prerequisite | Requires BACK-01, GEO-01, BAY-01/BAY-02, and uncertainty-bearing transforms. |
| BAY-REG-A | blocked with named prerequisite | Requires BAY-01/BAY-02, FND-06, and typed spatial covariate designs. |
| BAY-SBI | blocked with named prerequisite | Requires BAY-01/BAY-02 and the WS-70 canonical simulator library. |
| BULK-01 | data-dependent with named missing data | Missing specimen-linked bulk molecular measurements, outcomes, and patient/site covariates. |
| C-01 | complete | Typed identities and cohort hierarchy closed in the C-01 handoff. |
| C-02 | complete | Units, coordinate frames, dimensions, transforms, and uncertainty references closed. |
| C-03 | complete | Immutable artifact catalog and table/store boundary closed. |
| C-04 | complete | Bounded CellViT cell-embedding table, provenance, physical formats, and scale evidence closed. |
| C-05 | complete | Bounded patch/region/slide tables, links, graphs, receipts, formats, and finalizers closed. |
| C-06 | planned | Thirteen bounded slices are green; broader general marks remain in scope but are not the current increment. |
| CAU-01 | data-dependent with named missing data | Missing eligible treatment/exposure assignment, temporal ordering, positivity, confounders, and declared interference structure. |
| CAU-01A | data-dependent with named missing data | Missing genuine treatment assignment, exposure mapping, interference graph, positivity, and identification evidence. |
| CAU-01B | data-dependent with named missing data | Missing exposure/outcome cohorts with measured confounders, overlap, negative controls, and sensitivity variables. |
| CAU-01C | gated pending explicit user decision | The master plan permits hypothesis generation only; causal-evidence promotion is gated and also requires intervention or identification support. |
| CCC-01 | data-dependent with named missing data | Missing matched spatial molecular/protein data and external signaling validation; any output remains hypothesis-generating. |
| CLN-01 | data-dependent with named missing data | Missing provenance-complete clone/CNA assignments, uncertainty, and patient-linked validation cohorts. |
| CLN-01A | data-dependent with named missing data | Missing caller/version/input-provenance-complete clone or CNA probabilities. |
| CLN-01B | data-dependent with named missing data | Missing clone assignments plus compartments/intensity inputs for conditioned spatial nulls. |
| CLN-01C | data-dependent with named missing data | Missing linked clone, immune/stroma/IHC/morphology observations with explicit spatial relation semantics. |
| CLN-01D | data-dependent with named missing data | Missing imported phylogeny/evolutionary distances and assignment uncertainty. |
| CLN-02 | data-dependent with named missing data | Missing phylogeny and longitudinal/3-D evidence needed to separate association from growth direction. |
| CMP-01 | blocked with named prerequisite | Requires completion of FND-06 and cohort-valid COH-01 execution. |
| CMP-01A | blocked with named prerequisite | Requires stable endpoint families, FND-07 provenance, and COH-01 replication semantics. |
| CMP-01B | blocked with named prerequisite | Requires FND-06 patient/specimen permutation and functional endpoint contracts. |
| CMP-01C | blocked with named prerequisite | Requires COH-01 and leakage-safe patient-level kernel selection. |
| CMP-01D | blocked with named prerequisite | Requires COH-01 and patient-level exchangeability/finite-moment contracts. |
| CMP-01E | blocked with named prerequisite | Requires stable graph construction plus cohort-valid evaluation. |
| CMP-01F | data-dependent with named missing data | Missing stable filtration, interpretable pathology endpoint, and replicated cohort. |
| COH-01 | blocked with named prerequisite | Requires completion of the active FND-06 whole-pattern null-design foundation before patient-level execution is promoted. |
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
| DIM-01 | data-dependent with named missing data | Missing provenance-complete serial-section, 3-D, and longitudinal cohorts with section order, thickness, deformation, and missing-section states. |
| DL-CORE | blocked with named prerequisite | Requires BACK-01 plus licensed, checksum-pinned model manifests and patient/site validation contracts. |
| EMB-01 | blocked with named prerequisite | Requires FND-05 completion, stable spatial weights/geometry, and SIG-01 vector-statistic contracts. |
| EMB-02 | data-dependent with named missing data | Missing promotable real patch/region source identity, scale, provenance, and cell/patch correspondence; synthetic multiscale infrastructure already exists. |
| EMB-CORE | complete | Bounded CellViT single-cell artifact contract and import/format/resource evidence closed by C-04. |
| EMB-PATCH | complete | Bounded synthetic patch/region/slide artifact and explicit-link contract closed by C-05. |
| EMB-PRED-01 | data-dependent with named missing data | Missing patient-held-out labeled cohort with matched cell/patch embeddings and technical covariates. |
| EMB-PRED-02 | data-dependent with named missing data | Missing the same cohort plus a demonstrated late-fusion baseline that cross-attention could improve. |
| EQV-01 | blocked with named prerequisite | Requires COH-01, CMP-01, and prespecified clinically/scientifically justified margins. |
| EQV-01A | complete | Existing descriptive margin is characterized and retained strictly as descriptive. |
| EQV-01B | data-dependent with named missing data | Missing prespecified equivalence margins and sufficient independent biological replicates. |
| EQV-01C | data-dependent with named missing data | Missing directional noninferiority margin, clinical rationale, and sufficient biological replicates. |
| FND-01 | complete | Typed identities, hierarchy, parentage, and design roles closed by C-01. |
| FND-02 | ready | Exact bounded ObservationWindow2D, canonical topology, closed membership, and indexed unsigned boundary distance are complete for the classical caller; signed distance, typed frame binding, compartments, external differential evidence, and broader promotion remain ready work. |
| FND-03 | blocked with named prerequisite | Exact streaming ordered-pair traversal is complete for K/L; full promotion requires completed FND-02 semantics plus an immediate second canonical caller such as g, variograms, or mark functions. |
| FND-04 | planned | Measurement-status vocabulary and concrete consumers exist; general typed marks remain in total scope after the classical increment. |
| FND-05 | data-dependent with named missing data | Missing promotable real-source correspondence and provenance; synthetic cell/multiscale artifacts and bounded consumers exist. |
| FND-06 | ready | Conditional homogeneous CSR now has an explicit whole-pattern unit and typed result; the broader InferenceDesign families, hierarchy checks, multiplicity ownership, and patient-level units remain ready. |
| FND-07 | active | The classical result adds deterministic window/geometry/config/cache identities and strict claim-bounded output; the promoted durable project outcome now advances Git/toolchain/feature/input provenance, ledger persistence, replay, and schema evolution. |
| FR-01 | blocked with named prerequisite | Requires FND-03 and a stable graph/weight/Laplacian contract. |
| FR-01A | blocked with named prerequisite | Requires stable graph/weight/Laplacian ownership and exact small-graph oracles. |
| FR-01B | blocked with named prerequisite | Requires FR-01A graph spectral foundation and declared physical scales. |
| FR-02 | data-dependent with named missing data | Missing promotable matched cell/patch embeddings and a patient-held-out outcome cohort. |
| FR-02A | complete | Bounded synthetic patch/region/slide tables and explicit links were delivered by C-05. |
| FR-03 | blocked with named prerequisite | Requires BACK-01 and stable multimodal identity/mark/embedding contracts. |
| FR-03A | blocked with named prerequisite | Requires FR-03 admission, external solver contract, and descriptive non-correspondence result semantics. |
| FR-03B | blocked with named prerequisite | Requires FR-03A baseline plus non-identifiability and cost sensitivity gates. |
| GEN-01 | planned | Full mechanistic/neural simulator program retained for the Phase 7 sequence. |
| GEN-01A | planned | Neural point/Cox process track retained with calibration and classical/LGCP comparisons. |
| GEN-01B | planned | Diffusion/generative layout track retained with privacy, memorization, and mode-collapse gates. |
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
| GSP-01 | blocked with named prerequisite | Requires FND-03 and stable sparse graph, weight, Laplacian, and scale contracts. |
| GSP-02 | planned | Heterogeneous graph, hypergraph, and higher-order tissue program retained for WS-61. |
| GSP-03 | blocked with named prerequisite | Requires GSP-01 plus an incremental-value gate over simpler spectral summaries. |
| HET-01 | planned | Emerging heterogeneous/higher-order tissue representation remains in the frontier program. |
| IHC-01 | data-dependent with named missing data | Missing provenance-complete continuous/ordinal IHC with batch, threshold, registration, and measurement uncertainty. |
| INF-01A | complete | Canonical scalar permutation implementation exists and is preserved. |
| INF-01B | complete | Canonical functional global-envelope implementation exists and is preserved. |
| INF-01C | blocked with named prerequisite | Requires FND-06 endpoint-family and multiplicity design ownership. |
| INF-01D | blocked with named prerequisite | Requires FND-06 and method-specific interval/coverage designs. |
| MM-01 | data-dependent with named missing data | Missing matched measured multimodal cohort data and technical/site covariates. |
| MOL-01 | data-dependent with named missing data | Missing spatial molecular observations linked to cells/regions with correspondence uncertainty. |
| MRK-01 | blocked with named prerequisite | Requires completion of FND-04 and the relevant point/pair/weight plans. |
| MRK-01A | blocked with named prerequisite | Requires typed categorical/probabilistic marks and PP pair-plan semantics. |
| MRK-01B | blocked with named prerequisite | Requires typed continuous marks and explicit normalization/mean policy. |
| MRK-01C | blocked with named prerequisite | Requires PP-01 plus mark-weight and normalization contracts. |
| MRK-02A | blocked with named prerequisite | Requires a general CellId-keyed type table and an immediate multitype production caller. |
| MRK-02B | blocked with named prerequisite | Requires completion of the general mark artifact and an immediate order-sensitive caller. |
| MRK-02C | planned | Existing dense probability declarations remain; general simplex/null semantics await a later immediate caller. |
| MRK-02D | blocked with named prerequisite | Requires FND-05 row-bound vector artifacts and an immediate formal vector-statistic caller. |
| NIC-01 | blocked with named prerequisite | Requires FND-03 geometry, FND-04 marks, and cohort-valid execution. |
| NIC-01A | blocked with named prerequisite | Requires shared physical-scale geometry/graph plans and general mark inputs. |
| NIC-01B | blocked with named prerequisite | Requires NIC-01A plus imported assignment uncertainty contracts. |
| NIC-01C | blocked with named prerequisite | Requires BACK-01 and reproducible imported discovery results. |
| NIC-01D | blocked with named prerequisite | Requires stable/imported domains and GEO-01 boundary geometry. |
| NIC-01E | blocked with named prerequisite | Requires NIC-01 domains plus COH-01 compositional/hierarchical inference. |
| NIC-01F | data-dependent with named missing data | Missing independent reference/query cohorts with held-out mapping and missing-cell-type cases. |
| NUL-01A | complete | Conditional homogeneous CSR fixes the exact validated window and observed point count, resamples the whole location pattern with deterministic namespaced streams, and exposes exact simulation/draw-budget evidence. |
| NUL-01B | planned | Existing random labeling is preserved; broader typed mark/strata ownership awaits a later promoted workflow. |
| NUL-01C | blocked with named prerequisite | Requires FND-06 population-independence design and component simulators. |
| NUL-01D | blocked with named prerequisite | Requires FND-06 declared conditioning variables, units, and degeneracy reporting. |
| NUL-01E | blocked with named prerequisite | Requires exact overlap-aware windows and a justified stationarity contract. |
| PATH-01 | blocked with named prerequisite | Requires WS-22/GEO-01 compartments, boundaries, and object geometry. |
| PERT-01 | data-dependent with named missing data | Missing designed perturbation datasets with treatment assignment, controls, dose/time, and spatial readouts. |
| PLAT-01 | active | Immutable artifacts, catalog/store, cache, and one scientific execution exist; durable heads, execution ledger, replay, and resume remain. |
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
| SCALE-01 | planned | Exact-fallback, streaming, out-of-core, progressive, and approximation-error infrastructure remains in scope. |
| SIG-01 | blocked with named prerequisite | Requires FND-04 completion plus stable geometry/weight contracts. |
| SIG-01A | blocked with named prerequisite | Requires declared weights, centering, FND-06 permutation design, and typed scalar marks. |
| SIG-01B | blocked with named prerequisite | Requires the same stable weights contract as SIG-01A and an independent oracle. |
| SIG-01C | blocked with named prerequisite | Requires SIG-01A, multiplicity ownership, and cohort-valid exploratory-map policy. |
| SIG-01D | blocked with named prerequisite | Requires typed paired variables, direction semantics, weights, and multiplicity. |
| SIG-01E | data-dependent with named missing data | Missing a prespecified pathology hotspot use case and replicated endpoint data. |
| SIG-01F | blocked with named prerequisite | Requires FND-03 pair/bin plans and typed scalar marks. |
| SIG-01G | data-dependent with named missing data | Missing co-located or correspondence-qualified bivariate observations. |
| SIG-01H | blocked with named prerequisite | Requires the active classical geometry/weight foundation plus leakage-safe vector-input provenance. |
| SPC-01A | gated pending explicit user decision | The master plan rejects Bartlett branding/default use absent a prespecified endpoint and demonstrated advantage. |
| SPC-01B | planned | Multitaper spectral estimation remains a watch item pending a prespecified variance-reduction endpoint. |
| TIME-01 | data-dependent with named missing data | Missing repeated biological units with registered timepoints and deformation-versus-change evidence. |
| TOP-01 | data-dependent with named missing data | Missing an interpretable pathology endpoint and patient-level replicated topology study. |
| TOP-01A | data-dependent with named missing data | Missing a stable pathology-linked filtration and independent patient-level replication. |
| TOP-01B | data-dependent with named missing data | Missing explicit mask/interface basis, scale, pathology endpoint, and replicated cohort. |
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
| WS-11 | active | Catalog/store/cache exist; durable project heads, append-only execution ledger, replay, and interrupted-run recovery remain. |
| WS-12 | active | Typed single-node execution exists; general DAG composition, resource planning, workflow schema, and resume remain. |
| WS-13 | planned | Backend registry remains in scope after durable PLAT-01/WF-01 contracts. |
| WS-20 | complete | Typed identity and cohort hierarchy closed by C-01. |
| WS-21 | complete | Coordinate, unit, dimensionality, transform, and uncertainty slice closed by C-02. |
| WS-22 | blocked with named prerequisite | Exact bounded 2-D windows, canonical boundary distance, and one streaming K/L pair plan are complete; compartments, volumes, signed distance, and shared multi-endpoint plans require promoted immediate callers. |
| WS-23 | planned | Thirteen concrete slices exist; broader marks remain in scope after the classical workflow. |
| WS-24 | data-dependent with named missing data | Missing promotable canonical source identity, context, provenance, and linkage; synthetic artifacts remain complete evidence. |
| WS-25 | planned | SpatialData, AnnData, OME-NGFF, Arrow/Parquet/Zarr interchange remains in Phase 2 scope. |
| WS-30 | blocked with named prerequisite | The first complete homogeneous standard-border K/L workflow is complete; translation/isotropic corrections, inhomogeneous K/L, g, F/G/J, cross/marked functions, and intensity estimation require their named geometry/mark/intensity prerequisites. |
| WS-31 | blocked with named prerequisite | Conditional CSR and reused ERL are complete; random labeling generalization, population independence, conditional/shift nulls, scalar/functional families, and multiplicity require the remaining FND-06 design contract. |
| WS-32 | blocked with named prerequisite | Requires WS-22 geometry/weights and completion of general typed marks. |
| WS-33 | blocked with named prerequisite | Requires WS-22 compartments, boundaries, and object geometry. |
| WS-34 | blocked with named prerequisite | Requires FND-06 and cohort-valid COH-01 execution. |
| WS-40 | planned | Bayesian IR, priors, artifacts, diagnostics, and admitted backends remain in Phase 4 scope. |
| WS-41 | blocked with named prerequisite | Requires WS-40 and FND-06 cohort designs. |
| WS-42 | blocked with named prerequisite | Requires WS-40 and exact field/graph geometry. |
| WS-43 | blocked with named prerequisite | Requires WS-40 plus validated point-process and simulator foundations. |
| WS-44 | blocked with named prerequisite | Requires completed model families and normalized diagnostics from WS-40 through WS-43. |
| WS-50 | blocked with named prerequisite | Requires the active classical window/geometry/null foundation plus promotable real cell-embedding provenance. |
| WS-51 | data-dependent with named missing data | Missing promotable matched cell/patch source correspondence and overlap-aware cohort data; two synthetic contained-patch consumers exist. |
| WS-52 | data-dependent with named missing data | Missing patient-held-out labeled cohorts with matched technical, cell, neighborhood, patch, molecular, site, and OOD variables. |
| WS-53 | data-dependent with named missing data | Missing matched measured multimodal cohorts with modality-specific missingness and patient/site structure. |
| WS-54 | data-dependent with named missing data | Missing provenance-complete clone/CNA probabilities, phylogenies, compartments, and cohort outcomes. |
| WS-55 | data-dependent with named missing data | Missing paired serial/multimodal sections with landmarks, deformation/correspondence uncertainty, and downstream validation endpoints. |
| WS-60 | blocked with named prerequisite | Requires WS-22 geometry, WS-23 marks, and FND-06 cohort-valid designs. |
| WS-61 | planned | Heterogeneous graph and higher-order tissue work remains in Phase 6 scope. |
| WS-62 | blocked with named prerequisite | Requires stable graph/weight contracts from WS-61 and exact spectral oracles. |
| WS-63 | data-dependent with named missing data | Missing pathology-linked topology/morphology endpoints and replicated cohorts. |
| WS-70 | planned | Canonical simulator library remains in Phase 7 scope. |
| WS-71 | blocked with named prerequisite | Requires WS-70 simulators and pathology-specific mechanistic contracts. |
| WS-72 | blocked with named prerequisite | Requires WS-70 plus admitted learned backends and held-out calibration data. |
| WS-73 | blocked with named prerequisite | Requires WS-40 Bayesian lifecycle and WS-70 calibrated simulators. |
| WS-74 | blocked with named prerequisite | Requires validated WS-71 through WS-73 models, posterior predictive contracts, and privacy/memorization gates. |
| WS-80 | data-dependent with named missing data | Missing serial-section/3-D cohorts with section geometry, deformation uncertainty, and validation landmarks. |
| WS-81 | data-dependent with named missing data | Missing repeated registered biological units, treatment/time metadata, and clone trajectories. |
| WS-82 | data-dependent with named missing data | Missing eligible treatment/exposure designs, positivity, confounders, temporal ordering, and interference mappings. |
| WS-83 | data-dependent with named missing data | Missing designed perturbation/treatment cohorts with dose/time, controls, and spatial readouts. |
| WS-84 | data-dependent with named missing data | Missing prospective acquisition actions, costs, laboratory constraints, and validation outcomes. |
| WS-90 | planned | Interactive workbench remains in Phase 9 scope after stable project/workflow/result contracts. |
| WS-91 | planned | Server, collaboration, authorization, audit, and remote execution remain in Phase 9 scope. |
| WS-92 | planned | Python/R/notebook clients, bindings, zero-copy artifacts, and recipes remain in Phase 9 scope. |
| WS-93 | data-dependent with named missing data | Missing admitted benchmark datasets, external cohorts, and publication-grade validation partners. |
| WS-94 | planned | Stable 1.0 schemas, migrations, LTS, security review, and claim tiers remain the terminal release workstream. |
| WS-A | complete | Alias grouping for implementation control and baseline preservation closed. |
| WS-B | complete | Alias grouping for workspace replatforming and compatibility shell closed. |
| WS-C | planned | Initial substrate slices through C-06 exist; broader marks/interchange/source promotion remain in total scope after the active classical workflow. |
