# Interface contracts

Status: characterization freeze for WS-A. Exact field/symbol inventory is active under A-03.

## IC-0001 — Current marked analysis compatibility contract

- Input: strict current `Config` plus a finite `Pattern` loaded through supported CSV/Parquet paths.
- Observable output: result-format 0.3 marked result and referenced artifacts; current CLI/library parity.
- Scientific meaning: structure-factor, centered mark-pair covariance, anisotropy, and named multiscale residual diagnostics under the configured fixed-position label-null policies.
- Preserved invariants: deterministic seeds, finite serialization, typed availability, memory budgets, transactional final output.
- Prohibited reinterpretation: these outputs are not Ripley K/L, classical g, wavelets, tissue domains, patient-level inference, or equivalence.
- WS-B rule: compatibility output must be characterized before any move and remain identical unless a versioned breaking migration is explicitly approved.

## IC-0002 — Current multimodal compatibility contract

- Input: current multimodal config plus H&E/IHC cells, labels, and optional landmarks/transform settings.
- Observable output: result-format 0.3 multimodal summaries/artifacts and CLI behavior.
- Scientific meaning: registered coordinate-frame fusion, graph-edge enrichment, raw cross-label pair-count curves, MMR-abnormal territories, and nearby cell-type profiles.
- Preserved invariants: one reusable index/graph plan, transform QC, deterministic nulls, typed unavailable states.
- Prohibited reinterpretation: no same-cell identity, biological correspondence, cross-K/cross-g, general niche/domain, or causal signaling.

## IC-0003 — Current pre/post and margin contract

- Input: compatible already-aggregated marked or multimodal results.
- Observable output: aligned descriptive comparisons, explicitly approximate pooled-bin diagnostic, and descriptive margin assessment.
- Biological unit: not inferred by this layer.
- Prohibited reinterpretation: no patient-level population difference, noninferiority, or equivalence claim.

## IC-0004 — Result-format 0.3 and artifact boundary

- Stable envelope: strict versioned tagged results with unknown-field rejection and finite persisted values.
- Unavailability: typed state/reason, never NaN, infinity, empty-success, or numeric zero sentinel.
- Artifacts: large/local tables and curves remain referenced artifacts; final writes are transactional.
- Migration: semantics that are absent in 0.3 cannot be invented during conversion.

## IC-0005 — WS-A no-production-change boundary

- Allowed tracked writes: root `AGENTS.md` and `docs/implementation/**`.
- Baseline commands may create ignored build, benchmark, fuzz, package, and smoke outputs.
- Any source, test, manifest, dependency, schema, or workflow change stops WS-A scope and requires a recorded task/decision.

## IC-0006 — WS-B compatibility facade

- The existing crate-root API listed in `src/lib.rs` remains callable from package `marklab`.
- Existing `marklab` CLI command spellings and arguments remain accepted.
- Configuration 0.2 remains strict and is not repurposed as project/workflow configuration.
- Result-format 0.3 DTOs and artifact projections remain closed and unchanged.
- A compatibility path calls each existing canonical scientific implementation; it may not copy a formula or construct a second output projection.
- Mechanical source movement, if selected, is isolated from semantic changes and must retain Git history.

## IC-0007 — First project/workflow vertical slice

- Input: an immutable reference to the current marked-analysis inputs and configuration, plus one typed workflow node that owns the invocation.
- Output: the same current `MarkedPatternResult`/0.3 result and artifacts as direct `AnalysisEngine` execution.
- Cache identity: a deterministic content key over node version/specification, input/config content, declared execution policy, scheduler retained-artifact limit, and implementation identity. Cache support is added only for the demonstrated slice.
- Canonical output: a miss reaches a decode/re-encode fixed point before commit; a hit verifies digest/length and decodes the exact retained bytes. The inline cap bounds retained artifact size, not codec peak allocation.
- Failure: invalid/cyclic graph, missing/digest-mismatched input, resource failure, unstable codec, or analysis error remains an error; no successful node/result entry is committed.
- Non-goals: broad plugin system, remote scheduler, general schema registry, scientific migration, or arbitrary task runner.

## IC-0008 — Typed in-memory cohort hierarchy

- Identity: opaque, exact, 1–255-byte typed IDs for site, patient, timepoint, specimen, block, slide, section, core, region, cell, and patch; text never supplies parentage or replication.
- Relations: one enumerated containment parent plus an independent biological source for biological subsamples and technical replicates. A donor TMA core may therefore belong to a donor unit while being contained by a recipient slide.
- Replication safety: only patient/specimen objects may be declared biological units; cell/patch/site/timepoint objects are structural, and lower physical objects cannot become independent units from row count.
- Nested levels: nearest resolution remains specific, while explicit lineage membership permits downstream selection of an enclosing declared patient above biological specimens. C-01 itself performs no randomization or population inference.
- Repeated designs: declarations contain at least two distinct same-kind observations, one set per biological-unit/observation-kind pair, and globally unique observation membership. Retained order is deterministic but not temporal.
- Persistence: this is an immutable in-memory 0.1 boundary installed once into `MarklabProject`. C-03 supplies the schema-bound artifact substrate but does not serialize the hierarchy; a later DATA-01/WS-25 producer must define a streaming durable codec, schema evolution, and cross-artifact alignment. Result-format 0.4 remains separately gated.

## IC-0009 — Explicit in-memory coordinate substrate

- Identity and frames: opaque typed frame/transform/uncertainty IDs name ordered X/Y or X/Y/Z frames. Every frame declares image-versus-physical space, exact unit, and an image pixel-center/corner convention where applicable; names never supply semantics.
- Coordinates and maps: finite fixed-arity coordinates carry a frame ID. Forward affine matrices are finite, same-dimensional, explicitly directed, and interpreted in declared source-column/target-row axis order. Callers provide every transform chain; no inverse, path search, equal-unit identity, or missing-MPP calibration is inferred.
- Uncertainty: an optional reference is metadata expressed in a transform target frame. An absent conservative radius is not zero, and C-02 does not compose or propagate uncertainty.
- Serial sections: observed/distorted entries use a specialized physical 2-D placement into semantic volume X/Y plus explicit finite Z; missing entries retain a gap without placement. Distortion requires target-volume uncertainty. Only parallel sections are represented; no correspondence, interpolation, or oblique plane is implied.
- Validation: declaration duplicates precede missing/incompatible references; transform cycles use iterative declaration-order DFS; serial ordinals and Z are strictly ordered. Operations reject wrong dimension, space, frame, unit, and non-finite results through typed errors.
- Compatibility and persistence: this is an immutable `marklab-core`/`marklab-data` 0.1 substrate. Current root registration/config/result APIs are unchanged. C-03 supplies generic artifact digests/catalogs but no coordinate payload codec; durable coordinate schemas, migrations, interchange conformance, and result references remain DATA-01/WS-25 work.

## IC-0010 — Immutable artifact catalog and verified local store

- Identity: `ArtifactRef` binds exact encoded bytes; `ArtifactId` separately binds schema/version, content identity, exact table declaration, dependencies, and bounded semantic metadata. Locations never affect identity.
- Tables: Arrow IPC file and Parquet file manifests declare exact encoding version, row count, ordered columns, and stable scalar primary keys. A declaration alone is not physical-format proof. C-04/C-05 add exact bounded proof only for their embedding and link profiles; WS-25 remains responsible for general table/interchange conformance.
- Catalog: format `marklab.artifact_catalog` version 1 is strict, bounded, dependency-valid, and canonical to one final-newline JSON fixed point. No prior-version migration is invented.
- Location: catalogs contain portable store-relative keys and optional provenance versions, never absolute paths, endpoints, queries, credentials, or authorization tokens. Managed keys derive only from the artifact ID.
- Local integrity: one capability root confines operations; verification streams a regular file and checks length plus digest. Publication is synchronized, atomic no-replace, and directory-synced; recovery exclusively quarantines only recognized regular staging entries and reports partial states.
- Workflow: catalog-aware nodes must bind a store and verify every semantic input before cache lookup. Integrity failure executes nothing and never degrades to a miss. Nodes with no semantic inputs retain the exact B-04 cache-key path.
- Compatibility and limits: result 0.3, config 0.2, `OutputWriter`, current CSV/Parquet IO, and scientific methods remain unchanged. C-04/C-05 embedding-specific physical validation composes with this substrate; mutable project heads/ledgers, general physical/interchange validation, C-01/C-02 codecs, live cloud, and Windows runtime durability evidence remain separately gated.

## IC-0011 — Canonical single-cell embedding substrate

- Identity and shape: every selected typed `CellId` has exactly one canonical row in strictly increasing expected-set order. A table has one nonzero fixed dimension, contiguous finite canonical `f32` storage for present vectors, exact artifact bindings, and a physical-format-independent logical digest.
- Validity: `present`, `missing_vector`, `extraction_failed`, and `qc_rejected` are the complete extraction-validity vocabulary. Only present rows expose borrowed vectors; private positive-zero filler for non-present physical slots is never a usable vector or public value.
- Source boundary: the only C-04 CellViT source profile is exact little-endian C-order NPY `f32` plus the frozen CSV schema and explicit one-to-one source identity map. NPY object/pickle, implicit casts/transposition, positional identity inference, row dropping, imputation, unknown CSV profiles, and derived non-bijective bundles are rejected before promotion.
- Spatial/provenance boundary: expected cells, source identity, by-value calibrated spatial context, row link, model/checkpoint/source/license, preprocessing/run/environment/converter, and tensor contract are distinct exact artifacts. A candidate becomes a table only after an unforgeable verified artifact graph binds their exact IDs/digests and output dimension.
- Physical boundary: canonical Arrow IPC file and Parquet file profiles have version-owned schemas/metadata/writer bytes. Raw Footer/Message or compact-Thrift/page preflight enforces caller budgets before stock decode; managed reads retain one verified descriptor and at most one charged batch/row group. Logical table/link/QC identity must match exactly across domain, source, Arrow, and Parquet paths.
- Scale and claims: the verified substrate covers a 10,000 × 1,280 import/physical/DHAT smoke, a 1,000,000 × 256 fixed-order workload under the host RSS gate, and aggregate-only reconciliation of 32 authorized 1,280-dimensional bundles. It supplies no embedding statistic, inference, classifier, patch link, model execution backend, real-corpus promotion, result-0.3 field, or biological claim.

## IC-0012 — Canonical multiscale embedding substrate

- Typed tables: patch, region, and slide embeddings use distinct public typed tables over a private sealed matrix core. Each table has one owning slide, exact expected set, support, provenance, dimension, `f32` storage, closed extraction status, domain-separated logical identity, and factual QC; C-04 cell wires, digests, profiles, and public behavior remain frozen.
- Patch support: one exact per-slide/per-scale context binds C-02 frames and transform, MPP, source extent, patch/stride/overlap, receptive field, and boundary policy. A once-per-patch half-open footprint set owns sampled support; a deterministic graph owns actual positive-area patch overlap and minimum-`PatchId` connected-component identity. Neither artifact is automatically a tissue or inference window.
- Vector-free links: cell assignments store each anchor once and reference shared patch rows through complete all-containing or explicit one-to-four interpolation edges, including closed zero-edge states. Indexed containment has explicit retained, peak-working, and per-pass candidate-check budgets; degenerate bucket occupancy cannot create unbounded silent CPU work. Patch-region links derive only from one complete declared assessment, encode its checked Cartesian count and exact canonical nonzero fractions, and include both the borrowed assessment and new link in construction-peak accounting. Hierarchy proves ancestry, not geometry; FND-02 retains geometric truth.
- Provenance DAG: version one admits direct patch extraction plus deterministic region/slide derivation only. Strict canonical source chain, normalization, four support variants, weighted/arithmetic derivation, cell-link producer, exhaustive assessment, and all four multiscale provenance values/codecs exist. Direct-patch structural validation checks eighteen exact record roles, nested dependencies, canonical JSON bytes, cross-value bindings, structural footprint/overlap manifests, and each required managed replica before returning a runtime-only graph token. That token remains structural only, but it is required to mint exact physical footprint and overlap receipts; compatible receipts compose into a format-neutral patch-support receipt. Full Arrow/Parquet patch-table verification then checks expected-order PatchId/status equality with the graph-bound source-row link before dimension/support/provenance/logical bindings and mints a compact direct-patch table receipt carrying exact physical identity and recomputed QC. A separate vector-independent cell-link input graph checks nine exact roles, canonical expected/context/producer payloads, managed evidence, link bindings, and a full physical footprint decode without requiring a patch-vector artifact. It authorizes separate physically decoded assignment and edge receipts; exact graph/link/dependency/mode/digest/count bindings plus distinct physical IDs authorize a format-neutral paired link receipt. A separate six-role patch-region input graph checks exact expected-patch, expected-region, context, footprint, converter, and exhaustive-assessment records, streams the four canonical payloads, verifies converter availability/content binding, and fully decodes the footprint artifact. Exact graph/link/dependency/evidence/digest/count bindings then authorize one physically decoded patch-region receipt. Region-from-patches support composes the patch-support and patch-region receipts only when expected-patch, patch-context, and footprint lineage agree. Separate derived-region and two derived-slide graphs bind verified lower tables, exact expected sets, support, deterministic derivation contracts, execution roles, profiles, dependencies, canonical payloads, managed integrity, owning slide, and output dimension. Their tokens contain no output values or table authority; candidate-only finalizers recompute fixed-order values under explicit work/allocation bounds before full physical validation can mint compact region/slide table receipts. Opaque source-vector or coordinate availability never proves component or anchor correspondence, and producer-declared patch-region fractions never prove geometry without later admitted receipts.
- Physical boundary: all eight exact schema families—three matrices, footprints, overlap edges, cell assignments, cell edges, and patch-region links—implement narrow bounded Arrow IPC and Parquet proof using the locked C-04 raw-before-stock, deterministic-writer, integrity, and charged batch/row-group policy. Matrix profiles expose typed patch/region/slide write, preflight, full borrowed/managed validation, and publication APIs with exact three-role records, nine-key metadata, QC/logical parity, and no materializing read API. Footprint, overlap, assignment, edge, and patch-region profiles additionally issue graph-bound runtime receipts. Assignment and edge halves are independently publishable in either format, and their pair receipt accepts all four Arrow/Parquet combinations after format-neutral semantic binding; overlap continues to require a same-format compatible footprint receipt. Direct patch plus deterministic region and both slide paths have exact graph/finalization/physical-receipt authority. Receipts and candidates are unforgeable and nonserializable.
- Corpus and claims: authorized patch candidates remain aggregate inventory only; no HDF5/NPY/CSV/JSON production adapter, canonical real table, region/slide source profile, observation window, formal embedding-dependence statistic, prediction, or biological result is admitted. IC-0013 adds only one synthetic descriptive patch-overlap computation. Real promotion requires exact identity, context/support, adapter correspondence, complete provenance, reviewed snapshot, and license evidence.

## IC-0013 — Measurement-declared patch-overlap embedding dispersion

- Status vocabulary: `MeasurementStatus` is exactly `Measured`, `ImportedPrediction`, `MorphologyPrediction`, or `DerivedSummary`. It describes how a value was obtained and is independent of the existing `EmbeddingStatus` extraction-validity states. Direct patch provenance requires `MorphologyPrediction`; every deterministic region/slide provenance variant requires `DerivedSummary`. This slice admits no measured/imported adapter.
- Input binding: the computation accepts one canonical `PatchEmbeddingTable`, patch `MultiscaleEmbeddingSupport`, exact `PatchOverlapGraph`, exact direct-patch `MultiscaleEmbeddingProvenance`, an explicit status, and a maximum component-operation count. Table, provenance, support, expected-patch, owning-slide, dimension, footprint, and overlap artifact/logical identities must agree before any numeric work.
- Estimand: for canonical overlap edges whose two table rows are `present`, compute each squared Euclidean distance by promoting components to `f64`, subtracting and squaring in component order, then add edge distances in canonical graph order and divide once by the eligible-edge count. Signed zero is canonicalized positive. Non-present endpoints are excluded without exposing filler vectors; present all-zero vectors remain eligible.
- Availability and resources: the returned status is `Available` when at least one eligible edge exists and `InsufficientPairs` otherwise. An unavailable result carries no floating value. Required work is the checked upper bound `edge_count * dimension`; a one-short caller budget fails before traversal. Runtime is `O(E log P + E*D)` with `O(1)` additional retained storage under the existing canonical ID lookup.
- Claim ceiling: this is descriptive mean squared Euclidean embedding distance across declared overlapping sampled patches. It supplies no spatial-weights/window abstraction, inferential null, p-value, edge correction, biological interpretation, real-source promotion, result/config/CLI change, general MarkTable, unit/threshold/missingness ontology, physical format, validator framework, or receipt.

## IC-0014 — Version-one declared scalar-pattern workflow

- Row identity and frame: the in-memory input borrows one unchanged compatibility `Pattern` and one strictly increasing one-to-one `CellId` slice. Every cell must exist under the explicit owning `SlideId` in the project hierarchy. The project coordinate registry must contain the explicit physical, two-dimensional, micrometre `[X,Y]` frame asserted for `x_um/y_um`. This is a caller declaration, not source-correspondence proof. Row count and total CellId text are checked against caller maxima.
- Mark declarations: one bounded stable binary mark ID/display label, non-`DerivedSummary` `MeasurementStatus`, unitless value kind, and exact C-03 provenance `ArtifactId` are mandatory. An optional dense probability declaration has its own stable ID, status, and provenance. Every compatibility validity flag must be present, and scalar values must be one per row, finite, and in `[0,1]`; undeclared probability values and missing declared values are errors.
- Threshold semantics: binary origin is either independent or thresholded from the exact declared probability mark. Thresholded origin stores `>` or `>=`, one finite `f32` threshold in `[0,1]`, and a distinct threshold-evidence `ArtifactId`; status and every row must agree with the declared probability/comparator. Missing evidence, dependency/metadata drift, or row mismatch fails before engine construction.
- Provenance records: scalar mark records are exact schema `marklab.scalar_mark_provenance` version 1, have no table manifest, and have exact bounded metadata for mark ID, value kind, measurement status, unit, binary label/origin when applicable. Threshold records are exact schema `marklab.scalar_threshold_provenance` version 1 with exact source/binary IDs, comparator, threshold bits, and unit. Thresholded binary provenance depends on both probability provenance and threshold evidence; threshold evidence depends on probability provenance. The project scheduler verifies every declared record in the bound local store before execution.
- Endpoint routing: probability mode requires both binary and probability declarations. Probability values drive only pooled/component structure-factor spectra and their value permutations. Binary values drive counts, prevalence/QC, mark-pair covariance, anisotropy, periodogram, multiscale residuals/territories, diagnostics, and every other current endpoint. Binary mode accepts an independent binary declaration only and rejects an unused probability declaration.
- Compatibility/output: the declared engine path calls the unchanged compatibility engine. Its runtime wrapper adds a compact ordered-CellId digest/row count/slide/frame/declared-input identity plus the exact declarations, provenance IDs, status, threshold origin, and endpoint-routing summary. The concrete scheduler node binds those identities and semantic artifacts into its cache key but its codec remains exactly result-format 0.3; replay reattaches both node-bound runtime values. Config 0.2, result 0.3, Pattern/PatternLoader, CLI, current numerics, and existing node behavior remain unchanged.
- Deferrals: no general MarkTable, scalar missingness policy, non-unitless units/modality ontology, categorical/ordinal/count/continuous/simplex/vector/posterior owner, physical mark format, CLI/file adapter, durable declared-run manifest/result 0.4, source identity/frame proof, general validator registry, observation window, or new scientific estimand is admitted.

## IC-0015 — Declared patch-to-derived-region aggregation dispersion

