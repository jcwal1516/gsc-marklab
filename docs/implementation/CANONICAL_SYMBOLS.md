# Canonical symbols and ownership

Rows are characterization records at the implementation base. `Active task = none` means read-only during WS-A. A-03 verifies exact callers and tests before WS-B.

| Canonical owner/symbol | Location | Contract | Principal callers | Tests/evidence | Active task |
|---|---|---|---|---|---|
| `AnalysisEngine` | `src/api.rs` | Current marked workflow orchestration; no new scientific families | Public library, CLI analyze/batch | `tests/api_contract.rs`, `tests/workflow_contract.rs`, `tests/engine_spectrum.rs` | none |
| `Config` parse/validate | `src/config.rs`, `src/config/**` | Strict current configuration/default/validation policy | CLI and library workflows | config/unit/integration tests, fuzz config | none |
| `Pattern` / `PatternMeta` / `TumorWindow` | `src/data/pattern.rs` | Current finite 2-D marked-pattern contract | loaders and analysis engine | data/API/workflow tests | none |
| `PatternLoader` and row builder | `src/io/**` | Canonical CSV/Parquet-to-pattern conversion | CLI/library ingest | IO parity, fuzz load-cells, pattern-load bench | none |
| `SpatialIndex2D` | `src/geom/spatial_index.rs` | Exact deterministic 2-D spatial queries | marked/multimodal/neighborhood plans | geom/performance tests | none |
| scalar permutation p-value policy | `src/inference/scalar_pvalues.rs` | Checked finite permutation inference | spectrum/neighborhood/comparison | inference tests | none |
| BH multiple testing policy | `src/inference/multiple_testing.rs` | Canonical BH adjustment and invalid-state policy | neighborhood enrichment/results | inference/neighborhood tests | none |
| deterministic label permutations | `src/permutation/**` | Fixed-position label shuffling, optional strata, stable seed namespaces | marked and multimodal nulls | permutation/ERL tests and bench | none |
| ERL global envelope | `src/permutation/envelopes.rs` | Checked functional global envelope | spectrum/cross curves | ERL oracle fixture, random-labeling bench | none |
| structure-factor implementation | `src/spectra/structure_factor/**` | Current Fourier-domain marked endpoint | marked spectrum stage | spectra/engine tests and bench | none |
| `MarkPairCovariancePlan` | `src/spectra/mark_pair_covariance.rs` | Centered distance-binned mark-pair covariance; not g(r) | marked spatial stage | spectra/tests and performance evidence | none |
| current periodogram | `src/periodogram/**` | Hann-tapered raster periodogram; not Bartlett/multitaper | spectrum workflow | periodogram tests/bench | none |
| multiscale residual diagnostic | `src/multiscale_residual/**` | Raster residual energy/territories; not wavelets or domains | marked multiscale stage | unit/integration/bench | none |
| landmark transform/QC | `src/registration/**` | Rigid/affine 2-D landmark registration with QC | multimodal engine | registration tests | none |
| multimodal fusion | `src/multimodal/fusion.rs` | Concatenate registered sections in a common frame; no identity matching | multimodal engine | multimodal tests/CLI | none |
| neighborhood graph | `src/neighborhood/graph.rs` | Current radius/kNN union graph semantics | multimodal enrichment/profiles | neighborhood/performance tests | none |
| cross-label pair-count curves | `src/neighborhood/cross_curves.rs` | Raw distance-binned cross-label counts; not cross-K/g | multimodal engine | indexed/brute-force and ERL tests | none |
| territory/profile implementation | `src/neighborhood/territories.rs`, `profiles.rs` | MMR-specific density components and circular profiles | multimodal engine | territory/profile tests | none |
| pooled-bin diagnostic | `src/comparison/pooled_bin_difference.rs` | Approximate descriptive comparison only | pre/post | comparison/prepost tests | none |
| descriptive margin assessment | `src/comparison/margin_assessment.rs` | Threshold description; not equivalence/noninferiority | pre/post | comparison/prepost tests | none |
| `ResultDocument` / result-format 0.3 | `src/output/document.rs`, `src/output/result_types/**` | Strict tagged finite result schema | all workflows/output writer | `tests/result_v03.rs`, output tests, fuzz result | none |
| output transaction | `src/output/transaction.rs`, `writer.rs` | No partial final output; manifest/artifact consistency | all CLI workflows | output/workflow tests | none |
| C-05 typed matrix physical profiles | `crates/marklab-embeddings/src/columnar/{multiscale,arrow/multiscale/matrix,parquet/multiscale/matrix}/**`, `src/multiscale/physical.rs` | Exact patch/region/slide Arrow IPC and Parquet write, raw preflight, full borrowed/managed validation, deterministic publication, records, nine-key metadata, QC/logical identity, and bounded one-batch/group retention; raw/full validators alone mint no receipt | Crate/root typed public facade and direct-patch receipt wrappers | `tests/multiscale_matrix_columnar*.rs`, unchanged `tests/cell_embedding_{arrow,parquet}.rs` | C-05 |
| C-05 direct-patch runtime receipts | `crates/marklab-embeddings/src/multiscale/matrix_artifact.rs`, `src/multiscale/records/artifact_graph/{error,mod}.rs`, Arrow/Parquet matrix readers | `VerifiedPatchEmbeddingSupportArtifact` composes exact graph-bound footprint/overlap receipts across physical formats; `VerifiedPatchEmbeddingTableArtifact` requires full physical validation plus graph-bound source-row PatchId/status equality, expected/support/provenance logical identity, dimension, QC, and distinct dependencies; opaque source components remain unproved | Four borrowed/managed Arrow/Parquet receipt entry points; derived-region graph | `tests/multiscale_embedding_artifact_graph/matrix_receipts.rs` | C-05 |
| C-05 derived-region support and graph receipts | `crates/marklab-embeddings/src/multiscale/matrix_artifact.rs`, `src/multiscale/records/artifact_graph/{region_support,derived_region,error}.rs` | `VerifiedRegionEmbeddingSupportArtifact` joins exact patch support and patch-region receipts only when expected-patch, context, and footprint lineage agree; `VerifiedDerivedRegionEmbeddingArtifactGraph` validates the nine-role weighted-mean graph over that support and a verified source patch table, without proving an output row/table or geometry | Deferred deterministic region finalizer and physical region-table receipt | `tests/multiscale_embedding_artifact_graph/derived_region/**`; private physical-profile unit cases | C-05 |
| configuration deserializer/validator | `src/config/deserialize.rs`, `validate.rs`, `model.rs` | One strict config 0.2 conversion/default/validation policy | `AnalysisConfig` constructors, CLI | config tests and config fuzz target | none |
| 0.2-to-0.3 result conversion | `src/output/migrate_v02.rs` | Convert only unambiguous marked states; reject unrecoverable semantics | `ResultDocument::from_json` | `tests/result_v03.rs` | none |
| result document parser/validator | `src/output/document.rs` | Version gate, strict DTO decode, finite validation, result-file/directory resolver | CLI/prepost/output users | result/output tests and result fuzz target | none |
| core artifact plan/projection | `src/output/artifact_plan.rs`, `writer.rs` | Required/optional artifacts by result family and output config | `OutputWriter` | output/workflow/CLI tests | none |
| multimodal artifact projections | `src/output/multimodal_artifacts.rs`, `multimodal_result_artifacts.rs` | Canonical CSV/JSON/Parquet/GeoJSON projections from one retained run/result | multimodal CLI/output writer | multimodal CLI/output tests | none |

