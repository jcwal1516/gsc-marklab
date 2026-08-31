# Marklab program tracker

Last updated: 2026-08-31

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
| ACT-01 | data-dependent with named missing data | IC-0132 and IC-0194 provide analytic EIG plus bounded synthetic sequential ROI/stain/landmark/allocation/power workflows. The checkpoint-199 authorized CRC audit found no declared decision candidate, feasible candidate set, budget, utility/loss, outcome model, operational constraints, or pseudo-prospective evaluation split; no real prospective outcome is fabricated. |
| B-01 | complete | Workspace architecture decision and compatibility boundary closed. |
| B-02 | complete | Compatibility shell parity closed. |
| B-03 | complete | Workspace policy and feature-matrix milestone closed. |
| B-04 | complete | Minimal project/workflow vertical slice closed. |
| BACK-01 | active | Seventeen static typed PyMC 6.3.0 workflows, now including fixed-/inferred-kernel independent-patient scalar and multitype exact-window replicated LGCPs plus single-pattern and patient-replicated conditional hard-multitype fits, and POT 0.9.7.post1 run with exact Python-3.12 lock/worker/license/schema/control identity; durable scientific fits retain replay, while narrow pinned NumPyro workers promote the replicated spatial families through agreement, sensitivity, and calibration. Capability/security manifests, doctor/admission validation, CmdStan, GPU evidence, and broader backend classes remain. |
| BAY-01 | active | Typed Gaussian, Student-t, beta-binomial hierarchy, unadjusted and gender-adjusted beta-binomial group regressions, exact fixed-grid LGCP, and patient Dirichlet-multinomial multiclass group composition are runnable and independently fit in PyMC/NumPyro within declared agreement rules. Unified broader likelihood/predictor/field/point-process IR coverage and CmdStan remain. |
| BAY-02 | active | PyMC and NumPyro NUTS diagnostics, audited SMC, differentiated/nested Laplace, PSIS-LOO/comparison, and broad promoted-family agreement/SBC/sensitivity are runnable. Weighted exact-window IPP, scalar and multitype exact-window LGCP, and conditional hard-mark families now have bounded independent agreement, sensitivity, calibration, and applicable durable replay. CmdStan, actual GPU evidence, and broader model-family calibration remain. |
| BAY-03 | active | Gaussian/Student-t ROI hierarchies, beta-binomial/multiclass patient models, and a 217-slide repeated-count hierarchy run on real data; durable eight-patient/16-slide scalar LGCP, conditional hard-mark, and fixed/inferred-kernel multitype location-process hierarchies are runnable with null-compatible group results. Cohort, distinct ROI-within-slide, crossed, varying-slope, ordinal/hurdle, calibrated richer fields, and additional identified structures remain. |
| BAY-04 | active | Exact/multi-output/VFE GP, predictive process, NNGP, and IC-0200 bounded rectangular SPDE are runnable; holed/adaptive SPDE, arbitrary multi-output, anisotropic, nonstationary, and broader fields remain. |
| BAY-05 | active | Stable weights, CAR/GMRF, fixed/fitted SAR, exact-constraint BYM, and scaled-ICAR BYM2 are runnable; broader likelihoods, scale, and calibration remain. |
| BAY-EVO | data-dependent with named missing data | Missing provenance-complete clone probabilities, phylogeny, and replicated longitudinal or 3-D evidence. |
| BAY-FIELD-A | active | Exact 1-D physical Matérn, bounded two-output, VFE, predictive-process, and NNGP foundations are runnable; SPDE is blocked on a promoted exact 2-D window/mesh contract, while general multivariate/anisotropic/nonstationary/calibration work remains. |
| BAY-GMRF-A | active | IC-0047 through IC-0053 cover weights, CAR/GMRF, SAR, exact ICAR, BYM, and scaled BYM2; broader SGLMM and calibration remain. |
| BAY-HIER-A | active | Gaussian known-sigma, robust Student-t, beta-binomial patient, and patient-unit unadjusted plus gender-adjusted beta-binomial MSI/MSS group models have real execution, PyMC/NumPyro agreement, PPC, sensitivity, and SBC; applicable fits replay durably, and exact-conjugate resource-distance response plus site meta-regression also run. Additional identified covariates, ordinal/hurdle, crossed/nested levels, and random slopes remain. |
| BAY-MM | active | IC-0165–IC-0177 and IC-0200 cover paired/multiview, hierarchy, matrix/tensor, GP/graph/SPDE spatial, dropout, joint, comparison, and synthetic validation; matched external patient validation remains missing. |
| BAY-NP | blocked with named prerequisite | Requires Bayesian fit infrastructure and reproducible multiscale niche inputs from WS-60. |
| BAY-PP | active | Exact rectangular IPP/grid/SPDE LGCP, cluster/Strauss/Geyer, multitype, marked, replicated, and PPC mechanics are runnable; the exact-window physical IPP spatial PPC and exact-MultiPolygon CellViT scalar/multitype fixed/inferred-kernel LGCPs replay durably without a second backend. The existing multitype inferred-kernel model now has bounded five-scenario rank SBC and posterior-predictive stress checks. One normalized patient-replicated joint location/conditional-mark model has a real eight-patient diagnostic fit against separate baselines; it is nonconverged and null/uncertain. Correlated cross-type fields and richer identified coupled models remain caller-dependent. |
| BAY-PP-A | active | Exact rectangle-grid/SPDE and exact-MultiPolygon fixed-kernel adapters are runnable with conserved counts. The single-pattern caller is promoted; the replicated fixed field is kernel-sensitive, and a bounded inferred shared-kernel replacement now fits durably, agrees across backends, and has a passing synthetic calibration oracle. Full real-geometry SBC remains 19/20 under two bounded capacities; adaptive refinement, anisotropy/nonstationarity, and broader independent-pattern fields remain. |
| BAY-PP-B | blocked with named prerequisite | Requires BAY-PP foundation, interaction-process simulation, and backend admission. |
| BAY-REG | blocked with named prerequisite | Requires BACK-01, GEO-01, BAY-01/BAY-02, and uncertainty-bearing transforms. |
| BAY-REG-A | active | One centered exact 1-D Matérn varying-coefficient Gaussian workflow is runnable directly and through pinned durable PyMC replay; GMRF, multiple-coefficient, non-Gaussian, 2-D, calibrated real field-level callers, and extensions remain. |
| BAY-SBI | active | IC-0117–IC-0121 supply classical SBI, prior-predictive SBC, and reference-fit KNN/conformal simulation OOD; learned SBI remains blocked by `NEURAL-SBI-01`, while posterior-predictive lab and real calibration remain. |
| BULK-01 | data-dependent with named missing data | Missing specimen-linked bulk molecular measurements, outcomes, and patient/site covariates. |
| C-01 | complete | Typed identities and cohort hierarchy closed in the C-01 handoff. |
| C-02 | complete | Units, coordinate frames, dimensions, transforms, and uncertainty references closed. |
| C-03 | complete | Immutable artifact catalog and table/store boundary closed. |
| C-04 | complete | Bounded CellViT cell-embedding table, provenance, physical formats, and scale evidence closed. |
| C-05 | complete | Bounded patch/region/slide tables, links, graphs, receipts, formats, and finalizers closed. |
| C-06 | planned | Thirteen bounded slices are green; broader general marks remain in scope but are not the current increment. |
| CAU-01 | data-dependent with named missing data | IC-0130–IC-0135 and IC-0192–IC-0195 provide exact randomized mechanics, sensitivity, bounded synthetic observational/perturbation estimators, active design, and partial validation; randomized interference and analytic Gaussian EIG have typed durable project replay. The checkpoint-199 audit found no admitted CRC cohort with a fully defined treatment/exposure, treatment time, follow-up origin/interval, censoring policy, measured adjustment set, negative control, interference definition, and positivity support. |
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
| CMP-01C | active | Exact complete-fingerprint linear/fixed-RBF MMD with unblocked or exact patient-ID-keyed blocked permutations is runnable; spatial fingerprint kernels, leakage-safe trained kernel selection, broader validation, and promotion evidence remain. |
| CMP-01D | active | Exact complete-fingerprint Euclidean energy distance with unblocked or exact patient-ID-keyed blocked permutations is runnable; additional prespecified negative-type metrics, broader validation, and promotion evidence remain. |
| CMP-01E | blocked with named prerequisite | Requires stable graph construction plus cohort-valid evaluation. |
| CMP-01F | active with negative promotion evidence | Whole-patient exact bottleneck/energy permutation mechanics are runnable, and the frozen eight-patient/16-slide CRC witness filtration now has exact durable bottleneck stability plus an exact 70-assignment null-compatible group comparison. Zero of eight patients passes stability, so a stable filtration, external endpoint, and replicated cohort remain missing rather than tuned into existence. |
| COH-01 | active | Scalar, unblocked/blocked 1–32-covariate patient residual, paired, functional, blocked, paired, and serial-gated endpoint-family single-step/step-down Max-T, MMD/energy, fingerprint/region compatibility, equivalence, noninferiority, hierarchical bootstrap, repeated-measures, patient-row adjusted fixed/REML multisite, unadjusted and nuisance-adjusted whole-cluster permutation, and exact randomized-interference workflows are runnable; broader multiplicity/calibration and real multisite/cluster/interference validation remain. |
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
| DIM-01 | active | IC-0126 plus checkpoints 197–198 provide strict physical-unit dimensionality, cuboid and exact finite voxel-union windows, anisotropic metrics, homogeneous 3-D K/L, durable deterministic registered-serial execution with explicit missing planes, and paired patient/lesion/timepoint K-change diagnostics under supplied registration-deformation draws. Watertight mesh/tetrahedral windows, section-level transform-draw propagation, probabilistic cell correspondence, and provenance-complete real serial/longitudinal cohorts remain missing. |
| DL-CORE | blocked with named prerequisite | Requires BACK-01 plus licensed, checksum-pinned model manifests and patient/site validation contracts. |
| EMB-01 | active | Exact methods now have real row-bound TCGA/CPTAC raw 1,280-dimensional summaries and physical-scale vector variograms, bounded cell/field stability, patient/site-held-out retrieval, and orthogonal Schürch CODEX/H&E evidence. The current molecular-class result is null-to-negative; broader edge correction, direct same-schema external agreement, and stable promotion remain. |
| EMB-02 | active with named missing data | Exact real cell-to-patch links now drive cell-aggregated patch, multiscale-kernel, and patient-held-out complementarity evidence; independent raw patch-vector tensors remain absent and are not implied. |
| EMB-CORE | complete | Bounded CellViT single-cell artifact contract and import/format/resource evidence closed by C-04. |
| EMB-PATCH | complete | Bounded synthetic patch/region/slide artifact and explicit-link contract closed by C-05. |
| EMB-PRED-01 | active | Synthetic prediction controls now have one real 96-patient nested M0–M5 CellViT/patch/covariate run; neither the cell nor patch increment was supported, and transportability, calibration, site validation, and independent patch vectors remain missing. |
| EMB-PRED-02 | data-dependent with named missing data | A synthetic calibrated patient-OOF late-fusion baseline with missingness/ablations is runnable; a real matched cohort and demonstrated baseline gap are still required before cross-attention work. |
| EQV-01 | active | Patient-effect TOST equivalence and directional noninferiority with explicit rationale, Student-t interval/bound agreement, and R 4.5.2 oracles are runnable; unpaired/paired-from-raw, functional/bootstrap variants, power guidance, and real clinical validation remain. |
| EQV-01A | complete | Existing descriptive margin is characterized and retained strictly as descriptive. |
| EQV-01B | data-dependent with named missing data | Missing prespecified equivalence margins and sufficient independent biological replicates. |
| EQV-01C | data-dependent with named missing data | Missing directional noninferiority margin, clinical rationale, and sufficient biological replicates. |
| FND-01 | complete | Typed identities, hierarchy, parentage, and design roles closed by C-01. |
| FND-02 | active | Exact bounded ObservationWindow2D owns polygon/multipolygon/hole topology, closed membership, indexed unsigned/signed distance, and physical-frame identity. One exact segment-aligned binary compartment tessellation supplies an oriented shared-interface index, typed cell profiles, and explicit shared/outer/complete-boundary contact fractions with GEOS agreement and durable replay. Multiclass/residual/uncertain partitions, volumes, and broader external geometry agreement remain. |
| FND-03 | active | One exact SpatialGeometryPlan2D/SpatialIndex2D serves homogeneous/inhomogeneous K/L, g, F/G/J, compartment mixing, fixed-radius soft-simplex neighborhoods, and the shared retained pair plan consumed by cross-g and mark methods under hard bounds. General edge-weight/bin plans and broader adoption remain. |
| FND-04 | active | A row-aligned typed MarkTable binds stable CellIds plus binary, scalar probability, continuous, histologic categorical, measured-IHC ordinal, probability-simplex, and verified CellViT vector-reference columns with exact status/provenance. Genuine top-1 probabilities now support conservative multiclass pair bounds; the admitted CellViT pixel-support score instead drives a threshold-free hard-label sensitivity matrix with real durable replay. Complete simplexes retain their existing composition/neighborhood/full-pair owners. CSV/Parquet retain codebooks/source CellIds; broader interchange/export and genuine full-simplex CellViT input remain. |
| FND-05 | active with named missing data | A verified 366-slide CPTAC CellViT corpus supplies exact provenance and 1,542,389 finite raw rows; formal vector callers consume its verified no-copy reference. Frozen source inspection establishes that `type_prob` is winner-type pixel support, not class-posterior confidence, and corrected adapter output retains that identity. Native logits/full class vectors, independent patch vectors, and the model/environment/license graph needed to promote the admitted Schürch NPY/CSV bundles remain absent. Canonical source-import promotion and non-Arrow/Parquet interchange remain active. |
| FND-06 | active | Exact design owners cover patient labels, blocked population-independence MMD/energy/functional/Max-T families, ordered family gatekeeping, scalar and complete endpoint-vector paired signs, subject and unblocked/blocked 1–32-covariate patient residuals, adjusted site/cluster effects, patient-then-specimen bootstrap, whole-cluster labels/residuals, complete cell-mark random labeling, independent fixed-grid source/target location patterns, and clustered randomized interference with fixed outcomes. Broader multiplicity and calibration remain. |
| FND-07 | complete | Native Git/toolchain/feature/executable/input identity, canonical project head, append-only ledger, verified replay, pending recovery, and typed unified invocation are implemented for dependency-free native nodes. |
| FR-01 | active | IC-0145 supplies one canonical physical-radius binary graph, combinatorial Laplacian, exact Fourier transform, and band summaries; broader graph rules/operators and endpoint calibration remain. |
| FR-01A | active | Exact small-graph Fourier, heat, ERL spectrum null, and differentially checked Chebyshev heat are runnable through IC-0145–0149. |
| FR-01B | complete | Exact wavelet/scattering/Chebyshev paths, component-aware sparse bases, component-mean-separated scalar Fourier energy, exact small oracles, coordinate/scale/subsample/nested-slide stability, and patient-held-out inference are runnable. The fixed-eight-mode patient result is unstable and adds zero held-out accuracy, so broader kernels/bands are killed absent a new endpoint rather than counted as missing implementation. |
| FR-02 | data-dependent with named missing data | Links/context/dependency plus synthetic nested M0–M5 complementarity are runnable; promotable matched embeddings, patient-held-out outcomes, and external validation remain missing. |
| FR-02A | complete | Bounded synthetic patch/region/slide tables and explicit links were delivered by C-05. |
| FR-03 | active | Balanced, KL-unbalanced, fixed-mass partial, dustbin, balanced FGW, and fixed-mass partial FGW plans are runnable with strict descriptive semantics and sensitivity; real atlas inputs, external comparison, and KL-unbalanced FGW remain missing. |
| FR-03A | active | Native balanced/unbalanced, pinned-SciPy partial, and pinned-POT fixed-mass partial FGW expose plans, objectives, residuals, unmatched mass, and non-correspondence semantics; broader solver agreement and atlas integration remain. |
| FR-03B | active | Pinned-POT entropic FGW now reports three initialization plans, cost scaling, non-identifiability, and Rust-replayed objectives; fixed-mass partial sensitivity is runnable, while KL-unbalanced FGW lacks an established pinned backend and real atlas validation remains missing. |
| GEN-01 | active | Bounded mechanistic/resource/coupled simulators plus differentiable Gaussian pair-summary matching are runnable; vector/general/reciprocal PDE, neural, SBI inference, validation, and real calibration remain. |
| GEN-01A | blocked with named prerequisite | `NEURAL-GEN-01`: requires a reviewed pinned neural backend/serialization/runtime plus independent-patient patterns/windows/contexts and prespecified classical/LGCP count/K/g calibration evidence. |
| GEN-01B | blocked with named prerequisite | `NEURAL-GEN-01`: requires the pinned learned set-generator backend plus admitted held-out data, privacy/memorization, mode-collapse, cardinality, boundary-support, and simpler-model gates. |
| GEN-01C | gated pending explicit user decision | The master plan prohibits a digital-twin claim without prospective intervention/outcome evidence. |
| GEO-01 | active | One frame-bound exact binary compartment partition provides oriented shared-interface distance, explicit contact, component fragmentation, and fixed-physical-radius typed-cell mixing without aliasing tissue edges or cell replication. Multiclass, uncertainty, objects, broader morphology, patient inference, and real pathology validation remain. |
| GEO-01A | active | Exact aligned negative/positive compartment windows tessellate the analyzed domain, reject gaps/overlaps/frame or segment drift, and index only matched internal interface segments. Multiclass/residual/uncertain partitions and real-mask validation remain. |
| GEO-01B | active | Exact binary contact reports shared-interface, tissue-edge, and complete compartment-boundary lengths plus shared/complete fraction for both oriented roles, with GEOS agreement and durable replay. Multiclass contact matrices, uncertainty, phenotype contact, and real validation remain. |
| GEO-01C | active | One descriptive typed-cell profile retains signed micrometre distance, stable CellId/compartment identity, per-compartment range/mean absolute distance, hard bounds, annotation/geometry agreement, and durable replay. Phenotype-specific distributions, uncertainty, patient inference, and real validation remain. |
| GEO-01D | active | Exact vector-polygon component areas drive largest-component fraction, area entropy, and perimeter/area; physical-radius graphs drive binary/multiclass mixing, directed connection/cross-K, and cross-g. Real stable-ID CellViT validation passes at four radii; broader scale stability, full-simplex uncertainty, and patient inference remain. |
| GEO-01E | blocked with named prerequisite | Requires WS-22 validated polygon/morphology operations and parameter-sensitivity contracts. |
| GEO-01F | data-dependent with named missing data | Missing validated compartment/front annotations and pathology endpoint labels. |
| GEO-01G | data-dependent with named missing data | Missing independently segmented vessel, gland, nerve, and necrosis objects with uncertainty. |
| GEO-01H | data-dependent with named missing data | Missing validated interfaces for curvature and longitudinal observations for propagation claims. |
| GSP-01 | complete | Bounded canonical graph, exact/approximated spectra, heat, diffusion/wavelet/scattering, component-aware shifted-subspace/Ritz modes, signal energy, exact fixtures, perturbations, physical radius identity, durable replay, and patient-replicated CRC validation are implemented. The fixed patient Fourier block fails subsample/scale/nested-slide and incremental-information gates, so no broader transform surface is promoted. |
| GSP-02 | active | IC-0153–IC-0156 provide typed heterogeneous messages, normalized hypergraphs, typed motifs, clique/Hodge mathematics, and research-only cellular complexes. Typed motifs now use bounded sparse wedge closure and durable compact output on one real 2,000-cell graph; patient replication and pathology endpoint validation remain. |
| GSP-03 | blocked with named prerequisite | Requires GSP-01 plus an incremental-value gate over simpler spectral summaries. |
| HET-01 | active | Bounded heterogeneous, hypergraph, motif, simplicial/Hodge, and cellular-complex specializations are runnable; one real 2,000-cell typed-motif capacity result is null-compatible, while patient-level clinical/pathology utility and robustness remain unvalidated. |
| IHC-01 | data-dependent with named missing data | A bounded measured-IHC ordinal contract and durable composition/CDF/median-interval caller are runnable synthetically without interval-code arithmetic. Provenance-complete real continuous/ordinal IHC with batch, threshold, registration, and measurement uncertainty remains missing. |
| INF-01A | complete | Canonical scalar permutation implementation exists and is preserved. |
| INF-01B | complete | Canonical functional global-envelope implementation exists and is preserved. |
| INF-01C | active | Single-step and step-down Max-T over complete prespecified patient endpoint families are runnable for independent, exact-block, paired-vector, and ordered serial-gatekeeping designs; broader graphical/recycling policies and explicit calibration remain. |
| INF-01D | blocked with named prerequisite | Requires FND-06 and method-specific interval/coverage designs. |
| MM-01 | data-dependent with named missing data | Explicit-pair synthetic cross-modal covariance/random-label inference is runnable; matched measured multimodal cohort data, validated correspondence, missing-modality policy, and technical/site covariates remain missing. |
| MOL-01 | data-dependent with named missing data | Missing spatial molecular observations linked to cells/regions with correspondence uncertainty. |
| MRK-01 | complete | Categorical and expected probability connection, normalized continuous mark correlation, and cumulative product-weighted K are separate exact workflows sharing bounded pair geometry, complete-row random labeling, ERL, typed identities, hand/independent oracles, and durable replay. |
| MRK-01A | complete | Exact categorical directed-level and dense binary-probability positive-positive shell connection are runnable with standard-border eligibility, rare/effective-mass guards, analytic expectations, complete-row random labeling, ERL, hard limits, controls, and durable replay. |
| MRK-01B | complete | Positive finite nucleus-area mark correlation uses one fixed global arithmetic mean, standard-border directed shells, compensated f64 product sums, exact finite-row random-label expectation, complete-row ERL, zero-variance/work guards, an independent Python oracle, and durable replay. |
| MRK-01C | complete | Cumulative product-weighted K uses the fixed global arithmetic-mean normalization, exact standard-border eligible centers, identical unweighted K baseline, finite-row random-label expectation, complete-row ERL, hard bounds, controls, independent Python oracle, and durable replay. |
| MRK-02A | active | Exact categorical inputs supply directed source/target pair curves plus a symmetric all-pair conditional hierarchy; real stable-ID CellViT now runs at one slide and across eight independent patients with two slides each. General classical many-pair multiplicity, normalized joint fitting, replicated promotion, and broader interchange remain. |
| MRK-02B | blocked with named prerequisite | Requires completion of the general mark artifact and an immediate order-sensitive caller. |
| MRK-02C | active with named missing data | Exact dense binary probability supports expected positive-positive curves, and complete probability-simplex rows now support a full multiclass expected source-target pair matrix with without-replacement nulls and durable replay. Sampled-label uncertainty remains separate; authorized CellViT exports lack the complete class-probability vectors needed for real soft-multiclass evidence. |
| MRK-02D | complete | Binary centroid, probability cross-covariance, and nucleus-area cross-covariance all consume the same canonical row-bound no-copy CellViT artifact reference with exact CellId/status/QC/provenance identity; typed mismatch fails and the centroid retains store-aware replay. |
| NIC-01 | active | Complete probability-simplex rows support fixed and prespecified multiscale soft neighborhoods with one exact geometry plan and durable replay. Real patient/ROI stability, niches/domains, population inference, and broader validation remain. |
| NIC-01A | complete | A strictly increasing physical-radius list reuses one geometry plan and retains every scale's per-cell expected class mass, aggregate mass, typed zero-neighbor states, total work, adjacent-scale total variation, exact identities, independent oracle, hard bounds, and durable replay without output-selected radii. |
| NIC-01B | blocked with named prerequisite | Requires NIC-01A plus imported assignment uncertainty contracts. |
| NIC-01C | blocked with named prerequisite | Requires BACK-01 and reproducible imported discovery results. |
| NIC-01D | blocked with named prerequisite | Requires stable/imported domains and GEO-01 boundary geometry. |
| NIC-01E | blocked with named prerequisite | Requires NIC-01 domains plus COH-01 compositional/hierarchical inference. |
| NIC-01F | data-dependent with named missing data | Missing independent reference/query cohorts with held-out mapping and missing-cell-type cases. |
| NUL-01A | complete | Conditional homogeneous CSR fixes the exact validated window and observed point count, resamples the whole location pattern with deterministic namespaced streams, and exposes exact simulation/draw-budget evidence. |
| NUL-01B | active | Exact scalar, complete-vector, categorical, dense-probability, and continuous whole-row random labeling are runnable; connection, correlation, and weighted-K workflows reuse fixed geometry with explicit ERL families. Broader typed mark/strata calibration and stable promotion remain. |
| NUL-01C | active | One bounded two-type location-process independence null samples source and target patterns independently from separate fixed fitted intensity grids while conditioning on both counts; broader component simulators, calibration, and population designs remain. |
| NUL-01D | active | Exact typed histologic-compartment conditioning drives durable scalar-semivariogram whole-value random labeling with explicit strata, degeneracy, work bounds, and ERL. Sixteen real slide patterns across eight patients replay durably; 13/16 remain wholly inside their fixed three-bin envelopes, while patient Max-T is null-compatible. Broader conditioned location/mark families and external calibration remain. |
| NUL-01E | blocked with named prerequisite | Requires exact overlap-aware windows and a justified stationarity contract. |
| PATH-01 | blocked with named prerequisite | Requires WS-22/GEO-01 compartments, boundaries, and object geometry. |
| PERT-01 | active | IC-0193 provides a bounded randomized synthetic perturbation/control specialization with research-only mediation; real designed perturbations with dose/time and spatial readouts remain missing. |
| PLAT-01 | active | Immutable artifacts, exact identities, durable heads/ledger/recovery/replay, typed native/PyMC/POT execution, and one bounded parallel/resumable marked DAG are live. Broader project construction and heterogeneous multi-node orchestration remain. |
| PP-01 | blocked with named prerequisite | Complete unmarked standard-border, exact polygon-translation, and visible-arc isotropic K/L workflows are delivered; stable PP-01 promotion still requires remaining FND-02/FND-03/FND-06 contracts, pinned external agreement, broader null calibration, intensity-gradient policy, and scale evidence. |
| PP-02 | active with negative real-scale evidence | Standard-border inhomogeneous K/L and g consume both the persisted Gaussian leave-one-out/fixed-grid pilot and an exact binary-compartment `(n_c-1)/area_c` estimator. Gaussian K/L additionally supports prespecified leave-one-out-likelihood bandwidth selection that is explicitly independent of the final curve. All paths use shared inverse-intensity normalization, explicit conditioned nulls, ERL, hard bounds, and durable replay. The real Gaussian caller fails its grid-support diagnostic, a real exact binary partition is unavailable, and the pinned spatstat environment is absent. Pinned external agreement, broader calibration/corrections, and a stable real scale policy remain. |
| PP-03 | active | Standard-border plus exact polygon translation and visible-arc isotropic homogeneous Epanechnikov g, directed two-level cross-g, and Gaussian/exact-compartment inhomogeneous g specializations use explicit pair bandwidth, typed empty support, appropriate whole-pattern or complete-row nulls, ERL, hard bounds, and durable replay. Corrected families reuse one exact edge artifact per contributing displacement without changing existing bytes. Broader multitype corrections, pinned spatstat agreement, and calibration remain. |
| PP-03A | active | Bounded standard-border, translation-corrected, and isotropic homogeneous plus Gaussian-pilot and exact binary-compartment inhomogeneous compact-support g specializations have independent arithmetic/GEOS/visible-arc/direct-loop oracles, conditioned-pattern inference, finite/resource guards, and user-facing durable replay. Every corrected/inhomogeneous result retains its exact edge or intensity artifact while keeping pair bandwidth explicit. The real Gaussian pilot is inadequate and a real binary partition unavailable. Pinned external agreement and broader null/edge calibration remain. |
| PP-03B | active | Directed cross-K and two-level Epanechnikov cross-g include standard-border, exact polygon translation, source-centred visible-arc isotropic, and separate Gaussian type-specific inhomogeneous workflows with exact source/target counts, typed empty/sparse support, complete-row or independent-location nulls, hard bounds, and durable CLI replay. Standard-border/corrected cross-g have eight-patient evidence; the prespecified real inhomogeneous caller fails its fixed intensity floor without output. Broader multitype coverage, normalized joint fitting, admissible real intensity evidence, and pinned external agreement remain. |
| PP-04 | complete | Exact reduced-sample F/G/J runs on polygon/multipolygon/hole windows with fixed cell-centred probes, whole-pattern conditional CSR, separate ERL envelopes, pinned SciPy nearest-distance agreement, null/control calibration, typed sparse/denominator states, hard limits, and durable replay. |
| PP-04A | complete | Event-to-nearest-distinct-event G uses the canonical exact index, deterministic tie policy, explicit simple-point rejection, boundary-eligible denominators, and independent brute-force/SciPy oracles. |
| PP-04B | complete | Empty-space F uses a fixed declared rectangular cell-centred probe grid, exact window/hole membership, boundary-eligible denominators, spacing/discretization metadata, and the same probes for every null pattern. |
| PP-04C | complete | J is persisted only when F/G exist and `1-F` exceeds the exact configured floor; unavailable radii remain typed and no infinity or non-finite result is serialized. |
| PP-05 | active | The Gaussian estimator persists `n/(n-1)` leave-one-out event rows, deterministic window-mass quadrature, fixed probe intensities/masses, exact digests, finite floors, and hard bounds consumed by inhomogeneous K/L, unmarked g, and separately fitted two-type cross-g. K/L can select from a prespecified increasing bandwidth list by persisted mean leave-one-out log intensity without inspecting the spatial curve. The exact binary-compartment estimator persists `(n_c-1)/area_c` rows, rejects interface/sparse roles, fixes both counts under its exact-window null, and runs durably through K/L and g. Broader cross-fitting/calibration, real admissible evidence, and pinned external agreement remain. |
| PP-06A | complete | Standard reduced-sample border K/L is implemented as A*q(r)/(n*m(r)) with exact eligible-center/ordered-pair counts, hand and brute-force oracles, typed unavailable states, and deterministic CSR inference. |
| PP-06B | complete | Exact polygon/multipolygon translation overlap now feeds homogeneous K/L with whole-pattern conditional CSR, independent rectangle/concave/GEOS oracles, hard Boolean/pair/draw/memory ceilings, durable replay, and a bounded 512-cell CRC capacity run. |
| PP-06C | complete | Analytic segment-circle partitions now provide directed visible-circumference fractions for isotropic homogeneous K/L, with rectangle/hole/boundary/dense-concave oracles, hard angular-work ceilings, durable replay, and a bounded 512-cell CRC capacity run. |
| PP-06D | gated pending explicit user decision | The master plan rejects toroidal behavior by default; admission requires a genuinely periodic rectangular design and explicit user approval. |
| REG-01 | blocked with named prerequisite | Requires BACK-01 and GEO-01 uncertainty-bearing geometry. |
| REG-01A | complete | Existing rigid/affine 2-D registration is characterized and retained at its limited claim. |
| REG-01B | blocked with named prerequisite | Requires BACK-01, GEO-01, and an admitted nonrigid backend with transform-uncertainty output. |
| REG-01C | data-dependent with named missing data | Missing paired multimodal/serial sections with landmarks or correspondence ground truth and uncertainty evidence. |
| SCALE-01 | active | VFE inducing-point and low-rank predictive-process workflows expose explicit approximation state/error bounds; nearest-neighbor, out-of-core, progressive, calibrated-error, and broader scale infrastructure remain. |
| SIG-01 | active | Typed global Moran and Geary inference are runnable on the same exact fixed-radius weights, continuous marks, and blocked randomization design; semivariance integration, broader weights, external agreement, scale, and real validation remain. |
| SIG-01A | active | Global Moran I now exposes explicit binary-symmetric/row-standardized radius weights, centering, typed continuous mark/status, exact frame/window, deterministic random labeling, and compartment-stratified inference with a hand oracle. Broader weights/covariates/calibration remain. |
| SIG-01B | complete | Global Geary C reuses the exact Moran radius weights/design and matches independent binary-symmetric `0.38` and row-standardized `0.2925` hand oracles. |
| SIG-01C | blocked with named prerequisite | Requires SIG-01A, multiplicity ownership, and cohort-valid exploratory-map policy. |
| SIG-01D | blocked with named prerequisite | Requires typed paired variables, direction semantics, weights, and multiplicity. |
| SIG-01E | data-dependent with named missing data | Missing a prespecified pathology hotspot use case and replicated endpoint data. |
| SIG-01F | active | Bounded scalar and raw-vector variograms have exact provenance/frame/bin/pair-plan identity and hard work ceilings. The nucleus-area block runs durably across eight patients/16 nested slides but fails fusion. The complete-vector owner has strict durable identity, direct parity, and 169-patient M4 miss/resume/backend-disabled replay with explicit one-ULP frozen-runtime compatibility. Existing M4 stability/held-out null science remains; scalar edge/directionality, direct same-schema external agreement, and stable promotion remain. |
| SIG-01G | data-dependent with named missing data | Missing co-located or correspondence-qualified bivariate observations. |
| SIG-01H | blocked with named prerequisite | Synthetic distance and graph vector/covariance, leakage-safe projected/kernel, and complete-vector null workflows are runnable; stable promotion requires shared geometry/graph provenance/scale plus canonical vector-input calibration. |
| SPC-01A | gated pending explicit user decision | The master plan rejects Bartlett branding/default use absent a prespecified endpoint and demonstrated advantage. |
| SPC-01B | planned | Multitaper spectral estimation remains a watch item pending a prespecified variance-reduction endpoint. |
| TIME-01 | data-dependent with named missing data | Missing repeated biological units with registered timepoints and deformation-versus-change evidence. |
| TOP-01 | active with negative promotion evidence | All Part VIII functions have bounded experimental workflows, and pinned witness persistence has admitted 2,000-cell plus eight-patient/16-slide exact bottleneck, subsample, scale, nested-slide, and coordinate diagnostics. The current filtration is unstable and null-compatible; independent external pathology validation remains missing. |
| TOP-01A | active with negative promotion evidence | Pinned exact alpha/witness persistence, transforms, comparison, and controls are runnable; the stable-ID CellViT witness filtration now has direct/durable one-specimen and patient-replicated exact bottleneck evidence, plus subsample/scale/nested-slide diagnostics. It fails stability in every admitted patient, and external replication/segmentation uncertainty remain. |
| TOP-01B | active | Supplied-mask Crofton/Euler/morphology, connectivity, and perturbation laboratory are runnable; validated mask/interface basis, endpoint, and cohort remain missing. |
| UX-01 | blocked with named prerequisite | Requires stable PLAT-01/WF-01 project and result contracts for a UI that does not duplicate science. |
| WAV-01A | gated pending explicit user decision | The master plan rejects raster wavelets by default; admission requires a raster-defined question and explicit approval. |
| WAV-01B | planned | Genuine DoG remains in scope for a future explicit raster/image scale-space endpoint. |
| WF-01 | active | Typed keys, store-verified artifacts, exact admission, composition, and replay now include a resource-planned parallel marked pre/post DAG with process-boundary resume plus durable patient-level five-class CellViT Bayesian execution. General heterogeneous schema construction and arbitrary whole-graph scheduling remain. |

