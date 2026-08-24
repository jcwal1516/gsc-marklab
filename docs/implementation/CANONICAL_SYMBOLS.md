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
| C-05 derived-region support and graph receipts | `crates/marklab-embeddings/src/multiscale/matrix_artifact.rs`, `src/multiscale/records/artifact_graph/{region_support,derived_region,error}.rs`, `src/multiscale/finalization.rs` | `VerifiedRegionEmbeddingSupportArtifact` joins exact patch support and patch-region receipts only when expected-patch, context, and footprint lineage agree; `VerifiedDerivedRegionEmbeddingArtifactGraph` validates the nine-role weighted-mean graph; the candidate-only finalizer recomputes fixed-order values before full Arrow/Parquet validation can mint `VerifiedRegionEmbeddingTableArtifact` | Derived slide-from-region support; compact candidate-bound region receipt | `tests/multiscale_embedding_artifact_graph/derived_region/**`; private physical-profile unit cases | C-05 |
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
| C-05-CLOSURE `/root` | `Cargo.toml`; `benches/patch_embeddings.rs`; `benches/support/patch_embeddings.rs`; `tests/{patch_embedding_heap,multiscale_embedding_differential,workflow_contract}.rs`; closure docs/handoff | Private deterministic patch-vector/shared-link workload, fixed numeric/link checksum, DHAT/RSS gates, differential closure evidence | Existing C-05 production contracts and agent-owned fuzz target | No new production API/helper/profile/receipt, dependency, source adapter, workflow, promotion, or scientific estimand |
| C-05-CLOSURE-FUZZ `c05_fuzz_closure_map` | `fuzz/fuzz_targets/embedding_inputs.rs` only | Existing target's bounded C-05 context/footprint/table/link/provenance/Arrow/Parquet routes | All production/benchmark/test/docs files | No target/manifest/lock/dependency addition, corpus commit, production edit, or commit |
| C-05-CLOSURE-REVIEW `c05_scale_closure_map` | None | None | DEC-0038 and its exact generator/order/checksum/DHAT/RSS commands | Single independent pre-implementation decision review; no edit, remote access, or later review |

## C-06 first-slice ownership

| Task/agent | Writable files | Owned canonical symbols | Read-only context | Explicit non-goals |
|---|---|---|---|---|
| C-06-FIRST-SLICE `/root` | `crates/marklab-data/src/{lib.rs,measurement.rs}`; `crates/marklab-embeddings/src/{lib.rs,multiscale/mod.rs,multiscale/overlap_dispersion.rs,multiscale/records/provenance.rs,multiscale/table/wrappers.rs}`; focused root re-exports in `src/lib.rs`; `tests/{measurement_status,patch_overlap_embedding_dispersion}.rs`; C-06 task contract and affected implementation ledgers | `MeasurementStatus`; exact multiscale-provenance status mapping; `patch_overlap_embedding_dispersion`; its result/status/error types | Frozen C-01/C-03 identity/artifact contracts and complete C-05 patch table, support, overlap, provenance, and extraction-validity APIs | No MarkTable, new physical schema/format/receipt/validator framework, unit/modality/threshold/missingness ontology, Pattern/config/result/CLI change, generic spatial weights/window, inferential statistic, source adapter/promotion, dependency, remote access, publish/deploy |
| C-06-RESEARCH `c06_contract_map`, `c06_caller_map` | None | None | Master-plan ambiguity, current callers, compatibility contracts, C-05 provenance/status/support surfaces | Read-only bounded research only; no writer ownership, review approval, source/test/doc edit, remote access, or commit |

## C-06 declared-scalar workflow completed ownership

