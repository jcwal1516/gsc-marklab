# Repository map

This is the implementation-base map at `55fce12f10684a9081ca1f744f87d6f5feedcb24`. A-03 will add exact public symbols, callers, and migration dispositions.

## Root and process

| Path | Responsibility | Direction/constraints | WS-A disposition |
|---|---|---|---|
| `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml` | Single-package Rust build, features, pinned Rust 1.96.0 | No dependency changes in WS-A | Preserve; inspect for WS-B |
| `.github/workflows/**` | CI, calibration, benchmarks, release/WSI checks | Canonical gates; do not weaken | Reproduce locally where possible |
| `README.md`, `SPEC.md`, `docs/*.md` | Product, public API, result 0.3, validation | Claims require ledger audit | Preserve/characterize |
| `benches/**`, `fuzz/**`, `tests/**` | Performance, fuzz-build, integration and contracts | Baseline evidence | Run/record |

## Current crate layers

| Area | Representative owners | Responsibility | Dependency note | Migration intent |
|---|---|---|---|---|
| Public facade | `src/lib.rs`, `src/api.rs`, `src/errors.rs` | Library surface and marked analysis engine | Must not absorb new domains | Preserve through compatibility shell |
| Configuration | `src/config/**` | Strict TOML model/defaults/validation | One canonical parser/validator | Characterize and adapt once |
| Data/input | `src/data/**`, `src/io/**` | Patterns, CSV/Parquet/GeoJSON/intermediates | Finite validation and feature gates | Preserve; later adapt to typed data |
| Geometry | `src/geom/**` | Masks, components, hulls, scales, exact R-tree index | Reusable scientific substrate | Preserve/characterize before extraction |
| Inference | `src/inference/**`, `src/permutation/**` | P-values, BH, deterministic label permutations, ERL | One canonical formula per policy | Preserve and later centralize |
| Marked science | `src/spectra/**`, `src/periodogram/**`, `src/multiscale_residual/**` | Structure factor, mark-pair covariance, anisotropy, residual diagnostics | Names are narrow; not K/g/wavelets | Preserve semantics in compatibility layer |
| Multimodal science | `src/multimodal/**`, `src/neighborhood/**`, `src/registration/**` | H&E/IHC fusion, graph/cross curves, territories, rigid/affine transforms | No same-cell/correspondence claim | Preserve and characterize |
| Comparison | `src/prepost/**`, `src/comparison/**` | Descriptive pre/post and margins | Not cohort inference/equivalence | Preserve exact wording |
| Results/output | `src/output/**` | Result 0.3, availability, manifests, artifacts, transactions | Strict finite/transaction contract | Preserve reader/writer parity |
| CLI | `src/cli.rs`, `src/cli/**`, `src/bin/marklab.rs` | analyze/batch/prepost/multimodal/simulate/smoke/slide | No scientific formulas | Preserve as compatibility commands |
| Validation/performance | `src/synthetic_smoke/**`, `src/perf/**`, `src/algorithm_tests.rs` | Generators, calibration smoke, counters/contracts | Evidence, not production claims | Preserve and expand later |

## Dependency direction at implementation base

```text
CLI -> public facade/application engines -> scientific stages
    -> data/geometry/inference primitives -> output/artifacts
```

B-03 finalized the first workspace direction as `marklab` → `marklab-workflow` → `marklab-project`. B-04 keeps the generic DAG/scheduler below the compatibility package and places the concrete existing-engine node adapter in root `marklab`; lower packages never depend upward on the facade.

C-01 extends the descending workspace layers without changing the root facade:

```text
marklab (compatibility facade and current engine adapter)
  -> marklab-workflow (typed DAG and local scheduler)
    -> marklab-project (project state, references, cache, hierarchy slot)
      -> marklab-data (validated cohort hierarchy and design facts)
        -> marklab-core (opaque typed identities)
```

`marklab-data` is a root dev-dependency only for integration tests and the hierarchy benchmark; C-01 adds no stable root re-export. The two new packages use only already-locked `thiserror` 2.

## Current C-05 implementation ownership