## Planned control-plane ownership

| Task | Owner | Writable files | Owned symbols | Concurrent-write exclusion |
|---|---|---|---|---|
| A-01/A-03 | `/root` | `AGENTS.md`, `docs/implementation/**` | Documentation registries only | Sole tracked writer during WS-A |
| A-02 | `a02_baseline` | None | None | Read-only; generated ignored outputs only |
| SLIDE-INV | `remote_slide_inventory` | None | None | Read-only local/remote inventory |

## WS-B active ownership

| Task/agent | Writable files | Owned canonical symbols | Read-only context | Explicit non-goals |
|---|---|---|---|---|
| B-01 `/root` | `Cargo.toml`, `tests/workspace_contract.rs`, implementation docs | Root workspace/default-member/fuzz-boundary metadata only | All current source/tests/CI, `Cargo.lock`, and future crate roots | No new crate, lock change, scientific move, dependency change, public behavior, result/config/CLI change |
| B-01-REVIEW `b01_architecture_review` | None | None | Cargo/public API/tests/CI and B-01 contract | Read-only critique; no edits or commands that mutate tracked state |

Existing canonical scientific/config/parser/result/output symbols remain owned by the compatibility package and are read-only during B-01.

## B-02 active ownership

| Task/agent | Writable files | Owned canonical symbols | Read-only context | Explicit non-goals |
|---|---|---|---|---|
| B-02 `/root` | `tests/cli.rs`, `docs/implementation/**` | Compatibility characterization only; no production owner moves | `Cargo.toml`, `src/lib.rs`, `src/bin/marklab.rs`, current facade/CLI/config/result/output implementations | No source move/copy, dependency/lock change, API/config/result/CLI change, algorithm change, or new crate |
| B-02-REVIEW `b02_compatibility_audit` | None | None | Public facade/binary/features and API/CLI/config/result tests | Read-only critique; no tracked or remote mutation |