| Task/agent | Writable files | Owned canonical symbols | Read-only context | Explicit non-goals |
|---|---|---|---|---|
| C-06-SCALAR-DOMAIN `c06_caller_map`, then `/root` after handoff | New `src/scalar_mark/**` only | `ScalarMarkId`, scalar value/comparator/binary-origin declarations, `BinaryMarkDeclaration`, `ProbabilityMarkDeclaration`, `DeclaredScalarPatternInput`, `DeclaredScalarIdentity`, `DeclaredMarkUse`, focused input error and private digest/validation helpers | DEC-0040, IC-0014, root Pattern, project hierarchy/coordinate/artifact APIs | No overlapping writers; no API/workflow/root export/test/doc/manifest/dependency/lock/CLI/config/result/Pattern/loader/geometry change, commit, remote access, or subagent by the delegated owner |
| C-06-SCALAR-FLOW `/root` | Focused `src/{api.rs,workflow.rs,lib.rs}`; new `tests/{scalar_mark_input,declared_marked_workflow}.rs` plus `tests/support/declared_scalar.rs`; decision/interface/task/requirements/status/validation/claims/performance/repository/canonical/handoff docs | `AnalysisEngine::analyze_declared_scalar_pattern`, `DeclaredMarkedAnalysisRun`, `DeclaredMarkedAnalysisResult`, `DeclaredMarkedAnalysisNode`, exact result-0.3 codec reuse and cache binding | Agent-owned scalar domain, existing `MarkedAnalysisNode`, project scheduler/store, config/result compatibility suites | No scalar-domain file edit while agent writes; no general workflow/validator/MarkTable/format/CLI/config/result/Pattern/loader/geometry change, dependency, remote action, push/publish/deploy/history rewrite |
| C-06-SCALAR-REVIEW `c06_contract_map` | None | None | Final scalar domain/flow/tests and frozen C-06-S2 contract | Single read-only review; no edit, commit, remote access, or second approval pass |

## C-06 region-aggregation dispersion completed ownership

| Task/agent | Writable files | Owned canonical symbols | Read-only context | Explicit non-goals |
|---|---|---|---|---|
| C-06-REGION-DISPERSION `/root` | New `crates/marklab-embeddings/src/multiscale/region_dispersion.rs`; focused exports in embedding/root facades; private `RegionEmbeddingTable::row_index`; focused derived-region integration fixture/tests; `C-06-S3` task contract plus DEC-0041/IC-0015 and affected milestone ledgers/handoff | `patch_region_embedding_dispersion`; `PatchRegionEmbeddingDispersion`, `PatchRegionEmbeddingDispersionStatus`, and `PatchRegionEmbeddingDispersionError` | Complete C-05 patch/link/graph/finalizer candidates, DEC-0036, DEC-0039, C-06 status mapping, read-only `c06_caller_map`/`c06_contract_map` audits | No overlapping writer; no Arrow/Parquet redesign, general statistics/mark/weights/geometry abstraction, format/receipt/validator/node/codec, CLI/config/result/Pattern change, dependency, source promotion, remote action, push/publish/deploy/history rewrite |
| C-06-REGION-DISPERSION-RESEARCH `c06_caller_map`, `c06_contract_map` | None | None | Immediate-caller candidates and remaining foundation prerequisites at base `c370685` | Read-only bounded audit only; no writer ownership, review approval, edit, test, commit, or remote access |
| C-06-REGION-DISPERSION-REVIEW `c06_contract_map` | None | None | Final C-06-S3 production/test/docs diff after focused green | Single read-only correctness/resource/claim review; no edit, test, commit, remote access, or second approval pass |

## C-06 declared marked pre/post completed ownership

| Task/agent | Writable files | Owned canonical symbols | Read-only context | Explicit non-goals |
|---|---|---|---|---|
| C-06-DECLARED-PREPOST `/root` | New `src/prepost/declared_marked.rs`; focused `src/prepost/mod.rs` and root facade exports; new `tests/declared_marked_prepost.rs`; immediate test-fixture additions in `tests/support/declared_scalar.rs`; `C-06-S4` task contract plus DEC-0042/IC-0016 and affected milestone ledgers/handoff | `compare_declared_marked_prepost`; `DeclaredMarkedPrePostResult`; `DeclaredMarkedPrePostError` | Existing `DeclaredMarkedAnalysisResult`, scalar declarations/identity, unchanged `compare_marked_prepost`/`PrePostResult`/result 0.3, completed C-06 research audit | Sole writer; no new node/codec/result/schema/format/receipt/validator, generic comparison/marks/provenance abstraction, Pattern/config/CLI edit, dependency, geometry/window, inference, remote action, push/publish/deploy/history rewrite |
| C-06-DECLARED-PREPOST-REVIEW `c06_caller_map` | None | None | Final production/test/docs diff after focused green | Single read-only correctness/compatibility/claim review; no edit, test, commit, remote access, or second approval pass |

## C-06 declared binary prevalence completed ownership