## Dependency-ordered workstream coverage

| ID | State | Evidence, named prerequisite, missing data, or gate |
|---|---|---|
| WS-00 | complete | Implementation control plane and repository state installed. |
| WS-01 | complete | Legacy characterization and compatibility matrix closed. |
| WS-10 | complete | Workspace and compatibility-shell phase slice closed. |
| WS-11 | complete | Catalog/store/cache plus canonical durable project heads, append-only execution ledger, verified replay, and interrupted-run recovery are implemented. |
| WS-12 | active | Typed durable execution covers native/POT/PyMC/scientific nodes plus a fixed three-node marked DAG with explicit resource waves, parallel roots, roots-only interruption, cross-process comparison resume, and all-hit replay. General heterogeneous DAG construction and arbitrary whole-graph scheduling remain. |
| WS-13 | active | Durable PyMC patient hierarchies, group regressions, gridded/exact-window IPP/LGCP, the replicated exact-window patient hierarchy, and POT FGW share one closed static descriptor with exact environment/adapter/license/schema/control identity; direct NumPyro agreement callers are pinned for promoted families. Generalized discovery, broader backend classes, doctor/admission validation, and capability/security manifests remain. |
| WS-20 | complete | Typed identity and cohort hierarchy closed by C-01. |
| WS-21 | complete | Coordinate, unit, dimensionality, transform, and uncertainty slice closed by C-02. |
| WS-22 | active | Exact bounded polygon/multipolygon/hole windows, signed/unsigned boundary distance, physical-frame binding, and one segment-aligned binary compartment path are live with GEOS agreement, typed interface/contact, component fragmentation, fixed-radius cell mixing, hard bounds, and durable replay. Multiclass/residual/uncertain partitions, broader morphology, volume windows, and external agreement remain. |
| WS-23 | active | Declared scalar methods plus hard multiclass mixing/pair curves, genuine-top-1 conservative pair bounds, CellViT pixel-support sensitivity weighting, and soft probability-simplex composition/neighborhood/full-pair methods share the bounded row-aligned MarkTable. CSV/Parquet codebooks and canonical source CellIds are retained; broader interchange/export and real full-simplex CellViT input remain. |
| WS-24 | active | Real TCGA/CPTAC CellViT provenance, raw correspondence, coordinates, annotations, hierarchy, cell-patch metadata, and counts are verified and bundled; all three formal cell-vector statistics use one no-copy typed MarkTable reference. Read-only inspection shows the admitted Schürch NPY/CSV shape/header match the frozen source profile, but promotion still lacks its complete model/environment/license provenance graph; independent genuine patch-vector tensors also remain absent rather than substituted by cell vectors. |
| WS-25 | active | CSV/Parquet Pattern ingestion preserves categorical codebooks and source CellIds; canonical patch Arrow artifacts now materialize through exact store/provenance bindings into a spatial caller. SpatialData, AnnData, OME-NGFF, direct MarkTable Arrow/Parquet/Zarr, broader multiscale readers, and export remain caller-dependent. |
| WS-30 | active | Standard-border point/mark methods, translation/isotropic homogeneous K/L/g, Gaussian and exact binary-compartment inhomogeneous K/L/g, prespecified Gaussian bandwidth-selected K/L, binary/multiclass mixing, real four-radius categorical connection/cross-K/cross-g, genuine-top-1 pair bounds, CellViT pixel-support sensitivity weighting, and fixed/multiscale soft-simplex neighborhoods share exact window/index/geometry owners, deterministic traversal, hard limits, and durable execution. Pinned external agreement, broader multitype corrections, and calibration remain. |
| WS-31 | active | Conditional CSR, reused ERL, scalar patient-label, exact paired scalar/vector and subject-residual signs, unblocked/blocked 1–32-covariate whole-patient residual permutation, adjusted within-site effects, patient-then-specimen bootstrap, whole-cluster labels and complete cluster residuals, blocked population-independence MMD/energy/functional/single-step/step-down and ordered-gatekeeping Max-T families, exact randomized interference, complete-vector embedding nulls, and shared blocked cell-mark/variogram schedules are runnable; broader multiplicity and calibration remain. |
| WS-32 | active | Real raw-1,280 CellViT variograms now have corrected isolated field frames, 80% cell-subsample and field-bootstrap stability, patient/site-held-out evidence, and exact sparse-bin blockers. Scalar edge correction, direct external same-schema agreement, and stable promotion remain. |
| WS-33 | blocked with named prerequisite | Requires WS-22 compartments, boundaries, and object geometry. |
| WS-34 | active | Scalar/paired/functional permutation, Max-T, MMD, energy, structured fingerprint distance, patient-first bootstrap, repeated measures, and patient-row fixed/REML multisite inference are runnable; real 163–169-patient TCGA lanes and 34-patient Schürch CODEX validation remain null for current spatial fingerprints, while real multisite endpoint validation remains. |
| WS-40 | active | Typed PyMC and NumPyro NUTS, PyMC SMC, differentiated/nested Laplace, and pinned ArviZ diagnostics/PSIS-LOO are runnable; promoted likelihoods retain agreement/calibration, and the exact 1-D Matérn varying-coefficient family now replays durably. CmdStan, GPU evidence, broader identified families, and generalized artifacts remain. |
| WS-41 | active | Durable ROI, patient-count, binary/multiclass MSI/MSS, repeated-slide count, scalar/multitype exact-window LGCP, and conditional hard-mark hierarchies run on real CellViT inputs. All three eight-patient/16-slide spatial group results are null-compatible and have backend, sensitivity, and calibration promotion evidence. Cohort and distinct ROI-within-slide effects, calibrated richer fields, crossed models, additional covariates, and random slopes remain. |
| WS-42 | active | Exact/multi-output/VFE/predictive-process/NNGP 1-D workflows plus CAR/GMRF, SAR, BYM, and scaled BYM2 are runnable; SPDE/2-D requires promoted exact window/mesh ownership, while varying coefficients are next. |
| WS-43 | active | Fixed/fitted rectangular and exact-window IPP/LGCP plus cluster/Gibbs workflows are runnable; exact-MultiPolygon CellViT has durable fixed/inferred scalar and repeated-patient multitype kernels, with independent backend agreement and analytic generation calibration. The multitype inferred-kernel model now adds five-scenario rank SBC, and a patient-replicated joint location/mark factorization compares against both component baselines on fixed held-out patterns. The real joint fit is retained as nonconverged/null rather than promoted; correlated cross-type fields and richer coupled marked/location models remain. |
| WS-44 | active | Gaussian, Student-t, beta-binomial patient, unadjusted/gender-adjusted beta-binomial and multiclass Dirichlet-multinomial group-regression, exact fixed-grid LGCP, weighted scalar/multitype exact-window IPP/LGCP, single/replicated conditional hard-mark models, five-scenario multitype SBC, and one joint location/mark fit have concrete agreement, sensitivity, calibration, or applicable posterior-predictive callers. Retained diagnostics truthfully include kernel/prior/capacity/nonconvergence failures. Broader families, CmdStan, and actual GPU evidence remain. |
| WS-50 | active | Distance/kernel/covariance/envelope methods now have real CellViT provenance, physical scale, raw variograms, bounded stability, patient/site-held-out evidence, and a stability-selected null M6 fusion. Broader graph-scale calibration, direct external agreement, and stable release policy remain. |
| WS-51 | active with named missing data | Exact real cell-patch correspondence drives cell-aggregated multiscale and patient-held-out complementarity evidence; independent raw patch embeddings and overlap-aware multi-source validation remain missing. |
| WS-52 | active | One real 96-patient labeled CPTAC cohort now exercises nested M0–M5 cell/patch increments with technical, age, compartment, and acquisition covariates; neither increment was supported, and calibration, transportability, site/multimodal replication, and independent patch vectors remain missing. |
| WS-53 | active with data-dependent promotion | Synthetic pCCA, Bayesian pCCA, multiview/matrix/graph-spatial/tensor factors, hierarchy compilation, missing-view prediction, calibrated late fusion, stacking, and context-gated experts are runnable; matched measured cohorts, validated correspondence, calibrated missingness, hierarchy, and patient/site generalization remain missing. |
| WS-54 | data-dependent with named missing data | Missing provenance-complete clone/CNA probabilities, phylogenies, compartments, and cohort outcomes. |
| WS-55 | data-dependent with named missing data | Dustbin, FGW, and fixed-mass partial-FGW descriptive plans are runnable, but paired serial/multimodal sections with landmarks, deformation/correspondence uncertainty, atlas provenance, and downstream validation endpoints remain missing. |
| WS-60 | blocked with named prerequisite | Requires WS-22 geometry, WS-23 marks, and FND-06 cohort-valid designs. |
| WS-61 | active | Canonical homogeneous radius graphs plus bounded typed heterogeneous, hypergraph, motif, simplicial/Hodge, and perturbation-checked cellular structures are live. Sparse typed-motif closure and compact durable output run on the admitted 2,000-node/24,755-edge CellViT graph; broader construction rules and replicated pathology validation remain. |
| WS-62 | complete | Exact Fourier/bands, ERL nulls, heat/signatures/distances, wavelets, Chebyshev heat, scattering, sparse bases, scalar energy, real 2,000-node evidence, perturbations, and patient replication are runnable. The prespecified patient result is unstable/nonincremental, closing the experimental workstream without GPU/framework expansion. |
| WS-63 | active with negative promotion evidence | Bounded topology and morphology are live. Witness topology now has measured 2,000-cell and eight-patient/16-slide execution, subsample/scale/nested-slide and exact bottleneck perturbation evidence, and durable replay. The current filtration is unstable in every patient; external topology replication, segmentation uncertainty, and validated morphology endpoints remain missing. |
| WS-70 | active | `marklab-simulation` now owns seven consumed simulators, including one-way composition, with units, bounds, diagnostics, deterministic replay where stochastic, and resource limits; richer observation/noise models, validation generators, and artifact integration remain. |
| WS-71 | active | Fisher–KPP, scalar reaction-pattern, competition, level-set, vascular/resource response, and one-way vascular→density→interface→agent coupling are runnable with claim ceilings; vector/general PDE, signed/network response, reciprocal coupling, and biological calibration remain. |
| WS-72 | blocked with named prerequisite | `NEURAL-GEN-01` names the absent reviewed PyTorch/JAX-class backend contract and provenance-complete independent-patient calibration/memorization corpus; native simulators do not satisfy learned-generator admission. |
| WS-73 | active | `marklab-sbi` now provides rejection ABC, weighted SMC-ABC, and Gaussian synthetic-likelihood MCMC growth-front specializations alongside IC-0116 summaries; weighted SMC-ABC has typed durable project replay. Adaptive/neural SBI, biological calibration, and real lifecycle integration remain. |
| WS-74 | blocked with named prerequisite | IC-0122 supplies one synthetic two-summary growth-front posterior-predictive laboratory, but full digital-tissue validation still requires validated WS-71–WS-73 models, admitted held-out data, broader catalogs, and privacy/memorization gates. |
| WS-80 | active | IC-0126 and checkpoint 197 provide bounded cuboid plus disconnected/cavity-bearing voxel-union 3-D K/L, canonical deterministic serial-section placement, explicit missing planes, and typed durable replay. Transform-draw propagation, watertight mesh/tetrahedral windows, and real 3-D cohorts with deformation uncertainty and validation landmarks remain missing. |
| WS-81 | active | IC-0123 and checkpoint 198 provide bounded linear-Gaussian filtering/RTS replay plus paired registered specimens retaining patient, lesion, site, biopsy times, treatment interval, cross-time registration, deformation-control, negative-control, and independent-measurement identity. Real paired cohorts, probabilistic cell trajectories, and clone trajectories remain missing; nonlinear and particle specializations remain direct-only. |
| WS-82 | data-dependent with named missing data | IC-0130/IC-0192 supply randomized interference plus synthetic propensity/dose/AIPW/exposure-AIPW/DML/negative-control mechanics, and randomized interference has typed durable replay. TCGA/CPTAC expose no admitted treatment; Schürch has only a partly observed postoperative-therapy flag without treatment time; Stanford has treated/none/unknown without treatment time or a censored survival endpoint. Identification, interference, adjustment, negative-control, and positivity support therefore remain unavailable. |
| WS-83 | active | IC-0193 supplies a randomized synthetic perturbation and research-only mediation specialization; real designed cohorts remain missing. |
| WS-84 | data-dependent with named missing data | IC-0132/IC-0194/IC-0195 supply analytic EIG, synthetic sequential/constrained selection, allocation/power, and partial validation; scalar Gaussian EIG has typed durable replay. No authorized CRC artifact declares a concrete action, candidate set, budget, utility/loss, outcome model, operational constraints, or held-out historical decision outcome, so no prospective result is available. |
| WS-90 | planned | Interactive workbench remains in Phase 9 scope after stable project/workflow/result contracts. |
| WS-91 | planned | Server, collaboration, authorization, audit, and remote execution remain in Phase 9 scope. |
| WS-92 | planned | Python/R/notebook clients, bindings, zero-copy artifacts, and recipes remain in Phase 9 scope. |
| WS-93 | data-dependent with named missing data | Schürch H&E CellViT and CODEX are now admitted as independent/orthogonal CRC evidence, but the exact TCGA 67-feature schema does not transfer and the CODEX molecular-class result is null. A publication-grade same-schema external cohort and genuine patch vectors remain missing. |
| WS-94 | planned | Stable 1.0 schemas, migrations, LTS, security review, and claim tiers remain the terminal release workstream. |
| WS-A | complete | Alias grouping for implementation control and baseline preservation closed. |
| WS-B | complete | Alias grouping for workspace replatforming and compatibility shell closed. |
| WS-C | planned | Initial substrate slices through C-06 exist; broader marks/interchange/source promotion remain in total scope after the active classical workflow. |