- Immediate production caller: `patch_region_embedding_dispersion` consumes the exact source patch table, producer-declared patch-region link, deterministic derived-region candidate, copied verified graph, and both exact provenance values already produced by the C-05 finalization flow. It adds no generic statistics layer, physical format, receipt, validator, workflow node, or serializer.
- Binding: before work limits or arithmetic, source table identity/status/provenance, link logical and expected-set identity, graph source/link/expected-region/support/provenance roles, candidate table logical bindings, owning slide, and dimension must agree. Source provenance must be `DirectPatch`/`MorphologyPrediction`; output provenance must be `DerivedRegion`/`DerivedSummary`.
- Estimand: visit canonical nonzero link rows. When both source and finalized region rows are present, convert the exact declared fraction to `f64`, compute squared Euclidean distance from the source `f32` vector to the materialized finalized `f32` region vector in component order, multiply by the relation weight, and add distance and weight sequentially. The value is total weighted distance divided once by total eligible declared weight. Signed zero is positive; non-present rows are excluded; present all-zero vectors remain eligible. Per-axis sign flips are bitwise invariant. Arbitrary axis reordering is not claimed bitwise invariant because component order is part of the deterministic floating-point contract.
- Availability/resources: `Available` requires positive eligible weight. Otherwise return `InsufficientContributors` and no floating value. The caller must admit the checked conservative upper bound `nonzero_relation_count * dimension` before numeric traversal. Runtime is `O(R(log P + log G) + R*D)` with fixed-size additional retained state.
- Identity/claims: the result retains exact source-table artifact/logical identity, source-provenance artifact/logical identity, link artifact/logical identity, candidate region-table logical identity, and derived-provenance artifact/logical identity plus counts, dimension, and both measurement statuses. It describes dispersion around producer-declared weighted aggregation only. It proves no region geometry, tissue window, spatial dependence, inferential independence, model quality, or biology and changes no result/config/CLI/physical contract.

## IC-0016 — Runtime-only declared marked pre/post comparison

- Immediate caller: `compare_declared_marked_prepost` accepts two existing `DeclaredMarkedAnalysisResult` scheduler outputs and delegates their inner results to unchanged `compare_marked_prepost`. Before semantic comparison, each side must have matching result/declaration row count and mark label plus an exact scalar-identity digest recomputed from the attached mark use. It adds no node, cache entry, codec, physical format, receipt, validator framework, or generalized comparison abstraction.
- Semantic gate: binary mark ID, label, measurement status, structure-factor and other-endpoint routing must agree. Optional probability use must agree in presence, stable ID, and measurement status. Thresholded origins additionally require the same source probability ID, comparator, and exact threshold bits; independent versus thresholded is a mismatch. These category-only errors precede legacy comparison.
- Provenance/identity: pre/post binary, probability, and threshold-evidence artifact IDs may differ and both complete mark-use summaries remain visible. Ordered CellIds, row counts, owning slides, coordinate frames, and timepoints may differ; both compact declared identities and borrowed exact timepoint strings are returned without claiming row correspondence or selecting a biological unit.
- Compatibility/claims: the inner `PrePostResult`, its metadata flags, typed unavailable sections, curve diagnostics, interpretation, and result-format 0.3 encoding are exactly those of `compare_marked_prepost`. The wrapper is not serializable durable provenance or receipt authority. Because the existing public runtime type and result 0.3 carry no private producer proof, a caller-constructed same-row/same-label numeric-result substitution cannot be authenticated here. It proves no patient-level population effect, paired-cell effect, equivalence, noninferiority, correspondence, causality, or biology.

## IC-0017 — Declared binary prevalence change

- Immediate caller: `compare_declared_marked_prevalence` accepts two existing `DeclaredMarkedAnalysisResult` values and reuses IC-0016's exact per-output binding and cross-output semantic gate. Its fixed result borrows both mark-use summaries, scalar identities, and timepoints; it adds no node, codec, schema, format, receipt, validator framework, or generic comparison abstraction.
- Binding: in addition to IC-0016, require `n_marked <= n_cells` and exact finite canonical prevalence: positive `0.0` for zero rows, otherwise `n_marked as f64 / n_cells as f64`, compared by bits. Invalid pre binding precedes invalid post binding, which precedes cross-output semantic mismatches.
- Estimand/availability: report each exact binary count and prevalence. If both sides contain cells, `Available` carries the single `f64` value `post_prevalence - pre_prevalence`; otherwise `InsufficientCells` carries no delta. Probability-routed spectra do not change the binary count estimand. No NaN or infinity is returned.
- Compatibility/claims: `compare_declared_marked_prepost` and result-format 0.3 remain byte-for-byte unchanged. The result describes two supplied row collections and establishes no row correspondence, biological unit, patient/specimen prevalence, treatment effect, population inference, calibration, equivalence, noninferiority, causality, or biology. Available public-field checks remain caller assertion rather than durable producer authority.

## IC-0018 — Lineage-exact slide aggregation-path discrepancy

- Immediate caller: `slide_embedding_aggregation_path_discrepancy` consumes the existing patch-sourced and region-sourced derived slide candidates, their copied verified graphs and decoded provenance, and one existing verified region-table receipt. It adds no candidate/graph/receipt/format/validator/schema/general-statistics surface.
- Binding: require exact patch/region provenance variants and `DerivedSummary`; bind each candidate table to its graph/provenance/path kind; require common slide, expected-slide identity, positive dimension, and arithmetic-mean derivation; then use the region receipt to prove `patch graph source == region receipt ancestral patch source` and `region graph source == region receipt table/support/rows/dimension`. Every binding precedes budget and row access.
- Estimand: for two present singleton `f32` vectors, sequentially sum squared `f64` component differences and divide once by common dimension. Positive zero is canonical. `Available` has one finite value; any non-present path is `InsufficientComparablePaths` with no value or arithmetic. The caller must admit the exact dimension component count before status traversal. Runtime is `O(D)` and retained state is `O(1)`.
- Identity/claims: retain both path statuses/measurement statuses/candidate/provenance/support identities plus common expected-slide, patch-source, region-table, and patch-region-link identities. Approved wording is descriptive mean squared component discrepancy between the exact patch-direct and patch→region→slide aggregates. It proves no agreement, quality, geometry, spatial dependence, inference, path preference, real-source result, or biology.

## IC-0019 — Declared binary cell-embedding centroid discrepancy

- Immediate caller: `declared_binary_cell_embedding_centroid_discrepancy` consumes one existing `DeclaredScalarPatternInput`, `CellEmbeddingTable`, verified `CellEmbeddingArtifact`, and three explicit caller limits. It adds no mark/table/format/receipt/graph/validator/schema/workflow/general-statistics surface.
- Binding/order: require exact table/artifact QC and logical identity; admit the row count; then bind every ordered table CellId to the declared CellIds, count exact embedding statuses by the required binary row, and stream one domain-separated digest of those ordered assignments. Before allocation/arithmetic, admit conservative `present_rows*D + D` component work and exactly two `D`-component `f64` accumulators. Optional probabilities are retained but never select groups.
- Estimand/availability: sequentially form marked/unmarked present-vector means in stored row/component order and report their mean squared component difference with positive zero. `Available` requires one present vector in both groups; otherwise `InsufficientGroups` carries no value and exact group status counts. Non-present fillers are never exposed.
- Identity/claims: retain declared scalar and binary/optional-probability declarations, exact ordered binary-grouping digest, and embedding table/row-link/provenance artifact IDs, QC, dimension, and logical identity. The result is descriptive morphology-embedding centroid difference only; it proves no spatial association, separation/classification, quality, independence, patient effect, inference, real-source result, or biology.

## IC-0020 — Declared probability–cell-embedding cross-covariance energy

- Immediate caller: `declared_probability_cell_embedding_cross_covariance_energy` consumes one existing `DeclaredScalarPatternInput` with dense probability values, one `CellEmbeddingTable`, its verified `CellEmbeddingArtifact`, and three explicit caller limits. It adds no scalar/embedding/physical/workflow/general-statistics infrastructure.
- Binding/order: require the probability declaration/values and exact table/artifact QC; admit row count; bind every ordered table CellId while digesting all raw probability `f32` bits and detecting present-row variation. Before availability or arithmetic, admit conservative `2*P*D + 2*D` component work and exactly two `D`-component `f64` accumulators. Binary rows remain contextual only.
- Estimand/availability: use two stored-order population passes to report `D^-1 * sum_j(cov(p,x_j)^2)`. `Available` requires two present rows and nonconstant present probabilities; `InsufficientPresentRows` and `NoProbabilityVariation` carry no value and perform no component arithmetic after cap admission. Positive zero is canonical.
- Identity/claims: retain declared scalar/binary/probability identity, an ordered probability-values digest, exact embedding table/row-link/provenance IDs, QC, dimension, present count, and present probability mean. The result is descriptive unstandardized component cross-covariance energy only; it proves no correlation, spatial association/dependence, classification, calibration, quality, independence, patient effect, inference, real-source result, or biology.

## IC-0021 — Declared binary cell-centroid project workflow and private cache codec

- Immediate caller: `DeclaredBinaryCellEmbeddingCentroidNode` runs only through the existing `WorkflowNode`/`LocalScheduler::run_single_with_store` boundary and returns the existing S7 result. Its private codec has this exact node as its same-milestone production caller.
- Inputs/cache: ordinary inputs are whole Pattern and declared-input references. Semantic inputs are exact declared mark/evidence plus embedding table/row-link/provenance ArtifactIds. Constructor and per-run verification require one exact S7 binding snapshot; cache configuration binds all three limits, fixed arithmetic policy, adapter, and codec identity.
- Codec: exact 18 bytes `MLCBCENT | version=1 | status | f64 bits`; available values are finite, nonnegative, and canonical at zero; unavailable filler is exact zero. Encode requires every output field to match the binding; decode supplies no identity, reattaches the verified binding, and rejects every noncanonical field/length/status combination.
- Lifecycle/claims: semantic store verification precedes every cache lookup, misses round-trip to a fixed point before failure-atomic commit, and hits reattach current exact input identities. This is an internal cache contract, not result 0.3, a receipt, external producer authentication, or a new estimand; the S7 descriptive claim ceiling remains exact.

## IC-0022 — Declared nucleus-area cell-embedding cross-covariance energy

- Immediate caller: `declared_nucleus_area_cell_embedding_cross_covariance_energy` consumes the fixed `Pattern::nucleus_area_um2` declaration/profile, one existing declared scalar identity/project, one verified cell table/artifact, and three explicit caller limits. It adds no generic continuous mark, unit registry, physical format, workflow, or shared statistics layer.
- Declaration/profile: `NucleusAreaUm2MarkDeclaration` fixes ID, label, morphology modality, square-micrometre unit, continuous kind, non-summary per-cell status, and one exact no-table/no-dependency scalar-provenance record. Values are dense, finite, strictly positive raw `f32` values.
- Binding/order: revalidate project/provenance; require column/table/QC and row admission; bind every ordered CellId and area bit; count/sum/detect variation only for present embedding rows; then admit `2*P*D + 2*D` component work and exactly two `D`-component `f64` accumulators before availability or arithmetic.
- Estimand/availability: use two stored-order population passes to report `D^-1 * sum_j(cov(area_um2,x_j)^2)`. `Available` requires two present rows and nonconstant present areas; `InsufficientPresentRows` and `NoNucleusAreaVariation` carry no value and perform no component arithmetic. Positive zero is canonical.
- Identity/claims: retain scalar/binary/optional-probability context, the concrete area declaration, ordered area-value digest, exact embedding table/row-link/provenance IDs, QC, dimension, present count, and present mean area. The result is descriptive cross-covariance energy only; it proves no correlation, normalization, segmentation accuracy, spatial dependence, calibration, quality, inference, real-source result, or biology.

## IC-0023 — Contained-patch cell-embedding local dispersion

- Immediate caller: all-feature `contained_cell_patch_embedding_dispersion` consumes one verified C-04 table/artifact and the live C-05 contained-shared link, verified input graph, and paired physical link receipt. It is the sole caller of the retained expected-cell identity and adds no new physical or general statistics surface.
- Binding: exact table/artifact QC and logical identity; contained-shared mode; expected-cell ArtifactId/logical digest; row/assignment counts and every ordered CellId; graph mode/role IDs/digest/counts; receipt digest/counts and physical assignment/edge IDs. Binding errors precede caller budgets.
- Arithmetic: sort edge indices by patch/assignment/original row; patches with at least two present incidences contribute; report `sum ||x_i - mean_patch(i)||^2 / (eligible_incidences * D)`. Non-present rows and under-supported patches are excluded; overlaps remain repeated incidences. Exact zero is positive zero.
- Resources: assignment and edge caps precede allocation; conservative component work is `3*E*D`; exact incremental storage is `E*sizeof(usize) + D*sizeof(f64)` with one fallible edge-index vector and one reusable centroid. Runtime is `O(E log E + E*D)`.
- Identity/claims: retain counts, embedding/expected-cell/link graph/physical identities, QC, dimension, status, and optional value without arrays. The result is descriptive producer-declared containment-local heterogeneity only—not source correspondence, spatial autocorrelation, independent-patch evidence, embedding quality, inference, real-source evidence, or biology.

## IC-0024 — Declared binary-group nucleus-area contrast

- Immediate caller: `declared_binary_group_nucleus_area_contrast` consumes the existing declared scalar input, fixed nucleus-area declaration/profile, target project, and one row cap. It adds no general comparison, physical, workflow, or inference layer.
- Binding/identity: revalidate project and exact nucleus provenance; require dense finite strictly positive areas; hash ordered CellId/declaration/provenance context plus every paired binary byte and raw area bit. Optional probabilities remain visible context but do not route groups.
- Estimand/availability: stored-order `f64` means for exact binary `1` and `0` rows and signed marked-minus-unmarked difference. Both groups are required for `Available`; otherwise `InsufficientGroups` retains only nonempty means. Exact zero is positive zero.
- Resources/claims: `O(N)` time, fixed storage, and one exact row cap. The result is a descriptive within-input nucleus-area contrast only—not patient/specimen effect, segmentation validation, classification, spatial association, inference, real-source evidence, or biology.

## IC-0025 — Contained-patch binary-group nucleus-area contrast

- Immediate caller: `contained_patch_binary_nucleus_area_contrast` consumes the completed S12 whole-input contrast plus one existing contained-shared `CellPatchLink`, unforgeable managed input graph, and paired physical assignment/edge receipt. It adds no shared traversal/statistics abstraction, format, graph, receipt, validator, workflow, or serializer.
- Binding/order: exact S12 project/provenance/column/value/row admission precedes contained-only mode, exact owning-slide plus declared-row/assignment and ordered CellId binding, graph digest/count/producer binding, and receipt digest/count binding. Exact assignment/edge and `E*sizeof(usize)` working-byte limits precede allocation and patch arithmetic.
- Estimand/availability: sort exact incidences by patch/assignment/original edge. Eligible patches contain at least one binary-marked and one unmarked incidence. Report the equal-patch mean of each eligible patch's fixed-order `f64` marked-minus-unmarked nucleus-area contrast; positive zero is canonical. No eligible patch is typed `InsufficientEligiblePatches` with no value. Overlaps remain repeated, non-independent incidences.
- Identity/claims: retain the exact S12 result, counts, link logical identity, expected-cell/patch/context/footprint/producer IDs, and physical assignment/edge IDs without payload arrays. The result is descriptive producer-declared containment-local contrast only—not a window adjustment, spatial association, independent-patch estimate, segmentation validation, patient/specimen effect, inference, real-source result, or biology.

## IC-0026 — Durable declared binary/probability mark artifact

- Status: inactive planning contract; preserved but not authorized by the current roadmap.

- Immediate caller: the existing DeclaredMarkedAnalysisNode consumes one IC-0014 input bound to a newly verified managed mark-table artifact. No node, codec, result, config, CLI, or estimand is added.
- Physical schema: exact non-null cell_id: Utf8 and binary_value: Boolean, plus non-null probability_value: Float32 only for thresholded probability mode. Rows equal the live strictly increasing CellIds and Pattern bits. Version one admits no missingness.
- Semantic record: schema marklab.declared_scalar_mark_table version 1, exact Arrow IPC table manifest, content kind, metadata for slide/frame/declaration identities and exact mark/threshold semantics, and exact provenance dependencies. Independent binary forbids probability; probability mode requires its exact threshold source/evidence.
- Encoding/resources: canonical V5 Arrow IPC file, 64-byte alignment, no dictionaries/compression/custom metadata/null arrays, at most 8,192 rows per batch, bounded footer depth/table count, checked row/byte arithmetic, and a positive caller maximum file size enforced during dry-run and publication replay.
- Verification/lifecycle: managed store verification surrounds a full schema/array/row/value scan against the live input. Success returns an unforgeable receipt bound to ArtifactId, row count, ordered-CellId digest, and declared-input digest. Binding requires the same current project record and adds the table ArtifactId to scheduler semantic verification/cache identity.
- Compatibility/claims: engine arithmetic and result-format 0.3 bytes remain exact. The artifact proves the declared row payload was durably bound to the run; it does not prove source correspondence, assay/model validity, calibration, independence, patient effect, or biology.

## IC-0027 — End-to-end classical homogeneous K/L workflow

- Input/window: existing supported cell rows are filtered through a bounded exact physical MultiPolygon window. The window owns canonical rings, holes/components, area/perimeter/bounds, closed membership, boundary distance, topology policy, resource counts, and digest.
- Geometry: the exact window owns one canonical boundary-segment index. One reusable point plan binds finite unique points to that window, validates membership, owns the existing exact point R-tree and consumed boundary distances, and streams pair contributions without retaining all pairs or duplicating the window index.
- Estimator: standard border K/L uses exact m(r) eligible centers and q(r) ordered pairs at strictly increasing generated radii. Empty/singleton inputs and zero-center radii are typed unavailable; malformed inputs and exceeded limits are errors.
- Null/inference: conditional homogeneous CSR fixes n and the window and resamples the entire location pattern. Domain-separated deterministic streams, candidate-draw limits, exact simulation counts, and existing ERL global inference are recorded. Marks and lower-level rows are not replication units.
- Project/result: one dependency-free typed node binds cells, canonical window, radii, null, seed, alpha, simulations, limits, implementation, and codec into cache identity. Its strict canonical JSON codec emits a separate marklab.classical_spatial version-one document; result 0.3 is unchanged.
- CLI/report: marklab classical writes result.json, run_manifest.json, and report.md atomically. The report names the homogeneous stationary estimand, border correction, conditional-CSR null, whole-pattern unit, unavailable states, and prohibition on biological, patient-level, causal, or clinical claims.

## IC-0028 — Durable single-node project replay for the classical workflow

- Project surface: `DurableProject::open_or_create` admits one non-symlink local root under positive metadata, ledger, record-count, record-size, and object-size limits; holds one exclusive project lock; validates/recovers its strict version-one head, chained append-only JSONL ledger, optional pending intent, and existing local artifact store before use.
- Execution identity: `DurableExecutionRequest` binds exact node ID/spec digest, ordered input `ArtifactRef`s, configuration digest, execution-policy digest, scheduler output limit, current scheduler cache key, result schema, and `NativeRuntimeProvenance`. The native manifest binds crate version, explicit Git availability/SHA/dirty state, rustc, sorted exact compiled features, and streamed executable content. The native manifest's canonical digest is the implementation identity used by the durable classical node.
- Replay: lookup uses the scheduler-computed cache key and rejects a same-key identity conflict. An exact output record is reconstructed, verified through the existing `LocalArtifactStore`, read under the caller object limit, and restored with existing `MarklabProject::commit_success`; the current scheduler must then return its ordinary typed hit. Integrity failure never becomes execution or a miss.
- Commit/recovery: on a scheduler miss, exact canonical bytes are published as schema `marklab.workflow_node_output` version one through the existing store. Synced object publication precedes pending intent, ledger append, atomic head replacement, and intent removal. On open, a verified pending record deterministically completes the missing ledger/head step or is cleared if already complete. Conflicts and head/ledger drift without intent are errors.
- CLI/compatibility: `marklab project classical` accepts the completed classical arguments plus `--project` and writes the same strict three-file bundle with truthful miss/hit identity. `marklab classical`, result format 0.3, classical numerics/null/claims, catalog/store ownership, and legacy commands remain unchanged. This contract is not a backend registry, general DAG/schema/migration engine, scientific receipt, external-producer authentication, or source-data container.

