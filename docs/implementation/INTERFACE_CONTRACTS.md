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
- Tables: Arrow IPC file and Parquet file manifests declare exact encoding version, row count, ordered columns, and stable scalar primary keys. A declaration alone is not physical-format proof. C-04 adds exact bounded proof only for its embedding table/link profiles; WS-25 remains responsible for general table/interchange conformance.
- Catalog: format `marklab.artifact_catalog` version 1 is strict, bounded, dependency-valid, and canonical to one final-newline JSON fixed point. No prior-version migration is invented.
- Location: catalogs contain portable store-relative keys and optional provenance versions, never absolute paths, endpoints, queries, credentials, or authorization tokens. Managed keys derive only from the artifact ID.
- Local integrity: one capability root confines operations; verification streams a regular file and checks length plus digest. Publication is synchronized, atomic no-replace, and directory-synced; recovery exclusively quarantines only recognized regular staging entries and reports partial states.
- Workflow: catalog-aware nodes must bind a store and verify every semantic input before cache lookup. Integrity failure executes nothing and never degrades to a miss. Nodes with no semantic inputs retain the exact B-04 cache-key path.
- Compatibility and limits: result 0.3, config 0.2, `OutputWriter`, current CSV/Parquet IO, and scientific methods remain unchanged. C-04's embedding-specific physical validation composes with this substrate; mutable project heads/ledgers, general physical/interchange validation, C-01/C-02 codecs, live cloud, and Windows runtime durability evidence remain separately gated.

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
- Provenance DAG: version one admits direct patch extraction plus deterministic region/slide derivation only. Strict canonical source chain, normalization, four support variants, weighted/arithmetic derivation, cell-link producer, exhaustive assessment, and all four multiscale provenance values/codecs exist. Direct-patch structural validation now checks eighteen exact record roles, nested dependencies, canonical JSON bytes, cross-value bindings, structural footprint/overlap manifests, and each required managed replica in the supplied store before returning a runtime-only graph token. That token is not a support/table receipt and does not decode physical bytes. Derived/cell/link graph validators and all separate physical receipts remain pending. Opaque source-vector or coordinate availability never proves component or anchor correspondence without a later admitted adapter receipt.
- Physical boundary: eight exact schema families—three matrices, footprints, overlap edges, cell assignments, cell edges, and patch-region links—each require narrow bounded Arrow IPC and Parquet proof using the locked C-04 raw-before-stock, deterministic-writer, integrity, and charged batch/row-group policy. Runtime receipts are unforgeable and nonserializable.
- Corpus and claims: authorized patch candidates remain aggregate inventory only; no HDF5/NPY/CSV/JSON production adapter, canonical real table, region/slide profile, observation window, embedding statistic, prediction, or biological result is admitted. Real promotion requires exact identity, context/support, adapter correspondence, complete provenance, reviewed snapshot, and license evidence.