## SCIENCE-CRC-FINAL-01 checkpoint 147

The CRC scientific objective is complete without changing broader master-plan state. Existing TCGA
M0--M6, pinned M7, compatible Schuerch H&E, orthogonal Schuerch CODEX, and outcome evidence are
sealed with a bounded patient-replicated graph/topology analysis. The new graph and topology blocks
do not enter fusion because both have zero held-out incremental balanced accuracy and fail at least
one frozen stability gate. Exact unavailable lanes and acquisition-site identity blockers are
retained. Broader active/planned/blocked master-plan entries are intentionally not promoted by this
science-only checkpoint.

## SCIENCE-CRC-FINAL-01 checkpoint 151 addendum

The interrupted exact witness-bottleneck checkpoint is complete and retains a second, independently
named unstable coordinate-perturbation diagnostic: maximum finite exact distance 7,131.39 square
micrometres exceeds the frozen 600-square-micrometre ceiling and essential-interval counts mismatch.
It replays backend-disabled with byte identity and one ledger row. This addendum does not alter the
checkpoint 147 patient-level graph/topology exclusion, M0--M7 conclusions, or broader master-plan
states. No next general capability is promoted under the replacement science-only objective.

## SCIENCE-CRC-FINAL-01 checkpoint 152 final bundle