## IC-0029 — Patient-level scalar permutation workflow

- Scientific API: `marklab_cohort::patient_level_permutation_test` accepts one finite scalar `PatientEndpoint` per exact non-empty patient ID and a `PatientPermutationSpec` naming exactly two non-empty groups, optional exact exchangeability blocks, 1–1,000,000 permutations, one seed, and `less`, `greater`, or equal-tail `two-sided`. Duplicate patients, undeclared groups, fewer than two patients per group, surrounding identifier whitespace, fully group-confounded declared blocks, non-finite endpoints, zero standard error, and exceeded patient/permutation/work limits are errors.
- Estimand/randomization: report stable patient-level group means, signed group-A-minus-group-B difference, and Welch-style studentized contrast. Randomization operates on complete patient labels only, preserves each exact block's group counts, uses a domain-separated deterministic seed per replicate, retains inclusive ties, adds one to numerator and denominator, and fails the whole run if any requested replicate is undefined.
- CLI/input: `marklab cohort permutation --input <csv> --group-a <label> --group-b <label> --permutations <B> --seed <u64> --alternative <less|greater|two-sided> --out <json>`. CSV headers are exactly `patient_id,group,endpoint` with optional trailing `block`; input is limited to 16 MiB. No patient identity is inferred from paths, filenames, rows below the patient level, or repeated observations.
- Result/publication: strict pretty JSON plus final newline, format `marklab.cohort_permutation`, version 1, records input path; patient/block/design summaries; group labels/counts/means; signed effect; studentized statistic; p-value; requested/attempted/completed counts; seed; and alternative. Publication uses a same-directory synced staging file and rename after successful validation/computation. Existing result format 0.3 is unchanged.
- Claims: established patient-level permutation inference for the supplied prespecified scalar endpoint under the caller-declared exchangeability design only. It makes no cell-level, paired, longitudinal, repeated-measures, multisite, causal, clinical, biological, equivalence, noninferiority, calibration, or real-world validation claim.

## IC-0030 — Paired patient scalar sign-flip workflow

- Scientific API: `marklab_cohort::paired_patient_permutation_test` accepts exactly one finite scalar row for each of two distinct exact condition labels per non-empty patient ID, at least two complete patient pairs, a bounded replicate count, one seed, and one alternative. Missing, duplicate, undeclared, non-finite, zero-standard-error, surrounding-whitespace, and exceeded work states are errors.
- Estimand/randomization: form exact condition-B-minus-condition-A differences in stable patient-ID order; report both condition means, mean paired difference, and its sample-variance studentized statistic. Each null replicate independently sign-flips whole patient differences using a paired domain-separated deterministic seed. Inclusive ties and plus-one one-sided/equal-tail p-values match IC-0029; no failed replicate is removed.
- CLI/result: `marklab cohort paired-permutation` consumes exact `patient_id,condition,endpoint` CSV and writes failure-atomically published `marklab.cohort_paired_permutation` version-one JSON with input, patient-pair design, completed pair count, condition summaries, signed effect, statistic, p-value, replicate counts, seed, and alternative. The existing 16 MiB cohort input boundary and result format 0.3 compatibility remain unchanged.
- Claims: established paired scalar inference for the supplied complete-pair design only. It is not an unpaired fallback, repeated-measures model, longitudinal model, causal analysis, equivalence/noninferiority decision, or biological/clinical validation claim.

## IC-0031 — Patient-level functional L2 permutation workflow

- Scientific API: `marklab_cohort::functional_two_sample_permutation` accepts one finite curve per unique patient, two exact groups with at least two patients each, one identical finite strictly increasing axis of at least two points, L2 mode, a positive bounded replicate count, and one seed. Duplicate patients, mismatched axes, undeclared groups, non-finite values, and exceeded patient-by-axis-by-permutation work are errors.
- Estimand/randomization: compute stored-axis group means, their group-A-minus-group-B difference, and the trapezoidal integral of squared difference. Labels shuffle only as whole-patient curves using a functional domain-separated deterministic stream. The single joint p-value is inclusive-plus-one one-sided high; no pointwise p-values are emitted.
- CLI/result: `marklab cohort functional-permutation --statistic l2` consumes exact `patient_id,group,axis,value` CSV and writes failure-atomically published `marklab.cohort_functional_permutation` version-one JSON containing input, patient design/counts, axis, group means, difference, statistic, p-value, replicate counts, and seed. Existing result 0.3 remains unchanged.
- Claims: established joint L2 comparison under the supplied independent-groups exchangeability design only. It does not provide pointwise inference, simultaneous bands, paired/blocked curves, interpolation, smoothing, causal interpretation, or biological/clinical validation.

## IC-0032 — Patient-level single-step Max-T workflow

- Scientific API: `marklab_cohort::max_t_multiple_endpoint_permutation` accepts one complete finite ordered endpoint vector per unique patient, exact identical non-empty endpoint names, two groups with at least two patients each, a bounded permutation count, seed, and finite `0<alpha<1`. Duplicate/incomplete patients or endpoints, undeclared groups, zero/non-finite standard errors, and exceeded work are errors.
- Estimand/randomization: reuse the canonical stable Welch group-A-minus-group-B effect/statistic for every endpoint. Each replicate makes one whole-patient label shuffle shared by all endpoints, retains the maximum absolute statistic, and must complete. Adjusted p-values are inclusive-plus-one exceedance probabilities against that joint maximum; the critical value is the capped conservative empirical `(1-alpha)` order statistic.
- CLI/result: `marklab cohort max-t` consumes exact `patient_id,group,endpoint,value` CSV and writes failure-atomically published `marklab.cohort_max_t` version-one JSON with design, patient counts, ordered endpoint effects/statistics/adjusted p-values, alpha, critical value, replicate counts, and seed. Result format 0.3 remains unchanged.
- Claims: family-wise control for one prespecified complete patient endpoint family under the supplied unblocked independent-groups design. It is not endpoint selection, missing-data handling, step-down inference, causal analysis, or biological/clinical validation.

## IC-0033 — Patient-level maximum mean discrepancy workflow

- Scientific API: `marklab_cohort::patient_level_mmd` accepts one complete finite exact-feature fingerprint per unique patient, two groups with at least two patients each, linear or fixed-positive-bandwidth RBF kernel, unbiased or biased estimator, bounded permutations, and seed. Duplicate/incomplete features, undeclared groups, invalid bandwidth, non-finite kernel/statistic, and exceeded matrix/work limits are errors.
- Estimand/randomization: build one exact symmetric kernel matrix. Unbiased mode excludes within-group diagonals and divides by `m(m-1)`/`n(n-1)`; biased mode includes diagonals and divides by squared group counts; both subtract twice the cross-group mean. Whole-patient labels shuffle under one MMD-specific deterministic stream and reuse the matrix. The p-value is inclusive-plus-one one-sided high.
- CLI/result: `marklab cohort mmd` consumes exact `patient_id,group,feature,value` CSV and writes failure-atomically published `marklab.cohort_mmd` version-one JSON with design, counts, kernel/bandwidth, estimator, MMD-squared, p-value, replicate counts, and seed. Linear forbids bandwidth; RBF requires it. Result 0.3 remains unchanged.
- Claims: established distributional comparison for the supplied fingerprint/kernel procedure under the independent-patient design only. It is not feature selection, learned-kernel inference, missing-data handling, causality, or biological/clinical validation.

## IC-0034 — Patient-level Euclidean energy-distance workflow

- Scientific API: `marklab_cohort::patient_level_energy_distance` consumes the IC-0033 complete finite fingerprint boundary, two exact groups with at least two patients each, exact Euclidean metric, bounded permutations, and seed. Invalid groups/features, non-finite or unrepresentable distances/statistics, and exceeded matrix/work limits are errors.
- Estimand/randomization: build one exact symmetric Euclidean distance matrix. Report twice the cross-group ordered mean minus each within-group ordered V-statistic mean including diagonals. Whole-patient labels shuffle under an energy-specific deterministic stream and reuse the matrix. The p-value is inclusive-plus-one one-sided high.
- CLI/result: `marklab cohort energy --metric euclidean` consumes exact `patient_id,group,feature,value` CSV and writes failure-atomically published `marklab.cohort_energy` version-one JSON with design, counts, metric, energy distance, p-value, replicate counts, and seed. Result 0.3 remains unchanged.
- Claims: established metric distributional comparison for the supplied complete fingerprints under the independent-patient design only. It is not metric learning, missing-data handling, weighted inference, causality, or biological/clinical validation.

## IC-0035 — Versioned spatial fingerprint construction and distance

- Construction API: `build_spatial_fingerprint` accepts one exact sample ID, non-empty spec version, and 1–4,096 unique named components totaling at most one million entries. Each component has at least two aligned finite strictly increasing axis points, finite values, finite nonnegative uncertainties, and a finite positive prespecified weight. Version one fixes normalization/uncertainty weighting/training transform to `none` and missing-component policy to `reject`.
- Identity: canonical component-name order, length-delimited strings, big-endian raw finite `f64` bits, and fixed policy tags feed SHA-256. The specification digest excludes sample values/uncertainties; the content digest binds specification digest, sample identity, component values, and uncertainties. Golden digests are independently checked.
- Distance API: `fingerprint_distance` requires exact spec version/digest and component name/axis/weight equality. Each component distance is the square root of the trapezoidal integral of squared value difference; total distance is the stable sum of `weight*distance`. Every component contribution remains explicit; non-finite arithmetic is an error.
- CLI/result: `marklab cohort fingerprint-distance` consumes exact `sample_id,component,axis,value,uncertainty,weight` CSV for exactly two declared samples and writes failure-atomically published `marklab.spatial_fingerprint_distance` version-one JSON retaining specification policies/digest, both full structured fingerprints/content digests, contribution decomposition, and total. It supplies no p-value or biological claim.

## IC-0036 — Patient-effect TOST equivalence workflow

- Scientific API: `tost_equivalence` accepts 2–1,000,000 unique finite patient effects plus finite strict lower/upper margins, finite `0<alpha<0.5`, and a non-empty exact margin-rationale reference. Duplicate patients, zero/non-finite standard error, invalid margins/rationale, and non-finite distribution outputs are errors.
- Estimand/inference: use the stable mean effect, sample standard error, and `n-1` Student-t degrees of freedom. Lower test uses `(estimate-lower)/SE` with upper-tail p-value; upper test uses `(estimate-upper)/SE` with lower-tail p-value. The matching interval is `estimate +/- t_(1-alpha,df)*SE` at level `1-2*alpha`. Equivalence requires both `p<alpha` and strict interval inclusion.
- CLI/result: `marklab cohort equivalence` consumes exact `patient_id,effect` CSV and explicit margins/alpha/rationale, then failure-atomically publishes `marklab.cohort_equivalence` version-one JSON with all inputs, estimates, tests, interval, and decision. A failed decision is `not_demonstrated`, not `different`; result 0.3 remains unchanged.
- Claims: statistical equivalence only for the caller-supplied prespecified patient effect and justified margins. It supplies no margin rationale, power assurance, causal interpretation, or biological/clinical validation.

## IC-0037 — Directional patient-effect noninferiority workflow

- Scientific API: `noninferiority_test` consumes the IC-0036 patient-effect boundary plus explicit `higher_is_better` or `lower_is_better`, finite positive margin magnitude, finite `0<alpha<0.5`, and non-empty exact rationale. Invalid inputs, zero/non-finite SE, distribution failure, and p-value/bound disagreement are errors.
- Inference: higher-is-better uses null boundary `-margin`, favorable statistic `(estimate-boundary)/SE`, upper-tail p-value, and lower `1-alpha` bound; lower-is-better uses `+margin`, favorable statistic `(boundary-estimate)/SE`, the same upper-tail convention, and upper bound. Noninferiority requires `p<alpha` and strict bound crossing.
- CLI/result: `marklab cohort noninferiority` consumes exact `patient_id,effect` CSV and explicit direction/margin/alpha/rationale, then failure-atomically publishes `marklab.cohort_noninferiority` version-one JSON with estimate, SE/df, direction, signed boundary, statistic, p-value, bound, and decision. It is never labeled equivalence or superiority.
- Claims: statistical noninferiority only for the supplied patient effect, direction, and justified margin. It supplies no margin rationale, power assurance, causality, superiority, equivalence, or biological/clinical validation.

## IC-0038 — Patient-first two-level hierarchical scalar bootstrap

- Scientific API: `hierarchical_bootstrap` accepts unique finite specimen endpoints nested under exact non-empty patient IDs, at least two patients, at least one specimen each, positive bounded replicates, seed, and finite `0<alpha<0.5`. Duplicate specimens, invalid IDs/values, and exceeded conservative draw limits are errors.
- Resampling/statistic: observed statistic is the stable mean across supplied specimen rows. Each deterministic replicate samples `P` patients with replacement; for every sampled patient occurrence it samples that patient's original child count with replacement only from that patient's specimens. Occurrence multiplicity is retained. Replicate failure aborts the workflow.
- Interval/CLI: sort complete replicate means and use deterministic nearest-rank alpha/2 and 1-alpha/2 empirical bounds. `marklab cohort hierarchical-bootstrap` consumes exact `patient_id,specimen_id,endpoint` CSV and failure-atomically publishes `marklab.cohort_hierarchical_bootstrap` version-one JSON with levels, statistic/interval method, counts, observed mean, interval, complete replicate counts, seed, and alpha.
- Claims: resampling uncertainty for this supplied two-level specimen-row mean only. It is not a cell/specimen-only substitute for patient inference, deeper hierarchy, BCa coverage, causal inference, or biological/clinical validation.

## IC-0039 — Pinned PyMC normal-mean NUTS workflow

- CLI/input: `marklab bayes normal-mean` reads one strict `observation` CSV column bounded to 16 MiB and 100,000 finite scalar rows. Required explicit arguments are Normal prior mean/positive SD, known positive observation SD, 2–8 chains, 100–100,000 tune/draw iterations per chain, target acceptance in `[0.5,1)`, one `u64` seed, 1–3,600 second timeout, and a fresh output path.
- Model/backend: `marklab-bayes` owns `marklab.bayesian_model_ir` version one for `mu ~ Normal(prior_mean, prior_sd)` and `y_i ~ Normal(mu, known_sigma)`, real-valued population-mean interpretation, scalar observation unit, posterior-predictive observation mean, `nuts` capability, and experimental maturity. One static Python worker accepts no free-form model or command and verifies exact PyMC 6.3.0, Python-3.12 `uv.lock` SHA-256, strict request fields/version, observation and iteration limits, and finite inputs.
- Execution/failure: request bytes bind exact model/data/sampling/diagnostic/resource identity plus the static worker SHA-256. The adapter clears inherited environment, fixes single-process/thread execution, caps stdout/stderr at 1 MiB, kills at the requested timeout, and treats missing environment/worker, nonzero exit, output overflow, empty/malformed/unknown-field JSON, backend/lock/worker/request drift, missing diagnostics, changed data summary, and non-finite output as errors.
- Result/diagnostics: strict `marklab.bayesian_fit` version one reports backend/lock identity, typed model IR, input count/digest, seed/request digest, complete sampling counts, posterior mean/SD/equal-tail interval, finite prior/posterior checks, rank R-hat, bulk/tail ESS, mean/SD MCSE, minimum chain E-BFMI, divergences, maximum-tree-depth hits, constraint/identifiability checks, and posterior-predictive observed/replicated mean discrepancy. `complete` requires R-hat <=1.01, bulk/tail ESS >=400, minimum E-BFMI >=0.3, zero divergences/depth hits, and finite/constraint/identifiability checks; otherwise the result is `nonconverged` and `diagnostic_only_nonconverged`.
- Claims/compatibility: even a complete fit is experimental and supports only this supplied scalar likelihood and prior. It makes no patient, hierarchical, spatial, causal, clinical, calibration, cross-backend, or biological claim. Result format 0.3 and all cohort/classical formats remain unchanged.

## IC-0040 — Gaussian patient partial-pooling workflow

- CLI/input: `marklab bayes hierarchical-normal` consumes exact `patient_id,observation` CSV, at least three patients and two finite observations per patient, explicit global Normal prior, positive HalfNormal between-patient-SD prior scale, positive known observation SD, bounded NUTS controls/seed/timeout, and a fresh output path. Patient is the declared biological unit and IDs are canonical sorted request identity.
- Model: `global_mean ~ Normal`, `between_patient_sd ~ HalfNormal`, non-centered `patient_z ~ Normal(0,1)`, `patient_mean = global_mean + between_patient_sd * patient_z`, and row likelihood `Normal(patient_mean[patient], known_sigma)`. Model IR declares identity link, patient hierarchy, no spatial component, patient means/variance partition/posterior predictions, `nuts`, and experimental maturity.
- Result: strict `marklab.bayesian_hierarchical_fit` version one reports exact backend/environment/worker/request/input identities; complete sample counts; global mean, heterogeneity, variance partition; one patient raw/posterior mean, SD, equal-tail interval, approximate shrinkage and warning; and observed/replicated global mean plus patient-mean SD. Diagnostics aggregate worst R-hat/MCSE and minimum bulk/tail ESS/E-BFMI across global, heterogeneity, and patient effects, with finite/constraint/identifiability/divergence/depth state.
- Claims: complete status uses IC-0039's thresholds and remains experimental; nonconverged is diagnostic-only. Shrinkage is model-dependent, not a patient-quality score. The output makes no spatial, multisite, causal, calibration, clinical, biological, or arbitrary mixed-model claim.

## IC-0041 — Bayesian random-effects site meta-regression

- CLI/input: `marklab bayes meta-analysis` consumes exact `site_id,effect,standard_error,covariate` CSV with 5–10,000 unique sites, finite estimates/raw covariates, positive known standard errors, nonconstant covariate, explicit covariate name/new-site covariate, Normal global/slope prior scales, HalfNormal heterogeneity scale, bounded NUTS controls/seed/timeout, and fresh output.
- Model/backend: the typed IR declares site-effect-estimate observation unit, site/cohort biological unit, one raw unscaled covariate, latent site Normal effects around `global + x*gamma`, known-SE observation likelihood, and new-site latent prediction. PyMC 6.3.0 samples the exact marginalized likelihood with variance `tau^2 + se^2`; exact conditional Normal draws restore site effects, and a domain-separated seeded Normal draw restores the new-site distribution.
- Result/diagnostics: strict `marklab.bayesian_meta_analysis_fit` version one binds backend/environment/worker/request/input identities; reports global, slope, heterogeneity, every observed/latent site summary, new-site prediction, complete sampling counts, shared NUTS diagnostics, and observed/replicated effect mean/dispersion. Complete/nonconverged semantics use IC-0039.
- Claims: experimental model-based synthesis only. No hidden covariate transform, exchangeability proof, transportability, publication-bias correction, causal, biological, calibration, clinical, or real-world validation claim is made.

## IC-0042 — Exact one-dimensional Matérn GP regression

- CLI/input: `marklab bayes gp-regression` consumes exact `observation_id,x_um,value` and `prediction_id,x_um` CSVs, 5–128 unique finite observation coordinates, 1–2,048 finite prediction coordinates, explicit mean/amplitude/length/noise priors, positive jitter, bounded NUTS controls/seed/timeout, and fresh output. Coordinate dimension and micrometre unit are model identity.
- Kernel/inference: exact Matérn-3/2 covariance is `a^2(1+sqrt(3)r/l)exp(-sqrt(3)r/l)`; training covariance adds `noise_sd^2 + jitter` only on its diagonal. PyMC 6.3.0 NUTS infers mean/amplitude/length/noise through the strict static worker. Conservative requested iteration/conditioning work is capped at two billion units in addition to row/output/runtime limits.
- Prediction/result: each posterior draw performs exact Cholesky conditioning. Strict `marklab.bayesian_gp_fit` version one reports backend/environment/worker/request/data identities, hyperparameters, every latent prediction mean/SD/equal-tail interval, complete sampling/diagnostics, and observed/replicated field mean/SD. Observation noise is not added to latent prediction covariance.
- Claims: complete status uses IC-0039 and remains experimental synthetic field interpolation. No 2-D tissue window, anisotropy, nonstationarity, sparse approximation, causal, biological, clinical, calibration, or real-data claim is made.