| Path | Current responsibility | Checkpoint boundary |
|---|---|---|
| `crates/marklab-embeddings/src/multiscale/expected/**` | Canonical expected patch, region, and slide sets with strict bounded JSON and logical digests | Logical only; no artifact record or physical reader/writer |
| `crates/marklab-embeddings/src/multiscale/context/**` | Exact patch extraction geometry, coordinate-frame bindings, receptive field, boundary policy, and bounded canonical JSON | No source adapter or observation-window inference |
| `crates/marklab-embeddings/src/multiscale/footprint.rs` | Expected-order patch origins and half-open source-boundary validation | Sampled support only; never a tissue/observation window |
| `crates/marklab-embeddings/src/multiscale/overlap.rs` | Two-pass indexed positive-area edges and minimum-ID connected components over exact footprints | No extraction-grid inference, vector data, or region geometry |
| `crates/marklab-embeddings/src/multiscale/cell_patch/**` | Complete expected-cell assignments, finite anchors, all-containing or declared interpolation edges, exact bindings/digest, and byte/work budgets | Vector-free; no model bytes, inferred interpolation, or full observation-window claim |
| `crates/marklab-embeddings/src/multiscale/patch_region/**` | Exhaustive Cartesian assessment, canonical sparse declarations/link, exact evidence bindings/digests, compositional byte budgets, and strict assessment descriptor validation | Producer-declared area fractions only; no vector bytes, region geometry proof, or promotion; graph/physical proof is separately owned |
| `crates/marklab-embeddings/src/multiscale/records/{codec,binding,normalization,source/**,derivation,support/**,producer/**}` | Shared bounded canonical-record preflight plus strict source chain, normalization, support, deterministic derivation, and cell-link producer values/codecs | No source adapter, artifact graph/receipt, physical profile, or promotion |
| `crates/marklab-embeddings/src/multiscale/records/provenance{,/**}` | Four closed direct/derived provenance values, responsibility-typed model/execution/input evidence, exact canonical parser/serializer, logical identity, and distinct dependency roles | Structural declarations only; no artifact-record admission, graph verification, receipt, opaque-source correspondence proof, or promotion |
| `crates/marklab-embeddings/src/multiscale/records/artifact_graph/**` | Direct-patch eighteen-role validation, vector-independent cell-link and patch-region input graphs, nine-role derived-region graph, and both seven-role derived-slide graphs; role-preserving bindings, exact dependencies, source-bound slide lineage, constant-memory managed canonical comparison, redacted failures, and required lower physical receipts | Runtime prerequisites only; they prove no source-vector/component, opaque-coordinate/anchor correspondence, geometric patch-region overlap, tissue mask, or observation window |
| `crates/marklab-embeddings/src/multiscale/finalization.rs` | Candidate-only deterministic region weighted means with frozen scalar/order semantics and explicit contributor, component, retained, and peak-working limits | Synthetic declared-fraction derivation only; no geometry, source-component, slide, benchmark, or promotion claim |
| `crates/marklab-embeddings/src/multiscale/slide_finalization.rs` | Candidate-only deterministic singleton arithmetic means from verified patch or region tables, with source-bound slide authority and explicit contributor, component, retained, and peak-working limits | Synthetic transitive-support derivation only; no tissue/window, source-component, benchmark, or promotion claim |
| `crates/marklab-embeddings/src/multiscale/{physical.rs,artifact.rs,matrix_artifact.rs}` | Central exact C-05 physical profiles and private graph-bound receipt constructors; format-neutral cell-link/direct-patch/derived support receipts; compact direct-patch, derived-region, and derived-slide table receipts | All eight physical families and all contracted synthetic direct/derived receipt paths are active; no source-component correspondence or promotion |
| `crates/marklab-embeddings/src/columnar/multiscale{,/**}` | Shared C-05 physical errors, write summary, caller-budget enforcement, exact matrix/cell-link/patch-region metadata and formulas, and allocation-free domain checks for all eight families | Exact-profile infrastructure only; no general Arrow/Parquet interchange or matrix-receipt claim |
| `crates/marklab-embeddings/src/columnar/{arrow,parquet}/multiscale/**` | Deterministic writers, exact raw-before-stock preflight/readers, full borrowed/managed validation, and publication for all eight C-05 families; patch, region, and slide matrix readers expose the immediately called candidate-bound borrowed and managed receipt entry points | Exact synthetic physical proof; no broad interchange, source-component correspondence, or source promotion |
| `crates/marklab-embeddings/src/multiscale/table/**` | Sealed matrix core, distinct typed patch/region/slide rows/views/blocks, logical QC/digest scans, and crate-private physical row/binding access implemented only by those wrappers | No materializing columnar read API, public generic matrix, or receipt authority |
| `tests/multiscale_embedding_tables/**` | Behavior, wire/digest golden, brute-force differential, resource-budget, boundary, privacy, and scan-partition regressions | Twelve focused tests cover only the logical/overlap checkpoints |
| `tests/cell_patch_link/**` | Containment/interpolation modes, brute-force parity, exact budgets, CPU-work bound, binding/hierarchy drift, privacy, and digest goldens | Seven focused tests cover only logical cell assignments/edges |
| `tests/patch_region_link/**` | Exhaustive sparse equality, fraction/order/membership, slide/binding drift, digest goldens, aliases, and exact compositional resource boundaries | Nine focused tests plus private 400M-row/product-overflow boundaries cover only the logical declaration/link |
| `tests/multiscale_embedding_records/**` | Strict canonical record/digest/content goldens, four support/provenance and two producer modes, exact dependency roles, pairwise aliases, negative semantics, empty chains, privacy, and exact resource boundaries | Twenty-one focused tests plus private row/structure/raw-string/allocation limits cover only logical record values; no graph or physical proof |
| `tests/{multiscale_embedding_arrow.rs,multiscale_embedding_parquet.rs,multiscale_embedding_differential.rs,support/multiscale_columnar.rs,support/multiscale_matrix_columnar.rs}` | Shared synthetic fixtures plus exact profile/golden, deterministic, hostile declaration, raw/stock, borrowed/managed, multi-group, independent budget-oracle, and one declarative three-table differential | C-05 synthetic physical/differential closure; no real-source promotion or broad interchange claim |
| `tests/multiscale_embedding_artifact_graph/**` | Direct-patch, derived-region, and derived-slide fixtures; exact role/profile/dependency/payload/store/privacy/resource tests; lower support receipts; deterministic region/slide finalization; borrowed/managed Arrow/Parquet patch/region/slide table receipts | Sixty-three focused tests, including eleven slide support/graph/finalizer/receipt cases; no source-component proof, benchmark, real-corpus promotion, or scientific result |
| `tests/{cell_patch_columnar.rs,cell_patch_columnar_adversarial.rs,cell_patch_columnar_resources.rs,support/cell_patch_columnar.rs}` | Both cell-link modes, exact Arrow/Parquet profiles and metadata, deterministic and fragmenting writers, hostile raw declarations, optional-column encoding, independent resource oracles, exact/one-short budgets, and multi-group full decode | Fifteen focused integration tests; functional physical evidence only, with no source-coordinate correspondence, vector binding, benchmark, or promotion |
| `tests/cell_patch_input_artifact_graph.rs` | Dedicated vector-independent nine-role graph, canonical payload/profile/dependency drift, managed availability/integrity precedence, full footprint/assignment/edge decode, separate receipts, and all four format-neutral pair combinations | Seven focused tests; no source-coordinate adapter correspondence, vector binding, derived graph, or promotion |
| `tests/{patch_region_columnar.rs,patch_region_columnar_adversarial.rs,patch_region_columnar_resources.rs,support/patch_region_columnar.rs}` | Exact Arrow/Parquet profile and metadata, deterministic and fragmenting writers, empty/high-bit logical rows, hostile raw declarations, independent resource oracles, exact/one-short budgets, and multi-group full decode | Fourteen focused integration tests; functional declaration persistence only, with no region-geometry proof, benchmark, or promotion |
| `tests/patch_region_input_artifact_graph.rs` | Dedicated six-role graph, every role/profile/dependency/payload drift, managed availability/integrity precedence, full footprint/link decode, and both physical receipt formats | Eight focused integration tests plus private role-alias coverage; no geometric-overlap proof, source-vector correspondence, derived graph, or promotion |
| `tests/{multiscale_matrix_columnar.rs,multiscale_matrix_columnar_adversarial.rs,multiscale_matrix_columnar_resources.rs,support/multiscale_matrix_columnar.rs}` | Three typed matrix families in both formats; exact schemas/metadata/records; six fixed byte identities; in-process/fragmenting/fresh-process determinism; raw hostile declarations; borrowed/managed/integrity parity; independent exact/one-short resource oracles; empty, 8,192/8,193/16,386-row, and one-row 65,536-dimension full decode | Sixteen focused physical tests plus one private Arrow feature guard; all candidate-bound direct/derived receipt authority is tested with the graph suite; source correspondence and promotion remain pending |