## B-03 active ownership

| Task/agent | Writable files | Owned canonical symbols | Read-only context | Explicit non-goals |
|---|---|---|---|---|
| B-03 `/root` | `AGENTS.md`, `.github/workflows/**`, `tests/workflow_contract.rs`, `tests/workspace_contract.rs`, `src/cli/batch.rs` import cfg only, `docs/implementation/**` | Workspace command, feature matrix, dependency-layer, and CLI-gating policy only | All manifests, package targets/features, source ownership, current gates | No dependency/lock/schema/API/algorithm change, no source move, no xtask without demonstrated repeated local orchestration |
| B-03-REVIEW `b03_policy_audit` | None | None | Workflows/manifests/features/tests/docs | Read-only critique; no tracked or remote mutation |

## B-04 active ownership

| Task/agent | Writable files | Owned canonical symbols | Read-only context | Explicit non-goals |
|---|---|---|---|---|
| B-04 `/root` | `Cargo.toml`, generated local-package-only `Cargo.lock` and `fuzz/Cargo.lock`, `crates/marklab-project/**`, `crates/marklab-workflow/**`, `src/workflow.rs`, focused `src/lib.rs` re-exports/module declaration, `tests/project_workflow.rs`, `tests/workspace_contract.rs`, `docs/implementation/**` | `ContentDigest`, `ArtifactRef`, `MarklabProject`, `SuccessfulRun`, `NodeId`, `NodeSpec`, `WorkflowGraph`, `WorkflowNode`, `LocalScheduler`, `NodeRun`, `CacheStatus`, `SchedulerLimits`, `MarkedAnalysisNode` | Existing `AnalysisEngine`, `Pattern`, `AnalysisConfig`, result 0.3, `OutputWriter`, manifests/features/CI/fuzz | No algorithm/config/result/output rewrite, persistent project schema, generic multi-node executor, backend/plugin system, remote execution, input payload copies, new result field, or CLI command |
| B-04-REVIEW `b04_vertical_slice_audit` | None | None | Project/workflow plan, current engine/data/config/result/output/tests/manifests | Read-only design critique; no tracked or remote mutation |

## C-01 completed ownership

| Task/agent | Writable files | Owned canonical symbols | Read-only context | Explicit non-goals |
|---|---|---|---|---|
| C-01 `/root` | Workspace/local lock metadata; `crates/marklab-core/**`, `crates/marklab-data/**`; focused `marklab-project` hierarchy ownership; root dev dependency only; `tests/data_hierarchy.rs`, architecture/workflow contract updates; one hierarchy benchmark; implementation docs | Typed ID newtypes, `HierarchyId`, `HierarchyNode`, `ReplicationRole`/`ReplicationRoleKind`, independent biological-source relation, bounded lineage membership, `RepeatedMeasureSet`, `CohortHierarchy`, `CohortDesignSummary`, hierarchy validation/errors, project hierarchy slot | Current loose metadata/cell IDs, project/workflow APIs, compatibility facade, manifests/CI/bench patterns, remote inventory handoff | No root stable re-export, current adapter/schema/result/CLI/science change, filename inference, patient-level inference, persistence, remote ingest, or new registry dependency |
| C-01-REVIEW `c01_identity_audit` | None | None | C-01/FND-01/COH-01 plan, current identity fields/callers, proposed contract/package graph | Read-only critique; no tracked or remote mutation, no LSP server |