## IC-0043 — Identifiable two-output one-factor Matérn GP

- CLI/input: `marklab bayes multi-output-gp` consumes exact complete `coordinate_id,x_um,output_a,output_b` plus prediction coordinates, 5–64 unique varying rows, 1–1,024 predictions, two distinct output names, positive prior scales, explicit known positive output-A/output-B noise SDs, jitter, bounded NUTS controls/seed/timeout, and fresh output.
- Model/identifiability: separate Normal means share one Matérn-3/2 latent process. Loading A is exactly 1 and loading B has positive HalfNormal prior, fixing scale/sign and representing positive dependence. Exact block covariance is `[[K,bK],[bK,b^2K]]`; known output noises and jitter are diagonal only.
- Result: strict `marklab.bayesian_multi_output_gp_fit` version one binds input/model/backend/lock/worker/request identity; reports means, amplitude, length, loading B, every joint latent prediction, complete diagnostics, and observed/replicated output correlation and means. Exact dense iteration/conditioning work is bounded to two billion units.
- Claims: experimental positive-dependence synthetic coregionalization only. It is not two independent fits, a quality score, negative-dependence model, arbitrary LMC, missing-modality model, tissue validation, biological coupling, calibration, or clinical evidence.

## IC-0044 — Variational inducing-point Matérn GP

- CLI/input: `marklab bayes variational-gp` reuses IC-0042 CSV/prior/kernel/unit conventions for 8–2,000 observations and 1–2,048 predictions, with explicit 3–64 inducing points (`m<n`), 2–4 starts, 1,000–100,000 iterations, learning rate `(0,0.1]`, 500–10,000 draws, seed/timeout, and fresh output. Requested `starts*iterations*(n*m^2+m^3)` plus prediction work is capped at five billion units.
- Approximation: inducing locations start at deterministic coordinate quantiles and are optimized under a strictly ordered transform. PyMC 6.3.0 `MarginalApprox("VFE")` owns the Gaussian VFE bound and analytically collapsed inducing-state optimum; mean-field ADVI approximates hyperparameter and inducing-location uncertainty.
- Result/gate: strict `marklab.bayesian_variational_gp_fit` version one binds backend/lock/worker/request/data identity and reports selected hyperparameters, inducing-location posterior means, latent predictions, every start's initial/final ELBO and tail change, selected start, cross-start prediction RMSE, finite prior/posterior state, and posterior-predictive mean/SD. `approximate_only` requires every start finite/improved, tail change <=0.2, and cross-start RMSE <=0.5; otherwise `nonconverged`.
- Claims: no R-hat/HMC, gradient norm, or importance correction is fabricated. The output is experimental approximate inference, not exact GP, calibrated uncertainty, production sparse scale, tissue validation, biological inference, or clinical evidence.

## IC-0045 — Low-rank predictive-process approximation diagnostic

- API/input: `low_rank_predictive_process` and `marklab bayes predictive-process` consume 2–2,000 unique exact one-dimensional micrometre coordinates, 2–128 unique knots, positive Matérn-3/2 amplitude/length/jitter, and explicit diagonal-correction state. Storage `n*m+m^2` is capped at 10 million elements and work `n*m^2+m^3` at 500 million units.
- Representation: own `Kmm+jitter I` Cholesky, Knm, `diag(Knm Kmm^-1 Knm^T)`, and `max(Kii-low_rank_ii,0)` residual after rejecting material negative drift. Optional correction adds residual only to the diagonal.
- Result/claims: strict `marklab.predictive_process` version one reports paths/counts/kernel/unit/parameters, per-coordinate low-rank/residual/corrected variance, and mean/max/trace retained. It is an experimental approximation diagnostic, not a fitted posterior, exact GP, restored off-diagonal covariance, calibrated uncertainty, tissue field, biological result, or clinical evidence.

## IC-0046 — One-dimensional physical-order NNGP density

- API/input: `build_nngp`, `nngp_log_density`, and `marklab bayes nngp-density` consume 2–10,000 exact unique one-dimensional micrometre coordinates with finite field values, finite mean, positive Matérn amplitude/length/jitter/F tolerance, and 1–64 neighbors. Conservative `n*m^3` work is capped at 500 million units.
- Plan/density: order coordinates ascending; neighbors for row i are its last `min(m,i)` predecessors. Exact local Cholesky solves produce B and `F=Kii-BKni`; any non-finite or `F<=tolerance` is failure. Density sums every Normal conditional residual without omissions.
- Result/reference: strict `marklab.nngp_density` version one reports physical ordering, ordered IDs, neighbor counts, all F values, sparse log density, and optional independent full-GP Cholesky reference for n<=128. It is an experimental approximation diagnostic, not fitted inference, prediction, calibration, tissue validation, biology, or clinical evidence.

## IC-0047 — Validated sparse spatial weights

- API/input: `validate_spatial_weights` and `marklab bayes validate-weights` consume 1–100,000 exact region IDs and at most 2,000,000 unique directed finite positive edges. Policies explicitly select required/not-required exact symmetry, zero/allowed diagonal, and preserved/row-standardized weights; version one rejects signed and stored zero edges.
- Semantics: region order is lexical. Symmetry is checked before normalization. Row standardization divides each positive outgoing row and leaves zero island rows unchanged. Weak undirected connectivity names all components and islands. Canonical SHA-256 binds version/policies/sorted IDs/sorted normalized exact `f64` bits.
- Result/claims: strict `marklab.validated_spatial_weights` version one reports paths/counts/policies/components/islands/row sums/digest and is a foundational graph contract only—not fitted inference, geometric adjacency proof, biological interaction, causality, or clinical evidence.

## IC-0048 — Proper and intrinsic CAR field density

- API/input: `car_density` and `marklab bayes car-density` consume IC-0047 preserved symmetric zero-diagonal weights, one exact finite value per region, 2–512 regions, at most 200,000 edges, positive tau, finite rho, positive constraint tolerance, and explicit `reject`/`exclude` island policy.
- Precision/density: proper mode constructs `Q=tau*(D-rho*W)`, rejects islands, and admits rho only when dense Cholesky proves Q positive definite. Intrinsic mode constructs `Q=tau*(D-W)`, enforces one sum-to-zero constraint per non-island component, records one rank deficiency per constraint, and computes each constrained determinant as `log(k)+log(det(cofactor))`; excluded islands contribute no implicit prior or density.
- Result/claims: strict `marklab.car_density` version one binds the validated-weight digest and reports mode/parameters/log density/rank deficiency/constraints/excluded islands. It is an experimental field-density diagnostic, not posterior fitting, a disease map, biological interaction, causality, or clinical evidence.

## IC-0049 — General constrained GMRF field density

- API/input: `gmrf_log_density` and `marklab bayes gmrf-density` consume 2–256 exact regions, a dimension-matched finite symmetric precision matrix with at most 65,536 explicit sparse triplets and every diagonal declared, one exact finite field value per region, zero to 255 named homogeneous linear constraints, and positive finite tolerance.
- Subspace: constraints must be nonempty and linearly independent under the declared tolerance, and the field must satisfy each row. Deterministic twice-reorthogonalized modified Gram–Schmidt constructs an orthonormal null-space basis; projected precision must be positive definite by Cholesky. No symmetrization, jitter, rank repair, or pseudo-inverse is implicit.
- Result/claims: strict `marklab.gmrf_density` version one reports ambient/constrained dimensions, rank deficiency, constraint names/tolerance, projected log determinant, full quadratic, and normalized constrained-subspace log density. It is an experimental field-density diagnostic, not fitted inference, spatial adjacency proof, biological interaction, causality, or clinical evidence.

## IC-0050 — Fixed-parameter Gaussian SAR likelihood

- API/input: `sar_gaussian_log_likelihood` and `marklab bayes sar-likelihood` consume IC-0047 island-free row-standardized zero-diagonal weights without required symmetry, 2–512 exact regions and at most 200,000 edges, a complete finite response/design with 1–32 named predictors, exactly matched finite intercept/coefficients, finite rho, positive sigma, lag/error mode, and the version-one `descriptive` interpretation.
- Likelihood: deterministic partial-pivot LU proves `A=I-rho*W` nonsingular and supplies `log|det(A)|`. Lag residual is `Ay-Xbeta`; error residual is `A(y-Xbeta)`. Both report the complete Gaussian transformation likelihood. Lag impacts use the exact dense inverse to report average diagonal direct, average row-sum total, and indirect difference for each non-intercept coefficient; error mode reports no spillover impacts.
- Result/claims: strict `marklab.sar_likelihood` version one binds the validated-weight digest and reports declared inputs/coefficients, determinant, RSS, likelihood, and any descriptive impacts. It is a fixed-parameter experimental likelihood diagnostic—not estimation, posterior inference, causal effect, biological interaction, or clinical evidence.

## IC-0051 — Fitted Gaussian SAR lag/error lifecycle

- CLI/input: `marklab bayes sar-fit` reuses IC-0050 exact island-free row-standardized weights and complete response/design for 6–64 regions and 1–16 full-rank predictors. It requires explicit Normal intercept/coefficient scales, Uniform rho bound strictly inside `(-1,1)`, HalfNormal sigma scale, descriptive interpretation, bounded NUTS controls/seed/timeout, and fresh output.
- Model/backend: the pinned PyMC 6.3.0 worker includes `log|det(I-rho W)|` and exactly the IC-0050 lag/error residual. Strict lock/worker/request/data/weights identities and the shared prior/posterior/predictive, R-hat, ESS, MCSE, E-BFMI, divergence, and tree-depth policies control complete versus nonconverged state. Lag impact distributions use each posterior draw's exact inverse; error mode reports no spillover impacts.
- Result/claims: strict `marklab.bayesian_sar_fit` version one reports model IR, posterior intercept/coefficients/rho/sigma, optional descriptive impacts, diagnostics, and response-scale posterior predictive summaries. Complete remains experimental synthetic inference; nonconverged remains diagnostic-only. Neither is causal, biological, clinical, tissue-validated, or production-calibrated evidence.

## IC-0052 — Exact-constraint Poisson BYM lifecycle

- CLI/input: `marklab bayes bym-fit` consumes 6–64 exact regions, IC-0047 preserved symmetric binary zero-diagonal island-free weights, nonnegative integer counts, positive expected counts, 1–16 full-rank predictors, explicit Normal fixed-effect and HalfNormal component-scale priors, bounded NUTS controls/seed/timeout, and fresh output.
- ICAR/model: `build_icar_plan` canonicalizes connected components, constructs an orthonormal Helmert basis for one exact sum-to-zero constraint per component, projects `D-W`, proves constrained positive definiteness, and returns a noncentered transform whose columns satisfy `T'QT=I`. PyMC uses `log(E)+intercept+X beta+structured_sd*Tz+unstructured_sd*v` with Poisson likelihood; no soft constraint, ridge, island prior, or proper-CAR substitution occurs.
- Result/claims: strict `marklab.bayesian_bym_fit` version one binds weights/data/backend/request identity and reports fixed effects, separate component scales/region fields, positive relative risks, rank deficiency/constraints, normalized NUTS diagnostics, and replicated total/zero-count summaries. It is experimental synthetic disease-mapping mechanics, not calibrated epidemiology, patient-level risk, biology, causality, or clinical evidence.

## IC-0053 — Scaled-ICAR Poisson BYM2 lifecycle

- CLI/input: `marklab bayes bym2-fit` reuses IC-0052 count/design/weights/constraint and bounded PyMC inputs, replacing separate component scales with explicit positive HalfNormal total sigma and positive-parameter Beta prior for phi.
- Scaling/model: `build_icar_plan` computes each unit-precision generalized marginal variance from `TT'`, records their geometric mean, and divides T by its square root so the scaled geometric-mean marginal variance is one. PyMC uses `sigma*(sqrt(phi)*u_star+sqrt(1-phi)*v)` with exact component sum-to-zero structured support and Poisson log-offset likelihood. Worker validation recomputes the scaled-transform typical variance.
- Result/claims: strict `marklab.bayesian_bym2_fit` version one reports scaling convention/value, posterior fixed effects/sigma/phi, structured and unstructured contributions, combined field, risks, constraints/rank deficiency, diagnostics, and count PPC. It is experimental synthetic reparameterization evidence—not calibrated epidemiology, patient risk, biology, causality, or clinical evidence.

## IC-0054 — Exact 1-D Matérn spatially varying coefficient

- CLI/input: `marklab bayes spatial-varying-coefficient` consumes 8–64 exact unique micrometre coordinates with finite outcome/global/spatial predictors, exact distinct predictor names, full-rank intercept/global/spatial-mean design, explicit Normal fixed-effect and HalfNormal Matérn amplitude/length priors, known positive noise, jitter, and bounded NUTS controls.
- Field/model: Rust supplies a deterministic orthonormal Helmert basis for the sum-zero subspace. PyMC projects the exact dense Matérn-3/2 covariance into that basis and uses a noncentered Cholesky field, so `eta=intercept+global_x*beta_global+spatial_z*(beta_spatial+delta)` has exact centered delta. No varying-intercept alias, unconstrained field mean, interpolation, or 2-D meaning is implicit.
- Result/claims: strict `marklab.bayesian_spatial_varying_coefficient_fit` version one binds data/backend/request identity and reports fixed/kernel posteriors, every deviation/varying coefficient, exact constraint, diagnostics, and response PPC. It is experimental synthetic 1-D regression—not causal heterogeneity, tissue validation, biology, or clinical evidence.

## IC-0055 — Audited annealed SMC normal-mean inference

- CLI/input: `marklab bayes normal-mean-smc` reuses IC-0039 Normal prior/known-sigma observations with 100–10,000 particles, 2–4 chains, ESS target and IMH correlation threshold in `(0,1)`, deterministic seed, bounded total particles/runtime, and fresh output.
- Engine/audit: a source-bound subclass of pinned PyMC 6.3.0 IMH SMC records every stage's strictly increasing beta, conditional ESS, acceptance rate, and exact systematic-resampling ancestor index for every particle. Every chain must end at beta one and produce one finite log marginal likelihood. Parent worker capture is bounded at 16 MiB to retain declared ancestry; prior workers retain their stricter request-level 1 MiB limits.
- Result/claims: strict `marklab.bayesian_normal_mean_smc` version one reports equally weighted final particle summary/PPC, chain evidence and mean/SD, full stage/ancestry artifacts, method-specific finiteness/ESS/acceptance diagnostics, identities, and complete/nonconverged state. It does not fabricate NUTS R-hat, E-BFMI, divergence, or depth fields and remains experimental inference-engine evidence.

## IC-0056 — Differentiated Poisson log-rate Laplace approximation

- CLI/input: `marklab bayes poisson-log-rate-laplace` consumes 1–100,000 exact-ID nonnegative integer counts with finite positive exposures, a Normal prior on unconstrained log rate, finite initial state, 1–100,000 optimizer iterations, positive gradient tolerance at most `1e-2`, deterministic PPC seed, timeout, and fresh output. Aggregate counts must fit signed backend range and aggregate exposure remain finite.
- Approximation: pinned SciPy 1.18.1 BFGS uses exact PyTensor 3.2.4 log-joint gradients, followed by bounded exact-gradient Newton refinement. Success requires optimizer success and gradient norm within tolerance. Exact negative Hessian must be positive; its inverse is the Gaussian log-rate variance. Rate summaries use the explicit lognormal transformation.
- Result/claims: strict `marklab.bayesian_poisson_log_rate_laplace` version one reports mode/log joint, optimizer evaluations/message/gradient, Hessian/variance/conditioning, log-rate/rate approximations, PPC, and identities. A valid result is always `approximate_only`, never complete exact posterior; invalid optimization is diagnostic-only nonconverged.

## IC-0057 — Nested-Laplace Poisson-lognormal approximation

- CLI/input: `marklab bayes poisson-lognormal-inla` consumes 3–16 exact region IDs with nonnegative integer counts and positive finite exposures, a finite latent mean, positive Gamma shape/rate for precision tau, 21–201 evenly spaced finite log-tau grid points, endpoint-mass limit, bounded NUTS controls/seed/timeout, and fresh output. Counts and aggregates must fit the signed pinned backend range.
- Approximation/comparison: at every log-tau point, pinned SciPy 1.18.1 BFGS plus exact PyTensor 3.2.4 gradient/Newton refinement finds the independent latent-field mode; exact diagonal negative Hessians must be positive. The Laplace log density includes the Gaussian determinant, Gamma density, and log-tau Jacobian. Trapezoidal masses integrate conditional Gaussian latent marginals and tau moments. A noncentered same-model PyMC 6.3.0 NUTS fit must pass the shared R-hat/ESS/E-BFMI/divergence/depth gates and agree on latent means within RMSE 0.15.
- Result/claims: strict `marklab.bayesian_poisson_lognormal_inla` version one binds environment/worker/request/data identity and reports every grid coordinate/density/mass/mode-gradient/Hessian diagnostic, endpoint/normalization state, tau and latent marginals, NUTS comparison, and count PPC. Rust recomputes grid coordinates, tau transformation, weights, endpoints, mode and HMC gates before publication. Valid output is always `approximate_only`; it is a small independent latent-Gaussian approximation diagnostic, not general INLA, spatial dependence, calibrated uncertainty, epidemiology, biology, causality, or clinical evidence.

## IC-0058 — Held-out-unit-aware PSIS-LOO

- CLI/input: `marklab bayes psis-loo` consumes a complete strict `chain,draw,unit_id,log_likelihood` matrix with 2–8 zero-based contiguous chains, 100–100,000 draws per chain, 2–10,000 exact units, at most 500,000 finite values, caller-declared model name/likelihood target/held-out-unit kind, lowercase SHA-256 data/preprocessing declarations, explicit relative efficiency in `(0,1]`, timeout, and fresh output. Rust canonicalizes chain/draw/lexical-unit order and rejects missing, duplicate, or nonrectangular cells; declared comparison identities are not source authentication.
- Backend/diagnostics: pinned ArviZ 1.3.0 and arviz-stats 1.3.1 compute log-scale PSIS-LOO with pointwise output. The result retains total/pointwise ELPD, SE, effective parameter count, every Pareto-k, the backend's sample-size-dependent good-k threshold, and exact units exceeding it. Such units require refit or grouped K-fold follow-up; their scores are not called reliable.
- Result/claims: strict `marklab.bayesian_psis_loo` version one binds environment/worker/request/matrix identity and reports sample/unit dimensions, relative efficiency, pointwise/aggregate predictive accuracy, reliability, and refit/K-fold recommendations. Rust recomputes pointwise ELPD sum, maximum k, threshold membership, warning, and exact unit list. The held-out unit is caller-declared and never silently inferred as cell; output is an experimental predictive diagnostic, not automatic refitting, arbitrary model comparison, model truth, causality, biology, or clinical evidence.

## IC-0059 — Compatible pointwise Bayesian predictive comparison

- Input/compatibility: `marklab bayes compare-models` consumes 2–16 strict IC-0058 JSON artifacts bounded to 16 MiB each. Model names must be unique; likelihood target, held-out-unit kind, declared data/preprocessing SHA-256 identities, and exact ordered unit IDs must match. Every artifact's backend/lock/worker/request/matrix identities, dimensions, finite totals, pointwise sum, Pareto threshold/list, reliability, and claim ceiling are revalidated after deserialization.
- Comparison: rank descending total ELPD with lexical exact-tie break. For every lexical model pair, A-minus-B difference is the sum of aligned pointwise ELPD differences and SE is `sqrt(n*sample_variance(difference_i))`. Source paths, artifact/request digests, ELPD/SE/p-LOO, maximum Pareto-k, and reliability remain attached to each model.
- Result/claims: strict `marklab.bayesian_model_comparison` version one reports compatibility identity, unit count, deterministic ranks, highest-ELPD model, every pairwise difference/SE, and exact models needing refit/K-fold. Any source warning makes the comparison `requires_refit_or_kfold`; ranking is predictive description, not automatic model selection. No stacking, exact refit, K-fold execution, model truth, causality, biology, or clinical evidence is implied.