The C-05 implementation introduces no dependency or lockfile change and preserves the existing C-04 cell-table APIs and digest goldens.

## Active ownership

| Task/agent | Writable tracked files | Canonical symbols | Read-only scope | Non-goals |
|---|---|---|---|---|
| A-01 `/root` | `AGENTS.md`, `docs/implementation/**` | Registry/docs only | Entire repository | No production behavior |
| A-02 `a02_baseline` | None | None | Entire repository/CI | No edits, dependency installs, commits |
| SLIDE-INV `remote_slide_inventory` | None in repository | None | Authorized remote slide corpus | No remote mutations; no repo edits |
| A-03 `/root` | Implementation docs only | Registry/docs only | Entire repository | No source moves or API changes |
| C-05 patch-region `/root` | Shared patch-region columnar/domain profile, artifact graph/receipt, exports, tests, and ledgers | Shared metadata/dependencies/domain validation; `VerifiedPatchRegionInputArtifactGraph`; `VerifiedPatchRegionLinkArtifact` | Existing Arrow/Parquet implementations and all C-04 behavior | No source adapter, geometric-overlap proof, matrix profile, derived table, or promotion |
| C-05 patch-region `c04_embedding_audit` | New `crates/marklab-embeddings/src/columnar/arrow/multiscale/patch_region{.rs,/**}` only | Patch-region Arrow write/preflight/read/validate/publish functions and `PatchRegionArrowPreflight` | Shared profile and existing Arrow code | No exports, shared files, graph/receipt, tests, docs, or commit |
| C-05 patch-region `c04_feasibility_audit` | New `crates/marklab-embeddings/src/columnar/parquet/multiscale/patch_region{.rs,/**}` only | Patch-region Parquet write/preflight/read/validate/publish functions and `PatchRegionParquetPreflight` | Shared profile and existing Parquet code | No exports, shared files, graph/receipt, tests, docs, or commit |
| C-05 matrix physical `/root` | Shared sealed matrix physical access/profile, exact manifests/metadata/dependencies, exports, behavior/adversarial/resource tests, and ledgers | `MultiscaleMatrixArrowPreflight`; `MultiscaleMatrixParquetPreflight`; typed patch/region/slide public facade | Existing C-04 table codec and agent-owned format subtrees | No C-04 behavior change, matrix receipt, derived graph/finalization, source adapter, or promotion |
| C-05 matrix physical `c04_embedding_audit` | New `crates/marklab-embeddings/src/columnar/arrow/multiscale/matrix{.rs,/**}` only | Shared implementation behind typed patch/region/slide Arrow write/preflight/validate/publish functions | Frozen shared matrix profile and existing Arrow primitives | No exports, shared/table files, receipts/graphs, tests, docs, or commit |
| C-05 matrix physical `c04_feasibility_audit` | New `crates/marklab-embeddings/src/columnar/parquet/multiscale/matrix{.rs,/**}` only | Shared implementation behind typed patch/region/slide Parquet write/preflight/validate/publish functions | Frozen shared matrix profile and existing Parquet primitives | No exports, shared/table files, receipts/graphs, tests, docs, or commit |
| C-05 derived slide `/root` | Slide finalizer, columnar/root exports, cross-slide regression/fix, ledgers, final gates, and local commit | `DerivedSlideEmbeddingTableCandidate`; both finalizers; fixed source slide binding; four candidate-bound slide verifier entry points | Existing exact matrix formats and agent-owned support/graph/tests | No new format, generic lower-table abstraction, adapter, benchmark, corpus promotion, or scientific result |
| C-05 derived slide `c05_slide_production_map` | `multiscale/records/{support.rs,provenance.rs,mod.rs,artifact_graph/**}` and `multiscale/matrix_artifact.rs` only | Both slide support receipts, seven-role graph capabilities, compact slide table receipt | Root-owned finalizer/exports/tests/docs and existing formats | No commit, tests, ledgers, new physical profile, or broad abstraction |
| C-05 derived slide `c05_slide_test_map` | `tests/multiscale_embedding_artifact_graph/derived_region/fixture{.rs,/derived_slide**}` only | Both-path graph/finalization/receipt behavior cases | Production and ledgers | No production edits, commit, benchmark, or review |
| C-05 derived slide `c04_feasibility_audit` | None | None | Final milestone diff | One independent read-only review; no edits or second approval pass |
| C-05 closure `/root` | `Cargo.toml`; private patch benchmark/support; DHAT and differential/workflow tests; DEC-0038, ledgers, handoff, gates, commit | `PreparedPatchEmbeddingBenchmark`; checksum-pinned smoke/closure outcomes; exact DHAT/RSS commands | Existing C-05 production APIs and fuzz-agent file | No production API/format/helper, dependency, workflow, adapter, promotion, or scientific result |
| C-05 closure `c05_fuzz_closure_map` | `fuzz/fuzz_targets/embedding_inputs.rs` only after contract-red confirmation | Bounded C-05 context/footprint/table/link/provenance/Arrow/Parquet fuzz routes | Benchmark/tests/docs and production code | No new target, dependency, lock change, production helper, corpus artifact, or commit |
| C-05 closure `c05_scale_closure_map` | None | None | DEC-0038 and exact benchmark/resource contract | Milestone's single independent pre-implementation review; no edits or later approval pass |