The final canonical `/Volumes/1TB/marklab/runs/science-crc-final-01-v2` bundle contains 529 verified
artifacts: the complete previously sealed patient analysis plus the exact bottleneck addendum. The
addendum is explicitly one-specimen, unstable, backend-disabled replayed, and excluded from fusion.
No completed analysis was rerun and no patient-level conclusion changed. SCIENCE-CRC-FINAL-01 is
complete; broader master-plan states remain unchanged and no next software capability is promoted.

## SCIENCE-CRC-FINAL-01 checkpoint 155 final patient-evidence bundle

The final canonical `/Volumes/1TB/marklab/runs/science-crc-final-01-v3` bundle contains 666 verified
artifacts and adds the complete eight-patient/16-slide exact-bottleneck result without rerunning any
science. Its 16 backend-disabled replays are byte-identical with one ledger row each. The patient
result is unstable in all eight patients and null-compatible between MSI/MSS, so it remains outside
fusion. The scientific objective is complete; broader master-plan states remain unchanged and no
next software capability is promoted.

## Patient hard-categorical pair checkpoint 156

Hard multiclass pair curves now run through the durable project engine and a bounded frozen
eight-patient/16-slide workflow. Exact raw CellViT correspondence, 64 misses, 64 backend-disabled
hits, slide-within-patient reduction, fold-internal held-out evaluation, and step-down patient Max-T
are verified. The real block is lower-tail unstable and nonincremental, so it is retained without
fusion. MRK-01/MRK-02C, PLAT-01/WF-01, and WS-12/WS-23/WS-30/WS-31 advance but remain broader active
workstreams.