## IC-0060 — Conjugate normal-mean simulation-based calibration

- CLI/model: `marklab bayes normal-mean-sbc` consumes a finite Normal prior, positive known observation SD, 1–1,000 observations per replicate, 20–5,000 replicates, 20–5,000 posterior draws, deterministic seed, bounded total simulated values/timeout, and fresh output. The typed model uses exact conjugate independent posterior sampling through pinned NumPy 2.4.6/SciPy 1.18.1; this contract does not claim to calibrate NUTS.
- Replicates/diagnostics: each domain-separated replicate retains prior truth, exact posterior mean/SD, empirical equal-tail interval, strict-below rank with randomized tie handling, coverage, z-score, and shrinkage; invalid replicates are explicit failures and yield no rank. Diagnostics retain a bounded rank histogram, chi-square/p-value, ECDF/DKW envelope, 95% coverage/error, z mean/SD, shrinkage, and failure rate. Independent exact posterior draws are exchangeable, so autocorrelation correction is explicitly not required for this workflow only.
- Result/claims: strict `marklab.bayesian_normal_mean_sbc` version one binds model/backend/lock/worker/request/seed identity and reports every replicate/failure. Rust recomputes ranks-to-histogram, chi-square, ECDF/envelope, coverage/error, z, shrinkage, failure rate, and complete/nonconverged gates. It is an experimental calibration-procedure oracle—not NUTS/SMC/VI/Laplace calibration, arbitrary model validation, real-data evidence, biology, causality, or clinical evidence.

## IC-0061 — Conjugate Normal prior-sensitivity workflow

- CLI/input: `marklab bayes normal-mean-prior-sensitivity` consumes 2–100,000 finite observations, 2–32 unique exact named Normal priors with one declared base, positive known observation SD, finite decision threshold, decision posterior-probability threshold in `(0.5,1)`, positive material posterior-mean shift, timeout, and fresh output. Prior plausibility is caller responsibility.
- Refit/comparison: pinned SciPy 1.18.1 computes each exact conjugate posterior, `P(mu>threshold)`, declared binary decision, and exact leave-one-out Normal predictive ELPD. Each alternative retains posterior mean/SD, decision probability/conclusion, LOO ELPD, and differences from base; exact conclusion changes and material mean shifts are named.
- Result/claims: strict `marklab.bayesian_normal_mean_prior_sensitivity` version one binds observations/prior grid/model/backend/lock/worker/request identities and revalidates exact posterior/LOO arithmetic, deltas, decisions, and flag lists in Rust. It reports sensitivity for the supplied plausible grid without selecting a prior by outcome and is not arbitrary robustness, model truth, real-data validation, biology, causality, or clinical evidence.

## IC-0062 — Rectangular log-linear inhomogeneous Poisson likelihood

- CLI/input: `marklab bayes inhomogeneous-poisson-likelihood` consumes a finite positive-area half-open micrometre rectangle, 1–100,000 unique exact-ID finite events inside it, finite event covariate/offset, finite intercept/one coefficient, and a declared 1–1,024 by 1–1,024 grid with at most one million cells. Quadrature CSV supplies exactly one finite covariate/offset for every zero-based `(ix,iy)` cell.
- Quadrature/likelihood: node midpoints, cell widths/heights, and equal positive cell areas are derived from the rectangle and grid, proving complete nonoverlapping regular coverage with total mathematical weight equal to window area. Event term sums `intercept+beta*x+offset`; integral uses every `cell_area*exp(eta)`; both use compensated accumulation and reject non-finite terms/results.
- Result/claims: strict `marklab.inhomogeneous_poisson_likelihood` version one reports exact paths/content digests, units, rectangle/grid/cell geometry, event/node counts, parameters, event/integral terms, and their likelihood difference. It is a fixed-parameter experimental point-process likelihood—not fitted intensity, arbitrary polygons/covariate interpolation, pooled patterns, posterior inference, biology, causality, or clinical evidence.

## IC-0063 — Fitted rectangular inhomogeneous Poisson process

- CLI/input: `marklab bayes fit-inhomogeneous-poisson` reuses IC-0062's exact half-open micrometre rectangle, event covariate/offset, complete regular midpoint grid, and derived equal cell areas for 20–100,000 events and 4–4,096 cells. The grid covariate must vary materially. Caller supplies finite Normal means/positive SDs for intercept/coefficient and bounded NUTS controls/seed/timeout.
- Model/backend: pinned PyMC 6.3.0 samples `intercept` and one coefficient under a Potential equal to the exact IC-0062 event linear-predictor sum minus complete-grid integral. Rust canonicalizes events/nodes, derives geometry and exact event-to-cell counts, and caps iteration plus draw-cell work. Shared prior/posterior finiteness, R-hat, ESS, MCSE, E-BFMI, divergence, depth, constraint, and identifiability gates control complete/nonconverged state.
- Result/claims: strict `marklab.bayesian_inhomogeneous_poisson_fit` version one binds model/data/window/grid/backend/lock/worker/request identity and reports posterior parameters plus every derived midpoint's covariate/offset/observed count/intensity/expected count/Pearson residual. Posterior prediction simulates Poisson counts under the declared piecewise-constant cell approximation and reports total/zero-cell summaries. Complete remains experimental synthetic inference—not quadrature convergence, continuous within-cell intensity, arbitrary windows, process adequacy, biology, causality, or clinical evidence.

## IC-0064 — Berman–Turner rectangle-grid refinement

- CLI/input: `marklab bayes berman-turner-refinement` reuses IC-0062 events/window and accepts complete coarse/fine regular midpoint covariate/offset grids with dimensions 1–1,024, at most 100,000 observed-plus-dummy nodes per resolution, nested integer-multiple fine dimensions, finite fixed intercept/coefficient, positive convergence tolerance, and fresh output.
- Table/objective: each cell contains every assigned observed event plus one midpoint dummy. Exact cell area is split equally among those nodes; observed pseudo-response is `1/weight`, dummy response is zero, and node weights are positive with compensated total equal to window area. The parameter-dependent weighted Poisson objective is `sum weight*(response*eta-exp(eta))` with finite compensated accumulation.
- Result/claims: strict `marklab.berman_turner_refinement` version one binds all paths/content/window/grid/parameter identities and reports coarse/fine resolution, node-kind counts, cell area, weight sum, objective, absolute change/tolerance/convergence, and the complete fine table. Constant intensity agrees algebraically with IC-0062; general-covariate agreement is empirical approximation evidence only—not arbitrary-window accuracy, fitted convergence, process adequacy, biology, causality, or clinical evidence.

## IC-0065 — Exact rectangle-grid LGCP model construction

- CLI/input: `marklab bayes build-gridded-lgcp` reuses IC-0062's half-open micrometre rectangle/events and one complete regular 2–64-cell midpoint covariate/offset grid. Every full cell has derived exact area/midpoint and every event is assigned once; center evaluation is the explicit covariate/offset rule.
- Field/model: caller supplies finite Normal intercept/coefficient means/positive SDs and positive field amplitude/physical length/jitter. The zero-mean latent field covariance is dense Euclidean two-dimensional Matérn-3/2 at cell midpoints with jitter only on the diagonal; deterministic Cholesky proves positive definiteness under bounded `n^2` storage/`n^3` work. Cell likelihood is `Poisson(area*exp(intercept+beta*x+offset+z))`.
- Result/claims: strict `marklab.gridded_lgcp_model` version one binds paths/content/window/grid identities and reports typed model semantics, every cell/count, exact dense covariance/digest, positive-definite state, and resource counts. It is experimental model construction only—not a fitted LGCP, continuous-window exactness/interpolation, quadrature convergence, posterior prediction, biology, causality, or clinical evidence.

## IC-0066 — Fitted dense rectangle-grid LGCP

- CLI/input: `marklab bayes fit-gridded-lgcp` reuses IC-0065's exact half-open rectangle, complete 4–36-cell midpoint grid, conserved cell counts, Normal fixed-effect priors, and fixed positive Matérn amplitude/physical length/jitter. The cell covariate must vary materially; bounded NUTS controls, seed, timeout, iterations, draw-cell work, and fresh output are required.
- Model/backend: Rust supplies the exact IC-0065 covariance and deterministic Cholesky to a static pinned PyMC 6.3.0 worker. Independent standard-Normal field coordinates map through that fixed Cholesky, and observed cell counts follow `Poisson(area*exp(intercept+beta*x+offset+field))`. The worker proves covariance/factor agreement and applies the shared prior/posterior finiteness, R-hat, ESS, MCSE, E-BFMI, divergence, depth, support, and identifiability gates.
- Result/claims: strict `marklab.bayesian_gridded_lgcp_fit` version one binds model/data/window/grid/covariance/backend/lock/worker/request identity and reports fixed-effect posteriors, every cell's latent effect/intensity/expected count/Pearson residual, and posterior-predictive total/zero-cell counts. Complete remains experimental fixed-hyperparameter cell inference—not inferred kernels, continuous interpolation or locations, arbitrary windows, quadrature convergence, calibration, biology, causality, or clinical evidence.

## IC-0067 — Explicit gridded-LGCP posterior predictive patterns

- CLI/input: `marklab bayes simulate-gridded-lgcp-posterior-predictive` reuses the complete IC-0066 input and NUTS lifecycle and adds 1–32 replicas, an independent prediction seed, a positive realized-point cap no greater than 100,000, and fresh output. Only a fit satisfying every shared diagnostic gate produces patterns.
- Simulation: replica `s` selects exact flattened posterior index `floor(s*chains*draws/S)` with retained chain/draw identity. It samples independent Poisson counts from that draw's exact cell expected counts, then uniform coordinates inside each exact full rectangle cell because IC-0066's admitted intensity is piecewise constant there. Domain-separated seeded streams, cell-major point order, exact point IDs, total resource caps, and the 16 MiB process boundary are enforced.
- Result/claims: strict `marklab.bayesian_gridded_lgcp_posterior_predictive` version one binds fit/model/data/covariance/backend/request identities and reports complete diagnostics, every selected fixed/latent draw, cell intensity/expectation/count, generated point/cell/coordinate, totals, and approximation metadata. It is deterministic experimental discretized prediction—not a certified continuous intensity bound, thinning, within-cell interpolation, arbitrary-window simulation, posterior adequacy, biology, causality, or clinical evidence.

## IC-0068 — Bounded exact-window Thomas process simulation

- API/input: `simulate_thomas_process` and `marklab bayes simulate-thomas-process` consume a finite positive-area half-open micrometre rectangle, positive finite parent intensity per square micrometre, mean offspring, Gaussian displacement SD, one seed, 1–100,000 parent/generated-offspring caps, and fresh output. Expected and realized parent/offspring counts must fit their declared caps.
- Simulation: dilate the rectangle by exactly six displacement SDs as a Euclidean Minkowski sum with area `wh+2r(w+h)+pi*r^2`. A pinned seeded ChaCha20 stream and `rand_distr` 0.4.3 sample homogeneous Poisson parents on that exact rounded shape by bounded-box rejection, Poisson offspring per parent, and independent isotropic Normal-2D displacement; only children in the exact half-open observation window are retained.
- Result/claims: strict `marklab.thomas_process_simulation` version one reports exact parameters/window/RNG/sampler identity, latent parent coordinates and per-parent counts, every retained offspring/parent identity and coordinate, proposals/generated/retained/discarded totals, and six-SD radial tail bound `exp(-18)`. It is deterministic experimental finite-expansion simulation—not an infinite-plane exact realization, fitted model, adequacy check, biology, causality, or clinical evidence.

## IC-0069 — Bounded exact-window Matérn cluster simulation

- API/input: `simulate_matern_cluster_process` and `marklab bayes simulate-matern-cluster-process` reuse IC-0068's exact window, positive parent intensity/mean offspring, seed, 1–100,000 expected/realized resource caps, and fresh output with one positive finite offspring-disc radius.
- Simulation: the parent window is the exact Euclidean rectangle dilation by that radius. Pinned ChaCha20/`rand_distr` sample homogeneous Poisson parents on the rounded dilation by bounded rejection and Poisson offspring per parent. Each displacement is uniform on the exact disc through `r=R*sqrt(U), angle=2*pi*V`; retained children use the exact half-open observation window. The bounded displacement makes this parent expansion exact.
- Result/claims: strict `marklab.matern_cluster_process_simulation` version one reports exact parameters/window/RNG/sampler identity, boundary/displacement rules, latent parents and per-parent counts, every retained child/parent coordinate, and proposal/generated/retained/discarded totals. It is deterministic experimental exact-window simulation for the declared bounded process—not fitted inference, process adequacy, biology, causality, or clinical evidence.

## IC-0070 — Thomas K-function minimum-contrast fit

- CLI/input: `marklab bayes fit-thomas-minimum-contrast` consumes exact `radius_um,observed_k_um2,weight` CSV with 8–1,000 strictly increasing positive micrometre radii, finite nonnegative K, finite positive weights, positive observed point intensity, explicit positive ordered kappa/sigma bounds, 10–100,000 evaluations, timeout, and fresh output.
- Fit: the theoretical curve is `pi*r^2 + kappa^-1*(1-exp(-r^2/(4*sigma^2)))`. Pinned SciPy 1.18.1 bounded least squares optimizes log kappa/log sigma under fourth-root contrast for caller-weighted full range, unit-weight full range, and caller-weighted interior range. Mean offspring is identified only as observed intensity divided by fitted parent intensity. Rust independently recomputes every curve, residual, contribution, objective, bound, and derived mean.
- Result/claims: strict `marklab.thomas_minimum_contrast_fit` version one binds input/model/backend/lock/worker/request identities and reports primary/sensitivity parameters, objectives, convergence/status/message/evaluations/optimality, and full fitted curve. Valid output is experimental minimum contrast; likelihood comparison is explicitly unavailable while BAY-CLUSTER-FIT-01 lacks an admitted latent-parent backend. It is not likelihood inference, latent-parent recovery, finite-sample calibration, biology, causality, or clinical evidence.

## IC-0071 — Exact fixed-pattern Strauss statistic and Papangelou intensity

- API/input: `strauss_statistics`, `strauss_papangelou`, and `marklab bayes strauss-statistics` consume 0–100,000 exact-ID finite 2-D micrometre points, positive finite interaction radius, a finite proposal distinct from all existing coordinates, positive finite beta, gamma in `[0,1]`, a 1–100,000,000 exact pair-visit cap, and fresh output.
- Mechanics: `n` is point count and `s_R` counts each unordered existing pair once when Euclidean distance is `<=R`. Proposal delta counts existing neighbors under the same inclusive boundary; conditional intensity is `beta*gamma^delta`, with explicit gamma-zero convention `0^0=1` and `0^positive=0`. Exact pair work is bounded before iteration and non-finite distances/intensity are errors.
- Result/claims: strict `marklab.strauss_statistics` version one binds input/path/content/radius/proposal/parameter/resource identity and reports point/pair/delta/Papangelou values plus boundary convention. It is experimental fixed-pattern mechanics—not fitted/normalized Strauss inference, Gibbs simulation, model adequacy, biology, causality, or clinical evidence.

## IC-0072 — Bounded Strauss birth/death Metropolis simulation

- API/input: `simulate_strauss_birth_death` and `marklab bayes simulate-strauss-birth-death` consume a finite positive-area half-open micrometre rectangle, positive finite beta/radius, gamma in `[0,1]`, 100–100,000 iterations, burn-in leaving at least two states, seed, 1–10,000 point cap, 1–100,000,000 neighbor-visit cap, and fresh output. The chain starts empty.
- Transitions: each iteration selects birth/death with probability one half. Uniform-window birth accepts `min(1, area*lambda(u|x)/(n+1))`; uniform-existing-point death accepts `min(1,n/(area*lambda(x_i|x\i)))`; empty death is a null transition. IC-0071 inclusive-radius delta/gamma-zero mechanics and a domain-separated pinned ChaCha20 stream apply. Any point/visit/non-finite acceptance overflow aborts.
- Result/claims: strict `marklab.strauss_birth_death_simulation` version one reports exact parameters/window/seed/RNG/transition/resource identity, all proposal/acceptance/null counts, neighbor work, full count trace, post-burn overall/half means and drift, Poisson special-case expectation when gamma one, and final exact-ID pattern. It is experimental finite-chain simulation—not an exact independent draw, convergence proof, fitted model, adequacy evidence, biology, causality, or clinical evidence.

## IC-0073 — Nested-grid Strauss pseudolikelihood fit

- CLI/input: `marklab bayes fit-strauss-pseudolikelihood` consumes 1–10,000 exact-ID points in an exact half-open micrometre rectangle, positive radius, nested positive coarse/fine grids with at most 100,000 total rows, explicit positive beta bounds/gamma bounds in `(0,1]`, bounded iterations/neighbor work/timeout, and fresh output.
- Table/fit: at each resolution every cell gets one midpoint dummy and every observed point; exact cell area is split equally, observed response is `1/weight`, and IC-0071 neighbor count excludes self only for observed nodes. Pinned SciPy 1.18.1 fits `log(lambda)=log(beta)+t_R*log(gamma)` as bounded weighted Poisson pseudolikelihood and reports exact gradient/Hessian, model covariance, and 2x2-window-block sandwich SE. Rust recomputes tables, weights, features, objectives, bounds, state, and refinement differences.
- Result/claims: strict `marklab.strauss_pseudolikelihood_fit` version one binds input/model/window/grid/backend/lock/worker/request identity and reports coarse/fine parameters, objective improvement, derivatives/conditioning, model/robust SEs, weight sums, and refinement sensitivity. It is experimental pseudolikelihood—not normalized likelihood/posterior, calibrated interaction, biology, causality, or clinical evidence.

## IC-0074 — Pointwise-saturated Geyer statistic

- API/input: `geyer_saturation_statistic` and `marklab bayes geyer-saturation-statistic` reuse IC-0071's exact-ID finite 2-D points, positive finite radius, 1–100,000,000 pair cap, and fresh output with a nonnegative integer saturation.
- Statistic/result: count each unordered inclusive-radius pair once and increment both endpoints; report every raw neighbor count and `min(s,count)`, then sum saturated counts under the explicit pointwise-density convention. Strict `marklab.geyer_saturation_statistic` version one binds input and reports pair work/interactions/point values. It is fixed-pattern mechanics—not stability proof, fitted/simulated Geyer process, biology, causality, or clinical evidence.

## IC-0075 — Typed symmetric multitype Papangelou intensity

- API/input: `multitype_papangelou` and `marklab bayes multitype-papangelou` consume 1–64 exact types with finite constant log baselines, a complete unique exact-symmetric KxK matrix of finite log pair potentials/positive radii, 0–100,000 unique typed finite points, one distinct typed proposal, bounded visits, and fresh output.
- Mechanics/result: begin at the proposal type's log baseline; each existing type contributes its matrix potential iff Euclidean distance is `<=` its pair radius; exponentiate once with overflow rejection. Strict `marklab.multitype_papangelou` version one binds all three inputs and reports baseline/pair/log/final intensity plus every included/excluded typed contribution. It is constant-baseline symmetric conditional-intensity mechanics—not fitted/normalizable multitype inference, multiplicity-adjusted discovery, biology, causality, or clinical evidence.

## IC-0076 — Joint location–categorical-mark model construction