## A-03 public API migration inventory

`src/lib.rs` is the sole supported Rust export list. Every current re-export is assigned below; internal `pub` visibility does not make a crate-root API.

| Surface | Exact current items | WS-B disposition | Compatibility evidence |
|---|---|---|---|
| Marked application | `AnalysisEngine`, `MarkedAnalysisRun` | Preserve at crate root through one compatibility facade; route the first project/workflow slice through the same engine rather than copying formulas | `tests/api_contract.rs`, `tests/engine_spectrum.rs`, `tests/workflow_contract.rs` |
| Multimodal application | `MultimodalEngine`, `MultimodalInput`, `MultimodalAnalysisRun` | Preserve at crate root; later project node calls the existing engine once | `tests/api_contract.rs`, `tests/multimodal_cli.rs` |
| Configuration | `AnalysisConfig`; all 13 section types; `ComponentMode`, `PermutationStratum`, `RegistrationTransform`, `NeighborhoodNullModel`, `ThreadSetting`, `CurveMargins` | Preserve serialized configuration 0.2 and programmatic constructors during WS-B; new project/workflow config is additive and versioned | `tests/config_v02.rs`, examples, parser/fuzz tests |
| Marked input/domain | `Pattern`, `PatternMeta`, `TumorWindow`, `TumorMask`, `PatternLoader`, `PatternLoadResult`, `PatternLoadDiagnostics` | Preserve; adapt into later typed project/data contracts without adding filename inference or changing current rows | `tests/api_contract.rs`, CSV/Parquet parity and mask tests |
| Multimodal records | `HeCell`, `IhcCell`, `FusedCell`, `CellSection`; registration/null/extrapolation artifact records | Preserve current records; future stable identity types live in the new data boundary and use explicit adapters | multimodal unit/CLI tests |
| Registration/graph values | `LandmarkPair`, `Transform2D`, `TransformKind`, `SpatialGraph`, `SpatialEdge` | Preserve read-oriented contracts; no nonrigid/correspondence semantics added to these types | registration, graph, multimodal parity tests |
| Result 0.3 envelope | `ResultDocument`, `AnalysisResult`, `Provenance`, `RESULT_FORMAT_VERSION`, `AnalysisSection`, status/interpretation enums | Preserve byte/semantic compatibility and strict reader; any future 0.4 envelope is separate and versioned | `tests/result_v03.rs`, output and fuzz tests |
| Marked result records | Every marked result type re-exported on `src/lib.rs` lines 63–76 | Preserve closed 0.3 DTOs; do not insert new platform/scientific fields | result, engine, output tests |
| Multimodal result records | Every multimodal result type re-exported on `src/lib.rs` lines 63–76 | Preserve closed 0.3 DTOs and nullable/typed unavailable rules | result, multimodal/output tests |
| Pre/post comparison | `PrePostResult` records and three `compare_*` services | Preserve as descriptive compatibility services; do not rename as cohort inference/equivalence | pre/post, no-default API, CLI tests |
| Diagnostics | Beta posterior and graph-smoothing DTOs | Preserve as explicitly enabled exploratory diagnostics; no Bayesian-model claim | diagnostics interface/schema tests |
| Output | `OutputWriter`, `OutputManifest`, `ArtifactStatus` | Preserve transactional semantics and current artifact plan; project catalog references outputs rather than changing this writer first | output transaction/integration tests |
| Errors | `MarklabError`, `Result` | Preserve contextual error path; new crates may introduce focused internal errors but facade maps them without swallowing causes | error-path tests across boundaries |
| WSI adapter | Feature-gated `PlaneSelection`, `RegionRequest`, `RgbaRegion`, `Slide*` values and `SlideReader` | Preserve under `wsi`; no viewer/segmentation expansion in WS-B | `tests/wsi_integration.rs`, WSI fuzz/public workflow |
| Hidden CLI launcher | feature-gated, doc-hidden `run_cli` | Preserve current binary wiring; new commands remain outside scientific formula ownership | CLI integration tests |