| Task/agent | Writable files | Owned canonical symbols | Read-only context | Explicit non-goals |
|---|---|---|---|---|
| C-06-DECLARED-PREVALENCE `/root` | Focused `src/prepost/declared_marked.rs`, `src/prepost/mod.rs`, and root facade exports; focused additions to `tests/declared_marked_prepost.rs` and its existing shared fixture only if legitimate empty input requires them; `C-06-S5` task contract plus DEC-0043/IC-0017 and affected milestone ledgers/handoff | `compare_declared_marked_prevalence`; `DeclaredMarkedPrevalenceChange`; `DeclaredMarkedPrevalenceStatus`; shared private declared-output compatibility gate | Existing C-06-S2/S4 scheduler outputs, exact binary count/prevalence semantics, unchanged legacy pre/post/result 0.3, read-only `c06_caller_map` and `c06_contract_map` audits at base `d0822bd` | Sole writer; no public API break, new node/codec/schema/format/receipt/validator framework, general comparison/marks/inference abstraction, Pattern/config/CLI edit, dependency, geometry/window/weights owner, randomization/p-value, remote action, push/publish/deploy/history rewrite |
| C-06-DECLARED-PREVALENCE-RESEARCH `c06_caller_map`, `c06_contract_map` | None | None | Immediate-caller and dependency audits at clean base `d0822bd` | Read-only research only; no edit, test, review approval, remote access, or commit |
| C-06-DECLARED-PREVALENCE-REVIEW `c06_contract_map` | None | None | Final C-06-S5 production/test/docs diff after focused green | At most one read-only correctness/numerical/claim review; no edit, test, commit, remote access, or second review pass |

## C-06 slide aggregation-path discrepancy completed ownership

| Task/agent | Writable files | Owned canonical symbols | Read-only context | Explicit non-goals |
|---|---|---|---|---|
| C-06-SLIDE-PATH-DISCREPANCY `/root` | New `crates/marklab-embeddings/src/multiscale/slide_path_discrepancy.rs`; focused embedding/root facade exports; new focused derived-slide integration test module and one parent module declaration; `C-06-S6` task contract plus DEC-0044/IC-0018 and affected milestone ledgers/handoff | `slide_embedding_aggregation_path_discrepancy`; `SlideEmbeddingAggregationPathDiscrepancy`; `SlideEmbeddingAggregationPathDiscrepancyStatus`; `SlideEmbeddingAggregationPathDiscrepancyError` | Completed C-05 derived region/both slide candidates, graphs, provenance, existing region-table receipt, C-06 measurement mapping, read-only audits at base `767c0e9` | Sole writer; no candidate/graph/receipt/Arrow/Parquet redesign, new physical/schema/validator/general comparison/statistics abstraction, CLI/config/result change, dependency, source promotion, geometry/window/weights, randomization/inference, remote action, push/publish/deploy/history rewrite |
| C-06-SLIDE-PATH-DISCREPANCY-RESEARCH `c06_caller_map`, `c06_contract_map` | None | None | Lineage/immediate-caller and numerical/claim audits at clean base `767c0e9` | Read-only research only; no edit, test, review approval, remote access, or commit |
| C-06-SLIDE-PATH-DISCREPANCY-REVIEW `c06_contract_map` | None | None | Final C-06-S6 production/test/docs diff after focused green | At most one read-only lineage/numerical/claim review; no edit, test, commit, remote access, or second review pass |

## C-06 declared binary cell-embedding centroid discrepancy completed ownership

| Task/agent | Writable files | Owned canonical symbols | Read-only context | Explicit non-goals |
|---|---|---|---|---|
| C-06-DECLARED-CELL-CENTROID `/root` | New focused `src/cell_embedding_mark.rs`; root facade export; one new child module plus one module declaration in the existing CellViT artifact integration target; `C-06-S7` task contract plus DEC-0045/IC-0019 and affected milestone ledgers/handoff | `declared_binary_cell_embedding_centroid_discrepancy`; `DeclaredBinaryCellEmbeddingCentroidDiscrepancy`; `DeclaredBinaryCellEmbeddingCentroidDiscrepancyStatus`; `DeclaredBinaryCellEmbeddingCentroidDiscrepancyError`; `DeclaredBinaryCellEmbeddingGroupCounts` | Completed declared scalar input, verified C-04 cell table/artifact/QC contracts, read-only candidate audits at base `f00a9a4` | Sole writer; no embedding/scalar/physical/receipt/graph/validator framework edit, serializer/result/config/CLI/node, general mark/statistics abstraction, geometry/window/weights, inference, source promotion, remote action, push/publish/deploy/history rewrite |
| C-06-DECLARED-CELL-CENTROID-RESEARCH `c06_caller_map`, `c06_contract_map` | None | None | Immediate-caller/dependency and alternative-workflow audits at clean base `f00a9a4` | Read-only research only; no edit, test, review approval, remote access, or commit |
| C-06-DECLARED-CELL-CENTROID-REVIEW `c06_contract_map` | None | None | Final C-06-S7 production/test/docs diff after focused green | At most one read-only binding/numerical/resource/claim review; no edit, test, commit, remote access, or second review pass |