- CLI/input: `marklab bayes build-joint-location-mark-model` consumes 4–10,000 unique typed points in an exact half-open rectangle with finite location covariate/offset, mark covariate, and neighborhood effect plus a complete 4–4,096-cell location grid; require 2–16 marks, at least two points each, exact reference mark, and positive prior scales.
- Model/result: strict `marklab.joint_location_categorical_mark_model` version one declares the exact rectangular log-linear point-location component and reference-category softmax conditional mark component with reference coefficients/field fixed zero and independent nonreference mark fields. It binds inputs/counts/grid/priors and requires comparison to the nested zero-neighborhood/zero-mark-field random-labeling model. It is not fitted coupling, causal neighborhood influence, biology, or clinical evidence.

## IC-0077 — Joint location–continuous-mark shared-field construction

- CLI/input: `marklab bayes build-joint-continuous-mark-model` consumes 8–10,000 unique finite points with varying continuous mark/location/mark covariates in an exact rectangle plus complete grid, positive known mark noise, fixed positive Matérn amplitude/physical length/jitter, and positive loading/private-field prior scales.
- Model/result: strict `marklab.joint_location_continuous_mark_model` version one declares exact-grid log-linear location and Gaussian identity-link conditional mark components. A fixed-scale zero-mean shared 2-D Matérn field has location loading exactly one and positive HalfNormal mark loading; independent private location/mark fields remain separate. It requires comparison to nested zero-shared-loading separate models. Construction is not fitted correlation/coupling, biology, causality, or clinical evidence.

## IC-0078 — Joint location–embedding spatial factor construction

- CLI/input: `marklab bayes build-joint-location-embedding-factor-model` consumes 8–2,000 unique points in an exact rectangle with varying location covariate, complete grid, 2–128 exact ordered `embedding_*` dimensions each varying, 1–16 factors less than rows/dimensions, and positive fixed Matérn/loading/noise scales.
- Model/result: strict `marklab.joint_location_embedding_factor_model` version one declares `embedding=Wz+diagonal_noise`, zero-mean independent 2-D Matérn factor fields, and exact-grid location intensity with shrunk factor coefficients. The first K loading rows are lower triangular with positive diagonal; remaining loadings/location coefficients use regularized global-local shrinkage. It retains dimensions/features and requires vector-variogram/kernel-mark-correlation validation. Construction is not fitted domains, biology, causality, or clinical evidence.

## IC-0079 — Replicated hierarchical LGCP construction

- CLI/input: `marklab bayes build-replicated-hierarchical-lgcp` consumes 6–1,000 exact unique pattern rows nested under at least three patients/two patterns each; every row binds lowercase SHA-256 window/grid/covariate identities, positive event count, and 4–36 cells. Positive global/patient/field/jitter scales and explicit independent-replicate or shared-plus-replicate field policy are required.
- Model/result: strict `marklab.replicated_hierarchical_lgcp_model` version one declares shared fixed effects/population Matérn hyperparameters, patient random intercepts, and either independent replicate fields or patient-shared plus replicate fields. Every pattern retains its own likelihood/window; concatenation is forbidden. It is construction—not fitted pooling, exchangeability proof, calibrated contrast, biology, causality, or clinical evidence.

## IC-0080 — Translation-K posterior-predictive point-process diagnostics

- CLI/input: `marklab bayes point-process-posterior-predictive-diagnostics` consumes 1–100 observed exact-ID patterns and 20–1,000 complete replicated sets in one exact rectangle, 2–256 increasing positive radii within its diagonal, alpha `(0,0.5)`, exact content identities, bounded pair visits, and fresh output. Every pattern has at least two points and every replica contains every observed pattern ID.
- Estimator/envelope: compute count and the same rectangle translation-corrected ordered-pair K per pattern/replica/radius, then unweighted mean over declared patterns. Standardize replica curves pointwise, take each replica's maximum absolute deviation, and use nearest-rank `1-alpha` critical value for a simultaneous mean±critical×SD envelope. Retain all curve/count/critical/bound/exceeded-radius diagnostics with no binary pass. It is not model truth, g/mark diagnostics, biology, causality, or clinical evidence.

## IC-0081 — Omnibus vector semivariogram

- CLI/input: `marklab bayes vector-semivariogram` consumes 2–10,000 unique finite micrometre-coordinate rows with 2–4,096 ordered `embedding_*` dimensions, 1–256 contiguous physical bins, a bound on all unordered pair visits, and optional exact complete positive weights for eligible pairs. Bins are lower-inclusive/upper-exclusive except the final inclusive upper edge.
- Estimator/result: use compensated `f64` accumulation of `0.5 * weight * squared Euclidean vector distance`; report exact pair counts, weight sums, optional bin values, inference eligibility, ordered features, physical bins, and input identities. Empty bins are unavailable. The curve is invariant under orthogonal embedding rotation and is descriptive—not edge-corrected, inferential, patient-level, or real-asset validation.

## IC-0082 — Split-safe projected embedding variograms

- CLI/input: `marklab bayes projected-embedding-variograms` consumes exact biological-unit/split/permutation-stratum identities, finite coordinates, 2–128 ordered embedding dimensions, physical bins, 1–16 components, 20–10,000 permutations, deterministic seed, pair/work limits, and timeout. Biological units cannot cross train/validation/test splits.
- Projection/inference: pinned SciPy 1.18.1 fits centered PCA only on training rows, orders positive eigenvalues, and orients the largest absolute loading positive. Rust independently checks the training mean, covariance eigenproblem, orthonormality, sign, pair counts, and observed variograms. Frozen projections are random-labeled as complete vectors within split strata; single-step max-T controls every nonempty component × scale family. Retain the projection/backend/lock/worker artifact and typed degenerate-null cells; no held-out fitting, post-hoc selection, patient-level comparison, or biological claim is permitted.

## IC-0083 — Distance-binned embedding cross-covariance matrices

- CLI/input: `marklab bayes embedding-cross-covariance-by-distance` consumes the IC-0081 vector/bin contract plus explicit pair and pair-matrix element work bounds.
- Estimator/result: compute one compensated global vector mean, average ordered outer products by physical bin, and symmetrize each nonempty matrix as `(C + C^T) / 2`. Report pair counts, optional trace/Frobenius invariants, ordered features, exact identities, and a typed full-matrix artifact; empty bins are unavailable. This is unweighted descriptive within-modality covariance, not cross-modal correspondence, edge correction, null inference, or real-asset validation.

## IC-0084 — Cross-modal covariance with stratified random-label inference

- CLI/input: `marklab bayes cross-modal-covariance-by-distance` consumes two exact finite modality tables with stable object/source-section/compartment identities and 1–128 ordered embedding dimensions, contiguous bins, and a unique positive-weight A–B pair plan that exactly agrees with both rows' section/compartment. Mean policy is global or compartment-stratified; pair/permutation/component-work bounds are explicit.
- Estimator/inference: normalize weighted centered rectangular outer products by bin weight and report complete matrices plus independently rotation-invariant Frobenius norms. Deterministically relabel complete B vectors within source-section × compartment strata and single-step max-T adjust the nonempty-bin norm family. This exact synthetic association does not prove correspondence, registration, patient-level effects, biology, causality, or clinical utility.

## IC-0085 — Training-fitted kernel mark correlation

- CLI/input: `marklab bayes kernel-mark-correlation` consumes exact biological-unit train/validation/test rows, finite micrometre coordinates, 2–128 ordered embedding dimensions, physical bins, linear/cosine/RBF/Laplacian choice, positive-reference tolerance, and pair-work bound. Biological units cannot cross splits.
- Kernel/curve: center from training rows only; centered linear/cosine use exact dot/normalized-dot formulas, while RBF/Laplacian freeze median positive training-pair Euclidean/L1 scale. Retain the PSD kernel artifact and compute per-split complete global reference plus raw bin means; normalize only above tolerance, otherwise return `zero_global_kernel_reference`. No learned metric, outcome selection, inference, or real-asset claim is included.

## IC-0086 — Complete-vector ERL spatial-dependence envelope

- CLI/input: `marklab bayes embedding-spatial-dependence-envelope` consumes exact finite rows/strata/coordinates/vectors, contiguous bins, 20–10,000 permutations, alpha with `(B+1)alpha >= 1`, deterministic seed, and total pair-work bound. The first curve function is exact omnibus vector semivariance.
- Null/envelope: leave coordinates fixed and permute complete embedding rows only within declared strata. Apply average two-sided pointwise ranks, sorted extreme-rank vectors, normalized ERL depths, plus-one global p-value, and one simultaneous depth envelope over every nonempty bin; empty bins stay typed unavailable. This conditions on observed locations/strata and is not CSR, stationarity proof, patient-level inference, or biology.

## IC-0087 — Graph Dirichlet energy

- CLI/input: `marklab bayes graph-dirichlet-energy` consumes 2–100,000 exact finite node vectors, 1–1,000,000 unique unordered positive-weight edges, combinatorial or symmetric-normalized Laplacian, NONE/SIGNAL/EDGE_WEIGHT normalization, and component-edge work bound. The compact edge list implies symmetric zero-diagonal `W`; normalized Laplacian rejects islands.
- Estimator/result: compute the unique-edge quadratic form with compensated `f64` sums. SIGNAL uses globally centered signal variation; EDGE_WEIGHT uses twice the unique-edge sum, matching full symmetric `sum(W)`. Retain numerator/denominator/energy, ordered features, counts, exact graph digest, and conventions. This is descriptive, not inferential or scale-validating.

## IC-0088 — Complete-vector graph smoothness permutation test

- CLI/input: `marklab bayes graph-smoothness-permutation-test` adds exact node strata, 20–10,000 permutations, seed, and total work bound to IC-0087; every stratum has at least two nodes.
- Null/result: reuse the exact graph/Laplacian and invariant global SIGNAL denominator, permute complete signal rows within strata, and prespecify unusually low energy as smoothness. Return inclusive plus-one lower-tail p, retained inclusive count, null moments/range, observed numerator/energy, graph digest, and deterministic policy. It is not graph selection, patient-level inference, or local hotspot discovery.

## IC-0089 — Local embedding roughness artifact

- CLI/input: `marklab bayes local-embedding-roughness` consumes the IC-0087 graph/vector contract, finite positive epsilon, and work bound.
- Map/result: for each node divide incident weighted squared vector differences by `max(weighted_degree, epsilon)` and retain numerator, denominator, degree, neighbor count, value, and status. Islands remain with zero roughness and `isolated_node`; connected nodes are `available`. The node artifact is experimental/descriptive and emits no hotspot label or multiplicity-free inference.

## IC-0090 — Canonical cell–patch validation, weighted context, and dependency count

- Existing validation owner: C-05 `CellPatchLink` constructors validate exact expected cell/patch sets, owning slide/frame/context/footprints, contained or declared-weighted assignment semantics, shared vector-free edges, resource bounds, and logical identity; physical graph/assignment/edge receipts validate persisted boundaries. This is the canonical implementation of pseudocode `ValidateCellPatchLinks`.
- Context API: `marklab-embeddings::cell_patch_context` requires exact patch-table/link/overlap bindings, one assigned cell, all linked vectors present, and a linked-patch × dimension work bound. Equal contained weights or exact rational interpolation weights produce one `f64` context vector; missing assignment/vector is typed unavailable/error.
- Dependency API: `patch_dependency_weighting` normalizes positive unique patch weights, aggregates them by exact overlap connected component, and returns Kish `1/sum(group_weight^2)`, counts, graph identity, and `patient_not_patch`. The count is descriptive within-specimen evidence, not independent replication or inference.

## IC-0091 — Nested patient-held-out cell–patch complementarity

- `marklab bayes test-cell-patch-complementarity` fits M0–M5 ridge models through pinned SciPy 1.18.1. Patient outer folds are untouched; standardization/alpha selection are inner-fold-only. Results retain predictions, RMSE/MAE/calibration, fold alphas, and six existing paired-patient absolute-error comparisons. Synthetic evidence remains experimental.

## IC-0092 — Prespecified multiscale embedding kernel

- `marklab bayes multiscale-embedding-kernel` requires two exactly aligned scale grids/vectors and positive sum-one prespecified weights. Linear/cosine/RBF/Laplacian scale kernels produce per-scale contributions, compensated total, and renormalized drop-one-scale sensitivity.

## IC-0093 — Exact analogous-region retrieval

- `marklab bayes retrieve-analogous-regions` builds a training-only standardized exact index, validates query domain/provenance, applies patient/site leakage policy, and returns top-k distance/component explanations plus nearest-distance OOD ratio. Exact search reports recall one; analogy is not biological identity.

## IC-0094 — Patient embedding comparison and descriptive region compatibility

- `CompareEmbeddingDistributionsByPatient` maps its permitted prespecified per-patient mean/covariance/kernel-mean summaries to the completed complete-fingerprint `patient_level_mmd` or `patient_level_energy_distance` owners; cell rows are never treated as exchangeable patients.
- `marklab cohort region-compatibility` reuses exact COH-FINGERPRINT-01 distance and propagates independent endpoint standard uncertainties by a first-order delta method. Weighted component uncertainties combine by root-sum-square; a positive-uncertainty zero-distance nondifferentiable component is unavailable. Output is within-patient descriptive only.

## IC-0095 — Validation-calibrated shrinkage Mahalanobis OOD

- `marklab bayes ood-score` consumes unique finite train/validation/test representations over 2–128 exact ordered `embedding_*` features. It fits population mean/covariance on training only, shrinks off-diagonal covariance, requires validation domains absent from training, freezes a nearest-rank validation threshold, and returns Cholesky-whitened test scores with strict-exceedance flags.

## IC-0096 — Prespecified prediction abstention

- `marklab bayes apply-abstention` consumes finite predictions with nonnegative uncertainty/OOD scores and finite nonnegative validation-policy thresholds. Equality is retained; strict uncertainty and OOD exceedances independently produce canonical reasons and suppress the actionable prediction value. It does not select thresholds or establish utility.

## IC-0097 — Patient-OOF Platt probability calibration

- `marklab bayes calibrate-predictions` requires unique patient-level `training_oof` and `test` scores with both labels. Pinned SciPy 1.18.1 fits class-count-smoothed Platt logistic parameters only on OOF rows, applies them frozen to test, and reports Brier, ECE, calibration-in-the-large/slope, and reliability bins with Wilson intervals. Rust replays probabilities, Brier, ECE, and bins.

- RUST-MIGRATION-01 / DEC-0416 adds the native version-2 calibration fit and bounded CSV application. Raw training_oof scores, smoothing, all held-out diagnostics, identity/order/resource limits and legacy Python readers remain. A killable native child owns deadlines; neither the scientific library nor CSV interpreter discovers runtime assets.

## IC-0098 — Patient-level split-conformal classification

- `marklab bayes grouped-conformal` fits training-only standardization and positive-L2 logistic prediction, freezes corrected-rank `1-p(true label)` on separate calibration patients, and returns test binary sets plus overall/site/subgroup coverage. Alpha below the finite calibration resolution is rejected. Marginal exchangeability does not guarantee conditional or shifted-domain coverage.
- RUST-MIGRATION-01 / DEC-0415 adds `marklab_bayes::fit_grouped_conformal` and the root CSV application. The native version-2 envelope preserves those scientific fields while recording Rust package/source and distinct semantic request identities. Legacy Python version-1 request/response readers remain available. The schema-registered native child preserves hard deadlines and bounded streams; no Python runtime is required for this candidate workflow.

## IC-0099 — Calibrated patient-level late fusion

- `marklab bayes late-fusion` requires every base modality probability to declare patient-level OOF origin, fits probability-plus-availability logistic fusion on `meta_train`, fits Platt calibration on a separate calibration split, and evaluates test patients. It reports Brier, observed missing-modality scenarios, and per-modality ablation; these are not causal contributions.

## IC-0100 — Patient-grouped predictive stacking

- `marklab bayes predictive-stacking` consumes 8–500 unique patient-held-out rows and 2–16 finite `model_*` grouped log predictive densities. Pinned SciPy 1.18.1 maximizes mixture log density under nonnegative sum-one weights and exactly refits after leaving out each patient. Rust verifies weights, every mixture log density, and the objective; weights are predictive optimization values, not posterior model probabilities.

## IC-0101 — Context-gated calibrated mixture of experts

- `marklab bayes mixture-of-experts-fusion` consumes explicit patient-OOF expert probabilities over disjoint gate-train/calibration/test patients. Lowercase context excludes site/scanner/stain/batch names; training-only standardization plus availability gates use positive L2/entropy regularization, unavailable experts have zero weight, calibration is disjoint, and context-distance OOD is thresholded on calibration. Rust replays gates, mixtures, calibration, OOD, Brier, and mean entropy.

## IC-0102 — Balanced log-domain Sinkhorn transport

- `marklab bayes sinkhorn-ot` consumes equal positive source/target totals and a complete finite nonnegative cost matrix under explicit work bounds. Log-domain dual updates with periodic centering stop on maximum marginal residual. Zero-mass supports are removed from updates and restored exactly. Output retains plan, nullable duals, marginals, residual, cost, entropy, and regularized objective; it is not correspondence.

## IC-0103 — KL-unbalanced log-domain Sinkhorn

- `marklab bayes unbalanced-sinkhorn` accepts unequal totals and positive source/target KL penalties. Log scaling uses `tau/(tau+epsilon)` exponents and an explicit fixed-point residual. Output retains transported mass, relaxed marginals/deviations, cost, entropy, both generalized KL terms, and exact objective decomposition; it is not balanced or fixed-mass transport.

## IC-0104 — Fixed-mass entropic partial transport

- `marklab bayes partial-ot` consumes bounded capacities, complete costs, positive epsilon, and fixed transported mass no larger than either total. Pinned SciPy 1.18.1 SLSQP enforces nonnegative plan, row/column capacity, and total equality at `1e-8`; Rust replays the complete plan, feasibility, unmatched mass, cost, entropy, and objective. Unmatched mass is not automatically novel biology.

## IC-0105 — Dustbin entropic soft assignment

- `marklab bayes entropic-soft-assignment` augments each support by the opposite total mass and assigns the declared cost to every dustbin-incident edge before reusing IC-0102. Output separates real, source-unmatched, target-unmatched, and dustbin-to-dustbin mass and reports half/base/double-epsilon sensitivity. It is probabilistic compatibility, not cell identity.

## IC-0106 — Pinned-POT fused Gromov–Wasserstein

- `marklab bayes fused-gromov-wasserstein` consumes at most 32 probability-mass supports, finite features, symmetric zero-diagonal structure matrices, explicit feature/structure scales, and bounded solver controls. Pinned POT 0.9.7.post1 solves squared-loss entropic FGW from independent-mass, feature-EMD, and structure-profile-EMD starts. Rust replays every marginal, feature/structural/entropy term, objective, convergence state, and best-plan selection. Alpha is the feature weight; every plan is descriptive and non-unique.

## IC-0107 — Fixed-mass partial FGW ensemble

- `marklab bayes partial-fused-gromov-wasserstein` linearizes squared structural distortion and delegates every fixed-mass entropic subproblem to POT's log-domain partial-Wasserstein solver. Three initialization plans and seven alpha/mass/epsilon fits retain complete feasibility/objective artifacts with Rust replay. The POT partial-FGW wrapper is excluded by a reproduced scalar-feature-gradient defect. KL-unbalanced FGW remains named backend-blocked rather than being replaced by a different estimand.

## IC-0108 — Deterministic Fisher–KPP growth front

- `marklab simulate growth-front` consumes a finite regular 1-D density grid and explicit physical/numerical controls. Exact logistic half-steps surround a centered no-flux diffusion step; `D*dt/dx^2 <= 0.5`, density `[0,K]`, finite state, and declared cell-step work are mandatory. Output retains initial/final fields, mass/front trajectory, observed and theoretical speed, CFL/range diagnostics, and an experimental non-forecast claim.

## IC-0109 — Two-species spatial Lotka–Volterra competition

- `marklab simulate spatial-competition` consumes two finite density fields on one regular 1-D grid and explicit diffusion/growth/carrying/competition/treatment controls. Exact self-logistic and exponential cross-loss flows are symmetrically composed around per-species no-flux diffusion. Output retains complete states, mass/maximum trajectories, first extinction-threshold events, coexistence/exclusion status, and CFL/invariant diagnostics; treatment is a declared mortality input, not a causal effect.