## Real exact-window inhomogeneous K/L checkpoint 157

PP-02 now has a durable project CLI and one real provenance-complete physical-scale caller. Exact
grid-operation identity and backend-disabled replay are verified. The fixed 16x16 grid is inadequate
for the disconnected 12-component window and produces an extreme intensity range, so the result is
retained without promotion or tuning. PP-02, PLAT-01/WF-01, and WS-12/WS-30 advance but remain
active.

## SCIENCE-CRC-FINAL-01 checkpoint 158 final seal

The final v4 bundle adds the already completed eight-patient hard-categorical pair evidence to all
previously sealed M0--M7, graph/topology, external, outcome, and exact-bottleneck evidence without
rerunning any analysis. The sealer verifies 64 misses, 64 backend-disabled byte-identical hits, 64
one-row ledgers, patient nesting, held-out/permutation/Max-T inference, stability, and leakage
blockers. The block is null-compatible, lower-tail unstable, and nonincremental, so fusion remains
unchanged. All 1,905 manifest artifacts rehash. SCIENCE-CRC-FINAL-01 is complete; broader
master-plan states remain unchanged and no next software workstream is promoted.

## Real exact-window inhomogeneous pair-correlation checkpoint 159

PP-03A's existing persisted-pilot estimator now has a user-facing durable project command and one
real provenance-complete caller. The real miss/hit is byte-identical with one ledger row and exact
input/config/runtime identity. The common 16x16 pilot is inadequate for the disconnected window,
so the apparent short-range deviation and minimum-resolution p=0.05 are retained without
interpretation or tuning. PP-03/PP-03A, PLAT-01/WF-01, and WS-12/WS-30 advance but remain active.