No current crate-root item is approved for removal in WS-B. Deprecation decisions require a later version boundary and migration evidence.

## A-03 CLI migration inventory

| Command | Inputs/observable outputs | WS-B disposition |
|---|---|---|
| `marklab analyze` | cells CSV/Parquet, GeoJSON mask, TOML config; transactional marked run directory; optional logs/timings/heap profile | Preserve exact CLI and result/artifact parity; later expose as a convenience project recipe |
| `marklab batch` | CSV manifest, shared config/output root; safe single-component IDs; marked run directories | Preserve traversal/symlink protection and batch-level parallelism |
| `marklab prepost` | result file or directory pairs; `prepost.json` | Preserve descriptive semantics and path resolver |
| `marklab multimodal analyze` | H&E CSV or CellViT-class CSV, IHC CSV, landmarks, config, metadata; multimodal run directory | Preserve; no embedding-vector or same-cell semantics inferred |
| `marklab multimodal prepost` | multimodal result pair; `prepost.json` | Preserve descriptive semantics |
| `marklab multimodal batch` | mixed analyze/prepost manifest rows | Preserve row validation and safe output handling |
| `marklab simulate random-labeling` | `n`, prevalence, seed; CSV/Parquet cell table | Preserve deterministic fixture generation |
| `marklab smoke` | suite, replicate count, output directory; `smoke.json` | Preserve as smoke evidence only, not formal calibration |
| `marklab profile-plan` | workload and output file | Preserve current profiler-command document generation |
| `marklab inspect-slide` | WSI path; pretty metadata to stdout or JSON | Preserve under `wsi` |
| `marklab extract-region` | bounded unsigned level coordinates/plane and PNG output; optional force | Preserve preflight limits and overwrite policy under `wsi` |