## IC-0110 — Seed-replayable spatial agent competition

- `marklab simulate agent-competition` consumes exact 2-D agents/window, species event rates, neighborhood/movement controls, seed, and event/agent/pair/log limits. A bounded exact pair scan supplies opposite-species death increments; Gillespie event timing/selection uses a named ChaCha20 namespace; movement and births reflect at the window. Output retains final agents, complete event counts, bounded event log, resource usage, and exact termination reason without evolutionary or causal claims.

## IC-0111 — Periodic scalar reaction–diffusion and Fourier diagnostics

- `marklab simulate reaction-diffusion` consumes a finite nonnegative scalar field on a bounded regular periodic 2-D grid, a typed linear or logistic reaction, scalar diffusion, and explicit time/work controls. Exact reaction half-flows surround a five-point diffusion step under `D*dt*(1/dx^2+1/dy^2) <= 0.5`; the validated step plan is the executed resource plan. Output retains complete fields, trajectory/integral diagnostics, centered 2-D Fourier mode evidence, and discrete linearized wavelength bounds only at a homogeneous equilibrium. It is an experimental scalar specialization, not an arbitrary PDE solver or biological proof.

## IC-0112 — Bounded regular-grid level-set interface

- `marklab simulate evolve-interface` consumes a finite 2-D level set, static spatial normal-speed field, nonnegative curvature weight, explicit time/cadence, and cell/reinitialization work bounds. First-order Godunov upwinding plus centered curvature uses linear-extrapolation ghosts and a combined CFL at most `0.5`. Optional signed-distance reconstruction uses sampled zero nodes/edge crossings and exact bounded distance visits. Output retains complete fields, crossing diagnostics, trajectory, and solver/resource evidence without a tumour-forecast claim.

## IC-0113 — Declared-flow vascular transport

- `marklab simulate vascular-transport` consumes finite regular-grid concentration/diffusivity/static-velocity fields, sorted unique vessel sources and uptake cells, explicit numerical/work controls, and a mandatory approximation label. Sources/cells map to nearest nodes with retained distance. Exact local source/linear-uptake flows surround conservative arithmetic-face diffusion and upwind advection under no-flux boundaries and combined outgoing CFL at most one. Output retains complete fields/gradients/rates, mass decomposition, transport residual, and deterministic four-neighbor hypoxic regions; it is not hemodynamic or causal truth.

## IC-0114 — Exact hierarchical unsigned resource-distance response

- `marklab bayes distance-to-resource` consumes finite nondegenerate line-segment resources, unique finite cell outcomes/covariates, at least three replicated patients, prespecified positive spline knots, and proper known Gaussian scales. Exact point-to-segment distances feed a sorted linear-hinge/compartment/resource-density/accessibility design plus patient random intercepts. A bounded Cholesky solve returns the exact conjugate joint posterior, population response curve, per-cell predictions, and patient/resource predictive checks. The fit is associational and not a transport-causal model.

## IC-0115 — One-way interval-coupled mechanistic tissue

- `marklab simulate mechanistic-tissue` requires aligned regular-grid vascular/density/interface state and an exactly coextensive agent window. Each bounded interval calls IC-0113 vascular transport, IC-0111 logistic density, IC-0112 level-set evolution, and IC-0110 agents in order. Mean oxygen saturation and local density/oxygen are the explicit coupling values; module and aggregate resource/mass diagnostics are retained. Agent extinction is absorbing across intervals. Identity latent output is experimental and not a digital twin.

## IC-0116 — Gaussian soft pair-summary loss

- `marklab simulate summary-matching` consumes two finite same-window point patterns, ordered radius centers, positive bandwidth, prespecified aligned weights, and an exact total pair-bin work bound. Each curve is the Gaussian pair-probability density over unordered distances with analytic distance-derivative sums. The loss retains every weighted squared bin difference. No edge correction or inferential/generative-validity claim is implied.

## IC-0117 — Deterministic rejection ABC over growth-front mass

- `marklab bayes rejection-abc-growth-front` draws growth rates from a bounded uniform prior under a named ChaCha20 namespace, calls IC-0108 for each proposal, standardizes final-mass discrepancy by a declared scale, and accepts on inclusive absolute distance. Accepted proposal artifacts, acceptance/posterior diagnostics, proposal and aggregate cell-work bounds, and explicit target-not-reached failure are mandatory. The fit is a synthetic specialization, not biological calibration.

## IC-0118 — Weighted fixed-schedule SMC-ABC over growth-front mass

- `marklab bayes smc-abc-growth-front` initializes from the uniform growth prior and advances strictly decreasing epsilon stages through weighted ancestor sampling and Gaussian perturbation. Later weights use the complete previous weighted kernel mixture; each stage normalizes weights, reports ESS/moments/acceptance and next kernel scale, and fails if incomplete. All simulations call IC-0108 under a named seed namespace and aggregate work bound. Output is synthetic, not biological calibration.

## IC-0119 — Two-summary Gaussian synthetic-likelihood MCMC

- `marklab bayes synthetic-likelihood-growth-front` repeatedly calls IC-0108 and adds declared Gaussian noise to final mass/maximum density. Each likelihood estimate retains mean, unbiased covariance, off-diagonal shrinkage, determinant, Gaussian log density, and Monte Carlo mean SE. A uniform-prior Gaussian random-walk chain retains its noisy likelihood state on rejection and reports complete trace/burn-in/acceptance/posterior plus aggregate work. The result is approximate synthetic evidence only.

## IC-0120 — Prior-predictive rejection-ABC SBC

- `marklab bayes growth-front-rejection-abc-sbc` draws truth from the IC-0117 uniform prior, calls IC-0108 for observed final mass, and reruns IC-0117 under a replicate seed. Output retains every truth/observation/rank/equal-tail interval/coverage/proposal/status row, `N+1` rank histogram, normalized-rank ECDF diagnostic, aggregate coverage/failures, and worst-case cell work. It is implementation calibration on a synthetic control only.

## IC-0121 — Reference-fit KNN simulation OOD with conformal calibration

- `marklab bayes simulation-ood` requires disjoint identified reference/calibration simulations and observation over exact ordered features. Reference-only z-scoring precedes exact mean-kNN scores; calibration only freezes a nearest-rank threshold and conformal p-value. Strict threshold exceedance is out of support. Complete fit/calibration/score/work artifacts are retained; support is not model validity.

## IC-0122 — Growth-front posterior-predictive mass/maximum laboratory

- `marklab bayes growth-front-posterior-predictive-lab` selects declared posterior growth draws under named replay and calls IC-0108 for each bounded replicate. It retains every success/failure and, for final mass/maximum density, observed/replicate means, equal-tail interval/membership, finite-sample tails/two-sided p-value, and interval-or-alpha flags. Two synthetic summaries do not establish general model validity.

## IC-0123 — Bounded linear-Gaussian filter and RTS smoother

- `marklab longitudinal kalman-smooth` accepts one finite, dimension-checked transition/process/observation/noise system per time step, with componentwise `null` observations and an explicit matrix-work ceiling. It uses Cholesky solves, Gaussian innovation likelihoods, Joseph filtered covariances, and RTS backward recursion, retaining every predicted/filtered/smoothed state plus observed-component accounting. Positive-semidefinite process noise is allowed; singular required solves fail without hidden regularization. The result is linear-Gaussian numerical inference only.

## IC-0124 — Scalar quadratic EKF/UKF

- `marklab longitudinal nonlinear-filter` applies either analytic-derivative EKF or three-point UKF to one declared scalar quadratic transition/observation pair per time step. It retains all predicted/filtered moments, innovations, variances, gains, derivatives or sigma spreads, missing-update status, likelihood, and approximation ceiling. Process variance may be zero; every required variance and UKF scaling is checked. Both methods reduce to IC-0123's scalar linear oracle when quadratic coefficients are zero.

## IC-0125 — Scalar bootstrap particle filter and ancestry smoother

- `marklab longitudinal particle-smooth` propagates a bounded seeded scalar Gaussian particle law through time-varying quadratic transitions, accumulates prior plus Gaussian observation log weights, reports likelihood increments and ESS, and systematically resamples below a declared ESS fraction. Every post-step particle, normalized weight, ancestor, moment, and decision is retained. Terminal weighted draws trace exact stored ancestry into smoothed trajectories. Zero transition/initial variance is allowed for deterministic controls; particle-step work is rejected before execution.

## IC-0126 — Physical cuboid homogeneous 3-D K/L

- `marklab spatial3d k-function` requires exact three-column points, declared physical units/positive voxel spacing, an axis-aligned positive cuboid, optional SPD anisotropy, increasing micrometre radii, and bounded pair work. It normalizes all geometry to micrometres and computes `L3=(3K/(4π))^(1/3)` under distinct none, border-reference, or translation-overlap correction. The result retains compiled window measures, normalized points, metric/correction identity, curve denominators/status, and exact pair/work evidence. It is a descriptive homogeneous cuboid specialization only.

## IC-0127 — Supplied-intensity inhomogeneous and directed cross 3-D K/g

- `marklab spatial3d inhomogeneous-k` and `cross-k` reuse IC-0126's physical cuboid and anisotropic distance boundary and require a positive finite per-point intensity in `um^-3`. Inhomogeneous K accumulates ordered same-process inverse-intensity contributions; cross-K accumulates directed A-to-B contributions and cross-g is the successive 3-D shell increment. Full-volume versus border-eroded normalization and translation overlap remain explicit. A zero-volume first shell returns unavailable cross-g; all other non-finite states fail.

## IC-0128 — Sparse uncertainty-aware physical 3-D graph

- `marklab spatial3d spatial-graph` builds either a physical-radius graph or deterministic undirected union-kNN graph after IC-0126 normalization. Per-point radial uncertainty enters only through an explicit nominal, possible/lower-bound, or guaranteed/upper-bound distance. Binary or Gaussian weights, canonical edge order, symmetric CSR, exact pair/edge limits, and complete normalized graph settings are retained. SHA-256 binds normalized window/spacing/metric/points, rule parameters, uncertainty basis, weight parameters, and canonical edges independently of input row order.

## IC-0129 — Block-restricted phylogenetic–spatial association

- `marklab longitudinal phylogenetic-spatial-association` validates one imported connected acyclic positive weighted tree and uniquely mapped finite 3-D clone centroids. Only within exact patient/specimen blocks, it correlates tree path and Euclidean spatial distances. A named ChaCha20 stream permutes clone-to-tree-node assignments independently within blocks; output retains all pair rows, all null statistics, inclusive plus-one two-sided p-value, provenance, and exact work. Interpretation is always cross-sectional and noncausal.

## IC-0130 — Exact complete-randomization binary interference

- `marklab causal randomized-interference` validates unique eligible units/clusters, treatment-before-outcome timing, pre-treatment covariates, observed treated counts, and a prespecified undirected graph. It exactly enumerates bounded independent within-cluster complete-randomization states, derives every unit's four joint own/binary-any-neighbour exposure probabilities, and returns HT/Hájek means with explicit positivity/denominator states plus fixed-outcome rerandomization SD. Prespecified direct/spillover/total contrasts and a seeded inclusive plus-one HT randomization test retain complete null/work artifacts. Claims are randomized-design mechanics only.

## IC-0131 — Prespecified spatial exposure mapping catalog

- `marklab causal exposure-mapping` validates unique binary-treated units and a provenance-labelled weighted physical undirected graph, then applies exactly one named mapping: binary any-treated, treated count, weighted treated fraction, Gaussian physical-distance decay, multiscale cumulative treated counts, or a declared continuous field. Parameters, scalar/vector shape, isolate/unavailable status, canonical unit order, and exact directed visits are retained. No outcome or effect enters this artifact.

## IC-0132 — Analytically checked scalar Gaussian EIG

- `marklab causal gaussian-eig` samples outer prior/observation pairs and independent inner prior banks for a scalar linear-Gaussian candidate, computes stable nested-Monte-Carlo information values, and retains every draw plus mean/SE. It also reports the exact `0.5*log(1+sensitivity^2*prior_variance/noise_variance)` oracle, signed/absolute bias, deterministic seed namespace, and bounded `outer*(inner+1)` likelihood evaluations. It is scalar utility-estimator validation only.

## IC-0133 — Binary-confounder bias-function region

- `marklab causal bias-sensitivity` applies `bias=(prevalence_treated-prevalence_control)*outcome_effect` to a canonical bounded scenario grid and retains all parameters, biases, adjusted effects, reversals, extrema, and zero inclusion. Inputs are assumptions, not estimates.

## IC-0134 — Manski bounded-outcome ATE interval

- `marklab causal manski-bounds` validates unique finite binary-treatment observations inside known outcome support and computes exact assumption-light intervals for `E[Y(1)]`, `E[Y(0)]`, and ATE. Both observed groups are required; observed difference, treatment fraction, endpoint formulas, zero inclusion, and absence of sampling uncertainty are explicit.

## IC-0135 — Matched-pair Rosenbaum sign sensitivity

- `marklab causal rosenbaum-sign-sensitivity` validates unique one-treated/one-control pairs and an increasing finite `Gamma>=1` grid. Ties are excluded/reported; exact stable binomial upper-tail bounds use success probabilities `1/(1+Gamma)` and `Gamma/(1+Gamma)`. The curve, alpha decisions, critical Gamma, pair differences, and bounded work are complete.

## IC-0136 — Canonical stable numerical primitives

- `marklab numerics stable-primitives` evaluates finite bounded max-shift/Neumaier log-sum-exp and log-mean-exp, normalized Neumaier weighted mean, and two-pass symmetric f64 covariance. Covariance is unweighted sample or caller-weighted with denominator `1-sum(normalized_weight^2)` and explicit effective sample size; insufficient effective size is a typed failure. Exact element/cross-product work and algorithm identities are retained.

## IC-0137 — Monotone result-maturity decision

- `marklab policy determine-maturity` consumes closed declared maturity/mode and explicit provenance, convergence, approximation, predictive, causal, and Bayesian requirement states. It never upgrades; emits every applicable reason once in fixed order; applies terminal unsupported-for-claim failures before/alongside softer research/experimental caps; and retains declared/final maturity plus downgrade status.

## IC-0138 — Evidence-complete execution-mode selection

- `marklab policy select-mode` validates ordered unique mode/backend descriptors and checked resource estimates, assesses backend/memory/runtime/accuracy/approval for every candidate, and returns typed selected/unsupported/backend/resource/accuracy/approximation states. Explicit requests never fall back. Automatic selection follows preference among permitted modes and reports the first feasible unapproved approximation when approval is the only remaining barrier.

## IC-0139 — Contiguous real-data validation ladder

- `marklab policy validation-ladder` requires exactly ordered stages 0–5, evidence for every completed stage, and no resumed completion after a gap. It reports the highest contiguous completed stage/label, all evidence/risk rows, and unique promotion-blocking risks beginning at the first incomplete stage. It records claimed evidence state but does not verify referenced artifacts.

## IC-0140 — Exact axis-aligned anisotropic 3-D Matérn GP

- `marklab bayes anisotropic-gp-3d` requires unique finite three-dimensional micrometre observations with variation on every axis and explicit prediction coordinates. PyMC 6.3.0 NUTS infers mean, amplitude, three positive axis length scales, and noise under an exact dense Matérn-3/2 covariance. Output retains strict backend/request identity, posterior and conditional predictions, posterior-predictive summaries, the diagonal metric evaluated at posterior mean length scales, complete diagnostics, and bounded cubic work. The result is experimental interpolation, not a rotated anisotropy, tissue-window, deformation, or biological claim.

## IC-0141 — Subject-block Freedman–Lane specialization

- `marklab cohort repeated-freedman-lane` fits subject fixed effects with and without one target, sign-flips complete reduced residual blocks by independent subject, and retains the OLS coefficient/SE/t statistic, inclusive two-sided null, dimensions, seed, and explicit residual-exchangeability limitation.

## IC-0142 — Simultaneous functional equivalence bootstrap

- `marklab cohort functional-equivalence` resamples complete aligned patient difference curves and applies the `1-alpha` nearest-rank maximum absolute deviation to every scale. The complete common axis, prespecified margin curve/rationale, band, failing scales, seed, and replicate counts are retained.

## IC-0143 — Patient-first percentile bootstrap equivalence

- `marklab cohort bootstrap-equivalence` consumes IC-0038 patient/specimen resampling and declares equivalence only when its two-sided nearest-rank percentile interval is strictly inside prespecified margins. Interval method and experimental coverage limitation remain explicit.

## IC-0144 — Fixed and REML random-effects multisite pooling

- `marklab cohort multisite-inference` validates site patient-level effect summaries, uses inverse-variance fixed or scalar intercept-only REML random-effects pooling, and retains confidence/prediction intervals, Q/chi-square interaction evidence, every leave-one-site-out refit, and patient/site counts. It does not reconstruct a one-stage patient model.

## IC-0145 — Canonical bounded graph Fourier workflow

- `marklab graph spectral` canonicalizes exact-ID physical-radius binary graphs without isolates, builds the combinatorial Laplacian, performs deterministic bounded symmetric eigendecomposition, and projects one finite node signal. It retains graph digest/content, dense operator, canonical modes/coefficients, reconstruction error, declared band energies/fractions, and exact pair/rotation work under an experimental graph-signal ceiling.

## IC-0146 — Exact graph heat kernel, apply, signature, and distance

- `marklab graph heat` reuses IC-0145 and evaluates exact dense heat kernels for increasing nonnegative times, their declared-signal products, diagonals, and uniform-node-L2 distances between declared kernel rows. All matrices/vectors and bounded cubic work are retained; no approximate or physically calibrated diffusion claim is made.

## IC-0147 — Exact spectral graph wavelets and energies

- `marklab graph wavelet` reuses IC-0145 coefficients with fixed `x exp(-x)` band-pass and `exp(-x)` low-pass responses at declared positive scales. Every node coefficient and energy is retained under an experimental small-graph ceiling.

## IC-0148 — Restricted graph-spectrum ERL null

- `marklab graph spectrum-null` binds exact node strata to IC-0145, permutes complete signal rows only within those strata, recomputes every declared band energy, and applies the shared average-tie ERL envelope. It retains null curves, simultaneous bounds/depths/global p, scalar low-frequency p, seed, and bounded projection work.

## IC-0149 — Adaptive Chebyshev exact-heat comparator

- `marklab graph chebyshev-heat` scales the IC-0145 Laplacian to `[-1,1]`, selects deterministic heat-filter order from reference-tail and dense-grid evidence, applies the generic three-term recurrence, and rejects any result exceeding tolerance against IC-0146 exact signal application. Coefficients, order, interval, both error estimates, exact/approximate signals, and matrix-vector work are retained.

## IC-0150 — Exact-spectrum diffusion-wavelet transform

- `marklab graph diffusion-wavelet` reuses IC-0145, forms the symmetric lazy operator `I-L/lambda_max`, retains exact modes at dyadic powers above a positive tolerance, and emits every scaling/detail basis, compressed diagonal, rank, approximation value, coefficient, and reconstruction error. The result is bounded dense multiresolution mathematics, not a sparse/localized or physically calibrated basis.

## IC-0151 — Fixed graph scattering with declared stability cases

- `marklab graph scattering` reuses IC-0145 with fixed `x exp(-x)` wavelets, strictly increasing positive scale paths, modulus, and arithmetic node pooling through order two. Every feature and every declared finite signal-perturbation magnitude, feature delta, ratio, tolerance, and pass state is retained. No graph-deformation stability or endpoint claim is inferred.

## IC-0152 — Typed heterogeneous spatial-near messages

- `marklab graph heterogeneous-message` validates exact typed node IDs, common finite feature dimension, one directed source/target relation, physical-radius construction, relation scale, sum/mean aggregation, and bounded pair/edge work. It emits canonical nodes/edges, one updated feature vector per node, and a complete digest under an experimental one-layer ceiling.