## Durable real categorical cross-g checkpoint 160

The existing PP-03B directed cross-g node now has a user-facing durable project command and one real
four-radius CellViT caller. `categorical-pair` and cross-g share only their exact categorical input,
provenance, hierarchy, coordinate, project/store, and output-transaction adapter; their estimators,
configs, schemas, nulls, and codecs remain separate. Frozen v50 categorical-pair bytes are unchanged.
The real cross-g result is null-compatible and structurally unavailable at larger radii, so PP-03B
remains active for patient replication, broader multitype/correction coverage, and external agreement.

## Patient categorical cross-g checkpoint 161

The frozen eight-patient/16-slide design now executes all 64 directed cross-g workflows durably and
reduces them at the patient unit with fold-internal held-out evaluation, exact whole-patient
permutation/bootstrap, and step-down Max-T. Eight endpoints survive all slides, but the block lowers
held-out balanced accuracy by 0.25 beyond M0--M3 with interval [-0.625, 0], and its minimum adjusted
p is 0.921. It is explicitly nonincremental, not promoted, and not fused. PP-03B gains patient
replication but remains active for inhomogeneous/general multitype corrections and external agreement.

## Point-process/mark stabilization checkpoint 162

Checkpoints 154, 156, 157, 159, 160, and 161 pass a single non-loader workspace stabilization:
formatting, warning-denied all-target/all-feature Clippy, no-default workspace compilation,
all-feature doctests, and strict all-feature docs. No production finding is open. The documented
macOS loader loop and phase/release-only compile matrix remain outside this checkpoint. Tracker
states are unchanged; the next active dependency is PP-05's second explicit estimator through an
existing exact compartment caller, without hidden bandwidth or post-result tuning.