## C-06 declared probability–cell-embedding covariance completed ownership

| Task/agent | Writable files | Owned canonical symbols | Read-only context | Explicit non-goals |
|---|---|---|---|---|
| C-06-DECLARED-PROBABILITY-COVARIANCE `/root` | New focused `src/cell_embedding_probability.rs`; root facade export; one new child module plus one module declaration in the existing CellViT artifact integration target; `C-06-S8` task contract plus DEC-0046/IC-0020 and affected milestone ledgers/handoff | `declared_probability_cell_embedding_cross_covariance_energy`; `DeclaredProbabilityCellEmbeddingCrossCovarianceEnergy`; `DeclaredProbabilityCellEmbeddingCrossCovarianceStatus`; `DeclaredProbabilityCellEmbeddingCrossCovarianceError` | Completed declared probability input, verified C-04 cell table/artifact/QC contracts, and S7 binding lessons at clean base `ec9ca19` | Sole writer; no scalar/embedding/physical/receipt/graph/validator framework edit, serializer/result/config/CLI/node/codec, generic covariance/statistics abstraction, geometry/window/weights, inference, source promotion, remote action, push/publish/deploy/history rewrite |
| C-06-DECLARED-PROBABILITY-COVARIANCE-RESEARCH `c06_contract_map`, `c06_caller_map` | None | None | WS-50/WS-51 science and end-to-end caller audits at clean base `ec9ca19` | Read-only research only; no edit, test, review approval, remote access, or commit |
| C-06-DECLARED-PROBABILITY-COVARIANCE-REVIEW `c06_caller_map` | None | None | Final C-06-S8 production/test/docs diff after focused green | Sole read-only binding/numerical/resource/claim review; no edit, test, commit, remote access, or second review pass |

## C-06 declared binary cell-centroid project workflow completed ownership

| Task/agent | Writable files | Owned canonical symbols | Read-only context | Explicit non-goals |
|---|---|---|---|---|
| C-06-DECLARED-CELL-CENTROID-WORKFLOW `/root` | New focused `src/cell_embedding_mark_workflow.rs`; narrow crate-private S7 binding/reattachment seam in `src/cell_embedding_mark.rs`; one helper visibility in `src/workflow.rs`; root facade export; one new child module plus one parent declaration in the CellViT integration target; `C-06-S9` task contract plus DEC-0047/IC-0021 and affected milestone ledgers/handoff | `DeclaredBinaryCellEmbeddingCentroidNode`; private `MLCBCENT` cache codec v1; narrowly named crate-private S7 binding snapshot | Completed S7 computation, project/workflow/store boundaries, read-only S9 caller/codec audits at clean base `45a203e` | Sole writer; no embedding/scalar/physical/receipt/graph/validator framework edit, general serializer/result/codec abstraction, AnalysisEngine/config/result-0.3/CLI change, new dependency, source promotion, geometry/window/weights, inference, remote action, push/publish/deploy/history rewrite |
| C-06-DECLARED-CELL-CENTROID-WORKFLOW-RESEARCH `c06_caller_map`, `c06_contract_map` | None | None | End-to-end caller, exact codec, scheduler/store, fixture, and Immediate-Caller audits at clean base `45a203e` | Read-only research only; no edit, test, review approval, remote access, or commit |
| C-06-DECLARED-CELL-CENTROID-WORKFLOW-REVIEW `c06_s9_review` | None | None | Final C-06-S9 production/test/docs diff after focused green | Sole read-only security/cache/binding/codec review; no edit, test, commit, remote access, or second review pass |