## A-03 configuration inventory

Configuration 0.2 is strict (`deny_unknown_fields`). Every current section is preserved through WS-B:

| Section | Current controls | Disposition |
|---|---|---|
| `[analysis]` | mark label, probabilistic marks, component mode | Preserve |
| `[validation]` | sample/prevalence/area/shell/mask/interpretable-scale bounds | Preserve shared scale semantics |
| `[spectrum]` | shell counts, low-k summary, alpha fit, anisotropy shells | Preserve |
| `[periodogram]` | enabled | Preserve Hann-tapered diagnostic naming |
| `[multiscale_residual]` | enable, territory detection, z threshold | Preserve residual—not wavelet—semantics |
| `[permutation]` | count, seed, stratification, typed strata | Preserve deterministic null contract |
| `[inference]` | family-wise alpha | Preserve |
| `[diagnostics]` | beta posterior groups, graph smoothing | Preserve default-off exploratory status |
| `[registration]` | enable, rigid/affine, landmark/RMSE/resolution controls | Preserve |
| `[neighborhood]` | radius/kNN, label pairs, territory controls, typed null models | Preserve |
| `[comparison]` / margins | spectrum, mark-pair covariance, cross interaction, graph enrichment log2, territory profile | Preserve descriptive names; no equivalence alias |
| `[performance]` | thread policy, memory budget, k chunk, strict reproduction, intermediates | Preserve enforced budget and determinism |
| `[output]` | Parquet, GeoJSON, figures, manifest switches | Preserve artifact behavior |

New project/workflow/backend/resource/report configuration must use separate versioned documents. It must not overload configuration 0.2 keys.

## A-03 result and artifact inventory

| Result kind/artifact family | Current contract | WS-B disposition |
|---|---|---|
| `marked_pattern` | strict result-format 0.3 marked DTO | Preserve exactly through compatibility path |
| `multimodal` | strict result-format 0.3 multimodal DTO | Preserve exactly through compatibility path |
| `marked_prepost` | strict 0.3 descriptive `PrePostResult` | Preserve exactly |
| `multimodal_prepost` | strict 0.3 descriptive `PrePostResult` | Preserve exactly |
| Core run files | `result.json`, `qc.json` or `registration_qc.json`, `timings.json`, optional `run_manifest.json` | Preserve names, required-file validation, and atomic commit |
| Marked curves/maps | `spectra.parquet`, `mark_pair_covariance.parquet`, `scale_energy.parquet`, `residual_territories.geojson` | Preserve optionality and schemas |
| Marked figures | `figures/spectrum.svg`, `anisotropy.svg`, `scale_energy.svg`, `residual_territory_overlay.svg` | Preserve optionality; no project UI recomputation |
| Marked intermediates | `intermediates/filtered_cells.parquet`, `intermediates/kgrid.parquet` | Preserve behind `save_intermediates` |
| Multimodal result sidecars | neighborhood enrichment/cross curves/territories/profiles JSON; optional GeoJSON and Parquet | Preserve current populated/omitted policy |
| Multimodal retained-run artifacts | registration residuals/transform/extrapolation, fused cells, territories, profiles, cross curves, enrichment, null sensitivity in current CSV/JSON projections | Preserve; project catalog may reference them without duplicating data |
| Pre/post file | `prepost.json` | Preserve standalone comparison write path |
| Smoke file | `smoke.json` | Preserve attempted/completed/failed denominators |
| WSI outputs | metadata JSON/stdout and RGBA8 PNG | Preserve bounded adapter semantics |

The 0.2 reader remains deliberately narrow: only unambiguous marked documents migrate; 0.2 multimodal, populated obsolete placeholders/curve tests, unknown fields, and unrecoverable semantics require reanalysis.

## A-03 build, test, benchmark, and workflow inventory