## Piecewise binary-compartment intensity/K-L checkpoint 163

PP-05 now has a second separately named estimator through the existing exact oriented binary
partition and a complete user-facing K/L caller. It persists leave-one-out within-compartment event
intensities, conditions the null on both compartment counts, rejects interface and sparse-role
inputs, binds all exact identities and ceilings into durable execution, and replays byte-identically
across fresh processes with one ledger row. The admitted CRC artifacts contain whole observation
windows and categorical labels but no provenance-complete binary polygon tessellation, so real
evidence is blocked without inventing geometry. PP-02/PP-05 remain active for the same estimator's
immediate g caller, bandwidth selection, pinned external agreement, and broader calibration.

## Piecewise binary-compartment pair-correlation checkpoint 164

The DEC-0349 estimator and fixed-count exact-partition null now also feed compact-support
inhomogeneous g through a separate typed API, durable node, and project CLI. The direct unequal-count
oracle independently agrees on every center/kernel normalization term; pair bandwidth is explicit
and cache-bound; fresh processes prove byte-identical miss/hit behavior with one ledger row. Shared
partition preparation, null sampling, g accumulation, and result validation are extracted only
because both concrete callers now use them. PP-05 no longer lacks its piecewise K/L/g callers, but
bandwidth selection, pinned external agreement, broader calibration, and real partition evidence
remain.

## Prespecified Gaussian bandwidth-selection checkpoint 165

Gaussian K/L now has a complete direct, durable, and project-CLI path that scores an ordered
caller-supplied bandwidth list solely by boundary-corrected leave-one-out event intensity, persists
every score and charged evaluation, selects the maximum with a smallest-bandwidth exact-tie rule,
and runs K/L only afterward. Candidate identity and aggregate selection work are cache-bound. The
existing real disconnected-window pilot is inadmissible for selection because its 16x16 quadrature
retains only 37/256 probes; no tuning run is substituted. `Rscript` exists locally but
`spatstat.explore` and a pinned R environment do not, so external agreement remains an exact backend
blocker. PP-05 remains active for pinned agreement and broader calibrated cross-fitting rather than
another hidden/default selector.

## PP-05 estimator-family stabilization checkpoint 166

The checkpoint-163--165 estimator family passes one workspace non-loader stabilization: formatting,
warning-denied all-target/all-feature Clippy, no-default compilation, all-feature doctests, and
strict all-feature docs. No finding changes production or tracker states. The documented macOS
loader loop and phase/release feature matrix remain excluded. The next implementable dependency is
PP-06B's exact polygon-overlap translation correction through the existing homogeneous K/L caller;
pinned PP-05 spatstat agreement remains blocked on an absent repository-owned R environment.

## Translation-corrected polygon-window K/L checkpoint 167

PP-06B is complete through a separate strict translation result family, leaving the existing
standard-border bytes unchanged. Valid canonical polygon/multipolygon windows now compute
`area(W intersect (W+h))` with bounded Boolean work and output complexity; each unordered pair
contributes both ordered `area/overlap` weights before `n(n-1)` normalization. Rectangle, concave,
and static GEOS 3.14.1 holed-multipolygon oracles agree, and zero-measure overlaps fail explicitly.
The durable project CLI binds raw source, parsed point/window, full configuration, native runtime,
and `geo` adapter identities and replays byte-identically without execution. A frozen 512-cell
CPTAC window completes at 20 micrometres in 6.68 seconds with 22,528,000-byte maximum RSS, then
replays backend-disabled with one ledger row. This is bounded capacity evidence, not a biological
claim. Production next advances PP-06C visible-boundary/isotropic correction only through the same
homogeneous K/L caller and an independent arc-fraction oracle; no correction registry is promoted.

## Isotropic visible-arc K/L checkpoint 168

PP-06C is complete through its own strict isotropic result family. Each directed pair partitions
the exact polygon/multipolygon boundary at analytic segment-circle intersections, classifies open
arcs by midpoint membership, and contributes the reciprocal visible circumference fraction.
Rectangle, square-hole, boundary-centered, and million-angle concave differential oracles agree;
zero visible measure and exact/one-short angular work fail explicitly. Durable source/config/runtime
identity, stable numeric JSON, backend-disabled replay, and one ledger row are verified. The frozen
512-cell CPTAC capacity run completes in 6.28 seconds at 22,478,848-byte maximum RSS and remains a
one-specimen p=0.10 diagnostic. PP-06D remains gated on explicit periodic-design approval and is not
implemented. Production next applies the now-validated translation geometry to the existing
homogeneous PP-03A pair-correlation caller before considering isotropic g; no correction registry
or automatic selection surface is introduced.

## Corrected point-process stabilization checkpoint 171

The four-workflow translation/isotropic K/L/g sequence passes workspace formatting,
warning-denied all-target/all-feature Clippy, no-default compilation, all-feature doctests, and
strict docs without a finding. The macOS Nextest/full-integration loader loop remains excluded.
PP-01's native correction ladder is complete, while PP-03/PP-03A and WS-30 remain active for
pinned external agreement, broader multitype callers, and calibration rather than new correction
infrastructure.

## Corrected categorical patient sensitivity checkpoint 174

The existing patient owner now consumes both exact corrected categorical cross-g workflows under
separate identities and exact geometry ceilings. Each authorized eight-patient run proves 64
misses, 64 backend-disabled byte-identical hits, and one ledger execution. Translation and
isotropic blocks are individually null-compatible and nonincremental beyond M0/M3, so the
prespecified fusion gate excludes them. This closes the corrected categorical sensitivity needed
by SCIENCE-CRC-FINAL-01 without completing or promoting broader PP-03/WS-30 catalog work. The next
tracker action is only the remaining real CRC graph/topology analysis and final scientific seal.

## SCIENCE-CRC-FINAL-01 read-only completion audit checkpoint 175

Checkpoint 158 already owns the requested final patient-level graph/topology, M0--M7, external,
outcome, interpretation, and durable-replay bundle. The fresh 1-TB audit verifies all 1,905 listed
artifacts, 64 categorical-pair and 16 patient-witness byte-identical hits, and their one-execution
ledgers without rerunning science or backends. The conclusion and frozen fusion exclusions remain
unchanged. SCIENCE-CRC-FINAL-01 is complete, and no broader master-plan state is promoted.

## Real CellViT contour-area scalar variogram checkpoint 178

SIG-01F's immediate real scalar caller is now available: exact frozen CellIds and source-payload
hashes bind bounded CellViT contours and recorded base MPP to a physical
`nucleus_area_um2` morphology-prediction mark. Default categorical preparation remains
byte-identical. The first fixed 512-cell slide completes three prespecified lag bins, a
histologic-compartment whole-value null, and backend-disabled replay with one ledger execution. Its
global p=1.0 result is retained as null-compatible one-specimen capacity evidence. SIG-01F and
NUL-01D remain active for patient replication and independently justified correction/calibration;
no generic importer, new method family, or patient-level claim is added.