## IC-0153 — Normalized hypergraph signal workflow

- `marklab graph hypergraph` validates bounded typed weighted hyperedges and positive finite memberships, canonicalizes incidence, rejects isolated nodes/empty hyperedges, and emits degrees, full normalized incidence Laplacian, Rayleigh numerator/denominator/smoothness, digest, and incidence work. It makes no learned-membership or biological-niche claim.

## IC-0154 — Typed triangle motif count, adjacency, and null

- `marklab graph motif-triangle` validates a bounded simple graph, exact node labels/strata, one three-label multiset, permutations, seed, and triple-work cap. It exhaustively enumerates triangles, emits instances and motif adjacency, and uses complete-label within-stratum permutations with an inclusive plus-one upper p-value.

## IC-0155 — Dimension-two clique Hodge workflow

- `marklab graph hodge` canonicalizes finite edge flows, constructs clique triangles and oriented `B1/B2`, verifies boundary-of-boundary zero, emits lower/upper/full first Hodge matrices, solves gradient/curl/harmonic components with orthogonality and reconstruction gates, and applies `I-step*L1`. Rank-deficient unsupported specializations fail explicitly.

## IC-0156 — Interpreted cellular complex with segmentation perturbations

- `marklab graph cellular-complex` requires a nonempty pathology interpretation, bounded junction/interface/domain incidence, closed signed domain boundaries, and at least one named alternative segmentation. It emits canonical cells, both boundary matrices/digests, `B1 B2` residual, topology/incidence equality, and maximum matched-junction displacement. Claims remain research-only and limited to supplied segmentations.

## IC-0157 — Exact-fixture graph validation ledger

- `marklab graph validate` executes built-in path, incidence, triangle, Hodge, and cellular fixtures spanning every Part VII family plus radius sensitivity. Each algorithm receives a separate evidence/status row; unsupported kNN/kernel/barrier/component, registration, sparse-scale, and GPU dimensions remain explicit limitations. Overall success means only that the exact synthetic fixture set passed.

## IC-0158 — Pinned exact alpha persistence and transforms

- `marklab topology alpha-persistence` validates bounded finite Euclidean points/grids, invokes hash-bound GUDHI 3.13.0 exact alpha construction and persistent cohomology, and emits the validated face-first filtration, boundary columns/residual, finite/essential diagrams, one dimension's landscape and integrated persistence image, and Euler curve. Alpha values remain squared physical radii and claims remain synthetic.

## IC-0159 — Deterministic weak witness persistence

- `marklab topology witness-persistence` selects bounded farthest-point landmarks from canonical point IDs, reports coverage and nu zero, invokes GUDHI 3.13.0 weak Euclidean witness construction, validates the resulting filtration, and emits complete persistence with essential classes separated.

## IC-0160 — Supplied-raster Minkowski morphology

- `marklab topology raster-morphology` binds pixel scale, foreground connectivity, Crofton directions, and integer-pixel disk radii to pinned scikit-image 0.26.0/SciPy 1.18.1. Baseline and every dilation/erosion retain area, perimeter, Euler, normalized perimeter availability, and conventions under an experimental supplied-mask ceiling.

## IC-0161 — Bounded finite connectivity curves

- `marklab topology connectivity` validates finite points inside a declared rectangular window, evaluates bounded pair distances, and emits union-find component count, largest fraction, one-largest-excluded susceptibility, two spanning states, critical radius, and exact pair/edge work for every increasing radius.

## IC-0162 — Whole-patient persistence-distribution comparison

- `marklab topology compare-persistence` requires one finite diagram per unique patient, exact A/B groups and strata, GUDHI 3.13.0 bottleneck distance, and a bounded exact assignment space preserving every stratum's A count. It emits the full distance matrix/null, biased distance-energy statistic, and exact inclusive upper p-value under an experimental cohort-method ceiling.

## IC-0163 — Declared raster topology stability laboratory

- `marklab topology stability` validates a bounded supplied binary mask, physical pixel scale, unique candidate pixels, integer-pixel scales, seeded repetitions, and toggle count. Pinned GUDHI/scikit-image/SciPy recompute cubical diagram bottleneck, landscape L2, Euler-curve Linf, maximum Minkowski relative error, and connectivity-radius shift for every retained perturbation.

## IC-0164 — Exact-fixture topology validation ledger

- `marklab topology validate` runs nine built-in analytic/hand controls spanning Part VIII and emits one status/evidence row per control. The separate sparse-memory row remains `not_verified`; exact-fixture status cannot be interpreted as representative scale or real pathology validation.

## IC-0165 — Paired measured Gaussian pCCA EM

- `marklab multimodal pcca` validates exact patient joins, distinct measured Gaussian modality metadata, unique feature names, complete paired values, and train/test isolation. Pinned NumPy/SciPy EM emits standardization, loadings/noise, posterior scores, canonical correlations, likelihood/convergence evidence, and held-out Y-from-X RMSE under a synthetic experimental ceiling.

## IC-0166 — Pinned Bayesian pCCA with draw alignment

- `marklab multimodal bayesian-pcca` uses the IC-0165 design boundary and fixed CCA-Zoo priors, one factor, bounded warmup/draws, target acceptance, and seed. It retains aligned loading/noise/latent summaries, divergences, ESS, explicit single-chain R-hat unavailability, cross-view correlation, held-out RMSE, and a nonconverged claim downgrade.

## IC-0167 — MOFA multiview factors and masked-view prediction

- `marklab multimodal mofa` validates two-to-eight measured patient Gaussian views, feature-level observed masks, structural/MAR assumptions, train/test boundaries, and at least one observed anchor plus masked held-out target. Pinned mofapy2 VI retains ELBO, ARD factor activity, aligned scores/loadings, per-view variance explained, every missing prediction, and evaluation RMSE.

## IC-0168 — MOFA Gaussian matrix factors

- `marklab multimodal matrix-factor` validates exact patient/feature identities, observed-only training standardization, at least eight observed entries per feature, retained masked targets, and bounded factors/iterations. Pinned mofapy2 0.7.4 emits aligned factor/loading first and second moments, ARD activity, variance explained, finite ELBO diagnostics, and masked predictive uncertainty/RMSE.

## IC-0169 — Exact nested hierarchical factor graph

- `marklab multimodal hierarchical-factor` validates exact patient→specimen→region→cell ownership and level-matched modality attachments. It emits every latent node, conditional Gaussian edge, measured/predicted observation attachment, and patient replication declaration; it compiles rather than fits the graph.

## IC-0170 — Graph-Laplacian spatial matrix factors

- `marklab multimodal spatial-matrix-factor` requires a bounded region Gaussian matrix, complete canonical positive graph edges covering every region, held-out masks, positive prior/noise controls, and at most 512 latent parameters. Pinned SciPy 1.18.1 emits aligned MAP factors, masked Laplace predictions, optimization diagnostics, the exact operator convention, and `approximate_only` state.

## IC-0171 — Three-mode CP/Tucker Laplace factors

- `marklab multimodal tensor-factor` validates a bounded complete-index three-mode tensor with an explicit observed mask and legal CP or Tucker ranks. Pinned JAX 0.11.1/SciPy 1.18.1 fits only observed values and emits aligned factors/core or components, masked inverse-Hessian predictive uncertainty, objective diagnostics, and `approximate_only` state.

## IC-0172 — Exact Matérn spatial latent factor

- `marklab multimodal spatial-latent-factor` validates bounded region Gaussian views, unique 2-D physical coordinates/frame, observed-only standardization, held-out masks, one Matérn-3/2 factor, and bounded two-chain NUTS controls. Pinned PyMC 6.3.0 emits aligned field/loading draws, range posterior, prior predictive scale, masked predictions, R-hat/ESS/divergences, and a diagnostic claim downgrade.

## IC-0173 — Supplied-basis multiresolution factors

- `marklab multimodal multiresolution-factor` binds each canonical matrix row to two-to-eight named physical-scale basis sets, positive shrinkage, bounded factors, and masked evaluation values. Pinned JAX/SciPy jointly fits all scales and emits aligned basis/loadings, variance contribution/fraction by scale, held-out inverse-Hessian predictions, and `approximate_only` state.

## IC-0174 — Anchor-preserving modality-dropout training

- `marklab multimodal dropout-robust` validates measured patient Gaussian views, train/test isolation, normalized named retained-modality patterns, and anchor preservation. Pinned JAX/SciPy minimizes weighted reconstruction plus full/partial consistency and emits every held-out pattern's missing-view RMSE, latent consistency, and predictions.

## IC-0175 — Modular joint pathology compile-and-fit

- `marklab multimodal joint-pathology` validates complete patient→region ownership and measured Gaussian morphology/IHC, Poisson omics, Bernoulli clone, and Gaussian patient-clinical blocks. It emits a typed ModelIR and fits the same graph through bounded JAX/SciPy Laplace inference, retaining aligned latent maps, likelihood checks, held-out outcome uncertainty, diagnostics, and frontier claim limits.

## IC-0176 — Canonical nested M0–M5 model comparison

- `marklab multimodal compare-models` reuses IC-0091's training-fold preprocessing/tuning and outer patient-held-out predictions through pinned SciPy. It emits all six models, calibration/task metrics, and the prescribed six paired patient permutation increments under a noncausal synthetic comparison ceiling.

## IC-0177 — Executable multimodal validation ledger

- `marklab multimodal validate` executes eleven independent analytic/simulated controls and retains their metrics/thresholds. SPDE recovery, same-model HMC/VI/Laplace agreement, and admitted real-patient validation remain explicit blocked/not-verified rows, so overall status is partial rather than falsely complete.

## IC-0178 — Pinned multiresolution B-spline registration

- `marklab registration nonrigid` validates bounded physical images, frames, masks, same-stain metric, mesh, and coarse-to-fine plan. Pinned SimpleITK 2.5.5 emits the B-spline transform/displacement, warped image, objective/levels/stop state, masked before/after error, and Jacobian diagnostics.

## IC-0179 — Stationary-velocity diffeomorphism

- `marklab registration svf` validates bounded same-grid images, regularization, scaling/squaring depth, and Jacobian tolerance. Pinned JAX/SciPy optimizes the dense velocity, composes displacement fields, emits `exp(v)`/`exp(-v)`, warped image, Jacobian map summaries, inverse consistency, and objective diagnostics.

## IC-0180 — Landmark Hamiltonian LDDMM

- `marklab registration lddmm-landmarks` validates paired unique physical landmarks, Gaussian kernel scale, data weight, and integration/optimization bounds. Pinned JAX/SciPy emits optimized initial momentum, every RK4 position/momentum state, kinetic energy/drift, and landmark residuals.

## IC-0181 — Variational translation-SVF posterior

- `marklab registration probabilistic-svf` validates same-grid images and a constant-translation SVF family, then fits a Gaussian posterior through a fixed antithetic ELBO. It emits intervals, seeded transform draws, Jacobian checks, mean warp, and known-deformation calibration under `approximate_only` status.

## IC-0182 — Landmark posterior, propagation, delta, and soft compatibility

- `marklab registration landmark-uncertainty` validates paired landmarks/cells/features and a GP/noise/cost contract. It emits the analytic deformation posterior and draws, Monte Carlo versus delta nonlinear endpoint uncertainty, localization separation, uncertainty-averaged many-to-one target/dustbin probabilities, entropy, and target-unmatched probabilities.

## IC-0183 — Biological-similarity atlas build/map/validation

- `marklab registration atlas` validates frozen measured region representations and patient-replicated domain labels. It emits a hash-versioned prototype/covariance atlas, probabilistic query mappings with entropy/OOD-unmatched mass, leave-one-patient calibration/recovery, and declared perturbation results while denying a physical-registration claim.

## IC-0184 — Neural marked Cox process

- `marklab neural point-process` validates independent patient patterns, exact windows/contexts/marks, a bounded tanh-softplus/softmax architecture, and training/reference quadrature. Pinned JAX/SciPy emits serialized weights, held-out joint likelihood versus homogeneous baseline, mark accuracy, quadrature bias, probes, and optimization diagnostics.

## IC-0185 — Equivariant point-set flow and score diffusion

- `marklab neural point-set-generators` validates one exact window/context contract and independent patient splits. It emits the conditional iid-equivariant logistic-normal flow/count likelihood, exact permutation error, VP marginal score loss, reverse probability-flow samples, support/cardinality/diversity evidence, and seeded model state.

## IC-0186 — Pinned amortized and sequential sbi

- `marklab neural sbi` validates a bounded uniform/Gaussian-location oracle and pinned MDN/MLP training plan. sbi 0.26.1/Torch 2.13.0 train NPE/NLE/NRE separately, serialize each state, compare their normalized posterior moments to the analytic truncated Gaussian, and execute two isolated NPE proposal rounds.

## IC-0187 — Consumed generative tissue model card

- `marklab neural validate-generative` requires original train/held-out data and two repeated point-set artifacts. It executes supported reliability/baseline/privacy checks, records explicit unsupported endpoint families and missing real data, and returns only research-only synthetic maturity.

## IC-0188 — Bayesian landmark serial stack

- `marklab spatial3d serial-stack` validates ordered unique sections/z values, paired landmarks, reference identity, landmark noise, and posterior draws. It emits adjacent/composed transforms, Gaussian section posteriors/draws, reconstructed 3-D landmarks, landmark/cycle/gap diagnostics, and transform-only downstream centroid uncertainty.

## IC-0189 — Exact 3-D alpha complex

- `marklab spatial3d alpha-complex` validates bounded unique physical 3-D points/frame and squared-alpha/homology bounds. Pinned GUDHI emits the complete vertex-to-tetrahedron filtration, simplex counts, tetrahedra, persistence, and F2 boundary-squared proof.

## IC-0190 — Deformation versus biology separation

- `marklab spatial3d deformation-biology` validates paired fields, a supplied transform posterior, prespecified domain, deformation-only control, and independent measurement. Pinned SciPy recomputes baseline interpolation/change for every draw and emits effect intervals, negative-control result, and independent correlation.

## IC-0191 — Uncertain clone models and advanced validation

- `marklab spatial3d clone-models` validates one rooted imported tree, SPD location uncertainty, normalized clone probabilities, and replicated patient features. It emits branch-diffusion posterior draws and patient-centered mixture niche contrasts/LOPO sensitivity. `marklab spatial3d validate-advanced` executes the cross-family synthetic ledger and retains the missing-real-cohort row.

## IC-0192 — Cluster-cross-fitted observational estimator laboratory

- `marklab causal observational` validates bounded unique rows, independent cluster folds, predeclared finite baseline covariates, binary treatment, dose, neighbour exposure, outcome, negative-control outcome, and an overlap clip. Pinned SciPy/NumPy emit propensity bounds, dose response, AIPW/exposure-AIPW, two-exposure DML, negative-control regression, and every leakage-free fold under a synthetic/no-real-identification claim.

## IC-0193 — Randomized perturbation and research-only mediation

- `marklab causal perturbation` requires cluster-constant randomized treatment, treatment-before-mediator-before-outcome ordering, prespecified neighbour exposure, a declared mediator-outcome assumption, negative control, and independent cluster replication. It emits controlled direct/spillover estimates and an assumption-sensitive path decomposition whose mediation status is always research-only.

## IC-0194 — Sequential and constrained active-design simulation

- `marklab causal active-design` validates unique ROI/stain/landmark actions with finite information, quality, redundancy, robustness, coverage, observation, and positive cost; separate budgets; a positive scalar Gaussian posterior; replicate options; and power designs. It emits the acquisition/posterior history, family selections, allocation frontier, and seeded Wilson power surfaces without operational validation.

## IC-0195 — Partial causal/active validation ledger

- `marklab causal validate-active` executes ten bounded synthetic controls spanning randomized recovery, exposure probabilities, DR/DML, positivity, negative controls, analytic EIG, active ranking, biological replication, and power. It always retains the absent external prospective laboratory row and therefore cannot return fully validated status.

## IC-0196 — Fixed-step conjugate-normal HMC

- `marklab bayes hmc-normal` validates bounded finite Gaussian observations, known likelihood/prior scales, positive scalar mass/step, leapfrog/warmup/draw limits, and a seed. Pinned NumPy/SciPy execute explicit leapfrog Metropolis transitions and emit all draws, energy diagnostics, acceptance, identity-transform Jacobian, and the analytic posterior comparison.

## IC-0197 — Advanced synthetic cluster and finite Gibbs inference

- `marklab bayes advanced-cluster` validates independent rectangular Thomas patterns plus a bounded binary finite-site graph. It emits label-invariant parent-count/scale results, exact-state auxiliary exchange posterior and diagnostics, the consumed symmetric multitype fit, and partially pooled replicated log-parameters under a synthetic experimental ceiling.

## IC-0198 — Unified durable typed-node execution

- `marklab_workflow::execute_algorithm` accepts one existing typed dependency-free `WorkflowNode`, validated graph, scheduler, in-memory and durable projects, output schema, and native runtime provenance. It derives the sole cache identity, restores only verified exact durable bytes, invokes the normal scheduler, and durably publishes only canonical misses. `marklab project classical` is the immediate caller and preserves result-format 0.3.

## IC-0199 — Runtime diagnostics, calibration, reduction, and smoke scaling

- `marklab policy runtime-validation` consumes an IC-0196 HMC artifact and bounded Gaussian calibration/scaling specifications. Fixed contiguous scoped-thread reduction preserves global-item seed identity and partition-index reduction. The result emits applicable HMC checks, explicit nonapplicable families, bias/RMSE/coverage/rejection Wilson metrics, equivalent-work checksums, phase medians, and the self-persistence timing limitation.

## IC-0200 — Rectangular finite-element SPDE suite

- `marklab bayes spde-suite` validates a hole-free rectangle, two or more bounded regular resolutions, alpha two, fixed positive kappa/tau, event points, and region Gaussian observations. Pinned SciPy emits vertices/triangles, consistent mass/stiffness, positive Matérn precision, barycentric projections, triangle quadrature, fixed-hyperparameter LGCP MAP/intensity, one-factor spatial MAP/reconstruction, and mesh sensitivity under a synthetic experimental ceiling.

## IC-0201 — Concrete multiplex panel-to-patient application

- Input: strict `marklab.multiplex_study_recipe` v1 with named nullable assay channels, globally
  unique canonical Cell IDs, declared patients/groups, physical XY micrometre frames, exact
  polygon windows and prespecified channel/radius/weight/missingness/reduction/randomization limits.
- MarkTable admits several channels of the same kind without a dummy binary Pattern. Continuous
  values are finite signed f64; binary and categorical observations retain explicit units/codebooks.
  Compatibility constructors, public legacy enums, Pattern projection and v1 legacy identity bytes
  remain; assay-bearing tables have a separate v2 logical identity.
- Each selected quantitative/binary channel uses its observed-cell subgraph. Existing radius
  Moran/Geary arithmetic shares that graph; unavailable data or arithmetic stays explicit. Equal
  slide means produce one vector per patient; the canonical whole-patient Max-T implementation
  compares the complete family. Required unavailable slides prevent inference rather than deletion.
- `analyze_multiplex_study` is the filesystem-free service; `execute_multiplex_study` uses existing
  durable scheduler owners for slide, bounded collection and cohort nodes. Resume restores exact
  outputs without repeating completed statistics; one changed slide invalidates its dependent
  inference. `publish_multiplex_study` validates finite claim state and uses the existing atomic
  output transaction. The CLI and standard-library Python subprocess client call this service.
- Output: a separate experimental study-result v1 plus report/content manifest. Existing result 0.3
  is untouched. Software success does not establish assay provenance, biological calibration,
  clinical/causal validity or whole-slide capacity. AnnData 0.12.4 interchange is bounded in-memory
  H5AD/sparse input with explicit physical coordinates; broader formats/transforms are not inferred.
- Decisions: DEC-0412–0414. Acceptance: `multiplex_panel`, `multiplex_study`,
  `multiplex_study_cli`, real AnnData client tests and the bounded study fuzz target.