| Surface | Current items | Disposition |
|---|---|---|
| Default features | `cli`, `parallel`, `parquet`, `csv` | Preserve aggregate behavior |
| Optional features | `wsi`, `allocator-mimalloc`, `dhat-heap` | Preserve isolation and supported combinations |
| Integration suites | API, CLI, config 0.2, diagnostics, spectrum, multimodal CLI, performance, result 0.3, workflow, WSI, project/artifact, hierarchy/coordinates, and CellViT embedding domain/source/physical boundaries | Preserve and extend only through owned contracts |
| Fuzz targets | config, GeoJSON mask, CSV row parser, result document, WSI region request, artifact catalog, and bounded embedding inputs | Preserve/build; add only for changed boundaries |
| Criterion benches | structure factor, permutation engine, periodogram, multiscale residual, random-labeling envelope, pattern load, cohort hierarchy, and cell embeddings | Preserve names, frozen profile selection, and equivalent-work assertions |
| CI | fmt, all-target/all-feature Clippy, Nextest, docs, no-default, WSI, dependency policy, package, fuzz build, smoke benchmark, heap regression, synthetic smoke | Preserve or record exact workspace-equivalent commands |
| Scheduled | formal calibration, full benchmarks, public WSI oracle, release target archives | Preserve; do not recast scheduled evidence as per-PR proof |

Current HEAD is the audited SHA. Its parent-to-HEAD change modifies `README.md` and deletes 13 internal remediation/migration records; it changes no Rust source, tests, manifests, lockfile, CI, or fixtures. Because implementation base equals the plan's audited SHA, there is no post-audit diff to migrate.

## C-06 first-slice owner map

- `crates/marklab-data/src/measurement.rs` owns the in-memory four-state measurement-origin vocabulary; it is separate from extraction validity and has no wire format.
- `crates/marklab-embeddings/src/multiscale/overlap_dispersion.rs` is the sole new production caller. It binds an existing patch table, patch support, overlap graph, and direct-patch provenance before computing one bounded descriptive value.
- `tests/measurement_status.rs` and `tests/patch_overlap_embedding_dispersion.rs` own exact mapping, conflation, oracle, missingness, identity, determinism, and work-bound evidence.
- No MarkTable, scalar adapter, physical format, receipt, validator framework, config/result/CLI field, dependency, or external data path is added.

## C-06 declared scalar-pattern workflow owner map

- `src/scalar_mark/declaration.rs` owns only bounded unitless binary/probability declarations, threshold origin/comparator, measurement status, and endpoint routing.
- `src/scalar_mark/input.rs` borrows the unchanged `Pattern` and ordered CellIds, validates exact project hierarchy/frame/provenance bindings and caller limits, and owns no file or physical format.
- `src/scalar_mark/identity.rs` owns the compact runtime row/slide/frame/declaration identity and private deterministic framing; `provenance.rs` owns the two exact closed ArtifactRecord profiles; `error.rs` owns typed boundary failures.
- `src/api.rs` delegates declared runs to the unchanged `AnalysisEngine`; `src/workflow.rs` owns the separate concrete node, exact three-reference cache binding, existing store verification, and result-0.3-only codec.
- `tests/scalar_mark_input.rs`, `tests/declared_marked_workflow.rs`, and their shared fixture own the behavior, project-revalidation, direct parity, routing, identity, miss/hit, integrity, and unchanged-codec evidence.
- No `Pattern`/loader/config/result/CLI edit, general MarkTable, physical schema, receipt, validator registry, geometry/window owner, dependency, or external path is added.

## C-06 patch-to-derived-region dispersion owner map

- `crates/marklab-embeddings/src/multiscale/region_dispersion.rs` owns the sole new computation and its fixed-size result/status/error types. It consumes the exact C-05 patch table, patch-region link, verified graph, derived candidate, and both provenance values.
- `RegionEmbeddingTable::row_index` is private to the multiscale package and is used immediately by that computation; no general visitor, metric trait, statistics framework, or vector abstraction was added.
- `tests/multiscale_embedding_artifact_graph/derived_region/fixture/region_dispersion.rs` reuses the existing exact graph/finalizer fixture for oracle, identity, drift, status, determinism, and work-cap evidence.
- Root and embedding facades expose only the concrete function/result/error/status. No Arrow/Parquet code, receipt, validator, workflow, codec, config/result/CLI, manifest, lock, dependency, or external data path changed.

## Authorized external WSI and embedding asset map

Read-only SSH inventory on 2026-08-22 used the existing authenticated connection. No patient/sample identifiers or raw patient data were copied into this repository.

### Aggregate WSI inventory