## C-02 completed ownership

| Task/agent | Writable files | Owned canonical symbols | Read-only context | Explicit non-goals |
|---|---|---|---|---|
| C-02 `/root` | Focused coordinate-identity exports in `marklab-core`; coordinate registry/types in `marklab-data`; `tests/coordinate_frames.rs`; necessary architecture/documentation contracts | Coordinate frame/transform/uncertainty IDs; axis/dimension/unit/space types; framed 2-D/3-D coordinates; directed frame transforms and explicit chains; uncertainty references; explicit parallel serial-section placements/series; coordinate validation/errors | Current root `Transform2D`, registration/multimodal/config/result/WSI APIs and tests; C-01 hierarchy; remote inventory; OME-NGFF/SpatialData documentation | No current registration/result/config edit, fitter/resampler/inverse/auto-path, persistence/interchange claim, oblique/deformation/correspondence/uncertainty propagation, geometry/window/science, root re-export, remote ingest, manifest/lock/dependency change |
| C-02-REVIEW `c02_coordinate_audit` | None | None | C-02/WS-21/DATA-01 plan, proposed semantics, current transform/cell/WSI owners, standards notes | Read-only critique; no tracked/remote mutation and no LSP server |

## C-03 completed ownership

| Task/agent | Writable files | Owned canonical symbols | Read-only context | Explicit non-goals |
|---|---|---|---|---|
| C-03 `/root` | `crates/marklab-project/**`; focused workflow semantic-input/re-export code; focused root re-exports/project-workflow test; generated root/fuzz locks; one fuzz target/manifest contract; implementation docs | `ArtifactSchema`, `ArtifactId`, `ArtifactRecord`, `ArtifactCatalog`, table declaration types, `StoreId`, `ArtifactKey`, `ArtifactLocator`, `LocalArtifactStore`, capability-confined publication/verification/recovery, schema-aware cache-key inputs, project coordinate-registry slot | Existing B-04 digest/cache/project/workflow types; current `OutputWriter`; current Arrow/Parquet IO; C-01/C-02 substrate; remote inventory docs; official Arrow/Parquet/cap-std contracts | No result 0.3/CLI/config/current writer or Parquet edit, live cloud/network/credentials, physical table conformance claim, mutable project head/execution ledger, C-01/C-02 payload codec, patient data, `.pt`/pickle read, scientific method, publish/version action |
| C-03-REVIEW `c03_artifact_audit` | None | None | C-03/WS-11/FND-07 plan, project/workflow/output/path/table owners, C-01/C-02 handoffs, remote inventory documentation | Read-only ownership/security critique; no tracked/remote mutation, no LSP, no executable artifact read |
| C-03-IMPLEMENTATION-REVIEW `c03_implementation_audit` | None | None | Frozen C-03 contract, implementation/tests, capability publication/recovery boundary, package/platform evidence | Read-only correctness/security/complexity re-review; no tracked/remote mutation, no executable artifact read |

## C-04 completed ownership