## Patient CellViT scalar-variogram checkpoint 179

SIG-01F now has a patient-unit real scalar lane over the frozen eight-patient/16-slide subset.
Every slide completes and replays the same three-bin compartment-conditioned null; slides reduce
inside patients before held-out classification, exact patient permutation, bootstrap, and Max-T.
Median nested-slide stability is 0.881, but the M0/M3 balanced-accuracy increment is 0.0 and all
adjusted endpoint p-values are 0.099. The block is not fused. This completes scalar patient
replication for the admitted caller without completing broader edge/directional/external-calibration
coverage or changing SIG-01F/NUL-01D active status.

## Durable raw-vector semivariogram checkpoint 180

The existing complete-vector semivariogram now executes as a cache-addressed project node with
strict source/bin/weight/native-runtime/limit identity and exact direct-CLI result compatibility.
The real 512-cell by 1,280-dimension input completes and replays backend-disabled with one ledger
row. This advances WF-01/WS-12 for the admitted vector caller but is one-slide capacity evidence;
the existing sealed M4 patient inference remains the population result. SIG-01F stays active for
durable patient M4 execution and independently justified correction/external-calibration gaps.

## Durable patient M4 raw-vector checkpoint 181

All 169 admitted patient M4 inputs now use the durable vector project path with six bounded
processes, cross-process completion recovery, fresh backend-disabled hits, current-runtime byte
identity, frozen-reference structural identity, and one ledger each. Twenty-seven values differ
from the older runtime by exactly one ULP; no other field differs. This completes durable patient
M4 execution without changing its sealed null/stability/held-out findings. SIG-01F remains active
only for the independently named correction, external-agreement, and promotion gaps.

## Scalar/vector durable stabilization checkpoint 182

Checkpoints 178--181 pass workspace formatting, warning-denied all-target/all-feature Clippy,
workspace no-default compilation, all-feature doctests, and strict docs without a finding. The
documented loader loop is not retried. SIG-01F/NUL-01D and WF-01/WS-12 evidence is current through
real patient durable replay; active status remains only for independently named statistical and
external-validation gaps.

## Durable projected embedding-variogram checkpoint 183

The existing leakage-safe projected CellViT workflow now runs through the durable project engine
with exact SciPy/Python/lock/worker/input/bin/configuration identity, bounded process execution,
typed replay validation, direct-result parity, and a fresh backend-disabled hit backed by one
ledger row. Its admitted real result is unchanged from the frozen PCA/bin/curve evidence. This
advances EMB-01, SIG-01H, WF-01, and WS-12 only for this concrete caller; it does not promote the
block, generalize backend infrastructure, or change the existing null/unstable CRC conclusions.

## Full-tissue gastric CellViT interim checkpoint 184

Four of seven completed full-tissue gastric slides now have exact all-cell composition/nonspatial
embedding summaries plus sixteen label-blind bounded spatial fields. Existing durable sparse graph
and pinned topology callers complete and replay backend-disabled at 80 and 64 one-row executions.
Graph median field-rank stability is high, topology is lower-tail mixed, and one complete pre/post
pair remains descriptive. This is provisional real-data evidence for EMB-01, TOP-01, WS-61,
WS-62, and WF-01; it changes no capability state, applies no promotion gate, and does not count fields or
slides as independent patients.

## Dense durable witness and 64-landmark gastric checkpoint 185

The two existing witness project callers now carry a concrete 32-MiB typed-result ceiling proven
above the former one-MiB limit without changing their backend or mathematical contracts. The
provenance-complete four-slide gastric design therefore completes and backend-disabled replays all
64 topology requests at 512 witnesses and 64 landmarks, while the separate 80-workflow graph proof
remains complete. The lower-tail topology instability is retained and no promotion gate is applied.
This advances TOP-01, WF-01, WS-12, and WS-62 for the real dense caller but does not complete their
broader catalogs. Under the resumed full-program mandate, the next immediate workflow is durable
embedding cross-covariance by distance through its existing direct CLI and analytic oracle.

## Durable embedding cross-covariance checkpoint 186

The existing IC-0083 full-matrix embedding cross-covariance now executes through a bounded typed
project node with exact native/source/bin/configuration identity and backend-disabled durable
replay. Its analytic matrix/rotation oracle and direct bytes remain unchanged. A deterministic
600-row held-out view of the admitted projected CellViT artifact completes within 46,003,200 matrix
operations; the full 3,000-row table remains correctly outside the fixed work ceiling. This
advances EMB-01, SIG-01H, WF-01, WS-12, and WS-32 only for the concrete caller and makes no
patient-population claim. The next immediate workflow is durable IC-0085 kernel mark correlation;
IC-0084 remains data-blocked on matched cross-modal correspondence.

## Durable embedding kernel mark-correlation checkpoint 187

The existing IC-0085 training-frozen kernel mark correlation now executes through a bounded typed
project node with exact parsed-byte source/bin, kernel/configuration, native runtime/executable, and
result-schema identity. Core row/split/bin invariants and conservative radial scale-fit memory are
admitted before durable state exists. Its independent analytic oracle and direct output remain
unchanged. The complete admitted 3,000-row/30-patient/16-component caller fits the fixed
57,561,600-component-operation ceiling and replays backend-disabled with one ledger row. This
advances EMB-01, SIG-01H, PLAT-01, WF-01, WS-12, and WS-32 for the concrete caller only; it makes no
patient-population, recurrence, molecular, or significance claim. The next immediate workflow is
durable IC-0086 embedding spatial-dependence envelope; IC-0084 remains data-blocked.

## Durable embedding-envelope checkpoint 188

IC-0086, EMB-GLOBAL-ENV-01, PLAT-01, WF-01, WS-12, and WS-32 advance for one corrected concrete
caller. Pair construction now shares the declared permutation-stratum boundary with donor
shuffling, and the 960-cell/39-ROI result durably replays without execution. Broader EMB-01 and
patient-population inference remain active; IC-0084 remains blocked on matched cross-modal rows.

## Durable graph-signal checkpoint 189

IC-0087, IC-0088, and IC-0089 now have exact native project execution in addition to their existing
direct APIs and analytic oracles. This advances EMB-GRAPH-ENERGY-01, EMB-GRAPH-PERM-01,
EMB-LOCAL-ROUGH-01, PLAT-01, WF-01, and WS-12 for the admitted 960-cell graph. It does not change
the failed patient-level graph fusion gate or promote cells/edges as population replicates.

## Durable patch-summary and patient-inference checkpoint 190

EMB-MULTISCALE-KERNEL-01, EMB-COMPLEMENT-01, COH-MMD-01, COH-ENERGY-01, COH-01, PLAT-01,
WF-01, WS-12, WS-34, and WS-51 advance through six concrete project commands and five admitted
real miss/hit replays. Existing patient-level null results are retained. Ordered hierarchical
Max-T is production-complete but lacks a prespecified real family assignment; genuine independent
H-Optimus tensors and exact links remain unavailable inputs. No generic cohort/project framework is
counted as progress.

## Durable adjusted whole-cluster checkpoint 191

COH-01, FND-06, PLAT-01, WF-01, WS-12, WS-31, and WS-34 advance through one concrete durable
cluster-covariate workflow and a real 167-patient/22-site TCGA replay. Site is the population unit,
stage ordinal is fixed nuisance adjustment, and the 50-micrometre coordinate-L result is
null-compatible. This closes the missing real adjusted whole-cluster caller for the available
site-consistent COAD/READ design; broader calibration/multiplicity remain active, and mixed-within-
site MSI/MSS labels are not misrepresented as a cluster-level assignment.

## Durable patient-first hierarchical-bootstrap checkpoint 192

COH-HBOOT-01, COH-01, FND-06, PLAT-01, WF-01, WS-12, WS-31, and WS-34 advance through one
concrete durable hierarchical-bootstrap workflow and a real 627-specimen/169-patient TCGA replay.
The 50-micrometre coordinate-L interval is descriptive nested-sampling evidence under the existing
specimen-row mean estimand. No group, equivalence, causal, molecular, or clinical interpretation is
added, and no generic resampling framework is counted as progress.

## Durable adjusted multisite checkpoint 193

COH-01, FND-06, PLAT-01, WF-01, WS-12, WS-31, and WS-34 advance through one concrete durable
adjusted multisite workflow and a real 126-patient/nine-site TCGA MSI/MSS replay. Stage-adjusted
random-effects pooling is null-compatible, preserves patient as the population unit, and retains
heterogeneity, prediction, and leave-one-site-out uncertainty. Sites failing prespecified group-
count/rank/df support remain explicitly excluded. The checkpoint-191--193 cohort durability family
passes one non-loader workspace stabilization; broader multiplicity/calibration remain active.