| Privacy-safe root/corpus | High-confidence objects | Bytes | Notes |
|---|---:|---:|---|
| `/Volumes/500GB/marklab/raw/tcga-crc-he-validation-v1` | 222 SVS | 135,287,341,680 | Public TCGA CRC validation; nine selected slides lack usable scale metadata and are explicitly excluded downstream |
| `/Volumes/1TB/marklab/raw/cptac-coad-he-v1` | 366 SVS | 62,829,086,758 | Public CPTAC-COAD; 366 accessible versus 373 advertised remains unresolved |
| Other authorized validation roots | 7 NDPI, 4 MRXS plus four companion directories, 78 >100 MiB TIFF/BigTIFF candidates | 136,602,511,664 | BASISS, Stage III CRC/IMC, Heiser, HTAN, Tumoroscope, CalicoST, and prepared derivatives |
| Total high-confidence/compatible objects | 677 | 334,719,940,102 | Not necessarily 677 unique biological slides because prepared pyramids/derivatives exist |

The broader scan found 1,420 image-format files: 588 SVS, 7 NDPI, 4 MRXS, 818 TIFF/TIF, one tiny DCM, and two tiny J2K fixtures. No OME-TIFF, CZI, SCN, VMS/VMU, BIF, JP2, Zarr, or OME-NGFF corpus was found.

### Sidecars and modality assets

| Type | Count | Bytes |
|---|---:|---:|
| CSV/TSV | 10,555 | 20,183,892,108 |
| Parquet | 210 | 7,100,968,367 |
| H5/H5AD | 385 | 84,882,550,905 |
| NPY/NPZ | 12,256 | 62,468,053,020 |
| PT | 940 | 19,514,488,084 |
| GeoJSON | 414 | 71,907,935 |

Central manifest/protocol/clinical evidence is under `/Volumes/500GB/marklab/manifests` and `/Volumes/1TB/marklab/manifests`; 56 small files total 4,828,439 bytes.

### CellViT evidence and gaps

- Exact checkpoint: `/Volumes/500GB/marklab/env/models/CellViT-SAM-H-x40-AMP.pth`, 2,799,319,650 bytes, SHA-256 `356418f19d9d478f164c7a31f85274584fefaa02355815c09f52346c658c8ec4`.
- Upstream source commit: `09f90f804549d51a483600e654771e718c0d5f61`; source snapshot manifest SHA-256 `34380c6aabf4f12121fcdca26e91390af3db1aebc54cde90965261bd56ef45a0`.
- Verified inference: TCGA has 213 complete output/manifest/log families (4,706,100,610 bytes); CPTAC has 366 (8,317,782,098 bytes). Each manifest binds source WSI, model, source snapshot, protocol, environment, runner, and output hashes.
- Raw embeddings: 96 NPY matrices of 1,280-dimensional `float32` values, about 23.4 GB, plus derived 1,280/1,024/384/37/32-dimensional artifacts.
- TCGA full-embedding audit: 190 patients and 508,377 retained tumor cells; 15 Parquet outputs totaling 1,125,847,859 bytes, without duplicating raw vectors.
- Frozen Schürch-to-TCGA transform SHA-256: `80cc055c3ca354f5c7db00260b6e15e604a75995dd0df9085d200beedc4d3f5a`.
- Inspected cell JSON has centroid/contour/type/probability/patch-offset/status fields but no explicit stable `CellId`; `.pt` row linkage appears positional unless a separate alignment table is found.
- Patch imagery/coordinate metadata and candidate C-order `f32` feature matrices at widths 384 and 1,024 exist. No canonical one-vector-per-`PatchId` table, `CellPatchLink`, or promotable source profile was found; extraction stride/overlap, frame convention, effective receptive field, observation support, identity mapping, and complete provenance remain absent. Width-384 candidates are explicitly development-only, and width-1,024 candidates span seven incompatible source-local CSV signatures.
- Sampled-patch selection metadata exist, but the Otsu mask was not persisted in the TCGA/CPTAC inference trees and no exact observation window is admitted. After exact frame and support reconstruction, a declared footprint union may become a sampled-support artifact; it is never automatically the full WSI or tissue window.
- `.pt` files were not deserialized because PyTorch pickle is executable. Conversion requires the pinned trusted environment and a safe non-executable output format.

Selected provenance hashes: TCGA slide manifest `35f2d8bb7b729fa76f48ff78921f3b5af4737eafb9354dbab3f1e0956390dff`; CPTAC slide manifest `344f02a3d7c0c3f365e6f15b97fadd945ad6c5736fa951f19274568da1743fc9`; TCGA inference manifest `63d9a057e165a81dcdfa15f006e8c38d15e85357fe0a4b2a2e732e6f85f462ea`; TCGA full-embedding run `f61282812654d1e8fb7070b594fd2e25d10e81cc9def26acec436e5e3a13ed2e`; analysis config `e7b12c770556140fe44248e895778db5365fa4ee96672d68302004a2af39fcd6`; remote provenance document `a96a0f6c3e236a1badbed4d0c915157a296c3e60239fa4f034d317f9d0ae3e84`.