| Task/agent | Writable files | Owned canonical symbols | Read-only context | Explicit non-goals |
|---|---|---|---|---|
| C-04 `/root` | New `crates/marklab-embeddings/**`; focused verified-reader/`publish_send` additions/tests in `marklab-project`; root workspace/features/prerequisite and embedding re-exports/integration test/benchmark; fuzz target/locks; implementation docs | `CellEmbeddingTable`, `EmbeddingStatus`, `EmbeddingQcSummary`, `ExpectedCellSet`, `CellIdentityMap`, `EmbeddingSpatialContext`, `CellEmbeddingRowLink`, `CellEmbeddingProvenance`, `CellEmbeddingArtifact`, `CellEmbeddingTablePhysicalBindings`, `SourceBundleReconciler`, `SourceBundleReconciliation`, CellViT NPY/CSV source-bundle profile, bounded canonical Arrow/Parquet embedding codecs, embedding integrity budgets/errors, `ArtifactReadSeek`, verified-reader and send-publication callbacks | C-01 `CellId`/hierarchy; C-02 coordinate runtime values; C-03 catalog/store/table manifests; current root CellViT class CSV and columnar IO; remote aggregate/header/provenance evidence; official NPY/Arrow/Parquet specifications | No result/config/CLI/current importer or writer edit, general hierarchy/coordinate codec, scientific statistic/prediction/technical-confounder conclusion, patient data fixture, `.pt`/pickle execution, inferred identity/scale, real-corpus promotion, patch links, general mark status, live cloud, unsafe/mmap/zero-copy claim, publish/version action |
| C-04-REVIEW `c04_embedding_audit` | None | None | C-04/EMB-CORE/FND-05/WS-24 plan, proposed contract/decisions, existing identity/artifact/format owners, read-only remote evidence summary | Read-only ownership/provenance/security/scale critique; no tracked/remote mutation, no `.pt`/pickle read, no LSP server |
| C-04-FEASIBILITY-REVIEW `c04_feasibility_audit` | None | None | Proposed C-04 store/Arrow/Parquet/writer/dependency/memory contract; locked dependency source | Read-only API/allocation feasibility critique; no tracked/remote mutation, no executable artifact read, no LSP server |

## C-05 ownership

| Task/agent | Writable files | Owned canonical symbols | Read-only context | Explicit non-goals |
|---|---|---|---|---|
| C-05 `/root` | Focused new `crates/marklab-embeddings/src/multiscale/**` and columnar multiscale modules; focused exports/errors/status documentation; new C-05 integration/property/resource tests; existing embedding fuzz target only; root `Cargo.toml`, `src/lib.rs`, one new benchmark/support target, and benchmark/workflow contract test only as required; C-05 task contract, decisions, interfaces, requirements/status, repository/claims/validation/performance/canonical ledgers, and handoff | `EmbeddingEntityKind`; expected patch/region/slide sets, `PatchSourceEntitySet`, `PatchIdentityMap`, `PatchEmbeddingSourceRowLink`, and exact patch-input normalization; `PatchEmbeddingContext`; `PatchFootprintSet`; `PatchOverlapGraph`; `MultiscaleEmbeddingSupport`; opaque cell-anchor source role plus cell-patch producer/assignment groups/edges and their closed status; exhaustive declared patch-region assessment/link; `PatchEmbeddingTable`; `RegionEmbeddingTable`; `SlideEmbeddingTable`; multiscale derivation/provenance; `VerifiedPatchRegionInputArtifactGraph`; `VerifiedPatchRegionLinkArtifact`; separate runtime-only verified artifact/link/bundle receipts; eight exact C-05 Arrow/Parquet profile families | Frozen C-01 identity/hierarchy, C-02 frame/transform/runtime coordinates, C-03 artifact/store, complete C-04 cell substrate and physical helpers, current root compatibility surfaces, authorized aggregate-only patch evidence | No C-04 wire/digest/graph behavior change, generic geometry/window/mask owner, production real-source adapter or corpus promotion, source ID/frame/stride/overlap/RF inference, `.pt`/pickle read, vector repetition in links/results, general mark/scientific/confounder/prediction result, CLI/config/result/current importer edit, new registry tuple/version/publish/worktree/remote mutation |
| C-05-INVENTORY-REVIEW `c04_embedding_audit` | None | None | C-05/FND-05/EMB-PATCH/WS-24 plan, prior inventory, allowlisted aggregate/header metadata from authorized remote roots | Read-only aggregate source/profile/claim audit; no tracked/remote mutation, payload/vector/checkpoint or executable-serialization read, intentional identifier retention/forwarding, or production ownership; one bounded recursive-key-output defect is recorded in the validation ledger |
| C-05-FEASIBILITY-REVIEW `c04_feasibility_audit` | None | None | Frozen C-05 typed tables/context/support/links/provenance/physical/resource contract and current locked APIs | Read-only semantic/API/allocation critique; no tracked/remote mutation, source payload read, production ownership, or LSP server |
