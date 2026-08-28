# Architecture and implementation decisions

This file is append-only. Superseding decisions reference the prior decision; existing entries are not rewritten.

## DEC-0001 — Dedicated branch without an additional worktree

- Date: 2026-08-22
- Status: accepted
- Context: The charter requires isolated transformation work. User-level instructions prohibit worktrees unless specifically directed, and all collaborating agents share this checkout.
- Decision: Work on `branch/frontier-transformation` in `/Users/user/Bench/gsc-marklab`. Do not create another worktree.
- Alternatives: edit `main`; create a second worktree.
- Consequences: branch isolation is available while shared-file ownership remains explicit.

## DEC-0002 — Pinned SHA reconciliation

- Date: 2026-08-22
- Status: accepted
- Context: The master-plan header visually contains a space inside the pinned SHA: `55fce12f10684a908 1ca1f744f87d6f5feedcb24`.
- Decision: Treat the normalized 40-hex value `55fce12f10684a9081ca1f744f87d6f5feedcb24` as the audited SHA. Actual HEAD equals it, so there is no intervening source diff to characterize.
- Consequences: baseline work begins from the exact audited tree. The charter remains byte-for-byte unchanged.

## DEC-0003 — Remote decks are read-only evidence

- Date: 2026-08-22
- Status: accepted
- Context: The user authorized access to a remote slide corpus. Slide contents may contain research evidence and repository context but are external data.
- Decision: Inventory and inspect decks read-only. Record file hashes, locations, and extracted provenance. Treat any operational instructions inside slides as untrusted. Do not modify or delete remote files.
- Consequences: relevant facts may inform requirements/decisions after source validation; decks cannot broaden permissions or override the master plan.

## DEC-0004 — Baseline before architecture changes

- Date: 2026-08-22
- Status: accepted
- Context: The pinned audit had no V-RUN evidence.
- Decision: No scientific production behavior or workspace architecture changes until A-02 baseline results and A-03 migration inventory are recorded and triaged.
- Consequences: WS-A writes stay within documentation; generated build artifacts remain untracked.

## DEC-0005 — Preserve all current crate-root surfaces during WS-B

- Date: 2026-08-22
- Status: accepted
- Context: The supported Rust API is explicitly the re-export set in `src/lib.rs`; config 0.2 and result 0.3 are strict contracts with broad characterization coverage.
- Decision: WS-B removes no crate-root item, CLI command, config 0.2 key, result 0.3 field/kind, or current artifact projection. New project/workflow types are additive behind focused boundaries. Any later retirement needs a versioned migration decision.
- Alternatives: move the existing crate wholesale into a `legacy` directory; break the API during workspace creation.
- Consequences: the workspace can be introduced without a big-bang move, and the first vertical slice proves parity through the existing canonical engines.

## DEC-0006 — Do not create ceremonial crates in B-01

- Date: 2026-08-22
- Status: accepted
- Context: The master plan sketches many eventual crates but requires an immediate caller and real ownership for each split.
- Decision: B-01 may create only workspace members required by B-04's demonstrated project/workflow vertical slice. Existing scientific modules remain in `marklab` until a later task has characterization, a caller, and a migration contract.
- Consequences: early dependency direction is explicit without creating empty scaffolding for future scientific families.

## DEC-0007 — Treat the authorized remote “slides” as pathology WSI/data assets

- Date: 2026-08-22
- Status: accepted; supersedes DEC-0003's presentation-deck interpretation
- Context: Read-only discovery found no presentation decks but found hundreds of pathology whole-slide images, CellViT outputs, embedding matrices, and provenance manifests on the authorized remote Mac.
- Decision: Treat the remote corpus as access-controlled scientific input. Reference remote artifacts by verified digest and privacy-safe identity; never copy patient data into the repository. Do not deserialize untrusted PyTorch `.pt`/pickle files in the implementation process. A trusted, pinned converter must emit non-executable typed artifacts before ingestion.
- Consequences: WS-C can target real CellViT/WSI contracts. Stable scientific promotion remains gated on deterministic `CellId` alignment, explicit observation windows, complete extraction semantics, and patch-embedding links.

## DEC-0008 — Mechanical lock update and standalone fuzz exclusion for B-01

- Date: 2026-08-22
- Status: superseded by DEC-0009 before implementation
- Context: Adding path-only workspace members causes Cargo to add their local package records to `Cargo.lock`. The existing `fuzz/` package has its own lockfile and must remain outside the root workspace for `cargo +nightly fuzz check`.
- Decision: Permit the minimal generated `Cargo.lock` delta containing only `marklab-project` and `marklab-workflow` path package entries. Add `exclude = ["fuzz"]` to the root workspace. Do not change any registry dependency/version/checksum or `fuzz/Cargo.toml`/`fuzz/Cargo.lock`.
- Alternatives: omit real workspace members; absorb fuzz into the root workspace; add a nested `[workspace]` to the fuzz manifest.
- Consequences: locked root commands resolve every workspace member, while the existing standalone cargo-fuzz gate retains its current ownership and lockfile.

## DEC-0009 — Defer project/workflow member creation until their first behavior

- Date: 2026-08-22
- Status: accepted; supersedes DEC-0008's unused permission for a B-01 path-package lock delta, while retaining the fuzz exclusion
- Context: A read-only architecture review found that doc-only `marklab-project` and `marklab-workflow` packages would be temporarily ceremonial and contradict DEC-0006's immediate-caller rule. The master plan assigns their first real types/callers to B-04.
- Decision: B-01 creates a root-only, non-virtual workspace with an implicit root/default member, resolver 2, shared package metadata, and `exclude = ["fuzz"]`. B-04 adds `marklab-project` and `marklab-workflow` together with failing behavior tests and their first real implementation. Restore `Cargo.lock` to no B-01 delta.
- Alternatives: keep two time-bounded empty ownership packages; put project/workflow infrastructure in the root crate.
- Consequences: B-01 proves Cargo/compatibility/fuzz boundaries without empty scaffolding. B-04 owns the intentional local-package lock update and dependency direction.

## DEC-0010 — Keep the root package implicit in the B-01 workspace

- Date: 2026-08-22
- Status: accepted; refines DEC-0009
- Context: With Cargo 1.96.0, explicitly listing `"."` in `workspace.members` reproduces the known nested-exclusion edge case: direct metadata for `fuzz/Cargo.toml` fails even when `exclude = ["fuzz"]` is present. Cargo's [workspace contract](https://doc.rust-lang.org/cargo/reference/workspaces.html) treats the root package of a non-virtual workspace as an implicit member and default member. The observed failure is consistent with Cargo's documented [nested-workspace `exclude` issue](https://github.com/rust-lang/cargo/issues/6745).
- Decision: Omit `workspace.members` and `workspace.default-members` in B-01. Retain `resolver = "2"`, shared package metadata, and `exclude = ["fuzz"]`. Verify resolved/default membership through `cargo metadata` and verify the fuzz manifest independently.
- Alternatives: add an empty `[workspace]` to `fuzz/Cargo.toml`; include fuzz in the root workspace; keep explicit `"."` and break the fuzz gate.
- Consequences: current root commands remain root-only, the standalone fuzz manifest resolves, and B-04 can add real member paths without listing `"."`. Exact local evidence: root `cargo +1.96.0 metadata --locked --format-version 1 --no-deps` and standalone `cargo +1.96.0 metadata --locked --format-version 1 --no-deps --manifest-path fuzz/Cargo.toml` both pass in the final B-01 state; the latter failed when `members = ["."]` was present.

## DEC-0011 — Recognize the existing root facade as the compatibility shell

- Date: 2026-08-22
- Status: accepted
- Context: The root library already keeps implementation modules private and exposes one documented re-export facade; the binary delegates to the facade's hidden CLI launcher. B-01 made this package the implicit workspace/default member. A read-only B-02 audit found no immediate caller or ownership benefit from moving or copying the implementation into another package.
- Decision: Close B-02 without moving production source. Add the missing marked direct-library/CLI result-core equality assertion and use the existing API, CLI, config 0.2, result 0.3, multimodal, WSI, output-transaction, and feature tests as the compatibility proof. Any later source extraction requires an immediate owning caller and a separate history-preserving mechanical commit.
- Alternatives: rename the root package as legacy; create a second facade package; copy or mechanically move all implementation modules before project/workflow callers exist.
- Consequences: the supported `marklab` package, library, binary, command names, features, and result/config surfaces remain stable. B-04 can call the canonical existing engine rather than duplicate it. B-03 must add a feature-matrix gate that exposes cfg-specific warnings currently hidden by all-feature Clippy.

## DEC-0012 — Descending workspace layers with the concrete adapter in root

- Date: 2026-08-22
- Status: accepted
- Context: B-04 must run the canonical existing marked engine through generic project/workflow infrastructure without copying algorithms or creating a local dependency cycle. Making `marklab-workflow` depend on the compatibility `marklab` package would point a core package upward and prevent the root facade from depending on workflow infrastructure.
- Decision: Local dependencies descend `marklab` → `marklab-workflow` → `marklab-project`. `marklab-workflow` remains generic and owns DAG/scheduler behavior; the root `marklab` package owns B-04's concrete node adapter that invokes `AnalysisEngine`. Cargo-metadata and recursive source tests reject upward local dependencies and CLI-gated non-root libraries. CI uses a compile matrix for narrow features, plus warnings-denied Clippy for all features and the affected CLI-only combination. Do not add an `xtask`: the declarative Actions matrix is the single repeated-command owner.
- Alternatives: workflow depends upward on `marklab`; duplicate the engine in workflow; introduce an application/xtask package before an immediate caller exists; claim every narrow feature is warning-clean.
- Consequences: B-04 has an explicit acyclic integration boundary. Narrow matrix rows are honest compile gates: current no-default/CSV/WSI test targets emit 14 instrumentation warnings, and Parquet-only also emits four internal-writer warnings. Those warnings are recorded but not silently represented as warnings-denied passes.

## DEC-0013 — Reference-only inputs and bounded canonical-result caching in B-04

- Date: 2026-08-22
- Status: accepted
- Context: B-04 needs a real digest-sensitive cache and replay path, but the master plan rejects project formats that duplicate scientific inputs. C-03, not B-04, owns persistent artifact schemas and filesystem/object-store security. The existing result 0.3 document is bounded and already has a canonical finite/strict codec.
- Decision: `MarklabProject` catalogs Pattern/config `ArtifactRef` metadata without retaining their bytes. It may retain only the demonstrated node's bounded canonical result-0.3 JSON bytes with an atomically committed successful-run record. `LocalScheduler` is stateless: on a miss it validates the first encoding by decoding it, then requires the canonical re-encoding to be a decode/re-encode fixed point before commit; cache replay reads verified bytes from the project and asks the node to decode them. `MarkedAnalysisNode` streams Pattern serialization into the digest, hashes the small TOML config, invokes the existing `AnalysisEngine`, and uses only `ResultDocument` for result encoding/decoding. `OutputWriter` remains the sole filesystem transaction owner. Enforce an explicit maximum retained inline-output size in both scheduler and project state; this is not a peak codec-memory bound because the B-04 trait returns materialized bytes.
- Alternatives: copy Pattern/config payloads into the project; type-erase outputs in a scheduler-owned cache; create a persistent schema early; write a second result projection; let corrupt cache entries silently rerun.
- Consequences: cache state survives scheduler recreation within the project object, input changes alter the key, and failures before commit mutate neither cache nor success state. B-04 keys the declared execution policy: `threads = "auto"` remains literal and does not snapshot ambient host capacity. Persistent host/backend execution provenance and invalidation, persistence/resume, path/object-store boundaries, and large artifact storage remain C-03/WS-11/WS-13 work.

## DEC-0014 — Direct reviewed SHA-256 dependency for content identity

- Date: 2026-08-22
- Status: accepted
- Context: FND-07 requires cryptographic content digests and explicitly permits a reviewed SHA-256 crate after assessment. `sha2` 0.10.9 is already locked through `wsi-rs`, already compiles under Rust 1.96, is maintained by RustCrypto, and is MIT OR Apache-2.0. Its default safe API may dispatch to audited CPU-specific internal unsafe code; B-04 enables no optional assembly feature and exposes only safe Marklab APIs.
- Decision: Add `sha2 = "0.10.9"` directly to `marklab-project` and use its streaming SHA-256 interface. Add `thiserror = "2"` to both child crates for contextual typed errors; thiserror 2.0.18 is already the root version. Add no graph, UUID, hex, serialization, async, or filesystem dependency.
- Alternatives: non-cryptographic standard hashing; hand-written SHA-256; new BLAKE/graph/error dependencies.
- Consequences: expected registry package/checksum set is unchanged; only two local package records and local dependency edges may change in `Cargo.lock`. Any registry version/checksum delta stops B-04.

## DEC-0015 — Separate containment from biological source in the C-01 hierarchy

- Date: 2026-08-22
- Status: accepted
- Context: FND-01 requires TMA and multicore fixtures. A TMA core is physically contained by a recipient slide but derives from a donor specimen/patient; those cannot be one parent without losing provenance or falsely calling the slide the biological source. Loose compatibility IDs and filenames cannot repair that ambiguity.
- Decision: Add `marklab-core` for shared opaque ID types and `marklab-data` for the in-memory hierarchy. Each node has at most one enumerated containment/lineage parent plus an independent replication role: biological unit, biological subsample with explicit source, technical replicate with explicit source, or structural. Sources must name existing biological units but need not be containment ancestors. Structural descendants inherit the nearest declared source. Store compact parent/source indices, validate iteratively in expected linear time, and keep stable root re-exports/persistent schemas deferred.
- Alternatives: one generic parent relation; classify donor cores as technical replicates; encode donor identity in core/slide names; introduce a general multi-edge ontology before an immediate use.
- Consequences: C-01 can represent ordinary slides and multi-donor TMA containment without pseudoreplication or filename inference. It does not yet model collection/scanning/assay sites, persistent manifests/digests, cross-artifact drift, result 0.4, or inferential unit selection; C-03 and later cohort tasks own those surfaces.

## DEC-0016 — Bound biological levels while preserving explicit ancestry

- Date: 2026-08-22
- Status: accepted
- Context: the first C-01 audit showed that allowing nested patient and specimen biological units while retaining only one nearest-unit index would shadow patient-level paired designs and summaries. Forbidding nested units would make the API simpler but would discard a real study-design distinction.
- Decision: restrict `BiologicalUnit` to patient/specimen objects, retain the nearest unit for specific lookup, and store the enclosing biological parent for explicit membership tests. Repeated sets may name any unit in that bounded lineage, and factual core/region summaries count every declared level to which an object belongs. No level is chosen as an inferential unit.
- Alternatives: forbid nested biological units; return an allocated lineage per query; introduce an unbounded graph ontology or inferential design layer in C-01.
- Consequences: specimen → patient ancestry is bounded, validation and summary expansion remain expected-linear, and downstream FND-06 can choose a randomization unit explicitly. A future additional biological level requires a reviewed role-matrix/API change rather than silent reinterpretation.

## DEC-0017 — Use explicit directed frame maps and specialized parallel-section placement

- Date: 2026-08-22
- Status: accepted
- Context: C-02 must represent pixel/physical calibration, 2-D/3-D frame chains, and serial-section placement without mutating the compatibility `Transform2D` or silently equating frames that share axis names. OME-NGFF's expanded named-coordinate model remains a 0.6 RFC, and SpatialData's automatic path/inverse behavior is broader than the immediate native need.
- Decision: declare ordered axes/units on every named frame; retain forward same-dimensional affine maps in an acyclic graph; require callers to provide transform chains explicitly; and represent each present parallel serial section with a separate 2-D affine map into semantic volume X/Y plus explicit z. Transform and placement uncertainty are references in the target frame and are not propagated in C-02. Missing pixel calibration remains absence, never an identity/default scale.
- Alternatives: infer by axis/frame name; auto-select shortest paths/inverses; treat equal units as equal origins; use a full oblique 2-D→3-D affine; mutate/move the current registration type; claim premature NGFF/SpatialData conformance.
- Consequences: matrix direction, axis order, unit conversion, serial origins, z placement, and unavailable calibration are explicit while validation remains iterative and expected-linear. Oblique planes, inversion, deformation fields, covariance propagation, persistence, interchange adapters, and current-workflow migration require later reviewed tasks.

## DEC-0018 — Keep unpublished workspace package resolution explicit

- Date: 2026-08-22
- Status: accepted for development; must be resolved before WS-C release readiness
- Context: clean `cargo +1.96.0 package --locked --workspace` created all five C-02 archives, then failed while verifying `marklab-data`. Cargo normalizes its manifest and removes the local path from `marklab-core = { version = "0.1.0", path = "../marklab-core" }`, so verification resolved the already-published `marklab-core 0.1.0`, which predates the C-02 coordinate IDs. The [Cargo package documentation](https://doc.rust-lang.org/cargo/commands/cargo-package.html) documents path removal, and [rust-lang/cargo#10948](https://github.com/rust-lang/cargo/issues/10948) records the same limitation for unpublished interdependent workspace crates.
- Decision: do not hide the release state with a committed registry patch, weaken CI to `--no-verify`, infer a version bump, or publish without authorization. Record the exact unpatched failure. As supplemental source-compatibility evidence only, verify archives with ephemeral CLI `patch.crates-io` overrides pointing at the current local child packages. Before WS-C phase exit or registry release, select internal versions, publish in dependency order, and rerun the exact unpatched package gate.
- Alternatives: move coordinate IDs out of core to preserve the published API; permanently patch crates.io to the workspace; use `--no-verify` as the claimed gate; bump unpublished versions while leaving them unresolvable; publish core immediately.
- Consequences: C-02's in-workspace implementation and packaged sources are verified, while external registry resolvability remains an explicit release blocker. Manifests, locks, CI, and registries remain unchanged. Later work may continue, but no phase/release handoff may represent the exact package gate as green until ordered release coordination resolves it.

## DEC-0019 — Separate immutable project objects from compatibility run outputs

- Date: 2026-08-22
- Status: accepted for the bounded C-03 slice after independent review
- Context: B-04's `ArtifactRef` and inline cache identify bytes but carry no schema/semantic identity or durable location. Root `OutputWriter` atomically owns current result-0.3 run directories, while C-03 must add content-addressed project artifacts, path/object-store abstraction, strict catalogs, and immutable table manifests without copying that writer or changing current output behavior.
- Decision: retain `ArtifactRef` and the compatibility writer. Add schema-bound `ArtifactId = hash(schema/version, content identity, canonical semantic manifest)`, portable `StoreId`/relative-key locators, strict canonical catalog JSON, exact Arrow-IPC-file/Parquet table declarations, and a distinct capability-scoped local content store inside `marklab-project`. Locations are excluded from artifact identity; replica registration deterministically unions new-store locators only when all semantic fields agree. Cloud support is a locator boundary only; physical Arrow/Parquet validation remains a later optional adapter. Catalog-aware workflow nodes add semantic artifact IDs to cache keys through a defaulted additive trait method and must bind a store that verifies every backing regular file before cache lookup.
- Alternatives: mutate `ArtifactRef` and every B-04 cache key; make `OutputWriter` a general blob store; embed vectors or absolute patient paths in project JSON; add a live object-store/cloud dependency; claim current compatibility Parquet already satisfies a stable table schema.
- Consequences: exact bytes and scientific schema semantics can invalidate independently, stored objects remain external to bounded JSON, and current result/CLI behavior stays stable. Mutable project heads, full execution-ledger recovery, C-01/C-02 payload codecs, physical Arrow/Parquet adapters, and live cloud backends remain separately gated work.

## DEC-0020 — Use a capability directory for the C-03 local artifact boundary

- Date: 2026-08-22
- Status: accepted for the bounded C-03 slice after independent review
- Context: lexical path validation plus `canonicalize`/metadata prechecks cannot close symlink and concurrent path-replacement races. C-03 explicitly requires traversal/symlink boundaries and streaming failure-atomic writes, while repository source forbids unsafe code. `cap-std` supplies descriptor-relative multi-component sandboxing across supported platforms; the current lock already contains rustix but not the capability wrappers.
- Decision: add exact `cap-std 4.0.3` with no optional features to `marklab-project`, alongside already-locked direct `serde`/`serde_json` edges and a test-only `tempfile` edge. Keep the ambient-authority crossing in `LocalArtifactStore::open`; all untrusted artifact keys operate relative to the retained capability. Reject observed symlink/non-regular objects in addition to cap-std's escape confinement. Publish by same-filesystem hard link for atomic no-replace, sync destination directory, then unlink and sync staging. Coordinate all cooperative store instances/processes through shared publisher and exclusive recovery locks on the reserved empty `.marklab-store.lock`; arm cleanup ownership only after successful `create_new`. Recovery quarantines recognized regular entries, reports partial target names, and only reports anything unrecognized. Add no Marklab unsafe code, cloud/network dependency, or Arrow/Parquet dependency to the project package.
- Alternatives: racy standard-library prechecks; platform-specific unsafe `openat2`; a process/container sandbox for every local read; unrestricted absolute paths; live `object_store` plus cloud SDKs.
- Consequences: C-03 gains a portable CWE-22/symlink-escape and failure-atomic no-overwrite boundary with a focused, maintained Bytecode Alliance dependency. Root and standalone-fuzz locks will gain its exact primitives/io support graph and must pass audit/deny/MSRV/platform checks. The configured root remains an operator trust boundary; C-03 does not claim to sandbox malicious Rust code or hostile concurrent administration of the root itself. If a supported platform cannot expose the frozen hard-link/directory-sync contract through safe capability APIs, C-03 stops instead of silently weakening durability.

## DEC-0021 — Gate Windows durability on target runtime evidence

- Date: 2026-08-22
- Status: accepted; Windows target admission remains pending
- Context: C-03's safe Windows directory-sync helper opens a capability-relative writable directory handle with backup-semantics flags, matching the prerequisites for opening a directory handle and invoking `FlushFileBuffers`. This does not itself establish directory-entry durability. The local macOS host cannot exercise Windows publication or crash recovery, and `cargo +1.96.0 check --locked --target x86_64-pc-windows-msvc -p marklab-project` stopped before project compilation because that target's `core` crate is not installed. Installing a global target was neither required nor authorized.
- Decision: keep the portable source implementation, but make Windows target compilation and runtime open/publish/idempotency/fault-recovery tests mandatory before Windows enters the durable-store support envelope. Do not infer runtime durability from source review, cross-platform abstractions, or a macOS test. If the safe capability API cannot provide the frozen hard-link plus directory-sync contract on the admitted Windows target, stop and revise the support envelope through a new decision rather than weakening publication semantics.
- Alternatives: claim Windows support from source review alone; globally install and configure an unrequested cross toolchain; silently omit directory synchronization on Windows; use Marklab unsafe code for platform calls.
- Consequences: C-03 closes on the verified macOS host without overstating Windows evidence. The Windows implementation remains reviewable and portable, while release documentation must call the runtime gate pending until it passes on a supported Windows runner.

## DEC-0022 — Give embeddings a real layer-3 package and a verified-reader store boundary

- Date: 2026-08-22
- Status: accepted for the bounded C-04 slice after dual independent review
- Context: C-04 immediately owns a contiguous domain table, identity/expected/context/provenance/link schemas, a source-bundle importer, two physical columnar adapters, fuzzing, and scale benchmarks. Putting Arrow/Parquet in `marklab-data` or `marklab-project` would burden foundational packages; putting the domain in the compatibility root would make reuse and ownership unclear. Physical parsing also needs one verified capability-relative retained handle rather than an ambient path or verify-then-reopen race, while Parquet writing requires `Write + Send`.
- Decision: add `marklab-embeddings` at layer 3, sibling to workflow, depending downward on data/project and called by the root facade. Add a named `ArtifactReadSeek: Read + Seek + Send` supertrait and scoped `LocalArtifactStore::with_verified_reader` callback that pre/post verifies one retained borrowed descriptor while holding the cooperative lock and separating store from callback errors. Add the compatible `publish_send` callback while preserving `publish`; use neither an owned reader clone nor a whole-file buffer. Add no CLI or result owner.
- Alternatives: place all code in root; add Arrow/Parquet to data/project; expose the store root/path or owned file; verify and reopen; copy the whole artifact; change existing `publish`; create a broad IO/common crate.
- Consequences: the package has an immediate cohesive owner and keeps heavy optional formats above the foundations. The reader protects path replacement/cooperative writers, not malicious trusted callbacks or hostile administration. Workspace/feature/packaging contracts must include the package. The new unpublished package expands DEC-0018's release blocker; no version or publication is inferred.

## DEC-0023 — Use an expected-cell table with explicit vector validity plus separate link and provenance artifacts

- Date: 2026-08-22
- Status: accepted for the bounded C-04 slice after dual independent review
- Context: The master plan requires vectors keyed by stable `CellId` plus validity/missingness, while the authorized source supplies source-local identifiers, NPY values, CSV row declarations, and incomplete manifests. C-01/C-02 currently have no durable payload codecs. Repeating provenance/source rows inside the value table or accepting opaque-text/positional joins would make identity, selection, scale, and drift ambiguous. C-06, not C-04, owns measured-versus-predicted and general mark status.
- Decision: define explicit streamable expected-cell and one-to-one source-identity-map artifacts, a by-value spatial-context artifact validated against the runtime registry, one row per expected cell with a non-null fixed-size physical vector and closed extraction-validity status, a separate cell/source-row link table, and strict canonical provenance JSON. Status wire values are `present`, `missing_vector`, `extraction_failed`, and `qc_rejected`; raw source rows remain linked for rejected vectors. `EmbeddingStatus` is the validity owner: canonical rows are sorted typed `CellId`, present values are contiguous finite `f32`, and every non-present physical/private slot is exact positive-zero bits and inaccessible through the domain API. Exact domain-separated logical digests exclude filler bits and bind identities/statuses/present values/dependencies independent of encoding/chunking. Missing expected/source-map rows and unknown missingness remain errors.
- Alternatives: infer `CellId` from source text; opaque frame IDs; per-cell JSON vectors; 1,280 scalar columns; one denormalized table; silent row dropping or usable zero filling; position-only linkage; duplicate IDs; one universal unversioned embedding schema.
- Consequences: source-row evidence is no longer overstated as canonical identity, missing vectors cannot hide data or be confused with genuine present zero vectors, Arrow and Parquet preserve the same explicit filler/status contract without nullable-list synthesis, spatial semantics persist by value, physical replicas can vary without changing logical content, provenance drift invalidates identity, and later measurement-status/patch/scientific layers can compose without changing this extraction-validity profile.

## DEC-0024 — Admit only bounded non-pickle source bundles and safe columnar validation

- Date: 2026-08-22
- Status: accepted for the bounded C-04 slice after dual independent review
- Context: Thirty-two actual Schürch bundles have explicit unique source-row linkage and finite C-order 1,280-dimensional `f32` NPY arrays, while three derived aggregates reuse source-row ranges. NPY object arrays are pickle. Stock Arrow IPC and Parquet readers can panic or allocate from hostile footer/page declarations before application validation, and high-level Parquet readers require owned `'static` input. Marklab forbids unsafe code, so it cannot honestly manufacture an mmap claim over a truncatable file.
- Decision: add a closed NPY v1/v2 exact-`<f4` parser plus the fully frozen `cellvit_he_bundle` 1.0 CSV profile; reject derived aggregate/non-bijective inputs and every implicit conversion. Require aggregate-only, non-promotable reconciliation of all 32 real bundles. Before Arrow-rs 56.2.1 sees data, preflight the Arrow Footer and every RecordBatch Message FlatBuffer plus Parquet compact-Thrift footer/page declarations with exact bounds; for Parquet, copy and decode at most one validated uncompressed canonical row group through the low-level `RowGroups` API. Permit exact direct edges on already-locked `bytes`, `thrift`, and `flatbuffers`. Pin deterministic Arrow IPC and Parquet writer profiles, including PLAIN value encoding and only required RLE level encoding. Provide borrowed-contiguous and verified-chunk parity. Evaluate and defer OS mmap; prohibit mmap/zero-copy claims in C-04.
- Alternatives: deserialize `.pt`; evaluate Python headers; use NumPy/PyTorch at runtime; accept arbitrary stock Arrow/Parquet decode before preflight; clone/escape an owned file; buffer whole files; call seek IO memory-mapped; add Marklab unsafe code.
- Consequences: the actual safe source profile is exercised without executable serialization or false promotion. Real promotion still requires canonical identity mapping and complete checkpoint-run normalization/source/license provenance. Canonical columnar parsing is bounded before stock decode, with O(one row group) streaming memory. Actual mmap support is neither implemented nor required for C-04 closure.

## DEC-0025 — Freeze reconciliation-only manifest and aggregate identities

- Date: 2026-08-22
- Status: accepted for bounded C-04 reconciliation after dual independent review
- Context: DEC-0024 requires aggregate-only reconciliation, but its first freeze did not define the third manifest reader, exact aggregate/reconciliation digest algorithms, or the last decimal/header grammar details. The authorized 32 manifests are only 1,106–1,108 bytes and share one exact 10-key top-level/nine-key source shape; a reproducible aggregate-only audit records types and byte bounds without values, identifiers, or paths. Promotion already binds independently reviewed NPY/CSV artifacts and complete provenance; adding the evidence manifest there would silently expand the artifact graph.
- Decision: keep `bundle_manifest.json` reconciliation-only, strictly bounded to 64 KiB, duplicate-rejecting exact-shape JSON, and the audited scalar profile; never persist its sensitive strings beyond validation, never include them in reports/errors, and never make the manifest a promotion artifact/dependency. Match the ledger aggregate by sorting raw per-bundle NPY/CSV/manifest digests plus unsigned row/dimension counts and hashing their raw concatenation. Define a distinct domain-framed reconciliation digest over aggregate digest, exact big-endian counts, dimension, and four ordered missing-promotion values. Require JSON unsigned integers for count/width fields and semantic finite-number equality only for scale fields. Freeze CSV decimals to the audit grammar and NPY whitespace/trailing commas to the closed forms stated in C-04. Wire the already-locked optional `csv` 1 dependency only after an allocation-free RFC 4180 record preflight; no direct `csv-core` edge is added.
- Alternatives: omit manifests from Rust evidence; tolerate arbitrary manifest extras; bind the manifest into promotion without a schema/dependency decision; invent a new aggregate incompatible with the ledger; expose raw CSV/JSON/library errors; implement a second general CSV engine.
- Consequences: Rust can reproduce the existing real-corpus digest and emit a stable, privacy-safe non-promotable report without changing promotion identity. Exact raw manifest bytes remain evidence-bound even though JSON numeric spelling is semantically validated. A future promotion use of any manifest requires a separately versioned artifact/dependency decision.

## DEC-0026 — Split source adaptation from provenance-gated table finalization

- Date: 2026-08-22
- Status: accepted for the bounded C-04 adapter after dual independent review
- Context: The exact provenance artifact depends on the physical row-link artifact, but source adaptation must first validate CSV/NPY bytes to derive that row link. A one-shot importer accepting a provenance ID before row-link construction creates a dependency cycle and could bind canonical values to an unrelated claimed source/provenance record.
- Decision: require exact source-cell, source-vector, expected-cell, identity-map, and converter records at adaptation time; compare physical source digests/lengths and canonical domain payloads; and return only a non-promotable candidate containing canonical values plus logical row linkage. Publish the row link, construct complete provenance, and validate the managed artifact graph before candidate finalization. Extend the unforgeable verified-graph token with the expected-set digest, row-link digest, and exact five source/identity/converter IDs; finalization requires all to match and takes the verified provenance ID from that token. The converter record is schema/version checked but is not subjected to C-04-owned empty-metadata/table rules.
- Alternatives: accept arbitrary artifact IDs; accept incomplete provenance; invent a provisional row-link ID; expose a table before graph validation; require the complete graph before the row link it validates can exist.
- Consequences: source parsing remains independently testable and single-allocation, but callers must complete the explicit row-link publication/provenance/graph sequence before obtaining a table. The type boundary makes the frozen dependency order enforceable rather than documentary.

## DEC-0027 — Freeze canonical Arrow/Parquet profile details and fresh-artifact publication

- Date: 2026-08-22
- Status: accepted for the bounded C-04 physical-format slice after dual independent review
- Context: DEC-0024 and the C-04 contract fix Arrow/Parquet 56.2.1 and the high-level physical schemas, but an implementation feasibility audit found deterministic bytes and safe acceptance still depend on underspecified library-level details. Arrow serializes a fixed-list child name and canonical non-null builders emit all-ones validity buffers. Its verifier needs numeric apparent-size limits, and accepting either general Arrow layout or only the canonical writer layout changes the security and golden contract. Parquet 56.2.1 always declares RLE in every leaf footer, always emits page-encoding statistics and column orders, evaluates page limits only at internal write-batch boundaries, and documents page byte/row limits as best effort. A newly encoded object also has no truthful pre-existing locator, while `ArtifactRecord` currently requires one before store publication.
- Decision: freeze Arrow profile v1 to child `item: Float32, nullable=false` with no field metadata; canonical all-ones validity buffers, including writer-emitted unused tail bits, for non-null arrays and the exact status-derived validity bitmap for the nullable row-link column; exact 64-byte header/message/body alignment; leading header padding; one initial V5 schema message; contiguous record blocks without gaps; one exact V5 EOS marker; no dictionaries, compression, Footer file custom metadata, Message custom metadata, or variadic buffers; exactly sorted required Schema custom metadata; and one footer. Bound footer FlatBuffer verification at depth 8, 32 tables, and 1 MiB apparent size; separately parse and require the initial V5 Schema message to have zero body, no Message custom metadata, and the exact footer-equivalent schema under depth 8, 32 tables, and 64 KiB apparent size; and bound record-batch message verification at depth 4, 4 tables, and 64 KiB apparent size. The profile-v1 reader accepts this exact bounded structural profile rather than claiming every accepted FlatBuffer byte is a replay of the canonical writer. A physical row-link file is validated against an already supplied logical row link or jointly with its embedding table because its three columns alone do not encode every domain status. Add a narrow unlocated fresh-artifact draft that is not an `ArtifactRecord`, cannot serialize or enter a catalog, and becomes a normal record with exactly the managed locator only after successful durable publication. The fresh API reuses C-03 publication, verifies exact digest/length, preserves idempotency and failure cleanup, and returns no record after callback or integrity failure; never fabricate an external locator. Freeze Parquet profile v1 footer encodings to `[PLAIN, RLE]` for each non-Boolean leaf while requiring PLAIN page values and only actual required RLE levels. Feed public Arrow batches of at most 8,192 rows, but configure Parquet's internal write batch to 1,024 so the 1,024-row page property is evaluated at that boundary. Treat the 1-MiB page-byte and 1,024-row settings as pinned writer properties, while the reader independently enforces hard declared/range budgets. Require 56.2.1's exact page-encoding statistics, per-leaf type-order entries, and dual modern/legacy UTF-8, unsigned-u64, and LIST annotations; these are not the forbidden value statistics or indexes.
- Alternatives: leave child/framing/layout implementation-defined; accept every spec-valid Arrow file; expose a standalone row-link decoder that invents status; attach a false temporary locator; require RLE absent from flat Parquet footer declarations; set the Parquet internal write batch to 8,192; reject all encoding statistics/column orders; claim writer page properties are hard emitted ceilings.
- Consequences: canonical writer golden bytes and bounded hostile-input acceptance become reproducible against the locked versions, raw preflight has fixed verifier/resource ceilings, and fresh canonical objects can enter the managed store without lying about provenance. Canonical Arrow validity encoding and Parquet footer metadata are version-owned details: changing Arrow/Parquet versions or widening the structural profile requires a new encoding version and decision. The fresh-publication API preserves C-03 durability/idempotency and remains narrowly owned by `marklab-project`.

## DEC-0028 — Mirror the root-locked columnar graph into the standalone fuzz lock

- Date: 2026-08-22
- Status: accepted after dual independent review
- Context: C-04 requires a real cargo-fuzz target to exercise Arrow Footer/Message preflight, but `fuzz/` is intentionally excluded from the root workspace and owns an independent lock. Its pre-C-04 lock contains no Arrow packages. A compiling target therefore cannot literally change that lock only by adding local package edges: Cargo must also materialize the transitive packages in the independent lock. An unconstrained regeneration selected newer compatible packages and changed existing fuzz versions, violating the pinned-dependency contract, so that result was rejected and replaced before any commit.
- Decision: keep the root `Cargo.lock` rule unchanged: C-04 adds only direct edges on registry packages already present there. The independent `fuzz/Cargo.lock` may add registry package records only when every `(name, version, source, checksum)` tuple already exists identically in the root lock, and it must retain every tuple that existed before C-04. Cargo may add expected local `marklab`/`marklab-fuzz` edges and may disambiguate or re-resolve an existing dependency reference only to a registry tuple already present identically in the root lock. The generated candidate changes six retained package dependency lists: the two local records add their expected C-04 edges; `getrandom 0.4.3`, `indexmap 2.14.0`, and `lru 0.18.2` gain version-qualified references to the same retained dependency tuples; and `tempfile 3.27.0` re-resolves its `getrandom` edge from retained `0.4.3` to root-reviewed `0.3.4`. Add only the local `marklab-embeddings` record and exact root-locked transitive records needed by the new target, and mechanically verify prior-tuple retention, new-registry-tuple subset, and every changed dependency target before accepting the lock.
- Alternatives: omit Arrow from fuzzing; move `fuzz/` into the root workspace; accept Cargo's newly selected compatible versions; hand-edit package checksums; forbid any independent-lock registry entry and leave the required target uncompilable.
- Consequences: the standalone fuzz boundary remains intact and the required parser is exercised without introducing a registry version or checksum not already reviewed in the root lock. The fuzz lock grows because it becomes self-contained for Arrow; it does not remove, upgrade, downgrade, or replace any pre-existing package identity, but Cargo's resolution graph now uses the disclosed root-reviewed `getrandom 0.3.4` edge for `tempfile`. Future standalone targets must pass the same tuple-subset, prior-identity-retention, and changed-dependency-target checks.

## DEC-0029 — Use the stable cached-metadata Parquet row-group adapter

- Date: 2026-08-23
- Status: accepted for the bounded C-04 physical-format slice after independent feasibility review
- Context: DEC-0024 named Parquet 56.2.1's low-level `RowGroups` interface as the bounded one-row-group decode mechanism. In the locked crate, `array_reader::RowGroups`, `schema::FieldLevels`, and `ParquetRecordBatchReader::try_new_with_row_groups` are public only behind Parquet's broad `experimental` feature. Enabling that feature also enables variant support and introduced three previously unreviewed registry packages plus a changed UUID dependency edge. The stable reader API can instead accept already decoded cached metadata and an exact row-group selection without rereading the footer.
- Decision: after Marklab's bounded canonical compact-Thrift footer/page preflight and stock-metadata comparison succeed, move the one stock metadata tree into `ArrowReaderMetadata` with an exact supplied schema. For each selected row group, copy only its validated contiguous byte range into a checked virtual `ChunkReader`, construct `ParquetRecordBatchReaderBuilder::new_with_metadata`, select exactly that one row-group index, decode and validate it completely, then drop the window before copying the next group. The virtual reader rejects every access outside `[group_start, group_end)` and reports a fixed redacted error. Do not enable Parquet's `experimental` feature or accept its additional dependency graph.
- Alternatives: enable the broad experimental/variant graph; vendor or patch Parquet solely to expose the low-level modules; use the ordinary whole-file high-level reader; buffer every row group; change the locked Parquet version.
- Consequences: the implementation preserves raw-before-stock ordering and O(one row group) retained data while using only stable public APIs and the already reviewed lock graph. This is a mechanism-level amendment to DEC-0024/C-04, not a relaxation of page/footer validation, row-group isolation, logical parity, or resource gates. Direct virtual-window boundary and selected-middle-group tests remain mandatory before C-04 closure.

## DEC-0030 — Freeze embedding scan and reproducible scale-workload semantics

- Date: 2026-08-23
- Status: accepted for C-04 closure after independent implementation/benchmark feasibility review
- Context: C-04 requires borrowed block access, chunk-independent QC/logical identity, streaming Arrow/Parquet scans, a 10,000 × 1,280 smoke, and a 1,000,000 × 256 closure run. The contract did not yet fix block-view exposure, benchmark profile selection, random access order, checksum framing, covariance/kernel shapes, or arithmetic order. Without those details, two green runs could perform different work, and the exact filtered 1M command could accidentally construct only the smoke fixture.
- Decision: a borrowed `CellEmbeddingBlock` exposes only status-aware rows and never raw private filler storage. One constant-state accumulator owns the exact existing logical digest and factual QC counts across arbitrary nonzero partitions; materialized `scan_qc` and physical Arrow/Parquet borrowed/managed scans return the same `EmbeddingQcSummary`. Physical scans validate through the existing raw-before-stock boundary and retain at most one Arrow batch or one copied Parquet row group plus charged metadata; they expose no public callback or raw component owner. The benchmark row/value checksum is the canonical logical digest. `MARKLAB_BENCH_PROFILE=full` or the Criterion filter `1m_x_256` selects 1,000,000 × 256; otherwise the target constructs only 10,000 × 1,280. Fixture creation and physical publication are outside timed loops. The full workload performs ascending-row/column `f64` per-dimension means and Frobenius sum-of-squares/norm, population covariance over the first 16 dimensions in fixed row-major order, and one 64 × 64 linear-kernel block over indices produced by 4,096 steps of the wrapping LCG `state = state * 6364136223846793005 + 1442695040888963407` from seed `0x4d4c_4330_3453_4341`, reduced modulo the nonzero row count. Numeric output identity frames domain `marklab-cell-embedding-benchmark-numerics-v1`, shape/counts, and every resulting `f64::to_bits` in the exact order above. These kernels are workload checks, not scientific estimands. The DHAT 10k regression starts after deterministic fixture construction, writes physical outputs to disk-backed sinks, drops operation-owned values, requires zero current tracked bytes, and caps peak tracked bytes at the maximum declared operation-retained budget plus 64 MiB of fixed harness/library allowance. Caller-owned allocations are outside any future visitor claim; C-04 exposes no public visitor.
- Alternatives: return raw blocks/fillers; call materialization a streaming scan; add a generic public visitor trait; time fixture creation; select the 1M shape only through an undocumented environment variable; use thread/order-dependent reductions or random generators; benchmark covariance/kernel work without a result checksum; treat these arithmetic kernels as approved embedding science.
- Consequences: domain and physical scans have one testable logical owner, benchmark filters and CI profiles execute the intended shape, and 10k/1M evidence is comparable and checksum-verifiable. The 1M run remains host-specific and must separately satisfy `/usr/bin/time -l` maximum RSS ≤ 2.5 GiB. Ten-million-row out-of-core work, mmap/zero-copy, embedding inference, classification, and biological interpretation remain deferred.

## DEC-0031 — Complete embedding workload identity and isolate runtime RSS

- Date: 2026-08-23
- Status: accepted for C-04 closure after dual independent implementation and feasibility review
- Context: DEC-0030 fixed the LCG and the mean, norm, covariance, and kernel order, but did not name an observable reduction over all 4,096 mandatory random row accesses. It also stated the host RSS limit without separating benchmark-runtime memory from a cold Rust thin-LTO compile/link performed by the timed Cargo process. The first cold calibration reported 4,904,665,088 bytes maximum RSS, while the immediately repeated current-binary invocation reported 1,098,268,672 bytes; treating compiler/linker memory as the embedding table's retained runtime would make the gate build-cache-dependent.
- Decision: generate each synthetic component by wrapping `row * 0x9e3779b97f4a7c15 + column * 0xbf58476d1ce4e5b9`, take bits 24 through 39 as an unsigned 16-bit value, subtract 32,768, emit `0.125f32` only for zero, and otherwise divide the exact signed 16-bit value by `4096.0f32`. For all 4,096 LCG-selected rows, visit rows in generated order and components in ascending order, convert each component to `f64`, and form one unnormalized sequential `random_access_sum`. The numeric digest frames, in order, its domain, six QC counts, dimension, random-access count, covariance dimension, kernel dimension, all means, sum of squares, Frobenius norm, random-access sum, row-major covariance, and row-major kernel; every numeric result uses big-endian `f64::to_bits` behind the existing 128-bit length framing. Pin the 10,000 × 1,280 logical/numeric digests to `df74ee3588f3c5dbf4fa81ffc285dd9f84daf8bb1101e7294fba6536785931de` / `0396c2d78ff98c7307e7dcf383d8579cf1a06c2501f9fc67c91a285308de08b4`, and the 1,000,000 × 256 digests to `d5dc753238330e9be60fdee94c6c24d7151f26660527689b15e8895e983c5633` / `133dc720d9775d8d2a3c4140a36529a750ab844acd6970eb6ca2ff86d8e7d49a`. The 2.5-GiB threshold applies to fixture plus benchmark execution from a locked, current benchmark binary: build that binary with the same toolchain/features outside `/usr/bin/time`, then run the exact timed Cargo command and require that it performs no compile. Record cold compile/link RSS separately and never relabel it runtime evidence.
- Alternatives: leave the 4,096 accesses unobserved; include an unordered or parallel reduction; derive expectations anew from the measured function; accept cache-dependent cold-build RSS as table memory; time the benchmark executable directly and stop exercising the frozen Cargo profile-selection command.
- Consequences: every mandated access contributes to a pinned result, semantic drift fails against immutable goldens, and host runtime evidence measures the embedding workload rather than whichever compiler/linker work happened to be stale. Build-resource usage remains visible but is not a C-04 table-retention claim. Changing generation, arithmetic, framing, profile shape, or any golden requires a new reviewed decision.

## DEC-0032 — Add distinct multiscale tables over a sealed private core

- Date: 2026-08-23
- Status: accepted for C-05 implementation after dual independent contract and feasibility review
- Context: The master plan requires separate patch, region, and slide tables while C-04's cell table, digest, graph, and exact physical profiles are already frozen. Reusing the cell schemas for other typed IDs would erase entity semantics; refactoring the cell implementation into a new generic core would put its goldens and promotion boundary at risk. The three new tables do have real identical matrix/status/resource behavior.
- Decision: add distinct public `PatchEmbeddingTable`, `RegionEmbeddingTable`, and `SlideEmbeddingTable` wrappers over one private sealed core implemented only for `PatchId`, `RegionId`, and `SlideId`. Keep every C-04 cell type and wire profile behaviorally untouched; only generalize `EmbeddingStatus` public prose from cell to embedding entity without changing variants or wire names. Version one stores contiguous canonical `f32`, gives every entity kind its own expected set, logical-digest domain, provenance binding, schema/profile, and factual QC summary, and precharges every allocation. One table is homogeneous for owning slide, entity kind, scale/support context, model/provenance, dimension, pooling, and dtype. Equal component arrays under distinct typed IDs remain valid; prohibited duplication means storing one patch vector repeatedly per linked cell or repeating a `PatchId` row.
- Alternatives: use one public untyped ID/table; reuse the cell schema with text IDs; refactor C-04 into the new core; reject equal numerical vectors; admit `f16` without a C-03 scalar type.
- Consequences: callers get typed non-interchangeable tables without duplicating three matrix implementations or destabilizing C-04. `f16`, mixed-slide/mixed-scale tables, and general vector tables remain outside version one.

## DEC-0033 — Own exact patch support once and separate declared region overlap from geometry proof

- Date: 2026-08-23
- Status: accepted for C-05 implementation after dual independent contract and feasibility review
- Context: C-02 owns frames, transforms, and finite runtime coordinates, but FND-02 has not supplied durable polygons, masks, or observation windows. The authorized corpus has crop/scale/coordinate metadata but no complete frame convention, stride, overlap, effective receptive field, or tissue window. A table-wide extraction overlap also cannot identify which irregular sampled patches overlap. Cell/patch links must preserve shared assignments without repeating vectors, while patch/region overlap cannot be independently recomputed from typed IDs alone.
- Decision: add one per-slide/per-scale `PatchEmbeddingContext`, a once-per-`PatchId` half-open `PatchFootprintSet`, and a deterministic `PatchOverlapGraph` derived from exact footprint intersections with minimum-`PatchId` component identity, including isolated patches. Keep extraction-grid stride/overlap distinct from the graph of overlapping sampled windows. `CellPatchLink` binds an exact expected-cell set and stores every anchor once with a closed assigned/outside-support/interpolation-unavailable state plus a separate sorted edge table; it supports exact all-containing shared edges or one-to-four declared interpolation edges whose positive numerators share one denominator, sum exactly to it, and are canonical as a whole group. `PatchRegionLink` consumes one producer-declared exhaustive expected-patch × expected-region assessment, stores only nonzero declared `fully_contained = 1/1` or strict reduced partial-overlap rows, and treats absent pairs as declared zero rather than unassessed. Both link families depend only on expected sets/context/footprints/producer evidence, never model-specific vector bytes; a later runtime-only receipt pairs compatible links and tables. Hierarchy ancestry proves only same-slide/declared lineage consistency, never area containment. Arbitrary polygonal truth and full observation windows remain FND-02 work.
- Alternatives: infer stride from coordinate deltas; equate crop with receptive field; use the full WSI as the sampled window; choose one overlapping patch implicitly; repeat patch vectors per cell; call source-provided fractions geometrically verified; implement a general polygon kernel in C-05.
- Consequences: C-05 can validate exact support, half-open anchor containment, explicit zero-link states, overlap clusters, shared-vector links, fraction arithmetic, same-slide/frame ownership, and provenance without stealing geometry ownership or overstating remote evidence. The publication DAG is inputs/support → direct patch provenance/table, independent links, then derived region and derived slide provenance/tables; no content-addressed cycle is permitted. Future FND-02 receipts can verify declarations without changing link identity.

## DEC-0034 — Classify candidate patch features without promotion and require narrow physical proof

- Date: 2026-08-23
- Status: accepted for C-05 implementation after dual independent contract and feasibility review
- Context: Read-only aggregate evidence now finds real C-order `f32` patch-feature candidates at widths 384 and 1,024, but the 384 assets are explicitly development-only and the 1,024 candidates span seven materially different CSV signatures. Neither family supplies a reviewed canonical `PatchId` map or complete frame/stride/overlap/receptive-field/window/model-run-license provenance. Region-named candidates are cell-row or unbound bundles, and the slide-named candidate is multirow. C-03 table declarations alone are not physical proof, while WS-25 owns broad interchange rather than C-05's exact artifacts.
- Decision: record HDF5 header evidence, the 384-dimensional development family, and the heterogeneous 1,024-dimensional families as aggregate inventory classifications only, not production parsers. Reject width-6 manifest conflicts, region/slide candidates without typed row identity, `.pt`/pickle, and name-only formats. Admit no source adapter until a later decision freezes each exact non-executable grammar, bounds, and aggregate/reconciliation digest. C-05 closure separately requires narrow Arrow IPC and Parquet profiles for its three matrices, footprints, overlap edges, cell assignment/edge pair, and patch-region links with the same locked raw-before-stock, deterministic-writer, managed-integrity, and one-batch/row-group safety policy as C-04. No candidate source family is promoted by passing physical synthetic tests.
- Alternatives: preserve the old blanket statement that no patch features exist; merge heterogeneous CSVs; infer missing fields; deserialize executable formats; admit a logical/catalog-only checkpoint as C-05 closure; defer exact C-05 physical proof to general WS-25 interchange.
- Consequences: the repository records newly established candidate evidence without claiming a canonical real artifact. Physical synthetic contracts can close independently, while real promotion remains prohibited until exact identity, geometry/context, observation support, complete provenance, reviewed snapshot, and license evidence exist.

## DEC-0035 — Bound deterministic cell-containment candidate work

- Date: 2026-08-23
- Status: accepted for the bounded C-05 cell-link checkpoint after independent conformance and feasibility review
- Context: exact all-containing assignment can use the footprint-width bucket index without retaining candidate edges, but equal-origin or otherwise dense buckets admit a valid zero-output `Theta(c * p)` case. Edge-count and byte budgets do not bound that CPU work, so the originally correct two-pass implementation still exposed a practical resource-exhaustion path at the version-one row limits. The same review also found that retained accounting must include the owning `SlideId` text, not only its inline handle.
- Decision: require every contained-shared construction to receive an explicit maximum candidate-check count per deterministic pass. Increment and enforce it immediately before each exact footprint-containment comparison; require count/materialization passes to reproduce candidate, edge, and text counts exactly. Keep candidate count out of logical identity because it is an execution budget, not link semantics. Retain the simple sorted four-bucket index and its documented `O(p log p + c log p + k + e log e)` runtime until the frozen C-05 scale workload provides evidence for a more complex output-sensitive 2-D reporting index. Count owning-slide text in predicted, actual, retained, and peak storage.
- Alternatives: rely only on the 400-million-edge cap; silently accept the candidate-work cliff; add an unmeasured interval/R-tree dependency or bespoke 2-D reporting structure; include resource limits in content identity; reduce all overlaps to one primary patch.
- Consequences: hostile dense occupancy fails after a caller-bounded number of aggregate-safe checks in both passes, while exact assignments and digests remain unchanged for sufficient budgets. The logical checkpoint makes no throughput claim; Criterion/DHAT/full-scale evidence remains required before C-05 closure and may justify a later reviewed index without changing link identity.

## DEC-0036 — Stage derived-region authority before deterministic recomputation

- Date: 2026-08-23
- Status: accepted for the bounded C-05 derived-region checkpoint after dual independent review
- Context: region derivation composes an already verified direct-patch table with a separately verified producer-declared patch-region link. Treating the support JSON, provenance JSON, or physical table declaration as sufficient would permit unrelated context/footprint lineages to meet only at a shared expected-patch set, and issuing a region-table receipt before recomputation would turn declarations into value proof. Revalidating every lower payload at every layer would also erase the meaning of the existing unforgeable receipts.
- Decision: first issue a runtime-only region-support receipt from exact patch-support and patch-region-link receipts, requiring the same expected-patch identity plus exact patch-context and footprint artifact identities. Then validate a nine-role derived-region provenance graph over that support receipt and a fully verified source patch-table receipt. The graph streams the canonical provenance, expected-region, and weighted-mean derivation payloads; checks exact profiles, dependencies, managed integrity, logical identities, output dimension, and distinct roles; and returns only a nonserializable capability. It proves no output row or table. A later finalizer must consume this capability, recompute every region row in deterministic link/patch/component order under explicit budgets, and only then may full physical validation mint a region-table receipt.
- Alternatives: trust support/provenance declarations; join lower receipts only on expected-patch identity; have the derived graph materialize or bless a claimed output table; reparse every transitive lower payload; expose forgeable serializable proof fields.
- Consequences: mismatched physical replicas remain format-neutral, but independently valid receipts from different spatial lineages cannot compose. The publication order is lower physical receipts → managed region support → derived provenance graph → deterministic recomputation → physical region-table receipt. Producer fractions remain declarations rather than geometry, opaque source-vector correspondence remains unproved, and derived slide authority, region finalization, fuzz/scale evidence, and real-corpus promotion stay open.

Finalization addendum, accepted 2026-08-23 after one bounded design audit: convert every declared fraction as `(numerator as f64) / (denominator as f64)`, multiply each promoted component separately, add sequentially in canonical link/component order, divide once, cast once, and canonicalize signed zero. The finalizer receives independent retained-byte, peak-working-byte, sparse-relation, and present-component-operation maxima. It returns a private-graph candidate rather than a receipt; only complete Arrow or Parquet validation of that candidate can mint the format-neutral region-table receipt. This supersedes only the preceding historical statement that region finalization remained open: deterministic region values and physical receipt authority are now active without changing the declaration-only geometry or opaque source-vector claim ceilings.

## DEC-0037 — Bind and finalize both derived-slide paths without new infrastructure

- Date: 2026-08-23
- Status: accepted for the bounded C-05 derived-slide milestone after one independent review
- Context: the existing slide support variants name exact lower support/table artifacts, but a runtime support receipt that discarded owning-slide identity could compose with canonically decoded provenance naming another slide. Constructor validation did not protect the decoder boundary, and rejecting the mismatch only during finalization would leave the graph capability's claim false. Both slide variants otherwise need the same already-contracted arithmetic mean and existing slide physical profile.
- Decision: derive a domain-separated fixed-size owning-slide binding from the fully verified lower table and carry it through the slide-support receipt and derived graph. Before minting the graph, require that binding to match both decoded provenance and the expected singleton slide. Finalization visits lower rows in canonical order, excludes non-present rows, sequentially accumulates promoted components in one `f64[D]` buffer, divides by exactly `present_count as f64`, casts once, and canonicalizes signed zero; no present contributor yields one `missing_vector` row. Count every lower row against contributor work and only present rows times `D` against component work. Return an unforgeable candidate and reuse the existing Arrow/Parquet slide validators to mint the compact receipt.
- Alternatives: trust only constructor-created provenance; defer the ownership check to finalization; retain a variable-length slide ID in public receipts; introduce a generic lower-table trait/registry or another physical profile; bless caller-supplied slide bytes without recomputation.
- Consequences: patch-sourced and region-sourced singleton slide tables now have exact support/graph/finalization/physical-receipt authority with `O(D)` arithmetic scratch and no new format or generalized infrastructure. This remains synthetic infrastructure: sampled support is transitive only, opaque source components and region geometry remain unproved, and C-05 fuzz/scale/allocation/RSS evidence plus real-source promotion remain open.

## DEC-0038 — Freeze the C-05 shared-vector closure workload and resource gates

- Date: 2026-08-23
- Status: accepted for the mandatory C-05 closure-evidence milestone after one independent pre-implementation review and root verification of its findings
- Context: C-05 requires 10,000 × 1,024 patch vectors with 100,000 shared cell-patch edges, a 100,000 × 1,024 / 1,000,000-edge closure profile, fresh Arrow/Parquet publication under DHAT, and a prebuilt host-RSS run. Without exact fixture geometry, arithmetic, checksum framing, allocation ownership, and thresholds, two green runs could perform different work or hide per-edge vector multiplication. This evidence must call existing C-05 APIs without adding production benchmark hooks or generalized infrastructure.
- Decision: define profiles `10k_x_1024_100k_links` and `100k_x_1024_1m_links`. Both contain all-present patch rows and exactly ten lexicographically ordered cells per patch, so `patch_row = assignment_row / 10`; smoke has 10,000 patches/100,000 cells and closure has 100,000 patches/1,000,000 cells. Exact IDs are `patch-{row:06}` and `cell-{assignment:07}`. Lay nonoverlapping 16 × 16 footprints on a 400-column row-major grid with stride 16, zero overlap, fully-contained support, and each cell anchor at its patch center. The four-bucket index checks current, left, upper, and upper-left buckets even when only the current footprint contains the anchor. Therefore freeze `10 × (4P - 2(P / 400) - 799)` candidate checks: 391,510 for smoke and 3,987,010 for closure. `derive_contained_shared` receives exactly that maximum and must produce one edge per assignment, ten cells sharing each patch vector, and exactly the contracted edge count. The fixed widths preserve numeric and canonical order.
- Decision: reuse DEC-0031's component generator exactly: form wrapping `row * 0x9e3779b97f4a7c15 + column * 0xbf58476d1ce4e5b9`, take bits 24 through 39 as an unsigned 16-bit value, subtract 32,768, emit `0.125f32` only for zero, and otherwise divide the exact signed value by `4096.0f32`. Timed work visits patches and components in ascending order, promotes each component to `f64`, and separately performs one sequential component sum and one sequential sum-of-squares; no fused, parallel, covariance, kernel, model, or scientific calculation is equivalent. Both checksums use the existing 128-bit-length framing convention: byte strings are exact bytes, integers are fixed-width big-endian payloads, and each payload is preceded by its big-endian `u128` byte length. Numeric identity frames domain `marklab-patch-embedding-benchmark-numerics-v1`, then patch count (`u64`), value count (`u64`), assignment count (`u64`), edge count (`u64`), QC row/present/missing-vector/extraction-failed/qc-rejected/all-zero-present counts (six `u64` values in that order), dimension (`u32`), component-sum `f64::to_bits` (`u64`), and sum-of-squares `f64::to_bits` (`u64`). Link identity frames domain `marklab-patch-embedding-benchmark-links-v1`, assignment count (`u64`), edge count (`u64`), then for each canonical assignment its row/start/count (three `u64` values), followed by every edge's assignment row (`u64`) and exact UTF-8 `PatchId`. Every iteration asserts row/value/assignment/edge counts, table/link logical identities, and both checksum identities against pinned per-profile goldens. The first implementation run intentionally exposes placeholder-golden mismatches; root independently checks the scalar/link traversal before appending the observed immutable goldens without changing this algorithm.
- Decision: Criterion uses one thread, ten flat samples, one-second warm-up, and ten-second measurement. Fixture construction and initial patch-table plus assignment/edge Arrow/Parquet publication and validation occur once outside timed loops; both timed profiles perform the same domain numeric/link traversal. The DHAT smoke builds caller-owned table/context/expected/footprint/anchor inputs before profiling, then constructs the link and freshly publishes the patch table and assignment/edge tables in both formats inside the profile, dropping every operation-owned value before sampling. Require current tracked bytes to be zero and peak tracked bytes at most 320 MiB (`335,544,320` bytes), which is also strictly below the forbidden `100,000 × 1,024 × 4 = 409,600,000`-byte copied-vector payload before any other operation storage. The recorded Apple M4 Pro closure runtime must stay at or below 2.5 GiB (`2,684,354,560` bytes), below the forbidden 4,096,000,000-byte edge-vector payload alone. Build/link RSS is measured and recorded separately; the runtime command is accepted only when the current benchmark binary was prebuilt and Cargo performs no compilation.
- Decision: freeze the exact commands as the smoke Criterion command, the no-default `parquet,dhat-heap` exact integration test, `/usr/bin/time -l` prebuild with `--no-run`, and the `/usr/bin/time -l` closure Criterion command listed in the C-05 task contract. The private benchmark support module has exactly two immediate callers: Criterion and DHAT. Existing workspace benchmark/fuzz workflows already discover these additions; no new workflow, dependency, lock tuple, public API, retained-byte getter, checksum utility, physical profile, validator, receipt, adapter, or source promotion is authorized.
- Alternatives: one vector per edge; random or order-dependent values; one cell per patch without shared-vector pressure; time fixture construction; decode 400-MiB physical files in every Criterion iteration; calibrate thresholds after measuring; expose a public visitor/benchmark framework; add a general fixture crate or another workflow.
- Consequences: smoke and closure runs perform comparable, checksum-pinned storage/link work over the existing production contracts, while DHAT and RSS make vector multiplication observable. These are infrastructure resource checks only, not embedding quality, aggregation science, performance optimization, real-corpus promotion, or a biological result. Any change to shape, geometry, generator, order, framing, goldens, cap, threshold, or commands requires a superseding reviewed decision.

Closure addendum, accepted 2026-08-24 after the frozen calibration reds matched both independent oracles: smoke table/link/numeric/link-checksum identities are respectively `6aa1ef12bf4955b4f58460b01f3a35975ddf147a57803a42d83359c9d4eb11c0`, `c447edd055d8f6e44cecd739a31ac157c1c3b95c690eaf64469ab53806fc9a47`, `ccb9ab3ea4ca78f26c39b7b97028757feace6b746065d8e11abbdc74df91ce65`, and `b7472ac57b761db386a4cbd19470bc7b1006fbf69a1fbcf4869db5cc5ebe0ce9`. Closure identities are `113b3d67889d901bc987fd99a20e0fd94c9c1fd5d1f5179bd5b8ae3f995e9033`, `1972def442ed8c5d980f86f2f6a28b58f54a8fcec979fd2e579ad8fe281eefee`, `9520cc35c365025f630dd0d31d7bf0b77111ff347062b070a11064aeca8acc65`, and `c081e62e6c108de5645df53d4f7c87c3f4a2671cc35ecab34e200c056f22a6cd`. DHAT returned to zero current bytes and peaked at 180,729,962 bytes. The final prebuilt closure run used 1,311,342,592-byte maximum RSS; its separate compile/link build used 5,221,138,432 bytes. These values close the frozen evidence without changing any threshold or algorithm.

## DEC-0039 — Make measurement status observable through one bounded patch computation

- Date: 2026-08-24
- Status: accepted for the first bounded C-06 vertical slice
- Context: FND-04 ultimately describes a general `CellId`-keyed mark table, but the current marked `Pattern` has no canonical `CellId` rows and cannot distinguish measured from imported values without a new versioned input contract. Count, ordinal, simplex, posterior, unit, threshold, and general missingness declarations likewise have no immediate production caller. C-05 does have exact patch vectors, extraction validity, overlap support, and four closed provenance variants. Adding a general mark schema, physical format, receipt, or compatibility adapter now would therefore violate the repository Immediate-Caller Rule or invent identity/status semantics.
- Decision: add only the closed `MeasurementStatus::{Measured, ImportedPrediction, MorphologyPrediction, DerivedSummary}` vocabulary in `marklab-data`, distinct from `EmbeddingStatus`. Existing multiscale provenance determines exactly one status: direct patch extraction is `MorphologyPrediction`; deterministic region and slide aggregations are `DerivedSummary`. The first and only new consumer is `patch_overlap_embedding_dispersion`, which accepts a patch table, exact patch support, overlap graph, direct-patch provenance, an explicit declared status, and a caller maximum for `edge_count * dimension` component operations. It validates status, provenance, table, support, expected-set, footprint, and overlap bindings before arithmetic. It then visits canonical overlap edges and components in fixed order, excludes any edge with a non-present endpoint, and returns the arithmetic mean of squared Euclidean vector distances across eligible edges. Zero eligible edges is a typed `InsufficientPairs` outcome with no numeric value, never NaN. The result carries counts, dimension, measurement status, and table/support/overlap/provenance identities.
- Decision: this value is a descriptive embedding-dispersion summary over declared positive-area patch overlaps. It is not a variogram, Moran's I, Geary's C, spatial autocorrelation estimate, biological similarity, inferential result, calibrated endpoint, or tissue-window claim. The overlap graph is extraction-support adjacency rather than an admitted general spatial-weights or observation-window contract. No null, p-value, edge correction, patient aggregation, result-0.3 field, config-0.2 key, CLI command, serializer, physical schema, validator framework, receipt, dependency, or real-corpus promotion is added.
- Alternatives: implement the full MarkTable and Arrow/Parquet family; synthesize `CellId` from current row order; infer status from `analysis.mark_label`; add a generic measurement/provenance registry; change result 0.3; call the overlap summary a variogram or spatial autocorrelation statistic; defer all observable computation until every FND-04 mark family exists.
- Consequences: C-06 begins with an honest status/provenance contract that is immediately exercised by a bounded scientific computation over C-05 artifacts. `Measured` and `ImportedPrediction` are named canonical states but this slice creates no adapter that claims either state. General mark columns, units, modalities, threshold provenance, missingness policy, scalar probabilities/simplexes, posterior/vector references, stable result serialization, and full FND-04/C-06 closure remain deferred until their production workflows arrive.

## DEC-0040 — Bind declared scalar marks to the unchanged marked-analysis workflow

- Date: 2026-08-24
- Status: accepted for the second bounded C-06 vertical slice
- Context: the current marked engine has an immediate binary mark and optional dense probability input, while the project scheduler already verifies semantic artifacts and caches exact result-0.3 bytes. The compatibility `Pattern` and loader have no canonical `CellId` or coordinate frame, and result 0.3 has only a free-text mark label. Retrofitting CLI/files or a general MarkTable would invent identity or require new physical/result formats. Probability mode is also not analysis-wide: only structure-factor spectra use `mark_prob`; counts/QC, mark-pair covariance, anisotropy, periodogram, multiscale residuals/territories, and diagnostics continue to use the binary mark.
- Decision: add one version-one in-memory `DeclaredScalarPatternInput` bound to a `MarklabProject` with installed hierarchy and coordinate registry. It borrows an unchanged `Pattern`, requires strictly increasing typed `CellId` rows owned by one explicit slide, and requires one physical two-dimensional micrometre frame with axes `[X,Y]`. It always carries a unitless binary declaration and carries a unitless probability declaration exactly when probability values are declared. Per-cell `DerivedSummary` is rejected. Missing scalar rows are unsupported in version one.
- Decision: binary origin is either independent or thresholded from the declared probability mark. Thresholded input requires an explicit comparator, finite `[0,1]` threshold, threshold-provenance artifact, matching measurement status, and exact rowwise binary/probability agreement. Mark and threshold provenance reuse exact C-03 `ArtifactRecord` schemas/metadata/dependencies and the existing store verifier; no new provenance codec or validation framework exists. Caller row and CellId-text maxima bound construction before traversal.
- Decision: `AnalysisEngine::analyze_declared_scalar_pattern` delegates to the unchanged compatibility engine and returns the exact existing result plus a compact runtime-only `DeclaredScalarIdentity` and `DeclaredMarkUse`. The identity reports row count, ordered-CellId digest, owning slide, coordinate frame, and the full declared-input digest without copying row arrays. A separate concrete `DeclaredMarkedAnalysisNode` binds that identity and semantic provenance IDs into the existing scheduler cache key. It encodes and decodes only the inner result-0.3 document, reattaching the cache-bound identity and mark-use summary on replay. The summary explicitly reports probability only for structure-factor spectra and binary for all other current endpoint families. Neither runtime value is durable result provenance.
- Alternatives: mutate `Pattern`; infer CellId/frame from row order, filenames, or `x_um` names; call probability the analysis-wide mark; add a general MarkTable/Arrow/Parquet family; add config-0.3/result-0.4/CLI files now; copy the scientific algorithms; create a generic artifact-schema validator; implement FND-02 before source/frame/topology prerequisites exist.
- Consequences: measured and imported/morphology-predicted scalar marks can enter one honest project workflow with cache-visible identity and exact legacy numerical/result bytes. General scalar/categorical/ordinal/count/continuous/simplex/vector/posterior marks, missingness, modality/unit ontologies beyond fixed unitless values, CLI/file adapters, durable result citation, source correspondence, FND-02 windows, and full FND-04/C-06 closure remain deferred.

## DEC-0041 — Measure declared patch-to-derived-region aggregation dispersion directly

- Date: 2026-08-24
- Status: accepted for the third bounded C-06 vertical slice
- Context: C-05 now has an immediate complete data flow from an exact verified patch table and exhaustive producer-declared patch-region fractions through deterministic weighted-mean region finalization. The broader FND-04 mark vocabulary, FND-02 geometry, SIG-01 spatial weights, and EMB-01 distance-dependent estimands still lack their required callers or prerequisites. Adding a general statistic framework, region format, scheduler node, result schema, or geometry owner would violate the Immediate-Caller Rule, while leaving the finalized region values without any scientific consumer would not advance observable behavior.
- Decision: add one concrete runtime computation, `patch_region_embedding_dispersion`, over the existing source `PatchEmbeddingTable`, exact `PatchRegionLink`, unforgeable `DerivedRegionEmbeddingTableCandidate`, copied `VerifiedDerivedRegionEmbeddingArtifactGraph`, and exact direct-patch plus derived-region provenance values. Before budgeting or arithmetic, require exact source table, link, candidate table, graph, owning-slide, dimension, expected-set, support, and provenance bindings. Direct patch provenance supplies `MorphologyPrediction`; deterministic region provenance supplies `DerivedSummary`.
- Decision: for each canonical nonzero patch-region relation whose source patch and finalized region rows are both present, convert the declared fraction exactly as `(numerator as f64) / (denominator as f64)`, compute squared Euclidean distance against the materialized `f32` region mean in component order, and accumulate weighted distance and weight sequentially in canonical relation order. Divide once by total eligible declared weight and canonicalize signed zero. Non-present source relations are excluded; present zero vectors remain eligible. No eligible weight returns typed `InsufficientContributors` with no numeric value. Preflight the checked conservative bound `nonzero_relation_count * dimension` against one caller maximum before numeric traversal. Per-axis sign flips preserve the result bits; arbitrary axis reordering is not a bitwise contract because fixed-order floating accumulation intentionally makes stored component order observable in last-bit rounding.
- Decision: the fixed-size result reports availability, source/output measurement statuses, total/eligible/excluded relation counts, dimension, the optional value, and exact source-table/link/source-provenance/region-candidate/derived-provenance identities. It is descriptive declared-link aggregation dispersion only: not a geometric region validation, observation-window statistic, variogram, spatial autocorrelation coefficient, independent-patch estimate, inferential result, embedding-quality score, or biological conclusion.
- Alternatives: implement general vector marks or statistics; add a new physical result/receipt/validator; schedule or serialize the value before a caller exists; introduce geometry/weights; call patch overlap an observation window; wait for all FND-04 kinds before consuming completed region finalization.
- Consequences: the completed C-05 region flow gains one bounded observable scientific consumer with no new dependency, format, receipt, validator, abstraction, CLI/config/result change, null, or source promotion. Broader C-06/FND-04, FND-02, SIG-01, EMB-01, and cohort claims remain deferred until their immediate callers and prerequisites coexist.

## DEC-0042 — Compare declared marked scheduler outputs without a new result contract

- Date: 2026-08-24
- Status: accepted for the fourth bounded C-06 vertical slice
- Context: the existing `compare_marked_prepost` is an observable descriptive user workflow over two `MarkedPatternResult` values, while the completed declared scheduler node now returns the same result together with exact runtime CellId/slide/frame/declaration identity and endpoint routing. Calling the legacy comparator directly discards that declared context; adding a new node, codec, durable schema, generic comparison framework, or result-0.4 would create infrastructure without an immediate caller.
- Decision: add one runtime-only `compare_declared_marked_prepost` function accepting two existing `DeclaredMarkedAnalysisResult` scheduler outputs. Before delegation, require the same binary mark ID, display label, measurement status, structure-factor/other-endpoint value routing, optional probability presence/ID/status, and binary-origin semantics. Thresholded origins agree only when source probability ID, comparator, and exact `f32` threshold bits agree. Independent and thresholded origins never compare as the same mark definition.
- Decision: exact binary/probability provenance and threshold-evidence artifact IDs may differ between timepoints and are retained separately in the returned pre/post `DeclaredMarkUse` values. Different ordered CellIds, row counts, owning slides, physical frames, and timepoints are allowed and both `DeclaredScalarIdentity` values remain observable. This layer selects no biological unit and does not require identity correspondence across timepoints.
- Decision: after semantic compatibility succeeds, delegate unchanged to `compare_marked_prepost`. Return its exact `PrePostResult` plus cloned pre/post declared identities and mark-use summaries. Add no serializer; callers that serialize the inner result produce unchanged result-format 0.3 bytes with no declared fields.
- Alternatives: compare only provenance artifact IDs; require identical CellIds/slides/frames; discard declared context; add declared fields to result 0.3; create a scheduler node/cache codec; generalize a comparison trait or durable run manifest first.
- Consequences: one existing scheduler output now feeds an honest declared pre/post workflow without conflating measured/predicted marks or different threshold definitions. The output remains descriptive only: no correspondence, paired-cell analysis, population inference, noninferiority, equivalence, causal, or biological claim is admitted.

Review addendum, accepted 2026-08-24: `DeclaredMarkedAnalysisResult` predates this slice with public fields, so making it opaque would be an unauthorized breaking API change. Before semantic comparison, validate every binding available without such a break: result row count equals declared row count, result mark label equals the binary declaration, and the scalar identity digest recomputed from its cell digest/slide/frame plus attached mark use is exact. Borrow both timepoint strings into the runtime comparison output so exact visibility adds no unbounded metadata clone. A caller can still substitute an arbitrary same-row/same-label numeric result because result 0.3 carries no declared-input digest or private receipt; document that association as caller-asserted rather than durable authority. A future breaking result/proof contract must solve it at the producer, not through a speculative validator here.

## DEC-0043 — Add binary-prevalence change to the declared comparison flow

- Date: 2026-08-24
- Status: accepted for the fifth bounded C-06 vertical slice
- Context: the existing marked pre/post result compares aggregated spatial endpoints but omits the directly observable change in binary marked-row proportion. Both declared scheduler outputs already carry exact binary counts, prevalence, mark semantics, runtime identity, and separate evidence chains. A proposed random-labeling execution summary was rejected because result 0.3 does not retain every executed seed/stratum/endpoint count and a partial summary would overstate provenance.
- Decision: add one separate runtime-only `compare_declared_marked_prevalence` function instead of changing the newly public `DeclaredMarkedPrePostResult` layout. Reuse the exact C-06-S4 semantic gate, strengthen each available runtime binding with canonical marked-count/prevalence checks, and report fixed-size pre/post counts/prevalences plus optional post-minus-pre change and borrowed exact contexts.
- Decision: probability-routed structure-factor runs still use binary counts because every current non-spectrum endpoint and the result's `n_marked`/`p_hat` fields are binary. Empty input on either side returns typed `InsufficientCells` with no delta. The computation selects no correspondence or biological unit and performs no randomization or inference.
- Alternatives: add prevalence to result 0.3; add a field to the public C-06-S4 wrapper; infer a randomization design from incomplete result fields; add a general comparison trait or count validator; require identical rows/slides/frames; defer a directly available descriptive value until general marks exist.
- Consequences: the existing declared scheduler path gains one O(1), deterministic, user-visible scientific summary without a new node, codec, format, receipt, validator framework, dependency, or general abstraction. Existing legacy comparison/result bytes remain exact. FND-04/C-06 generality, FND-06, paired/cohort inference, and durable producer proof remain open.

## DEC-0044 — Compare both completed slide aggregation paths through existing lineage proofs

- Date: 2026-08-24
- Status: accepted for the sixth bounded C-06 vertical slice
- Context: C-05 materializes singleton slide embeddings by arithmetic mean directly from patches and indirectly from deterministically finalized regions. The paths intentionally weight the hierarchy differently, but no observable value reports their materialized discrepancy. Comparing candidates by SlideId alone would be false path-sensitivity evidence because the region path could descend from another patch table.
- Decision: add one concrete runtime `slide_embedding_aggregation_path_discrepancy` function. Compute fixed-order mean squared component difference between the two present `f32` singleton vectors using `f64`, one final division, positive-zero canonicalization, an exact dimension work cap, and typed non-present availability.
- Decision: require both existing slide graphs and provenance values plus the existing `VerifiedRegionEmbeddingTableArtifact`. Use that receipt as the exact lineage bridge from the region-source table back to the patch-source table used by the direct path. Reuse these already-complete proofs; add or alter no candidate, graph, physical profile, receipt, validator framework, or serializer.
- Alternatives: compare only SlideId/dimension; add a new cross-path receipt or general comparison trait; add another lower-to-slide dispersion; compare cosine distance or declare one path preferable; wait for real-source promotion before exposing any synthetic bounded diagnostic.
- Consequences: both completed finalization paths gain one lineage-exact observable scientific caller without physical-system redesign or new infrastructure. The result describes only materialized aggregation-path discrepancy; it does not establish agreement, quality, geometry, spatial dependence, inference, path preference, FR-02/EMB-01 closure, or biology.

## DEC-0045 — Join exact declared binary rows to verified cell embeddings before broader mark or spatial infrastructure

- Date: 2026-08-24
- Status: accepted for the seventh bounded C-06 vertical slice
- Context: the completed declared scalar input binds exact binary rows, CellIds, measurement status, and provenance; the completed C-04 artifact binds a materialized cell-embedding table to verified physical, row-link, and model-provenance identities. A descriptive within-input embedding comparison is now possible without general marks, geometry, weights, inference, or source promotion.
- Decision: add one concrete `declared_binary_cell_embedding_centroid_discrepancy` caller. Compare the fixed-order `f64` mean present embedding vectors for exact declared binary-marked and unmarked rows using mean squared component difference, typed insufficient-group availability, explicit row/component/working-byte caps, and positive-zero canonicalization.
- Decision: bind the materialized table to the existing `CellEmbeddingArtifact` through exact QC/logical identity, then bind every ordered table CellId to the declared input before component work. Always group on the required binary rows; retain but never silently substitute optional probabilities. Give the exact ordered CellId-bound binary assignments their own local domain-separated result digest because declaration identity alone does not contain row values.
- Alternatives: bundle the three existing multiscale diagnostics without a new estimand; add another lower-to-aggregate dispersion; add a direct/scheduler pre/post orchestrator; add categorical/general mark infrastructure; attempt technical-confounder QC from untyped `stain_batch`/`qc_bin`; wait for real-source promotion.
- Consequences: WS-23 and WS-24 gain one bounded WS-50-facing scientific observable through existing verified values. It adds no physical/profile/receipt/validator/schema/workflow infrastructure and makes no spatial, classification, quality, independence, patient, inferential, real-source, or biological claim.

## DEC-0046 — Exercise the declared probability modality through one fixed WS-50 covariance endpoint

- Date: 2026-08-24
- Status: accepted for the eighth bounded C-06 vertical slice and second WS-50-facing computation
- Context: S7 joins exact binary rows to verified cell embeddings. The same declared input already owns a dense finite `[0,1]` probability column with measurement/provenance identity, but no current embedding computation uses it. Exact spatial covariance, multiscale complementarity, inference, and real-source promotion remain blocked by geometry, weighting, replication, and source-contract gaps.
- Decision: add one concrete `declared_probability_cell_embedding_cross_covariance_energy` caller. Report the mean across components of squared population cross-covariance between present cell-embedding components and their aligned declared probabilities, using fixed two-pass `f64` arithmetic, distinct typed insufficient-row/constant-probability availability, a probability-value digest, and explicit row/component/working-byte limits.
- Decision: keep binary values contextual and never use them to select or weight this endpoint. Bind the existing declared and verified embedding values directly; add no scalar/embedding owner, generic covariance/statistics layer, serializer, physical profile, receipt, graph, validator, scheduler node/codec, config/result/CLI field, geometry/weights/null, or inference surface.
- Alternatives: add a scheduler node and node-local codec for the existing S7 result; extend S7 with another binary within-group dispersion; implement nearest-neighbor retrieval; bundle existing multiscale diagnostics; compare cell and patch coordinates without a proven shared vector space; implement spatial covariance/variograms before FND-02/FND-03.
- Consequences: the existing probability modality gains one observable, bounded WS-50 scientific consumer without another foundation contract. The value is descriptive unstandardized covariance energy in model-component units; it proves no correlation, spatial dependence, classification, calibration, quality, patient effect, inference, real-source result, or biology. A dedicated scheduler workflow remains a separately bounded later option.

## DEC-0047 — Make the exact S7 centroid question a cacheable project execution through one private codec

- Date: 2026-08-24
- Status: accepted for the ninth bounded C-06 vertical slice
- Context: S7 already answers one exact binary-group cell-embedding question, but only as an in-memory public computation. The existing project scheduler supplies semantic-store verification, deterministic cache keys, canonical codec round trips, and failure-atomic success records; result format 0.3 cannot represent this endpoint and must remain unchanged.
- Decision: add one concrete `DeclaredBinaryCellEmbeddingCentroidNode` executed by `LocalScheduler::run_single_with_store`. Reuse S7 through one narrowly named crate-private binding/reattachment seam; register only the whole Pattern and declared-input references; require the existing mark plus embedding table/row-link/provenance records as semantic inputs; bind all scientific limits into cache identity.
- Decision: use one private fixed 18-byte v1 codec carrying only magic/version/status/value. Reattach and validate every count/identity from the constructor-bound, per-run-revalidated S7 snapshot. The exact scheduler execution is this codec's immediate production caller; add no general serializer/result framework, embedding physical format, receipt, config/result/CLI field, or engine wrapper.
- Alternatives: engine/project delegation without lifecycle behavior; result-0.3 extension; generalized workflow result schema; CLI before canonical CellId/artifact-manifest admission; recomputing S7 on cache decode; centroid pre/post subtraction without comparable embedding-space proof.
- Consequences: the S7 computation gains one observable miss/hit project workflow with fixed 18-byte cache cost and exact semantic re-verification. The cache remains within existing public project-state trust and is not described as durable producer authentication or a scientific receipt. S7 numerics and claim ceiling remain unchanged.

## DEC-0048 — Declare nucleus area concretely and consume it in one embedding covariance endpoint

- Date: 2026-08-24
- Status: accepted for the tenth bounded C-06 vertical slice and third WS-50-facing computation
- Context: C-06 still lacks a continuous measurement contract, while the compatibility Pattern already carries a dense positive `nucleus_area_um2` column and C-04 supplies verified aligned cell embeddings. A generic continuous-mark layer, unit registry, or physical format would have no broader current production caller; another scheduler wrapper would repeat S9 without advancing scientific breadth.
- Decision: add one fixed `NucleusAreaUm2MarkDeclaration` with exact morphology/square-micrometre provenance and one concrete `declared_nucleus_area_cell_embedding_cross_covariance_energy` caller. Revalidate the target project and provenance, bind raw ordered positive `f32` area values and CellIds, and use the explicit bounded two-pass population covariance-energy definition without extracting a shared statistics kernel.
- Alternatives: a generic continuous declaration/MarkTable; a scheduler workflow for S8; centroid pre/post subtraction without timepoint/biological-unit/comparable-space proof; embedding retrieval before admitted comparable-source provenance; spatial covariance/variograms before FND-02/FND-03.
- Consequences: one current morphology column gains truthful measurement status/provenance and one observable WS-50 diagnostic without Arrow/Parquet, result/CLI, codec, receipt, validator, dependency, or generalized mark/statistics infrastructure. The result is descriptive and proves no correlation, normalization, segmentation quality, spatial dependence, inference, real-source result, or biology.

## DEC-0049 — Measure contained-patch cell-embedding dispersion through the completed C-04→C-05 boundary

- Date: 2026-08-24
- Status: accepted for the eleventh bounded C-06 vertical slice and next WS-50/WS-51 computation
- Context: C-04 has verified cell embeddings and C-05 has exact CellId-ordered contained-shared cell-patch links plus managed input graphs and paired physical receipts. Another scheduler wrapper would repeat S9 without new scientific behavior; formal graph smoothness/variograms remain blocked by window/weights contracts. The composed cell artifact currently validates but drops the expected-cell ID/digest needed to prove the cross-boundary join.
- Decision: retain those two already-verified expected-cell identities on `CellEmbeddingArtifact` and consume them immediately in one `contained_cell_patch_embedding_dispersion` caller. Restrict v1 to contained-shared links, sort exact edge incidences by patch and assignment, and report bounded incidence-weighted squared component deviation from eligible patch-local cell centroids. Reuse the existing graph and receipt unchanged.
- Alternatives: S10 or S8 scheduler wrappers; generic local-diversity/statistics framework; declared interpolation weights as variance weights; cell/patch vector comparison without comparable axes; spatial graph statistics before FND-02/FND-03; binary nucleus-area contrast.
- Consequences: one synthetic descriptive local-diversity endpoint exercises the exact C-04→C-05 data flow with no new format, receipt, validator, graph, dependency, workflow, or source path. Overlaps are repeated incidences, not independent patches; source-anchor correspondence, tissue-window/spatial claims, inference, real-source results, and biology remain prohibited.

## DEC-0050 — Contrast the concrete nucleus-area measurement across exact binary groups

- Date: 2026-08-24
- Status: accepted for the twelfth bounded C-06 vertical slice
- Context: S10 established one exact positive nucleus-area measurement/provenance contract, while the declared scalar input already carries exact binary assignments. A direct within-input group contrast answers a common pathology-facing question without another embedding/cache wrapper or a generic continuous-mark comparison layer.
- Decision: add one `declared_binary_group_nucleus_area_contrast` caller with an exact CellId-bound paired binary/area digest, fixed-order `f64` group means, typed insufficient-group availability, one row cap, and fixed metadata. Optional probabilities remain contextual and never route groups.
- Alternatives: generic continuous group statistics; standardized effect size without an immediate decision threshold; inferential comparison without biological units; scheduler/CLI wrapper; further covariance normalization.
- Consequences: the existing concrete mark gains one observable descriptive consumer with constant storage and no format, receipt, validator, dependency, workflow, or generalized statistics surface. Image-derived circularity, patient/specimen inference, segmentation validation, real-source results, and biology remain prohibited.

## DEC-0051 — Contrast nucleus area within exact producer-declared contained patches

- Date: 2026-08-24
- Status: accepted for the thirteenth bounded C-06 vertical slice and second WS-51-facing computation
- Context: S12 supplies exact binary/nucleus-area semantics and S11 proves one contained-shared cell-patch incidence authority through an existing managed graph and paired physical receipt. Formal spatial weights, observation windows, and inference remain blocked, while a direct patch-local contrast can answer one descriptive multiscale pathology question without creating another physical or statistics subsystem.
- Decision: add one `contained_patch_binary_nucleus_area_contrast` caller. Reuse S12 unchanged for whole-input validation and paired-value identity; require the existing link/graph/receipt and ordered CellIds; sort exact incidences deterministically; and report the equal-patch mean of eligible within-patch marked-minus-unmarked area contrasts with bounded counts/storage.
- Alternatives: shared patch-statistics or group-statistics kernels; interpolation fractions as statistical weights; binary/probability scoring before an independent-reference contract; trace-normalized embedding association with a new numerical-bound policy; formal variograms/Moran/Geary before FND-02/FND-03; another cache codec around an existing endpoint.
- Consequences: one observable C-06/WS-51 computation composes two completed authorities without Arrow/Parquet, graph, receipt, validator, dependency, workflow, or generalized scalar/patch infrastructure. Overlapping patches repeat incidences and are not independent; tissue-window, spatial, segmentation-quality, patient/specimen, inferential, real-source, and biological claims remain prohibited.

## DEC-0052 — Persist only the declared binary/probability marks consumed by the current workflow

- Date: 2026-08-24
- Status: inactive planning record; not accepted for implementation unless explicitly promoted by a later active roadmap
- Context: IC-0014 already validates exact CellId-bound binary/optional-probability rows and their provenance, but its declared identity is runtime-only and the scheduler verifies only provenance records. A general MarkTable covering every future mark kind would violate the Immediate-Caller Rule because the current production workflow consumes only binary/probability values.
- Decision: add one exact managed Arrow IPC table for the two IC-0014 execution shapes: independent binary only, or thresholded binary plus its dense probability source. Publish through the existing fresh-artifact store boundary, verify managed bytes against the live declared input, and return an unforgeable receipt whose ArtifactId is added to the existing node's semantic inputs and cache identity. Freeze the schema, manifest, metadata, provenance dependencies, V5/64-byte Arrow profile, 8,192-row batches, and caller file-byte limit. Reuse the unchanged node, engine, and result-0.3 codec.
- Alternatives: generalize all mark kinds before an immediate caller; put rows in result JSON; add Parquet and external import simultaneously; trust an ArtifactId without scanning bytes; create another node or cache codec; mutate Pattern or result 0.3.
- Consequences: current binary/probability analyses gain durable row provenance and semantic-store/cache binding without speculative mark infrastructure or a new estimand. Other mark kinds, missingness, modalities/units, external adapters, Parquet, result 0.4, source correspondence, and population claims remain represented in the program tracker.

## DEC-0053 — Make exact window-bound border K/L the first complete classical workflow

- Date: 2026-08-24
- Status: accepted by explicit user direction for the first active program increment
- Context: the committed code has a mask area/containment helper, exact R-tree, deterministic seeds, ERL inference, project scheduler, CLI, strict result/report patterns, but no estimand-defining window, reusable boundary/pair plan, homogeneous K/L, valid location-process null, or end-to-end classical surface. Continuing isolated C-06 statistics would not establish the shared foundations required by later spatial science.
- Decision: evolve TumorMask into a compatibility wrapper over one bounded ObservationWindow2D owner; add one immediately consumed SpatialGeometryPlan2D; implement standard border K/L; declare conditional homogeneous CSR with the whole location pattern as randomization unit; reuse ERL for global inference; add a separate strict version-one classical result, one typed project node/private codec, and marklab classical CLI output through the existing atomic result/manifest/report surface. Use only current dependencies and preserve result 0.3.
- Alternatives: another narrow C-06 statistic; rectangular bounding-box pseudo-window; uncorrected K; random labeling of marks as a location-process null; all-pairs retention; a general geometry framework without K/L; changing result 0.3; library-only implementation without user workflow.
- Consequences: the first classical workflow establishes exact window, reusable geometry, explicit null/unit, deterministic inference, project/cache, and user-facing evidence in one increment. Broader geometry, corrections, point-process methods, cohort inference, and every later master-plan workstream remain in PROGRAM_TRACKER.md.

Checkpoint addendum, accepted 2026-08-24: the exact window owns its canonical boundary-segment index and the reusable point plan immediately consumes indexed boundary distances plus the existing point R-tree; this preserves one geometry owner without duplicating the boundary index. Canonical window identity is invariant to valid ring start/orientation and component/hole order, and unmarked scientific result identity is invariant to point-row order. The strict standalone result carries physical window, geometry, complete configuration/resource, private node-artifact, and scheduler-cache identities. The completed workflow preserves sparse zero/singleton outcomes through its own loader policy while legacy `analyze` retains its nearest-neighbor requirement. PP-06A and NUL-01A close; broader parent FND/PP/workstream rows remain open in the program tracker.

## DEC-0054 — Preserve integrated multi-backend execution and require evidence before Rust ports

- Date: 2026-08-24
- Status: accepted by explicit user direction and consistent with the authoritative master plan
- Context: upcoming Bayesian, embedding, registration, topology, simulation/SBI, predictive, and other advanced workflows have mature Python, R, Stan, GPU, and specialized implementations. Reimplementing them natively by default would spend program capacity on duplicate numerical engines and could weaken scientific validation, licensing visibility, diagnostics, or reproducibility.
- Decision: follow the master plan's integrated multi-backend strategy. Marklab owns versioned adapters, pinned environment and license records, typed input/output schemas, command/config/model/checkpoint digests, deterministic controls, diagnostics normalization, artifact identity, security/resource policy, and execution provenance. Scientifically or operationally appropriate established backends perform the numerical work as first-class Marklab nodes. No native Rust port is presumed; promotion of a port requires measured scientific agreement plus a concrete operational, portability, security, maintenance, or performance advantage over the established backend.
- Alternatives: require every algorithm in Rust; treat backend runs as disconnected manual scripts; admit arbitrary unpinned Python/R/command execution; port first and compare later.
- Consequences: the promoted durable project/ledger outcome precedes BACK-01/WS-13 so backend executions can be replayed and audited. Advanced roadmap contracts must name the selected backend strategy and evidence gate. Native exact Rust remains appropriate for the current bounded window/geometry/classical core and other capabilities with a demonstrated ownership or operational reason; this decision neither removes nor weakens any master-plan workstream.

## DEC-0055 — Persist classical success through one chained project ledger and the existing object store

- Date: 2026-08-24
- Status: accepted for the promoted durable classical project outcome
- Context: the existing `MarklabProject`, scheduler, artifact catalog, and capability-confined local store provide exact in-memory cache and immutable-object semantics, but process exit loses success state. PLAT-01/WS-11 require a human-readable head, append-only execution ledger, integrity-checked replay, and interrupted-run recovery before external backends can be admitted reproducibly.
- Decision: add one bounded `DurableProject` owner in `marklab-project`. It owns strict canonical project-head, chained JSONL execution-ledger, exclusive project lock, and one canonical pending intent. It delegates all immutable output publication, addressing, verification, store locking, and object-staging recovery to the existing `LocalArtifactStore`. The current scheduler exposes its deterministic cache-key computation read-only; durable replay restores verified bytes with the existing failure-atomic `MarklabProject::commit_success` and then uses the scheduler's normal hit path.
- Decision: add `marklab project classical` as the immediate production caller. Version-one records bind exact node/spec/input/config/resource/cache/result identities and a native runtime manifest with crate, Git availability/dirty state, rustc, compiled features, and streamed executable identity. Unsupported versions or semantics are rejected; no migration invents fields. A small build script captures available local build provenance without network access and degrades Git fields explicitly to unavailable outside a checkout.
- Alternatives: duplicate the store inside a project directory manager; serialize private in-memory maps; trust result-directory bytes; recompute the scheduler key in the CLI; execute first and compare afterward; add a general workflow schema/backend registry in the same increment; append a ledger record before object durability; silently repair conflicting state.
- Consequences: exact classical runs replay across processes while changed execution identity misses and tampering fails closed. Result format 0.3 and ordinary one-shot behavior remain compatible. This closes only the promoted single-node durability workflow; general DAG composition/schema/migrations and multi-backend execution remain explicit later work.

## DEC-0056 — Admit the first cohort-valid scalar permutation workflow

- Date: 2026-08-24
- Status: accepted for COH-PERM-01 under the full-program implementation mandate
- Context: the repository has typed patient identities and lower-level random-labeling machinery, but no public workflow compares one prespecified endpoint per independent patient. The paused PLAT-DUR-01 work owns the existing root CLI/library files, so cohort behavior must advance without changing those preserved hunks.
- Decision: add `marklab-cohort` as the scientific owner of a bounded independent-groups patient-label permutation test and expose it through `marklab cohort permutation` at the clean binary dispatch boundary. Version one accepts one strict CSV row per patient, two exact groups, optional exact blocks, a bounded replicate count, one seed, and one alternative. It reports stable group means, signed group-A-minus-group-B effect, Welch-style studentization, and the inclusive-plus-one equal-tail or one-sided p-value. Whole-patient labels are shuffled only within declared blocks using domain-separated deterministic native Rust mechanics; every requested replicate must complete.
- Decision: add the separate `marklab.cohort_permutation` version-one JSON result because result format 0.3 cannot represent cohort inference and remains unchanged. Freeze the exact IC-0029 fields, a 16 MiB CSV limit, one million patient and permutation limits, a 100 million patient-by-permutation evaluation limit, finite-result rejection, and failure-atomic single-file publication. This native implementation has no external backend because the required mechanics are simple, portable, deterministic, and independently oracle-tested.
- Alternatives: treat cells or rows as replicates; modify result 0.3; hide the formula in the CLI; wait for durable project replay; introduce a generalized inference/backend registry; invoke an external statistics product for a four-operation deterministic resampling kernel.
- Consequences: FND-06, COH-01, INF-01A, WS-31, and WS-34 gain one runnable patient-level scalar workflow and a reusable cohort-engine boundary. Paired, repeated, functional, Max-T, bootstrap, MMD/energy, equivalence, noninferiority, intervals, multisite, durable-project, and external-backend workflows remain open. PLAT-DUR-01 production and test files are not advanced.

## DEC-0057 — Add paired patient sign-flip inference to the cohort engine

- Date: 2026-08-24
- Status: accepted for COH-PAIR-01
- Context: COH-PERM-01 establishes the cohort engine and independent-groups CLI, while pseudocode §5.3 requires complete patient pairs to remain the independent units for paired inference. Treating condition rows as independent would discard pairing and misstate uncertainty.
- Decision: add `paired_patient_permutation_test` beside the independent-groups owner and expose `marklab cohort paired-permutation`. Version one accepts exactly one finite scalar row for each of two exact conditions per patient, computes condition-B-minus-condition-A differences, reports their stable mean and studentized mean, and applies deterministic domain-separated whole-pair Rademacher sign flips. Every requested replicate completes or the run fails; there is no unpaired fallback.
- Decision: add strict `marklab.cohort_paired_permutation` version-one JSON through the existing cohort single-file publication boundary. Reuse the cohort patient/permutation/evaluation limits and native portable mechanics. No external backend is warranted for this simple deterministic paired resampling operation.
- Alternatives: reuse the independent-groups label shuffle; silently drop incomplete pairs; add repeated-measures residual permutation in the same workflow; generalize all cohort designs before this immediate caller; mutate result format 0.3.
- Consequences: the established paired scalar workflow advances FND-06, COH-01, INF-01A, WS-31, and WS-34 without changing PLAT-DUR-01 or claiming longitudinal/repeated-measures, causal, clinical, equivalence, or biological evidence.

## DEC-0058 — Make L2 the first joint patient-level functional comparison

- Date: 2026-08-24
- Status: accepted for COH-FUNC-01
- Context: scalar and paired workflows now preserve patients as biological units, but spatial endpoints are commonly common-axis curves. Pointwise tests would create an undeclared multiplicity family, while the pseudocode authorizes a prespecified joint functional statistic.
- Decision: add `functional_two_sample_permutation` to `marklab-cohort` and expose `marklab cohort functional-permutation`. Version one requires one exact common finite strictly increasing axis and one finite curve per patient, at least two patients per group, and uses the trapezoidal integral of the squared group-A-minus-group-B mean curve as the prespecified L2 statistic. Whole-patient labels shuffle under deterministic domain-separated unblocked independent-groups permutations; inference is one-sided high with inclusive-plus-one counting.
- Decision: add strict `marklab.cohort_functional_permutation` version-one JSON with the common axis, both group mean curves, observed difference curve, L2 statistic, p-value, patient counts, replicate counts, and seed. Admit at most 100 million patient-by-axis-by-replicate evaluations. Defer supremum, integrated studentized, and ERL modes to their own behavior slices rather than implying they are present.
- Alternatives: radius-wise p-values without correction; interpolate mismatched axes; add all functional statistics and envelope presentation at once; treat curve points as replicates; use an external backend for deterministic means, trapezoids, and label shuffles.
- Consequences: FND-06, COH-01, INF-01A, WS-31, and WS-34 gain one runnable joint functional test with a hand and slow differential oracle. Pointwise inference, ERL, blocked/paired curves, interpolation, smoothing, and biological claims remain unavailable.

## DEC-0059 — Control a prespecified scalar endpoint family with single-step Max-T

- Date: 2026-08-24
- Status: accepted for COH-MAXT-01
- Context: independent scalar inference does not control family-wise error when several prespecified patient endpoints are tested, and separately permuting each endpoint would discard their within-patient dependence.
- Decision: add `max_t_multiple_endpoint_permutation` to `marklab-cohort` and expose `marklab cohort max-t`. Require one complete finite endpoint vector with identical exact endpoint names per patient. Reuse one canonical stable Welch contrast and apply the same deterministic whole-patient label permutation to every endpoint per replicate. Single-step adjusted p-values count null maximum absolute statistics at least as large as each observed absolute statistic; a conservative empirical `(1-alpha)` critical value is retained.
- Decision: add strict `marklab.cohort_max_t` version-one JSON with patient counts, ordered endpoint effects/statistics/adjusted p-values, alpha, critical value, replicate counts, and seed. Bound patient-by-endpoint-by-replicate work at 100 million evaluations and fail rather than drop any undefined endpoint replicate.
- Alternatives: independent unadjusted p-values; endpoint-specific permutations; Bonferroni without using dependence; step-down Max-T before an immediate requirement; missing-endpoint imputation; external backend for deterministic studentization and shared shuffles.
- Consequences: INF-01C, FND-06, COH-01, WS-31, and WS-34 gain one multiplicity-controlled patient workflow. Endpoint selection, missingness, blocks, step-down adjustment, and clinical/biological interpretation remain unavailable.

## DEC-0060 — Add exact patient-level MMD over prespecified fingerprints

- Date: 2026-08-24
- Status: accepted for COH-MMD-01
- Context: cohort-valid comparison of vector fingerprints requires one vector per patient, a frozen kernel, and patient-label inference. Selecting bandwidth on the final cohort or permuting features independently would change the procedure and invalidate the null.
- Decision: add `patient_level_mmd` to `marklab-cohort` and expose `marklab cohort mmd`. Version one accepts complete exact feature vectors, exact linear or fixed-positive-bandwidth RBF kernels, and separately named unbiased U-statistic or biased V-statistic MMD-squared. Build one bounded symmetric kernel matrix and reuse it under deterministic whole-patient label permutations with an inclusive-plus-one one-sided-high p-value.
- Decision: add strict `marklab.cohort_mmd` version-one JSON with patient/feature counts, kernel kind and optional bandwidth, estimator, MMD-squared, p-value, replicate counts, and seed. Bound the matrix at 25 million elements and matrix-by-permutation work at 100 million evaluations. Use native Rust because the admitted kernels and estimator are small deterministic mechanics with direct hand and independent differential oracles; no maintained external solver is required for this slice.
- Alternatives: data-dependent median bandwidth on the test cohort; coordinate-wise testing; rebuild the kernel per permutation; add learned/spatial kernels before immediate callers; external backend process overhead for dot products/RBF and group sums.
- Consequences: CMP-01C, COH-01, FND-06, INF-01A, and WS-34 gain one runnable patient fingerprint test. Kernel learning, missingness, blocks, spatial curve kernels, and biological interpretation remain unavailable.

## DEC-0061 — Add exact patient-level Euclidean energy distance

- Date: 2026-08-24
- Status: accepted for COH-ENERGY-01
- Context: MMD supplies kernel distributional comparison, while energy distance supplies a complementary metric-based comparison when one complete fingerprint exists per patient. The metric must be fixed and of negative type for the standard interpretation.
- Decision: add `patient_level_energy_distance` to `marklab-cohort` and expose `marklab cohort energy`. Version one admits exact Euclidean distance only, reuses the complete-fingerprint validation boundary, builds one bounded symmetric distance matrix, and reports `2 mean(D_ab) - mean(D_aa') - mean(D_bb')` with ordered within-group diagonals included. Deterministic whole-patient label permutations reuse the matrix and produce an inclusive-plus-one one-sided-high p-value.
- Decision: add strict `marklab.cohort_energy` version-one JSON with patient/feature counts, metric, energy distance, p-value, replicate counts, and seed. Reuse the 25-million matrix-element and 100-million matrix-by-permutation limits. Native Rust is appropriate for this exact deterministic distance/sum workflow with direct hand and independent oracles.
- Alternatives: learned or data-selected metrics; exclude diagonals without naming a U-statistic variant; rebuild distances per permutation; approximate nearest-neighbor distances; external backend overhead for an exact Euclidean matrix and group means.
- Consequences: CMP-01D, COH-01, FND-06, INF-01A, and WS-34 gain one runnable metric distributional test. Missingness, blocks, weighted patients, learned metrics, approximations, and biological interpretation remain unavailable.

## DEC-0062 — Keep spatial fingerprints structured, versioned, and digestible

- Date: 2026-08-24
- Status: accepted for COH-FINGERPRINT-01
- Context: downstream MMD, energy, retrieval, and comparison need compatible versioned sample summaries. Concatenating unnamed values or silently zero-filling absent components would discard endpoint identity and create false comparability.
- Decision: add `build_spatial_fingerprint` and `fingerprint_distance` to `marklab-cohort`, exposed through `marklab cohort fingerprint-distance`. Version one admits named components with exact common axes, finite values, retained finite nonnegative uncertainties, and finite positive prespecified weights. It declares normalization `none`, uncertainty weighting `none`, missing-component policy `reject`, and no training transform. Components are canonicalized by name without discarding identities.
- Decision: bind the exact specification and each fingerprint's sample/value/uncertainty content with SHA-256, using already locked `sha2` 0.10.9 (MIT OR Apache-2.0) as a direct local crate dependency. Distance requires identical specification digests and exact component identities/axes/weights, reports each trapezoidal curve-L2 contribution, and sums prespecified weighted contributions stably. Add strict `marklab.spatial_fingerprint_distance` version-one JSON retaining both structured fingerprints and the full decomposition.
- Alternatives: opaque concatenated vector; missing-as-zero; interpolation or normalization without a training artifact; hash with a local ad hoc implementation; total-only similarity score; retrieval index before the distance contract.
- Consequences: CMP-01A, FR-02, EMB-PATCH, WS-34, and WS-51 gain versioned fingerprint and distance owners with golden digest and hand-distance oracles. Component computation from raw spatial data, normalization, uncertainty weighting, missingness, learned weights, retrieval, and population inference remain separate workflows.

## DEC-0063 — Admit one-sample patient-effect TOST with prespecified margins

- Date: 2026-08-24
- Status: accepted for COH-EQV-01
- Context: the pseudocode accepts design-valid biological-unit effects. The current immediate caller supplies one already-defined signed effect per independent patient; adding an unpaired group model or inferring effect direction would broaden the estimand beyond those inputs.
- Decision: add `tost_equivalence` to `marklab-cohort` and expose `marklab cohort equivalence`. Require unique finite patient effects, at least two patients, finite ordered lower/upper margins, finite `0<alpha<0.5`, and a non-empty exact margin-rationale reference. Report the stable mean, sample standard error, `n-1` degrees of freedom, both one-sided Student-t statistics/p-values, and matching two-sided `1-2*alpha` interval. Equivalence requires both p-values below alpha and the interval strictly inside margins; internal disagreement is a numerical failure.
- Decision: use already locked `statrs` 0.18.0 (MIT) for Student-t CDF/quantile mechanics and add strict `marklab.cohort_equivalence` version-one JSON. Validate against R 4.5.2 base `pt`/`qt` static oracle values. SciPy/statsmodels were unavailable locally and are not claimed as executed evidence.
- Alternatives: call a process backend for a single Student-t distribution; unpaired Welch TOST without that design input; post-hoc margins; declare equivalence from a nonsignificant difference test; label failed equivalence as difference.
- Consequences: EQV-01, COH-01, INF-01D, and WS-34 gain one runnable established TOST surface. Unpaired, paired-from-raw, functional, bootstrap, power, and real clinical equivalence validation remain separate requirements.

## DEC-0064 — Keep noninferiority directional and separate from equivalence

- Date: 2026-08-24
- Status: accepted for COH-NI-01
- Context: noninferiority has one directional null boundary and must not be inferred from TOST or a nonsignificant difference. The immediate caller supplies already-defined signed patient effects, a favorable direction, and one justified margin magnitude.
- Decision: add `noninferiority_test` to `marklab-cohort` and expose `marklab cohort noninferiority`. Reuse the canonical patient-effect mean/SE and locked `statrs` 0.18.0 Student-t distribution. Higher-is-better tests boundary `-margin`, favorable statistic `(estimate+margin)/SE`, and a lower one-sided bound; lower-is-better mirrors at `+margin`, favorable statistic `(margin-estimate)/SE`, and an upper bound. The p-value and bound decisions must agree.
- Decision: require finite positive margin, finite `0<alpha<0.5`, explicit direction, and non-empty margin-rationale reference. Add strict `marklab.cohort_noninferiority` version-one JSON with the signed boundary, direction, statistic, upper-tail p-value, matching bound, and `noninferior`/`not_demonstrated` decision. Validate both directions against R 4.5.2 static oracle values.
- Alternatives: reuse equivalence decision; omit sign convention; report superiority; post-hoc margin; separate duplicated mean/SE implementation; process backend for a Student-t tail.
- Consequences: EQV-01, COH-01, INF-01D, and WS-34 gain one runnable directional noninferiority workflow. Superiority, unpaired group modeling, functional endpoints, power, and biological/clinical validation remain separate.

## DEC-0065 — Resample patients before specimens in the first hierarchical bootstrap

- Date: 2026-08-24
- Status: accepted for COH-HBOOT-01
- Context: specimen or lower-level row resampling alone cannot produce patient-level uncertainty. The immediate caller has exactly two declared hierarchy levels and one scalar specimen-row mean; adding arbitrary hierarchy/statistic callbacks or BCa infrastructure would exceed that caller.
- Decision: add `hierarchical_bootstrap` to `marklab-cohort` and expose `marklab cohort hierarchical-bootstrap`. Version one validates unique specimens nested under at least two exact patients, computes the stable observed specimen-row mean, samples the original patient count with replacement, then samples each selected patient's original specimen count only within that patient occurrence. Multiplicity is preserved explicitly; every replicate completes.
- Decision: report a deterministic nearest-rank percentile interval and retain replicate means in the scientific API for oracle/reuse consumers while keeping the CLI JSON bounded to summary fields. Bound conservative patient-by-maximum-child draws across replicates at 100 million. Use native deterministic mechanics and validate every replicate plus interval against an independent slow reference.
- Alternatives: resample specimens globally; equalize patient weights without declaring a different statistic; arbitrary-depth framework before callers; BCa without design-aware jackknife; silently omit empty/failed replicates.
- Consequences: COH-01, FND-06, INF-01D, and WS-34 gain one patient-first two-level bootstrap. Deeper hierarchies, missing-child policy, other statistics, BCa, repeated/multisite designs, and biological interpretation remain open.

## DEC-0066 — Pin the first Bayesian workflow to PyMC 6.3.0 and a typed Normal model

- Date: 2026-08-24
- Status: accepted for BAY-NORMAL-01 under the full-program implementation mandate
- Context: BAY-01/BAY-02 require an executable typed model and diagnostic lifecycle, while the full-program mandate authorizes established external inference products and prohibits a generalized backend registry before concrete reuse. A conjugate unknown-mean/known-sigma Normal model supplies an analytic oracle for the first complete boundary.
- Decision: add `marklab-bayes` as owner of one backend-neutral `marklab.bayesian_model_ir` version-one Normal model, its explicit prior/likelihood semantics, NUTS sampling request, diagnostic policy, strict PyMC result normalization, and complete/nonconverged fit state. Execute one static repository worker with PyMC 6.3.0 (Apache-2.0) in a Python-3.12 `uv` environment whose complete transitive lock, exact lock digest, static worker digest, and PyPI artifact hashes are retained locally. Bind request bytes, backend version, environment lock, worker, seed, sampling controls, and resource limits; clear the inherited process environment; bound runtime and output; and reject schema/version/drift/non-finite failures.
- Decision: complete status requires finite prior/posterior predictive draws, rank-normalized R-hat at most 1.01, bulk/tail ESS at least 400, zero divergences, and zero maximum-tree-depth hits. A finished sampler that misses the policy is published only as `nonconverged` with `diagnostic_only_nonconverged` claim status. The result also reports an observed-mean posterior predictive discrepancy and remains experimental even when complete.
- Alternatives: implement native HMC/NUTS; accept free-form Python/model code or arbitrary JSON; create BACK-01's generalized registry before a second backend workflow; treat returned samples as success without diagnostics; wait for the paused PLAT-DUR-01 outcome.
- Consequences: `marklab bayes normal-mean` recovers the analytic `Normal(2, sqrt(0.2))` posterior for `[1,2,3,4]`, produces byte-identical seeded output, and exposes typed nonconvergence. This advances bounded BAY-01/BAY-02/WS-40 behavior but does not complete the generalized registry, cross-backend promotion gate, hierarchical/spatial models, SBC, LOO, sensitivity, GPU execution, durable projects, or stable biological claims.

## DEC-0067 — Make patient varying intercepts the first partial-pooling model

- Date: 2026-08-24
- Status: accepted for BAY-HIER-01
- Context: the first Normal model validates the PyMC lifecycle but has no biological hierarchy. `BuildHierarchicalMixedModel` and `SummarizePartialPooling` require one explicit observation/biological-unit boundary and model-dependent group summaries; a general formula compiler, arbitrary random-effects graph, or spatial component would have no second immediate caller.
- Decision: add one exact non-centered Gaussian patient varying-intercept IR and `marklab bayes hierarchical-normal`. Version one requires at least three exact patients with at least two finite scalar observations each; explicitly owns Normal global-mean, HalfNormal between-patient-SD, and known observation-SD conventions; and fixes patient as the biological hierarchy. Reuse the pinned PyMC 6.3.0 environment through a separate static model worker and the existing bounded process boundary, with exact lock/worker/request identities.
- Decision: summarize global mean, heterogeneity, variance partition, patient raw/posterior means and intervals, and `1 - posterior_variance / approximate_unpooled_variance` shrinkage with the mandatory warning that shrinkage is model-dependent and not a quality score. Gate all global, heterogeneity, and patient-effect draws on prior/posterior finiteness, MCSE, E-BFMI, rank R-hat, bulk/tail ESS, divergences, tree depth, constraints, and identifiability; report global-mean and patient-mean-dispersion posterior-predictive distributions without binary model-truth claims.
- Alternatives: pool all rows without patient effects; fit separate patients without pooling; use a centered parameterization on the deliberately weakly identified first fixture; add arbitrary formulas/random slopes/correlation/spatial fields; treat patient summaries as quality scores; implement native NUTS.
- Consequences: BAY-03/BAY-HIER-A/WS-41 gain one runnable patient partial-pooling workflow with synthetic parameter recovery and inward shrinkage of both extreme raw means. Repeated/multisite effects, covariates, random slopes, missingness, unknown observation dispersion, sensitivity/SBC/cross-backend calibration, spatial fields, and stable biological claims remain open.

## DEC-0068 — Marginalize the site effects in the first Bayesian meta-regression backend

- Date: 2026-08-24
- Status: accepted for BAY-META-01
- Context: `BayesianRandomEffectsMetaAnalysis` requires global, covariate, heterogeneity, site, and new-site distributions. The direct centered PyMC transcription produced 13,621 divergent tree events on the recovery fixture and was correctly rejected; weakening the diagnostic gate or publishing that fit is prohibited.
- Decision: expose `marklab bayes meta-analysis` for at least five unique sites with exact effect, positive known standard error, and one named varying raw covariate. Own explicit Normal global/slope priors, HalfNormal heterogeneity prior, latent site Normal hierarchy, and a requested new-site covariate. Use the algebraically equivalent marginalized observed likelihood `Normal(global + x*gamma, sqrt(tau^2 + se^2))` for NUTS, then reconstruct each latent site effect from its exact conditional Normal posterior and generate the new-site latent effect with a domain-separated deterministic seed.
- Decision: retain exact sorted site/input/backend/lock/worker/request identities; summarize global/slope/heterogeneity, every site, and new-site prediction; and gate hyperparameters on the shared finite, constraint, identifiability, MCSE, E-BFMI, rank R-hat, bulk/tail ESS, divergence, and depth policy. Posterior predictive effect mean/dispersion remain diagnostics, not binary model truth or transportability evidence.
- Alternatives: publish the divergent centered fit; weaken divergence thresholds; drop latent site effects; hide covariate centering/scaling; introduce arbitrary formulas/multiple covariates; implement native NUTS.
- Consequences: BAY-03/BAY-HIER-A/WS-41 gain one runnable site/cohort meta-regression with parameter recovery and new-site prediction. Exchangeability, publication bias, multiple covariates, real external cohorts, sensitivity/SBC/cross-backend agreement, transportability, and stable biological/clinical claims remain open.

## DEC-0069 — Bound the first exact GP to one physical axis and explicit dense work

- Date: 2026-08-24
- Status: accepted for BAY-GP-01
- Context: `ExactGaussianProcessRegression` and `MaternKernel` are the first BAY-04/WS-42 functions. An exact dense workflow provides direct scientific value and a kernel convention oracle before sparse, multi-output, anisotropic, or nonstationary variants; unrestricted row counts would make user-selected NUTS work unbounded.
- Decision: expose `marklab bayes gp-regression` for 5–128 unique one-dimensional micrometre observations and 1–2,048 explicit prediction coordinates. Own a constant Normal mean, positive HalfNormal amplitude/Matérn-3/2 length/noise priors, covariance `a^2(1+sqrt(3)r/l)exp(-sqrt(3)r/l)`, explicit positive jitter, and exact dense multivariate-Normal likelihood. Bind requested iterations and posterior conditioning to a two-billion-unit conservative `iterations*n^3 + draws*n^2*m` cap.
- Decision: generate latent-field predictions with per-draw Cholesky conditioning; observation noise is present in the training covariance but not added to latent prediction covariance. Report all hyperparameters, predictions, observed-field mean/dispersion posterior predictive distributions, and the shared complete/nonconverged diagnostics. The initial 0.95-target run's 353 divergences were rejected; longer warmup and 0.99 target acceptance close the same fixture with zero divergences.
- Alternatives: weaken the divergence gate; call row order a coordinate; omit physical units/range convention; use a rectangular pseudo-window; admit unbounded dense matrices; add sparse/multi-output/nonstationary abstractions before their callers; port NUTS.
- Consequences: BAY-04/BAY-FIELD-A/WS-42 gain exact one-dimensional GP interpolation and the canonical consumed Matérn-3/2 kernel. This remains experimental synthetic evidence and makes no two-dimensional tissue, anisotropy, nonstationarity, sparse-scale, calibration, biological, or clinical claim.

## DEC-0070 — Fix one loading and use known noise in the first multi-output GP

- Date: 2026-08-24
- Status: accepted for BAY-MOGP-01
- Context: `MultiOutputGP` requires explicit identifiability for latent loadings. A first exact one-factor model with loading A fixed to 1 and positive loading B still inferred two near-zero output-noise scales; it produced zero divergences but 535 maximum-tree-depth hits and a 142-second nonconverged run. Weakening the gate or accepting that runtime is prohibited.
- Decision: expose `marklab bayes multi-output-gp` for exactly two complete outputs on 5–64 shared one-dimensional micrometre coordinates and one latent Matérn-3/2 process. Fix output-A loading to 1, constrain output-B loading positive, infer separate means/amplitude/length/loading, and require caller-supplied known positive noise SD for each output. Own the exact block covariance `[[K,bK],[bK,b^2K]]`, explicit output names, jitter, and bounded dense-work request.
- Decision: perform per-draw exact joint Cholesky conditioning for both latent outputs and report loading/hyperparameters, both prediction distributions, and observed/replicated cross-output correlation under the shared diagnostics. Reject constant outputs and all missing/non-finite rows.
- Alternatives: publish the depth-limited fit; weaken depth criteria; infer weak near-zero noise without stronger data; leave loading sign/scale unidentified; fit two independent GPs; add arbitrary output/latent counts or rotational post-processing before callers.
- Consequences: `MultiOutputGP` has a runnable identifiable one-factor/two-output owner with positive-loading recovery and joint prediction. Negative dependence, more outputs/processes, missing modalities, inferred noise, rotationally ambiguous factors, sparse scale, biological complementarity, and general multimodal modeling remain open.

## DEC-0071 — Keep the inducing-point GP explicitly approximate and multi-start

- Date: 2026-08-24
- Status: accepted for BAY-VIGP-01
- Context: exact GP work is bounded but cubic. `VariationalInducingPointGP` requires inducing coordinates, VFE/ELBO optimization, multiple starts, and exact-GP comparison without presenting an approximate posterior as converged exact inference. PyMC 6.3.0 provides a maintained Titsias VFE marginal approximation for Gaussian likelihoods.
- Decision: expose `marklab bayes variational-gp` for 8–2,000 one-dimensional micrometre observations, 3–64 inducing points with `m<n`, and 1–2,048 predictions. Initialize inducing locations at coordinate quantiles, infer them under an ordered transform, use PyMC's VFE bound (analytically collapsed Gaussian inducing-state optimum), and run two-to-four independently seeded mean-field ADVI starts with explicit iteration/rate/draw/work limits.
- Decision: stable output is always `approximate_only`, never `complete`. Require finite prior/posterior/predictive values, terminal ELBO improvement, per-start tail relative change <=0.2, and maximum cross-start prediction RMSE <=0.5; otherwise return typed `nonconverged`. Retain every start's ELBO diagnostics and selection. Record gradient norm as unavailable and importance correction as not run rather than inventing evidence.
- Alternatives: call VFE exact; use one favorable start; hide inducing initialization/optimization; fabricate HMC/R-hat or gradient diagnostics; omit exact comparison; add an unpinned GP framework.
- Consequences: `VariationalInducingPointGP` and its ELBO have a runnable approximate-only owner with direct exact-GP agreement on the small fixture. Minibatching, natural gradients, held-out/importance correction, large-scale calibration, alternative inducing initialization, non-Gaussian likelihoods, and production-scale evidence remain open.

## DEC-0072 — Expose predictive-process loss without materializing a dense field matrix

- Date: 2026-08-24
- Status: accepted for BAY-PREDPROC-01
- Context: `LowRankPredictiveProcess` is the deterministic prerequisite for predictive-process approximation, but a raw `n*n` matrix would waste memory and a library-only helper would violate the immediate-caller rule. Its primary scientific risk is hidden residual variance/oversmoothing.
- Decision: implement native exact Matérn-3/2 `Kmm + jitter I` Cholesky, streamed Knm rows, low-rank diagonal `diag(Knm Kmm^-1 Knm^T)`, and nonnegative residual diagonal under 10-million-element and 500-million-work-unit caps. Retain Kmm Cholesky/Knm/residuals in the scientific API and expose `marklab bayes predictive-process` with explicit knots/kernel/units/diagonal-correction state and bounded per-coordinate residual/trace summaries.
- Decision: diagonal correction adds only the exact residual variance to the represented diagonal; it does not restore off-diagonal covariance or make the approximation exact. Materially negative residuals are numerical failure rather than clamped silently.
- Alternatives: publish a dense low-rank matrix; hide residual variance; call diagonal correction exact inference; add a general matrix package; leave the function internal without a production caller.
- Consequences: `LowRankPredictiveProcess` has a runnable diagnostic owner and exact knot/interior oracle. Fitting, posterior uncertainty, off-diagonal error, oversmoothing calibration, 2-D tissue fields, and production-scale claims remain open.

## DEC-0073 — Use ascending physical order for the first NNGP

- Date: 2026-08-24
- Status: accepted for BAY-NNGP-01
- Context: `BuildNNGP` requires an ordering and nearest predecessor policy that are part of the approximation. In one dimension, ascending physical order makes the nearest predecessors exactly the immediately preceding rows, avoiding an unnecessary quadratic neighbor search while retaining deterministic semantics.
- Decision: implement native `build_nngp` for 2–10,000 unique micrometre coordinates and 1–64 predecessors. Sort by coordinate, select the last `min(m,i)` rows, solve each Matérn-3/2 predecessor covariance with explicit jitter, require `F_i` above caller tolerance, retain indices/B/F, and cap conservative `n*m^3` work at 500 million units. `nngp_log_density` consumes every centered field value in conditional order.
- Decision: expose `marklab bayes nngp-density` and optional full dense reference for at most 128 rows. The reference is an independent Cholesky likelihood over the same jittered covariance; with `m=n-1` both densities must agree.
- Alternatives: arbitrary input order; all-pairs predecessor search in 1-D; silent nonpositive F clamp; omit jitter/tolerance; call the sparse density fitted inference.
- Consequences: `BuildNNGP` and `NNGPLogDensity` have runnable owners and an exact full-factorization oracle. Space-filling 2-D order, approximate-neighbor indexing, fitted NNGP posterior, prediction, calibration, tissue validation, and production-scale evidence remain open.

## DEC-0074 — Freeze spatial-weight semantics before CAR/SAR/GMRF

- Date: 2026-08-24
- Status: accepted for BAY-WEIGHTS-01
- Context: CAR/SAR/GMRF functions cannot safely share an untyped edge table because symmetry, diagonal, normalization, islands, and component structure change the estimand and admissible parameter range. A general graph framework is unnecessary for the immediate sparse matrix callers.
- Decision: implement native `validate_spatial_weights` for exact sorted region IDs and unique positive directed edges with explicit required/not-required symmetry, zero/allowed diagonal, and preserve/row-standardize policy. Apply symmetry to supplied weights before normalization, identify weak undirected components and islands, retain normalized sparse rows, and compute a versioned SHA-256 over policies/IDs/exact weight bits.
- Alternatives: infer regions from edges; silently symmetrize; silently drop self/zero/negative weights; always row-standardize; hide islands; digest input file order.
- Consequences: `ValidateSpatialWeights` has a runnable deterministic owner through `marklab bayes validate-weights`. Signed weights, alternative symmetrization, higher-order adjacency, geometric construction, fitted models, and biological graph meaning remain open; CAR callers must request preserved symmetric zero-diagonal weights explicitly.

## DEC-0075 — Keep intrinsic CAR normalization and islands explicit

- Date: 2026-08-24
- Status: accepted for BAY-CAR-01
- Context: proper and intrinsic CAR share an adjacency graph but not a normalization measure. Treating singular intrinsic precision as an ordinary full-rank Gaussian, silently assigning island priors, or omitting component constraints would make reported densities incomparable or undefined.
- Decision: implement native bounded `car_density` over IC-0047 preserved symmetric zero-diagonal weights and expose `marklab bayes car-density`. Proper mode uses `Q=tau*(D-rho*W)` only when Cholesky proves positive definiteness and rejects islands. Intrinsic mode uses `Q=tau*(D-W)`, requires a sum-to-zero field per non-island component, reports one rank deficiency per constraint, and normalizes each constrained subspace with `log(k)+log(det(any cofactor))`.
- Decision: version one makes island handling `reject` or `exclude`; exclusion contributes neither field value nor implicit iid prior. Dense diagnostic factorization is limited to 2–512 regions and 200,000 edges. The strict result binds the validated-weight digest and retains mode, parameters, constraints, rank deficiency, excluded region IDs, and an experimental diagnostic claim ceiling.
- Alternatives: pseudo-inverse density without naming the measure; silent ridge regularization; implicit iid island effects; an unconstrained intrinsic density; fitted CAR/BYM/SAR inference before a direct likelihood caller; a general sparse linear-algebra abstraction before its immediate workflow.
- Consequences: `ProperCARPrecision`, `IntrinsicCARPrecision`, and their normalized log-density mechanics have one runnable owner and hand oracles. General constrained GMRF input, SAR likelihood, BYM/BYM2 fitting, sparse-scale solvers, spatial confounding, tissue validation, and biological or clinical claims remain open.

## DEC-0076 — Define general GMRF density on an explicit Euclidean constrained subspace

- Date: 2026-08-24
- Status: accepted for BAY-GMRF-01
- Context: CAR supplies graph-specific constraints, but `GMRFLogDensity` must also support caller-declared homogeneous linear constraints. A singular-matrix pseudo-determinant alone does not identify the support measure or prove that the supplied field belongs to it.
- Decision: implement native bounded `gmrf_log_density` and expose `marklab bayes gmrf-density` over an exact named sparse-triplet precision and named constraint rows. Require exact finite symmetry and an explicit diagonal; never silently symmetrize, jitter, or repair rank. Reject dependent rows and constraint-violating fields under caller tolerance.
- Decision: construct a deterministic orthonormal null-space basis with twice-reorthogonalized modified Gram–Schmidt, project Q, prove positive definiteness by Cholesky, and normalize with constrained dimension `n-rank(C)`. Version one is dense and diagnostic-only, capped at 256 regions, 65,536 stored entries, and 255 constraints.
- Alternatives: an unnamed pseudo-inverse density; arbitrary non-orthonormal coordinates without Jacobian accounting; silent constraint dropping; implicit ridge repair; a sparse solver abstraction before a fitted caller; reuse only CAR component constraints and call it general.
- Consequences: `GMRFLogDensity` has a runnable owner and independent projected-diagonal oracle. Sparse-scale factorization, affine/nonzero constraints, fitted GMRF posteriors, SAR, BYM/BYM2, spatial confounding, tissue validation, and biological or clinical claims remain open.

## DEC-0077 — Separate the Gaussian SAR likelihood core from fitted inference

- Date: 2026-08-24
- Status: accepted for BAY-SAR-LIKE-01
- Context: lag and error SAR inference both require the exact transformed Gaussian likelihood and `log|det(I-rho W)|`. Hiding that numerical core inside a sampler would make determinant, residual, and impact oracles harder to test, while calling fixed parameters a fitted model would overstate the result.
- Decision: implement native bounded `sar_gaussian_log_likelihood` and expose `marklab bayes sar-likelihood` over IC-0047 island-free row-standardized zero-diagonal weights, complete response/design, matched coefficients, rho, and sigma. Partial-pivot LU proves nonsingularity and supplies the log absolute determinant; lag uses `Ay-Xbeta`, error uses `A(y-Xbeta)`.
- Decision: version one accepts only a declared descriptive interpretation. Lag mode reports direct/total multipliers from the exact dense inverse and coefficient-scaled indirect effects; error mode reports no spillover impacts. The strict result is explicitly a fixed-parameter experimental likelihood, not a fit or posterior.
- Alternatives: omit the Jacobian; use ordinary least squares residuals for both modes; silently constrain rho by an assumed symmetric spectrum; call fixed coefficients estimated; report causal impacts without a causal design; add a general dense linear-algebra dependency for bounded LU.
- Consequences: the numerical likelihood core of `SpatialAutoregressiveModel` has a runnable owner and hand/pivot oracles. Parameter fitting, prior sensitivity, posterior diagnostics, uncertainty in impacts, sparse-scale solvers, causal interpretation, tissue validation, and biological or clinical claims remain open.

## DEC-0078 — Fit SAR through the pinned lifecycle without changing likelihood semantics

- Date: 2026-08-24
- Status: accepted for BAY-SAR-FIT-01
- Context: IC-0050 provides independently testable lag/error likelihood mechanics, but fitted uncertainty requires a maintained sampler, strict parameter support, and the same diagnostic/identity policy as the existing Bayesian workflows. Reimplementing NUTS or allowing a worker-specific residual would break those owners.
- Decision: add a typed `SarFitModelIr` and static PyMC 6.3.0 worker for 6–64 regions and 1–16 full-rank predictors. Use explicit Normal intercept/coefficient, Uniform bounded rho, and HalfNormal sigma priors. The worker evaluates exactly the IC-0050 Jacobian/residual, while Rust rejects nonstandardized/island weights and confounded designs before execution.
- Decision: posterior predictive draws solve the declared lag/error system per posterior draw. Lag descriptive impact distributions use each exact dense inverse; error mode emits none. Exact lock/worker/request/data/weights identities and the shared NUTS diagnostic policy determine `complete` or `nonconverged`; the latter is never upgraded based on favorable parameter summaries.
- Alternatives: duplicate the likelihood only in Python without a native oracle; native NUTS; omit determinant; constrain rho using symmetry-specific eigenvalues despite directed weights; report plug-in impacts without posterior uncertainty; weaken ESS/divergence gates for a small fixture.
- Consequences: `SpatialAutoregressiveModel` has a runnable fixed and fitted Gaussian lag/error owner. The lag recovery fixture completes; applying error mode to lag-generated data truthfully remains diagnostic-only when ESS fails. Larger error-model calibration, sparse scale, non-Gaussian SAR, causal interpretation, tissue validation, and biological or clinical claims remain open.

## DEC-0079 — Represent BYM ICAR support with an exact noncentered transform

- Date: 2026-08-24
- Status: accepted for BAY-BYM-01
- Context: PyMC's available ICAR primitive enforces sum-to-zero with a narrow Normal penalty, while the pseudocode and IC-0048 require an exact constrained subspace. BYM also needs structured and unstructured components to remain separately identifiable and reportable.
- Decision: add native `build_icar_plan` for 6–64 regions using IC-0047 preserved symmetric binary zero-diagonal island-free weights. Canonicalize each connected component, build its orthonormal Helmert basis, project `D-W`, Cholesky-factor the constrained precision, and return `T=ZL^-T`, verified by component sums and `T'QT=I`.
- Decision: fit Poisson log-offset BYM through a static PyMC 6.3.0 worker as `structured_sd*Tz + unstructured_sd*v`, with independent standard-Normal raw effects and explicit HalfNormal scales. Preserve separate field/risk summaries, exact component constraints/rank deficiency, identities, PPC, and standard complete/nonconverged diagnostics.
- Alternatives: PyMC soft ICAR constraint; ridge the singular precision; use proper CAR; silently drop islands; combine both components before reporting; implement NUTS natively; call unscaled BYM equivalent to BYM2.
- Consequences: `BYMModel` has a runnable exact-constraint owner and synthetic recovery evidence. BYM2 scaling/mixing, broader priors/likelihoods, disease-map calibration, spatial confounding, real epidemiology, biology, causality, and clinical claims remain open.

## DEC-0080 — Scale BYM2 ICAR variance before mixing components

- Date: 2026-08-24
- Status: accepted for BAY-BYM2-01
- Context: BYM's structured scale depends on graph topology, so directly mixing its unit-precision field with iid unit variance would make phi graph-dependent and uninterpretable. BYM2 requires an explicit graph scaling convention before sigma and phi can represent total magnitude and structured fraction.
- Decision: extend the immediate ICAR owner to compute diagonal generalized variances from `TT'`, use their geometric mean as the graph's typical marginal variance, and divide T by its square root. Retain original and scaled values and require the scaled geometric mean to equal one within `1e-12`; the worker independently recomputes it within `1e-10`.
- Decision: fit `sigma*(sqrt(phi)*u_star+sqrt(1-phi)*v)` through a static PyMC 6.3.0 worker with caller-declared HalfNormal sigma and Beta phi priors. Preserve separate contributions, combined field, risks, exact constraints, identities, PPC, and the standard diagnostic/claim gates.
- Alternatives: call unscaled BYM a BYM2 model; arithmetic-mean or undocumented scaling; estimate separate component scales plus phi; soft constraints; omit scale provenance; report phi as causal attribution.
- Consequences: `BYM2Model` has a runnable scaled-ICAR owner and synthetic recovery evidence. Prior calibration, larger graphs, other likelihoods, spatial confounding, real epidemiology, biology, causality, and clinical claims remain open.

## DEC-0081 — Center the varying coefficient field in an orthonormal subspace

- Date: 2026-08-24
- Status: accepted for BAY-SVC-01
- Context: a spatial mean coefficient and an unconstrained GP field mean are aliased, while multiplying a varying field by a predictor does not itself solve that identifiability problem. The existing exact GP worker does not own predictor-specific coefficient semantics.
- Decision: add a typed one-global/one-spatial-predictor Gaussian workflow for 8–64 one-dimensional micrometre coordinates. Require the fixed intercept/global/spatial-mean design to have full rank. Supply a deterministic Helmert basis from Rust, project the exact Matérn-3/2 covariance, and sample a noncentered Cholesky field whose deviation sums exactly to zero.
- Decision: retain known observation noise, inferred positive amplitude/length, every deviation/varying coefficient, strict identities, response PPC, and established NUTS gates through a static PyMC 6.3.0 worker. The result remains experimental 1-D regression and does not imply causal heterogeneity or tissue meaning.
- Alternatives: leave the field mean unconstrained; center only posterior summaries; treat the spatial predictor as a varying intercept; reuse independent pointwise coefficients; introduce 2-D/SPDE before its mesh prerequisite; implement NUTS natively.
- Consequences: `SpatiallyVaryingCoefficientModel` has a runnable bounded owner and synthetic recovery evidence. Multiple fields/predictors, GMRF coefficients, non-Gaussian likelihoods, 2-D tissue, multiplicity-aware map decisions, calibration, causality, biology, and clinical claims remain open.

## DEC-0082 — Audit PyMC SMC stages instead of discarding its resampling state

- Date: 2026-08-24
- Status: accepted for BAY-SMC-01
- Context: pinned PyMC SMC exposes beta, acceptance, and evidence by default, but its kernel already computes conditional weights, ESS, and systematic-resampling indices without publishing them. Returning only default summaries would not satisfy the pseudocode's ESS path and ancestry artifact.
- Decision: subclass the exact pinned PyMC 6.3.0 IMH kernel inside a source-digested static worker. After each weight update record `1/sum(w^2)`; after each systematic resample retain every ancestor index. Add these bounded arrays to PyMC sample stats alongside beta/acceptance/evidence and strictly validate them in Rust.
- Decision: expose `marklab bayes normal-mean-smc` on the existing conjugate typed model with explicit particles/chains/ESS target/correlation threshold. Report method-specific diagnostics and never fabricate NUTS metrics. Raise the shared process-capture ceiling to 16 MiB for complete ancestry while all earlier request contracts keep their 1 MiB output limit.
- Alternatives: omit ancestry; publish only an ancestry digest; reimplement SMC mechanics natively; infer ESS from final equally weighted particles; label SMC output with NUTS diagnostics; accept nondeterministic process scheduling.
- Consequences: `RunAnnealedSMC` has a runnable maintained-backend owner with exact posterior/evidence oracle and byte-deterministic ancestry. Other models/kernels, calibration, parallel reproducibility, large-particle memory evidence, and scientific claims remain open.

## DEC-0083 — Keep Laplace optimization and curvature exact but its posterior approximate

- Date: 2026-08-24
- Status: accepted for BAY-LAPLACE-01
- Context: Laplace approximation needs a robust mode, small gradient, and positive identifiable-space Hessian. A favorable mode alone is insufficient, while a Gaussian approximation must not inherit the `complete` state used for exact NUTS/SMC workflows.
- Decision: expose `marklab bayes poisson-log-rate-laplace` for a typed scalar Poisson-exposure log-rate model. Use pinned SciPy 1.18.1 BFGS with exact PyTensor 3.2.4 gradient and bounded Newton refinement; evaluate the exact second derivative at the mode and require positive negative Hessian before inversion.
- Decision: report optimizer/evaluation/message/gradient, mode/log joint, Hessian/variance/condition, Normal log-rate and explicit lognormal rate summaries, deterministic PPC, and strict identities. Valid output is always `approximate_only`; failures remain `nonconverged`. Reject count/exposure aggregates outside exact backend ranges before execution.
- Alternatives: finite-difference gradients/Hessian; accept optimizer status without gradient; absolute-value a negative Hessian; call the Gaussian approximation exact; hide transformed-rate semantics; implement a general optimizer/Hessian framework before another caller.
- Consequences: `RunLaplaceApproximation` has a runnable established-backend owner and analytic oracle. Multivariate/sparse modes, transformations beyond log rate, numerical marginal correction, HMC comparison, INLA nesting, calibration, and scientific claims remain open.

## DEC-0084 — Validate nested Laplace integration against the same latent model

- Date: 2026-08-24
- Status: accepted for BAY-INLA-01
- Context: the pseudocode requires conditional latent modes, Hessian determinants, low-dimensional hyperparameter integration, and a small-problem HMC comparison. Calling one Gaussian mode “INLA” or comparing against a different parameterization would not establish those mechanics.
- Decision: expose a bounded Poisson-lognormal model with independent latent log rates conditional on one Gamma precision. Integrate on the caller's evenly spaced log-tau grid using exact PyTensor gradients, bounded SciPy optimization/Newton refinement, exact diagonal Hessians, Gamma density, log-coordinate Jacobian, and trapezoidal quadrature. Integrate conditional Gaussian mixtures for latent/tau summaries.
- Decision: compare latent means with a noncentered PyMC 6.3.0 NUTS parameterization of the exact same model. Require shared NUTS diagnostics, endpoint masses, normalization, mode gradients, positive Hessians, and RMSE at most 0.15. Rust recomputes worker-reported grid and diagnostic gates. Even valid output remains `approximate_only`.
- Alternatives: label scalar Laplace as nested inference; omit the Jacobian or determinant; normalize grid points as equal discrete masses; compare with a different prior/likelihood; trust worker booleans; use an unavailable external R-INLA environment; introduce a general approximation framework before its next caller.
- Consequences: `RunInlaStyleApproximation` has one runnable established-backend small-model owner and NUTS comparison. Sparse non-diagonal GMRFs, adaptive integration designs, simplified/full marginal corrections, 2-D/SPDE models, external INLA agreement, calibration, and scientific claims remain open.

## DEC-0085 — Keep the held-out scientific unit explicit in PSIS-LOO

- Date: 2026-08-24
- Status: accepted for BAY-PSIS-LOO-01
- Context: pointwise log likelihood alone does not prove that each column is a valid independent leave-one-out unit. The pinned ArviZ API already owns maintained Pareto smoothing and its sample-size-dependent reliability threshold, while its former top-level `psislw` helper is not present in ArviZ 1.3.0.
- Decision: accept one complete chain/draw/unit log-likelihood matrix plus a required held-out-unit declaration and explicit relative MCMC efficiency. Canonicalize and validate the full rectangular matrix in Rust, then call pinned ArviZ 1.3.0/arviz-stats 1.3.1 `loo(pointwise=True)` through a static source-bound worker.
- Decision: retain every pointwise ELPD/Pareto-k and exact high-k unit. Recompute totals, maximum k, threshold membership, warning, and recommendations in Rust. High-k output remains available but is labeled `requires_refit_or_kfold`; no unavailable model refit is invented.
- Alternatives: treat rows or cells as implicitly independent; implement generalized-Pareto fitting natively; use the retired API name; omit chain shape/relative efficiency; suppress high-k units; reject all output when one unit is influential; build generalized model comparison before a concrete PSIS result exists.
- Consequences: `PSISLOO` has a runnable maintained-backend owner with benign exact-LOO agreement and an influential-unit warning oracle. Exact refits, grouped K-fold execution, transformed-response Jacobians, mixture importance sampling, moment matching, compatible model comparison, calibration, and scientific claims remain open.

## DEC-0086 — Compare only compatibility-bound pointwise predictive artifacts

- Date: 2026-08-24
- Status: accepted for BAY-COMPARE-01
- Context: equal-length ELPD arrays do not prove that models target the same observations, likelihood, preprocessing, or held-out scientific unit. Aggregate ELPD differences also lose the pointwise variation needed for comparison uncertainty.
- Decision: extend the immediate PSIS producer with exact model name/likelihood target and declared data/preprocessing SHA-256 identities. Compare only strict published PSIS artifacts whose target, identities, held-out-unit kind, and ordered unit IDs match exactly. Revalidate each artifact from physical JSON before use.
- Decision: report all lexical pairwise A-minus-B pointwise ELPD sums with `sqrt(n*sample variance)` SE and deterministic descending-ELPD ranks. Preserve all source identities and Pareto reliability; any high-k source propagates `requires_refit_or_kfold`. Do not add stacking or generalized comparison abstractions without an immediate validated caller.
- Alternatives: compare totals only; align units by position without IDs; accept mismatched preprocessing; suppress Pareto warnings after ranking; auto-select the highest score; introduce stacking before its validation fixture; require equal posterior sample counts despite compatible held-out predictive targets.
- Consequences: `CompareBayesianModels` has a runnable native compatibility/arithmetic owner using established PSIS artifacts. Stacking, model weights, exact refits, grouped K-fold, multiple likelihood targets, calibration, model-selection decisions, and scientific claims remain open.

## DEC-0087 — Validate SBC mechanics first against an exchangeable exact posterior

- Date: 2026-08-24
- Status: accepted for BAY-SBC-01
- Context: simulation-based calibration combines generative simulation, inference, randomized ranks, failures, uniformity, coverage, and autocorrelation handling. Running many NUTS fits first would confound calibration-procedure defects with sampler convergence and serial-correlation defects.
- Decision: implement one bounded typed Normal-mean/known-sigma SBC workflow using the exact conjugate posterior and independent posterior draws through pinned NumPy/SciPy. Derive a separate deterministic seed for every replicate's simulation and posterior draws; retain strict-below ranks with randomized tie handling and every truth/posterior/coverage/z/shrinkage value.
- Decision: use a bounded 20-bin discrete-uniform chi-square diagnostic, rank ECDF with a declared 1% DKW envelope, 95% coverage standardized error, z mean/SD, shrinkage, and failure rate. Rust recomputes all mechanics except SciPy's chi-square survival probability. Explicitly state that autocorrelation correction is unnecessary only for these independent draws.
- Alternatives: begin with repeated NUTS and ambiguous failures; use one global random stream; drop failed fits; assume continuous draws make tie policy irrelevant; report ranks without uniformity/coverage; add a general calibration framework before one typed caller; claim exact-posterior SBC calibrates PyMC.
- Consequences: `SimulationBasedCalibration` has a runnable tractable owner with byte-deterministic rank/coverage evidence. NUTS/SMC/VI/Laplace SBC, autocorrelation-adjusted ranks, multivariate quantities, hierarchical/field/point-process calibration, power, and scientific claims remain open.

## DEC-0088 — Tie prior sensitivity to posterior, prediction, and one declared decision

- Date: 2026-08-24
- Status: accepted for BAY-SENS-01
- Context: a table of posterior means alone neither tests predictive consequences nor identifies whether an actual declared conclusion changes. Selecting an alternative prior because it yields a preferred result would invert the purpose of sensitivity analysis.
- Decision: expose one exact conjugate Normal-mean workflow over 2–32 caller-declared named priors with one base. For every prior compute exact posterior mean/SD, exact leave-one-out predictive ELPD, and `P(mu>threshold)` for a caller-declared threshold/probability decision. Report all base differences and exact conclusion changes/material mean shifts.
- Decision: use the pinned SciPy worker for Normal tail probability while Rust independently recomputes conjugate posterior, leave-one-out predictive, delta, decision-threshold, and flag mechanics. Preserve exact observation/prior/request identities and state that scientific plausibility of the grid remains external.
- Alternatives: compare posterior means only; use in-sample likelihood; choose the most favorable prior; silently invent a decision threshold; importance-reweight without diagnostics; build a general sensitivity framework before one typed caller; rerun NUTS for an exactly tractable model.
- Consequences: `RunPriorSensitivity` has one runnable tractable owner with posterior/predictive/decision evidence. Hierarchical/field priors, reweighting diagnostics, sampler refits, multiple decision quantities, elicitation, real-data plausibility, and scientific claims remain open.

## DEC-0089 — Prove quadrature coverage by deriving it from a rectangle grid

- Date: 2026-08-24
- Status: accepted for BAY-IPP-LIKE-01
- Context: arbitrary supplied nodes and weights can sum to window area without covering the window, so that check alone cannot satisfy the point-process likelihood's integration contract. Existing polygon/window infrastructure is entangled with the explicitly paused root workflow and is not generalized for this milestone.
- Decision: admit one exact half-open micrometre rectangle and a complete regular midpoint grid. Require one `(ix,iy)` covariate/offset row per cell, derive all midpoint geometry and equal weights internally, cap the grid at one million nodes, and enforce event membership in the exact rectangle.
- Decision: expose one fixed intercept/one-covariate log-linear likelihood with compensated event/integral sums and strict non-finite failure. Retain exact input digests, units, rectangle/grid/cell geometry, terms, and claim ceiling. Do not introduce general quadrature, polygon, field-interpolation, or fitted-model infrastructure before its immediate caller.
- Alternatives: accept arbitrary weights by sum only; reuse paused PLAT/root geometry; approximate a polygon by its bounding box; hide physical units; saturate exponential overflow; jump directly to fitted IPP/LGCP; introduce a general formula engine.
- Consequences: `InhomogeneousPoissonLogLikelihood` has a runnable native rectangular owner and constant-intensity oracle. Multiple covariates, arbitrary exact windows, quadrature refinement, Berman–Turner data, fitted Bayesian IPP, residuals, posterior prediction, LGCP, and scientific claims remain open.

## DEC-0090 — Fit the exact rectangular likelihood before generalizing point processes

- Date: 2026-08-24
- Status: accepted for BAY-IPP-FIT-01
- Context: fitting against a Poisson cell-count surrogate without the event term would not exercise the IC-0062 point-process likelihood. Conversely, continuous posterior prediction within cells would claim interpolation unavailable from midpoint-only covariates.
- Decision: fit the exact IC-0062 event-sum minus derived-grid integral as a PyMC Potential with caller-declared Normal intercept/coefficient priors. Require materially varying grid covariate, canonical event-to-cell counts, bounded NUTS/draw-cell work, and shared diagnostics.
- Decision: report posterior cell intensity, expected count, and Pearson residual at every derived midpoint. Simulate posterior-predictive Poisson cell counts under the explicit piecewise-constant quadrature approximation; do not fabricate continuous locations or quadrature convergence. Rust revalidates cell geometry/data bindings, supports, totals, and diagnostic state.
- Alternatives: fit ordinary Poisson regression on cell counts; omit event covariates; interpolate a continuous surface; simulate uniform within-cell locations without declaring the approximation; introduce arbitrary formula/polygon support; move directly to LGCP; implement NUTS natively.
- Consequences: `FitBayesianInhomogeneousPoisson` has one runnable bounded maintained-backend owner with positive-effect synthetic recovery. Multiple predictors, replicated patterns, quadrature refinement, arbitrary windows, residual-process diagnostics, continuous posterior simulation, LGCP, calibration, and scientific claims remain open.

## DEC-0091 — Make Berman–Turner weights a cell partition and retain refinement

- Date: 2026-08-24
- Status: accepted for BAY-BT-01
- Context: simply unioning observed and dummy points with the full cell weight would overcount window measure, while returning one table without a finer comparison would omit the pseudocode's required resolution/convergence evidence.
- Decision: in each regular rectangle cell, split exact cell area equally among every observed event and one midpoint dummy. Use namespaced deterministic node IDs, `1/weight` observed response, zero dummy response, and the parameter-dependent weighted Poisson objective. Require positive weights and compensated total equal to exact window area.
- Decision: construct both a coarse and nested integer-multiple fine table, cap each output at 100,000 nodes, retain the full fine table, and report objective change against caller tolerance. General covariate convergence remains empirical; only constant intensity has exact IC-0062 equivalence.
- Alternatives: assign full cell weight to every node; omit observed nodes; omit dummy nodes in occupied cells; accept unrelated grids; report only the finest resolution; hide parameter-independent constants as full Poisson likelihood; build arbitrary Voronoi weights before exact geometry ownership.
- Consequences: `BuildBermanTurnerData` has a runnable rectangular owner with refinement artifact and exact constant oracle. Arbitrary windows/tessellations, adaptive dummy placement, fitted GLM comparison, multiple predictors, stronger convergence studies, and scientific claims remain open.

## DEC-0092 — Construct LGCP cells and covariance before fitting the latent field

- Date: 2026-08-24
- Status: accepted for BAY-LGCP-BUILD-01
- Context: an LGCP fit is not interpretable until cell/window intersection, event counting, covariate evaluation, physical kernel distance, jitter, and covariance positive definiteness have one exact artifact. The available rectangle grid permits exact full cells but not arbitrary polygon intersections.
- Decision: build a 2–64-cell exact half-open rectangle model with center-evaluated covariate/offset, exact event-to-cell counts, and caller Normal fixed-effect priors. Use a zero-mean dense two-dimensional Euclidean Matérn-3/2 field with physical length scale, diagonal-only jitter, and deterministic Cholesky proof.
- Decision: retain every cell, exact covariance values/digest, storage/work counts, and a typed Poisson area-offset likelihood declaration. Return `not_fitted`; do not add posterior fields, continuous interpolation, mesh semantics, or reuse the existing one-dimensional GP as if it were two-dimensional.
- Alternatives: fit before validating cells; treat the existing 1-D GP as a 2-D field; approximate polygons by rectangles; omit event-count conservation; add jitter off diagonal; hide center evaluation; introduce SPDE without its mesh prerequisite.
- Consequences: `BuildGriddedLGCP` has a runnable bounded exact-cell/model owner. Fitted latent fields, inferred hyperparameters, posterior prediction, refinement/calibration, arbitrary windows, SPDE, replicated patterns, and scientific claims remain open.

## DEC-0093 — Fit the validated dense LGCP through its exact supplied factor

- Date: 2026-08-24
- Status: accepted for BAY-LGCP-FIT-01
- Context: reconstructing a covariance or field discretization inside the numerical backend could drift from IC-0065, while centered latent sampling would add avoidable posterior geometry cost. Inferring weakly identified kernel hyperparameters in the first nine-cell synthetic workflow would also expand the scientific claim before calibration evidence exists.
- Decision: retain fixed caller-declared amplitude, physical length, and jitter; send IC-0065's exact dense covariance and deterministic lower Cholesky to a source-bound PyMC 6.3.0 worker; and sample independent standard-Normal field coordinates with a noncentered Cholesky transform. Verify covariance symmetry, triangularity, positive diagonal, and factor product before fitting.
- Decision: use the exact cell-area Poisson likelihood and shared NUTS diagnostic gates. Retain every cell's latent effect, intensity, expected count, Pearson residual, and posterior-predictive total/zero-cell summaries while binding exact model/data/covariance/request identities. Complete output stays experimental.
- Alternatives: rebuild the kernel in Python; use a centered multivariate Normal; infer kernel amplitude/length immediately; reuse the event-sum IPP Potential without the latent field; fabricate continuous within-cell locations; introduce mesh/SPDE fitting before its exact 2-D mesh prerequisite.
- Consequences: `FitGriddedLGCP` has a runnable bounded maintained-backend owner with positive fixed-effect and nonconstant latent-field recovery. Inferred kernels, retained posterior draws, explicit replicated pattern artifacts, continuous interpolation, arbitrary windows, refinement/calibration, SPDE, and scientific claims remain open.

## DEC-0094 — Materialize gridded LGCP replicas without inventing continuous intensity

- Date: 2026-08-24
- Status: accepted for BAY-LGCP-PPC-01
- Context: aggregate predictive count summaries do not satisfy `SimulateLGCPPosteriorPredictive`, but the admitted center-evaluated grid has no information that could justify continuous within-cell variation, an adaptive intensity bound, or thinning. Publishing raw posterior arrays in the existing fit schema would also expand that result solely for a downstream workflow.
- Decision: add a separate predictive command that runs the exact IC-0066 lifecycle, deterministically selects one exact chain/draw per requested replica, and materializes patterns before the posterior arrays leave the source-bound worker. Sample cell counts from the selected expected counts and locations uniformly inside exact full cells, which is exact for the declared piecewise-constant discretization.
- Decision: retain every selected fixed/latent draw, intensity, expectation, count, point ID/cell/coordinate, and approximation limitation. Rust independently recomputes posterior index selection and intensities, validates every point against its cell, enforces realized-point/output bounds, and permits publication only for complete fits. The existing fitted-LGCP result schema remains unchanged.
- Alternatives: expose arbitrary posterior arrays from the fit; reconstruct draws from marginal summaries; claim a continuous field; interpolate cell centers silently; use thinning without a certified bound; omit point coordinates and call counts a point pattern; proceed to SPDE before exact mesh ownership.
- Consequences: `SimulateLGCPPosteriorPredictive` has a runnable deterministic exact-cell owner with byte-repeatable replicated patterns. Spatial K/g envelopes, continuous/refined intensity, arbitrary windows, inferred kernels, SBC, model adequacy, replicated hierarchical LGCPs, SPDE, and scientific claims remain open.

## DEC-0095 — Use pinned distribution samplers and an exact dilated rectangle for Thomas simulation

- Date: 2026-08-24
- Status: accepted for BAY-THOMAS-SIM-01
- Context: a Thomas simulator needs scientifically standard Poisson and Normal draws plus reproducible streams. Hand-rolling rejection algorithms would add numerical risk, while sampling parents only in the observed rectangle creates boundary bias and sampling from an expanded bounding box without rejection changes the parent measure.
- Decision: add direct pinned `rand` 0.8.6, `rand_chacha` 0.3.1, and `rand_distr` 0.4.3 dependencies to `marklab-bayes`; all are already present in the locked dependency graph. Use seeded ChaCha20 with maintained Poisson/Normal samplers. These dependencies become the shared immediate simulation boundary for the Thomas workflow and its next matched Matérn-cluster caller.
- Decision: dilate the exact rectangle by six Gaussian SDs, sample a homogeneous parent process over the exact rounded Minkowski shape by bounded-box rejection, and retain the exact expanded area and radial Gaussian tail bound. Abort on realized resource overflow and retain latent parents plus generated/retained/discarded offspring accounting.
- Alternatives: implement Poisson/Normal samplers locally; use operating-system entropy; sample parents only inside the observed window; silently use the expanded bounding box; use an unbounded Gaussian expansion; omit latent parents or discarded offspring.
- Consequences: `SimulateThomasProcess` can have deterministic maintained distribution mechanics and explicit finite boundary approximation. Exact infinite-plane simulation, adaptive expansion, arbitrary windows, fitted cluster inference, adequacy, biology, and clinical claims remain open.

## DEC-0096 — Share exact parent-window mechanics but preserve Matérn disc semantics

- Date: 2026-08-24
- Status: accepted for BAY-MATERN-CLUSTER-SIM-01
- Context: Thomas and Matérn cluster processes share homogeneous Poisson parents on a dilated rectangle, seeded streams, and resource accounting, but their offspring laws and boundary claims differ materially. Duplicating parent rejection risks geometric drift; collapsing both into one public generic simulator would obscure those scientific differences.
- Decision: reuse narrow crate-private seed, Poisson-count, exact dilation-area, rounded-rectangle rejection, and half-open membership functions from the immediate Thomas owner. Keep a separate typed Matérn API/result and sample offspring uniformly on the disc using square-root radius and uniform angle.
- Decision: expand by exactly the bounded offspring radius and label that parent-window treatment exact for the declared rectangle process. Retain latent parents, parent-child identities, all generated/retained/discarded counts, distribution identity, and caps independently from the Thomas result.
- Alternatives: duplicate all geometry; sample radius uniformly; sample parents from the expanded bounding rectangle; reuse Gaussian displacement; introduce a speculative public Neyman–Scott framework; omit latent parents; call the simulator fitted inference.
- Consequences: `SimulateMaternClusterProcess` has a deterministic exact-window bounded owner sharing only stable mechanics with Thomas. Latent-parent inference, arbitrary windows, cluster adequacy, hierarchical replication, SBC, biology, and clinical claims remain open.

## DEC-0097 — Fit Thomas K contrast while naming the missing likelihood comparison

- Date: 2026-08-25
- Status: accepted for BAY-THOMAS-MC-01
- Context: the pseudocode admits minimum contrast but requires fitting-range/weight sensitivity and likelihood comparison where feasible. The local environment has a pinned SciPy optimizer but no admitted latent-parent likelihood backend, and the K curve does not separately identify mean offspring without observed intensity.
- Decision: fit the established Thomas K formula through source-bound SciPy 1.18.1 least squares on log kappa/log sigma with exact caller bounds and fourth-root contrast. Require caller point intensity and derive `mu=intensity/kappa`. Run caller-weighted full-range primary, unit-weight full-range, and caller-weighted interior-range fits.
- Decision: retain full optimizer state and fitted-curve arithmetic, and revalidate all formulas/objectives in Rust. Publish likelihood comparison as typed unavailable with the BAY-CLUSTER-FIT-01 backend reason; do not substitute the simulator or call contrast a likelihood.
- Alternatives: optimize natively; fit raw K without declared transform; omit range/weight sensitivity; infer mu from K alone; fabricate latent parents; report optimizer success without independent arithmetic; block all independent cluster work on RJMCMC.
- Consequences: `FitClusterProcessMinimumContrast` has a runnable typed Thomas owner and exact-curve recovery oracle. Noisy finite-sample calibration, Matérn contrast, likelihood agreement, latent parents, arbitrary windows, model adequacy, biology, and clinical claims remain open.

## DEC-0098 — Make Strauss pair and insertion boundaries identical

- Date: 2026-08-25
- Status: accepted for BAY-STRAUSS-STAT-01
- Context: a Strauss sufficient statistic and Papangelou insertion intensity disagree if one uses `<R` and the other `<=R`, counts ordered pairs, or silently permits insertion on an existing point. Gamma zero also makes an unstated `0^0` convention observable.
- Decision: admit exact simple finite point patterns and use Euclidean `distance<=R` for both unordered existing pairs and proposal neighbors. Reject coordinate-duplicate insertion; define gamma-zero intensity as beta for zero neighbors and zero otherwise; cap exact O(n^2) pair visits before execution.
- Decision: expose separate native `StraussStatistics` and `StraussPapangelou` APIs with one CLI artifact binding their common radius and pattern identity. Retain counts, proposal, beta/gamma, pair work, and claim ceiling without a partition function or fit label.
- Alternatives: ordered pairs; inconsistent radius boundaries; allow duplicate insertion; rely on floating `pow(0,0)` implicitly; call the unnormalized statistic a likelihood; add a spatial index before a workload requires it.
- Consequences: both pseudocode mechanics have exact hand-oracle owners and can serve the immediate Gibbs simulator. Fitting, normalization, edge-conditioned inference, scaling, adequacy, biology, and clinical claims remain open.

## DEC-0099 — Retain finite-chain evidence for Strauss birth/death simulation

- Date: 2026-08-25
- Status: accepted for BAY-GIBBS-SIM-01
- Context: birth/death acceptance requires the area/proposal Jacobian and leave-one-out Papangelou intensity. Returning only a terminal pattern would hide null transitions, finite burn-in, count drift, resource consumption, and whether either move type was accepted.
- Decision: start empty and use equal birth/death proposals with exact rectangle-uniform births and uniform existing-point deaths. Apply the standard area/count Metropolis ratios with IC-0071 inclusive-radius/gamma-zero mechanics and a domain-separated pinned ChaCha20 stream.
- Decision: retain the complete bounded count trace, proposal/acceptance/null/rejection counts, neighbor visits, maximum/final count, post-burn overall and half means/drift, and the gamma-one Poisson expected count. Label the terminal pattern finite-chain experimental simulation regardless of favorable diagnostics.
- Alternatives: omit Hastings area/count factors; force birth from empty instead of retaining a null death; discard the trace; claim the final state exact; use unbounded iterations/point work; introduce a generalized Papangelou callback before another typed process exists.
- Consequences: `SimulateGibbsBirthDeath` has a runnable bounded Strauss owner with byte-repeatable Poisson special-case evidence. General mixing validation, perfect simulation, fitted Gibbs inference, arbitrary windows, scaling, adequacy, biology, and clinical claims remain open.

## DEC-0100 — Fit Strauss pseudolikelihood on exact area-partition tables at two resolutions

- Date: 2026-08-25
- Status: accepted for BAY-GIBBS-PL-01
- Context: fitting only one dummy grid hides quadrature sensitivity, while ordinary Poisson regression without observed/dummy area partition or leave-one-out observed features does not represent the Strauss pseudolikelihood.
- Decision: build nested coarse/fine Berman–Turner-style tables in Rust, split every cell's exact area among its observations and one dummy, and compute IC-0071 inclusive-radius neighbor counts with self excluded at observed nodes. Bound table and neighbor work before execution.
- Decision: use source-bound SciPy 1.18.1 for constrained log-beta/log-gamma weighted-Poisson optimization. Retain exact gradient/Hessian conditioning, inverse-Hessian SE, 2x2-window-block sandwich SE, objective improvement, and coarse/fine sensitivity; Rust recomputes the objective and result state.
- Alternatives: one grid; full cell weight per node; include an observed point as its own neighbor; unconstrained gamma above one; call pseudolikelihood normalized likelihood; omit robust/refinement evidence.
- Consequences: `FitGibbsPseudolikelihood` has a runnable bounded inhibitory-Strauss owner. Likelihood normalization, exchange MCMC, arbitrary windows, large-scale indexing, calibration, biology, and clinical claims remain open.

## DEC-0101 — Keep Geyer saturation pointwise and distinct from attractive Strauss

- Date: 2026-08-25
- Status: accepted for BAY-GEYER-STAT-01
- Context: attractive Strauss gamma above one is generally not a valid substitute for a saturated stable model, and Geyer statistic conventions can count saturation pointwise or through alternative density parameterizations.
- Decision: count inclusive-radius unordered pairs into endpoint neighbor counts and return `sum_i min(s,n_i)`, retaining every raw/saturated value and exact pair work. Saturation zero is explicitly zero. Do not attach a fitted interaction parameter or stability claim.
- Consequences: `GeyerSaturationStatistic` has an exact fixed-pattern owner. Stable Geyer simulation/fitting, normalization, calibration, biology, and clinical claims remain open.

## DEC-0102 — Require a complete symmetric matrix for the first multitype conditional intensity

- Date: 2026-08-25
- Status: accepted for BAY-MULTITYPE-PAP-01
- Context: missing type-pair rows, implicit zero interactions, asymmetric radii, or silent directionality make a multitype Gibbs model ambiguous. A general spatial baseline-field evaluator has no current typed owner.
- Decision: admit exact constant log baselines and a complete exact-symmetric ordered KxK potential/radius table. Apply each proposal-to-existing pair under an inclusive pair-specific radius, retain every contribution, and reject exponent overflow. Directional and spatially varying models remain separate.
- Consequences: `MultitypePapangelou` has an exact typed owner. Fitting, regularization, multiplicity, varying baselines, simulation, calibration, biology, and clinical claims remain open.

## DEC-0103 — Factor joint categorical marks conditionally on exact-grid locations

- Date: 2026-08-25
- Status: accepted for BAY-JOINT-MARK-01
- Context: a mark-only classifier does not model locations, while separate type-specific point processes do not express conditional mark allocation or a nested random-labeling comparison. Softmax fields are unidentified without a reference.
- Decision: combine the exact rectangle/grid location component with a reference-category softmax mark component using mark covariate, declared neighborhood effect, and independent nonreference fields. Fix all reference parameters/field to zero and require comparison to zero neighborhood/field random labeling.
- Consequences: `BuildJointLocationMarkModel` has a typed construction owner. Fitting, shared fields, posterior comparison, calibration, biology, and clinical claims remain open.

## DEC-0104 — Identify the continuous shared field by fixing its location loading

- Date: 2026-08-25
- Status: accepted for BAY-JOINT-CONT-MARK-01
- Context: shared-field scale and two unrestricted loadings are aliased, and sign can flip with the field. Separate private fields must not be silently conflated with shared correlation.
- Decision: fix shared Matérn amplitude/length/jitter and location loading one, require a positive mark loading, and keep independent private location/mark fields. Require nested zero-shared-loading comparison before joint structure is supported.
- Consequences: `BuildJointContinuousMarkModel` has an identified typed construction owner. Posterior fitting, inferred kernels, comparison evidence, calibration, biology, and clinical claims remain open.

## DEC-0105 — Identify embedding factor rotation through a positive lower-triangular loading block

- Date: 2026-08-25
- Status: accepted for BAY-EMBED-FACTOR-01
- Context: unconstrained latent factors/loadings admit rotations and sign flips, while high-dimensional location coefficients overfit without shrinkage.
- Decision: require ordered embedding dimensions, constrain the first K loading rows lower triangular with positive diagonal, use fixed-scale physical Matérn factor fields, and regularize remaining loadings/location-factor coefficients. Require simpler vector-variogram and kernel-mark-correlation comparisons.
- Consequences: `JointLocationEmbeddingLatentFactorModel` has a typed construction owner. Fitting, selected factor interpretation, calibration, domains, biology, and clinical claims remain open.

## DEC-0106 — Preserve replicated LGCP windows and make field sharing explicit

- Date: 2026-08-25
- Status: accepted for BAY-REPL-LGCP-01
- Context: concatenating replicate patterns invents cross-window distances and erases patient/specimen units; an unspecified shared field confounds population, patient, and replicate variation.
- Decision: bind each pattern's window/grid/covariate identity, retain patient nesting, and require either shared-hyperparameter independent fields or patient-shared plus replicate fields with separate amplitudes. Never concatenate likelihoods into one window.
- Consequences: `ReplicatedHierarchicalLGCP` has a typed construction owner. Joint fitting, calibration, field-policy comparison, biology, and clinical claims remain open.

## DEC-0107 — Use identical translation-K estimators and a max-deviation simultaneous PPC envelope

- Date: 2026-08-25
- Status: accepted for BAY-PP-DIAG-01
- Context: comparing observed and replicated curves with different edge correction is invalid, while pointwise intervals do not control a curve-level excursion and a binary PPC pass implies more than diagnostics support.
- Decision: use exact rectangle translation correction for every observed/replicated pattern, aggregate patterns identically, and form a standardized maximum-deviation simultaneous envelope at caller alpha. Retain exceeded radii descriptively and state that consistency is not model truth.
- Consequences: `PosteriorPredictivePointProcessDiagnostics` has a count/K owner. g/F/G/J/mark summaries require their own same-estimator contracts; calibration and scientific claims remain open.

## DEC-0108 — Keep complete vectors intact and isolate training-fitted projection

- Date: 2026-08-25
- Status: accepted for EMB-VARIO-01, EMB-PROJ-VARIO-01, and EMB-CROSS-COV-01
- Context: coordinate-wise embedding tests are rotation-dependent, fitting PCA on held-out rows leaks evaluation information, pointwise component/scale p-values do not control their joint family, and an unsymmetrized outer product depends on arbitrary unordered-pair orientation.
- Decision: make squared Euclidean vector semivariance the primary exact invariant curve; fit centered PCA only on training biological units through pinned SciPy 1.18.1, freeze and independently validate its eigenvectors before applying it to held-out splits, random-label complete projected vectors within declared strata, and use single-step max-T across component × scale. For matrix summaries, center once globally and symmetrize each undirected distance-bin cross-covariance before reporting trace/Frobenius invariants and the full matrix artifact. Keep these bounded synthetic workflow owners beside the immediate joint embedding model consumer; do not claim the still-missing canonical real-asset geometry/provenance foundation.
- Consequences: the three pseudocode functions have runnable, typed owners and behavior oracles. Real `CellEmbeddingTable` admission, canonical shared spatial plans/edge correction, technical-confounder validation, patient-level comparison, null calibration beyond the projected workflow, and stable EMB-01/WS-50 promotion remain open.

## DEC-0109 — Make cross-modal eligibility and exchangeability explicit

- Date: 2026-08-25
- Status: accepted for EMB-CROSS-MODAL-COV-01
- Context: aligning modality rows by position fabricates correspondence, arbitrary pair orientation/weight defaults change the estimand, and unrestricted relabeling breaks section and compartment structure.
- Decision: require an exact A-ID/B-ID/source-section/compartment/bin/positive-weight pair plan. Center globally or by declared compartment policy, normalize rectangular outer products by bin weight, report full matrices plus Frobenius invariants, and relabel complete B vectors only within source-section × compartment strata. Use single-step max-T across physical bins.
- Consequences: `CrossModalCovarianceByDistance` has a bounded synthetic owner. Registration/correspondence validation, missing-modality policy, patient-level comparison, real multimodal data, biology, and clinical claims remain open.

## DEC-0110 — Freeze kernel preprocessing and scale from training units

- Date: 2026-08-25
- Status: accepted for EMB-KERNEL-01
- Context: centering or bandwidth selection on validation/test rows leaks evaluation information, while an unnamed similarity may not be positive semidefinite and a nonpositive global reference makes ratio normalization invalid.
- Decision: fit feature centering only on training biological units; use exact centered linear/cosine kernels or freeze RBF/Laplacian scale at the median positive training-pair Euclidean/L1 distance. Label the exact four formulas PSD, retain the artifact, compute complete split-specific global references, and return typed unavailable normalization whenever the reference does not exceed caller tolerance.
- Consequences: `BuildEmbeddingKernel` and `KernelMarkCorrelation` have one immediate runnable workflow. Learned kernels, nested outcome validation, approximate pair sampling, random-label inference, real technical-confounder evidence, and stable promotion remain open.

## DEC-0111 — Random-label complete embedding rows and reuse canonical ERL semantics

- Date: 2026-08-25
- Status: accepted for EMB-GLOBAL-ENV-01
- Context: independently permuting embedding dimensions destroys the observed multivariate mark distribution, pointwise bands do not control the curve family, and the repository's canonical ERL implementation is private to the paused root package.
- Decision: permute complete vector rows within exact declared strata, recompute the full vector-semivariogram curve, and reproduce the canonical average-tie, sorted extreme-rank-vector, normalized-depth ERL algorithm with a plus-one global p-value and simultaneous depth envelope. Keep this owner local rather than modifying the explicitly paused PLAT/root tree; preserve byte-deterministic seeded order and bounded work.
- Consequences: `TestEmbeddingSpatialDependence` is runnable for the vector-semivariogram curve. Additional curve functions can enter only with an immediate caller; general inference ownership, calibration, edge correction, patient-level designs, and real-asset promotion remain open.

## DEC-0112 — Own one exact symmetric graph contract across global, null, and local roughness

- Date: 2026-08-25
- Status: accepted for EMB-GRAPH-ENERGY-01, EMB-GRAPH-PERM-01, and EMB-LOCAL-ROUGH-01
- Context: hidden directed-edge duplication changes the Laplacian quadratic form and edge-weight denominator, normalized Laplacians are undefined at islands, coordinate-wise permutation destroys vector marks, and local roughness values are not multiplicity-controlled hotspots.
- Decision: represent `W` as unique unordered positive-weight edges implying a symmetric zero-diagonal matrix and bind one canonical graph digest. Support combinatorial and symmetric-normalized Laplacians; require positive degrees for the latter. Define NONE/SIGNAL/full-symmetric-EDGE_WEIGHT denominators explicitly. For inference, permute complete signal rows within strata and use the inclusive plus-one low-energy alternative. For local output, retain islands with epsilon denominator/typed status and prohibit inferential labels.
- Consequences: `GraphDirichletEnergy`, `GraphSmoothnessPermutationTest`, and `LocalEmbeddingRoughness` share one bounded synthetic graph owner. Canonical FND-03 graph provenance/scale, graph-selection calibration, multiplicity-controlled local inference, patient-level validation, real data, and stable GSP/WS-62 promotion remain open.

## DEC-0113 — Reuse C-05 links and aggregate patch dependence before effective count

- Date: 2026-08-25
- Status: accepted for EMB-CELL-PATCH-CONTEXT-01 and EMB-PATCH-DEPENDENCY-01
- Context: C-05 already owns exact cell/patch IDs, context/scale, declared rational weights, shared vector-free references, overlap components, physical receipts, and link identity. A second link validator would duplicate and weaken that authority; raw linked-patch count would also overstate independent information when patches overlap.
- Decision: map pseudocode `ValidateCellPatchLinks` to the existing `CellPatchLink::{derive_contained_shared,from_declared_weighted_interpolation}` constructors and receipt validation. Build context only after table/link/overlap identities agree and all linked vectors are present. Normalize equal or exact declared weights, accumulate the vector in `f64`, aggregate normalized mass by `PatchOverlapGraph` component, and define descriptive effective count as Kish `1/sum(component_weight^2)`. Retain `patient_not_patch` as the mandatory inferential-unit policy.
- Consequences: `ValidateCellPatchLinks`, `CellPatchContext`, and `PatchDependencyWeighting` have canonical production owners without another graph/link schema. Real cell/patch source correspondence, predictive complementarity, calibration, patient-held-out outcomes, biology, and stable FR-02/WS-51 promotion remain open.

## DEC-0114 — Nest every predictive choice inside patient folds and reuse paired inference

- Date: 2026-08-25
- Status: accepted for EMB-COMPLEMENT-01
- Decision: run M0–M5 standardization and ridge-alpha selection solely inside inner patient folds through pinned SciPy 1.18.1, refit on outer training, predict each patient once, retain calibration, and reuse Marklab's paired patient sign-flip owner for six prespecified absolute-error increments. Synthetic success cannot establish real incremental value.

## DEC-0115 — Keep multiscale weights prespecified and retrieval exact

- Date: 2026-08-25
- Status: accepted for EMB-MULTISCALE-KERNEL-01 and EMB-RETRIEVAL-01
- Decision: combine exact aligned scale summaries with positive caller-prespecified sum-one weights and report drop-one-scale sensitivity. Fit retrieval standardization only on training regions, use exact Euclidean search with recall one, enforce patient/site filters, decompose distance by component, and label matches analogous rather than biologically identical.

## DEC-0116 — Propagate fingerprint endpoint uncertainty without upgrading the inferential unit

- Date: 2026-08-25
- Status: accepted for COH-REGION-COMPAT-01
- Decision: reuse the exact versioned fingerprint distance, interpret its endpoint uncertainty values as independent standard uncertainties, and propagate through each curve L2 norm by the first-order delta method. Combine weighted component uncertainty by root-sum-square, but return unavailable at a zero-distance nondifferentiable component with positive uncertainty. Keep the result within-patient descriptive only.

## DEC-0117 — Separate OOD fitting, threshold calibration, and abstention application

- Date: 2026-08-25
- Status: accepted for EMB-OOD-01 and EMB-ABSTAIN-01
- Decision: fit population mean/covariance on training representations, shrink only off-diagonal covariance by a caller-prespecified amount, calibrate a nearest-rank Mahalanobis threshold on domains held out from training, and score test units only. Apply separately prespecified uncertainty/OOD thresholds with strict exceedance and suppress abstained predictions while retaining canonical reasons.

## DEC-0118 — Keep calibration and conformal quantiles on disjoint patient splits

- Date: 2026-08-25
- Status: accepted for EMB-CALIBRATE-01 and EMB-CONFORMAL-01
- Decision: fit Platt calibration only on patient-level OOF scores and reserve test labels for evaluation. For binary conformal prediction, fit a regularized model on train patients, compute `1-p(true label)` on separate calibration patients, use corrected rank `ceil((n+1)(1-alpha))`, and construct test sets without test-label access. Use pinned SciPy 1.18.1 for both numerical fits and make Rust replay probabilities, metrics, quantiles, sets, and coverage accounting.

## DEC-0119 — Fuse declared OOF probabilities with explicit availability indicators

- Date: 2026-08-25
- Status: accepted for EMB-LATE-FUSION-01
- Decision: require every base-probability row to declare `patient_level_out_of_fold`, use neutral probability `0.5` plus an availability indicator for a missing modality, fit a positive-L2 logistic meta-model on meta-training patients, and fit Platt calibration on a disjoint calibration split. Report observed missingness scenarios and test-time modality ablations as predictive diagnostics, never causal contributions.

## DEC-0120 — Optimize fusion only on patient-held-out predictive evidence

- Date: 2026-08-25
- Status: accepted for EMB-STACKING-01 and EMB-MOE-01
- Decision: predictive stacking maximizes patient-grouped mixture log density under a simplex and reports exact leave-one-patient refit sensitivity; its weights are not posterior probabilities. Mixture-of-experts gates only declared patient-OOF expert probabilities, excludes named technical shortcut contexts, masks unavailable experts exactly, and uses positive L2 plus entropy regularization. Both use pinned SciPy 1.18.1 with strict Rust response replay; gating calibration and context OOD thresholds use disjoint calibration patients.

## DEC-0121 — Separate balanced, relaxed-marginal, and fixed-mass transport semantics

- Date: 2026-08-25
- Status: accepted for REG-SINKHORN-01, REG-UNBAL-OT-01, and REG-PARTIAL-OT-01
- Decision: balanced transport requires equal totals and uses explicit log-domain dual updates; unbalanced transport relaxes marginals with separately declared source/target KL penalties; partial transport instead enforces a fixed transported mass with capacity inequalities through pinned SciPy 1.18.1. All three report complete objective decompositions, convergence/feasibility, zero/unmatched mass, exact identities, and the non-correspondence claim ceiling. Do not silently substitute one mass model for another.

## DEC-0122 — Pin POT for FGW and replay its nonconvex plans independently

- Date: 2026-08-25
- Status: accepted for REG-SOFT-ASSIGN-01, REG-FGW-01, and REG-PARTIAL-FGW-01
- Decision: reuse balanced log-domain Sinkhorn for explicit dustbin compatibility and pin MIT-licensed POT 0.9.7.post1 for advanced FGW work. Balanced FGW runs independent-mass, feature-EMD, and structure-profile-EMD starts. Fixed-mass partial FGW uses POT's log-domain entropic partial-Wasserstein subproblem with an explicit matrix-valued FGW linearization because POT 0.9.7's partial-FGW convenience wrapper collapses its feature gradient to a scalar. Rust replays every plan, constraint, objective component, convergence state, and selection. Keep the pseudocode alpha convention as feature weight and prohibit correspondence claims.
- Consequences: entropic soft assignment, balanced FGW initialization sensitivity, and fixed-mass partial FGW alpha/mass/epsilon/initialization sensitivity are runnable. KL-unbalanced FGW remains backend-blocked: POT's unbalanced co-optimal transport is a different two-coupling estimand and cannot be silently substituted.

## DEC-0123 — Establish simulation ownership with one consumed mechanistic solver

- Date: 2026-08-25
- Status: accepted for SIM-GROWTH-01
- Decision: add `marklab-simulation` as the dedicated layer for mechanistic simulator contracts and numerical behavior, with an immediate `marklab simulate growth-front` caller. Start with a deterministic 1-D Fisher–KPP specialization using exact logistic reaction splitting and CFL-bounded conservative no-flux diffusion. Do not place the formula in CLI/Bayesian code, generalize an unused simulator framework, or describe the output as a tumour forecast.
- Consequences: WS-70 has one canonical consumed simulator and WS-71 has one mechanistic growth-front workflow. General reaction–diffusion, coupled agents/fields, stochastic sources, observation models, validation banks, and SBI remain open.

## DEC-0124 — Split spatial competition into invariant-preserving flows

- Date: 2026-08-25
- Status: accepted for SIM-COMPETE-01
- Decision: model self-limitation with the exact logistic flow, cross-species competition and declared treatment as simultaneous exponential loss, and spatial diffusion with the same no-flux CFL-bounded operator as the growth-front control. Use symmetric composition so zero-competition/treatment and one-species limits have direct oracles. Record threshold crossings as simulation events, not empirical extinction or causal treatment effects.
- Consequences: `SimulateSpatialCompetition` has a live deterministic density-field owner. Agent competition, stochastic process noise, parameter fitting, biological calibration, and causal interpretation remain separate.

## DEC-0125 — Bound exact stochastic agent competition before adding scale machinery

- Date: 2026-08-25
- Status: accepted for SIM-AGENT-01
- Decision: implement the continuous-time event process with deterministic ChaCha20 replay, exact pairwise opposite-species neighborhood counts, reflected rectangular movement, complete event accounting, and hard event/agent/pair/log bounds. Reuse already locked random packages. Do not copy the paused root spatial index or claim dynamic-index scale without a representative workload and benchmark.
- Consequences: `SimulateAgentCompetition` has a live bounded owner and absorbing-state/replay oracles. Efficient dynamic indexing, richer local rate models, calibration, and evolutionary inference remain open.

## DEC-0126 — Specialize general reaction–diffusion before admitting arbitrary solver callbacks

- Date: 2026-08-25
- Status: accepted for SIM-RD-01
- Decision: expose a typed scalar linear/logistic reaction on a bounded regular periodic 2-D grid, using exact reaction flows and the CFL-bounded explicit five-point diffusion operator. Reuse the already locked RustFFT implementation for a centered periodic spectrum. Do not accept executable reaction callbacks or imply arbitrary mesh, stochastic, stiff, or boundary-condition support. Linear instability bands are available only at a homogeneous reaction equilibrium.
- Consequences: the scalar periodic specializations of `SimulateReactionDiffusion` and `AnalyzeReactionDiffusionPattern` have a live CLI caller and analytical controls. Vector Turing systems, boundary/discretization sensitivity ensembles, adaptive/implicit solvers, stochastic forcing, and biological calibration remain open.

## DEC-0127 — Keep level-set speed data-bound and reinitialization explicitly quadratic

- Date: 2026-08-25
- Status: accepted for SIM-INTERFACE-01
- Decision: admit one static spatial normal-speed field plus nonnegative curvature weight on a regular 2-D grid, rather than executable speed callbacks or speculative coupling interfaces. Use first-order Godunov Hamilton–Jacobi updates with linear-extrapolation ghosts. Reinitialize from explicit zero-contour samples and expose/bound every cell-by-sample distance visit instead of implying fast-marching scale.
- Consequences: `EvolveInterfaceLevelSet` has a consumed deterministic specialization with planar motion and reinitialization oracles. Time-varying/local-field speeds, higher-order schemes, fast marching/sweeping, adaptive grids, topology guarantees beyond retained sign/crossing evidence, and mechanistic calibration remain open.

## DEC-0128 — Admit declared flow fields without inventing vessel hemodynamics

- Date: 2026-08-25
- Status: accepted for SIM-VASCULAR-01
- Decision: require the caller to label a complete static velocity field as an approximation, map declared vessel sources and cell uptake to a bounded regular grid, and advance them with exact local source/linear-uptake flow around conservative arithmetic-face diffusion and first-order upwind advection. Retain complete mass accounting and four-neighbor hypoxic regions. Do not infer flow from source geometry or claim vessel-graph hemodynamics.
- Consequences: `SimulateVascularTransport` has a live bounded specialization with source/uptake, diffusion, and advection oracles. External hemodynamics, graph/boundary exchange, nonlinear uptake, dynamic coupling, implicit/high-order schemes, and real calibration remain open.

## DEC-0129 — Use a proper exact Gaussian hierarchy for the first resource-response fit

- Date: 2026-08-25
- Status: accepted for BAY-RESOURCE-01
- Decision: compute unsigned Euclidean distance to declared resource segments and fit a prespecified linear hinge spline with compartment, resource-density, accessibility, and patient random-intercept terms. With declared known noise and proper fixed Normal prior scales, solve the complete joint Gaussian posterior exactly by bounded Cholesky algebra. Do not introduce a numerical backend, sample an already conjugate posterior, estimate unsupported variance components, or interpret the response as transport causality.
- Consequences: `FitDistanceToResourceModel` has a deterministic live owner with patient/resource posterior predictive checks. Signed distances, inferred dispersion/hierarchy scales, GP/non-Gaussian response, network accessibility, real calibration, and causal interpretation remain open.

## DEC-0130 — Couple existing simulators through explicit one-way interval state

- Date: 2026-08-25
- Status: accepted for SIM-MECH-01
- Decision: compose the canonical vascular, reaction–diffusion, level-set, and agent solvers in a declared one-way interval order. Mean oxygen saturation scales density growth and agent birth/additional death; local oxygen/density sets interface speed. Require exact shared grid/window geometry, aggregate work bounds, deterministic interval seed derivation, and absorbing extinction. Do not introduce a generic coupling framework or bypass module validators.
- Consequences: `SimulateMechanisticTissue` has a consumed bounded specialization with analytical cross-module evidence. Reciprocal/substep coupling, dynamic vessels, richer fields, observation noise, calibration, and digital-twin claims remain open.

## DEC-0131 — Start SBI summaries with one explicit differentiable pair-density estimand

- Date: 2026-08-25
- Status: accepted for SIM-SUMMARY-01
- Decision: define the first differentiable spatial summary as a Gaussian kernel density over exact unordered pair distances, normalized by pair count and bandwidth, with no edge correction and same-window comparison. Expose analytic pair-distance derivatives and immediately consume the curve in a caller-weighted squared summary loss. Do not introduce a generic autodiff or summary framework.
- Consequences: `SoftPairHistogram` and the pair-summary specialization of `SummaryMatchingLoss` are runnable with a closed-form oracle and permutation invariance. Other summaries, boundary correction, inference, learned losses, and simulator calibration remain open.

## DEC-0132 — Give SBI its own layer and begin with exact rejection accounting

- Date: 2026-08-25
- Status: accepted for SBI-ABC-01
- Decision: add `marklab-sbi` as the narrow orchestration layer above canonical simulators, with an immediate root CLI caller. Implement the first `RejectionABC` specialization over Fisher–KPP growth rate, final mass, uniform prior, scaled absolute distance, inclusive epsilon, deterministic ChaCha20 proposals, and hard aggregate declared work. Do not put inference inside the simulator or CLI, or generalize an unused SBI framework.
- Consequences: classical rejection ABC is runnable and exactly replayable on a synthetic analytic control. General simulator/summary registries, adaptive ABC/SMC, real calibration, discrepancy models, and biological posterior claims remain open.

## DEC-0133 — Preserve the full SMC-ABC importance mixture at every stage

- Date: 2026-08-25
- Status: accepted for SBI-SMC-ABC-01
- Decision: use a caller-declared strictly decreasing epsilon schedule, weighted categorical ancestor selection, a Gaussian perturbation with scale adapted to `sqrt(2)` times weighted SD, and exact importance denominator over every previous weighted particle. Normalize and retain ESS/stage accounting; bound all possible stage proposal work. Do not replace the mixture denominator with ancestor-only weights or call rejection sampling SMC.
- Consequences: the fixed-schedule growth-front specialization of `SMC_ABC` is runnable and deterministic. Adaptive tolerances, multivariate kernels, resampling policy comparisons, general catalogs, discrepancy, and real calibration remain open.

## DEC-0134 — Make stochasticity and noisy likelihood state explicit in synthetic likelihood

- Date: 2026-08-25
- Status: accepted for SBI-SYNTH-01
- Decision: augment deterministic growth-front mass/maximum summaries with caller-declared independent Gaussian observation noise, estimate a two-dimensional Gaussian synthetic likelihood from full simulator replicates, shrink only off-diagonal covariance, and require positive determinant. Random-walk MCMC retains the current noisy estimate on rejection. Bound the initial plus all possible proposal evaluations and report Monte Carlo mean uncertainty.
- Consequences: `EstimateSyntheticLogLikelihood` and `SyntheticLikelihoodMCMC` have a consumed deterministic-seed specialization. The likelihood remains approximate; unbiased correction, richer summaries/noise, adaptive replication, general catalogs, discrepancy, and biological calibration remain open.

## DEC-0135 — Calibrate the complete prior–simulator–inference composition

- Date: 2026-08-25
- Status: accepted for SBI-SBC-01
- Decision: draw truth from the same uniform prior used by IC-0117, simulate observed mass through the canonical growth-front owner, rerun rejection ABC under a domain-separated replicate seed, and rank truth among every accepted draw. Retain all replicate failures and equal-tailed coverage; propagate non-acceptance-independent errors. Bound observed plus worst-case inference simulation work before calibration.
- Consequences: the growth-front/rejection-ABC specialization of `SimulationBasedCalibration` is runnable with rank and coverage evidence. This validates implementation calibration on one synthetic control, not real-model adequacy or general SBI calibration.

## DEC-0136 — Separate simulation-bank representation fitting from OOD threshold calibration

- Date: 2026-08-25
- Status: accepted for SBI-OOD-01
- Decision: fit featurewise mean/scale on reference simulations only, score calibration and observed vectors by exact mean k-nearest-reference distance, freeze a nearest-rank calibration threshold, and also report a finite-sample conformal p-value. Require exact split/ID/feature/work contracts and strict threshold exceedance. Do not fit on calibration/observed data or interpret support as validity.
- Consequences: the KNN/conformal specialization of `DetectSimulationOOD` is runnable. Learned encoders, density/classifier methods, conditional support, real simulation banks, and model-validity claims remain open.

## DEC-0137 — Keep the first posterior-predictive laboratory catalog small and complete

- Date: 2026-08-25
- Status: accepted for SBI-PPC-LAB-01
- Decision: draw exact indices from caller-supplied growth-rate posterior draws, call the canonical simulator, and prespecify only final mass and maximum density. Retain every replicate/failure, equal-tail interval, finite-sample one/two-sided discrepancy probabilities, and explicit interval-or-p-value flags. Do not create an unused generic dashboard or imply that two summaries validate a model.
- Consequences: the growth-front specialization of `PosteriorPredictiveLaboratory` is runnable. Broader spatial/mark/graph/topology/embedding/multimodal catalogs, multiplicity, real posterior provenance, and biological model validation remain open.

## DEC-0138 — Admit dense bounded linear-Gaussian state-space inference before spatial fields

- Date: 2026-08-25
- Status: accepted for LONG-KALMAN-01
- Decision: implement time-varying finite-dimensional Kalman filtering and Rauch–Tung–Striebel smoothing in a dedicated longitudinal package. Accept componentwise missing observations, solve positive-definite systems by Cholesky without forming inverses, retain Joseph covariance updates, admit positive-semidefinite process noise, and reject singular required solves instead of adding hidden jitter. Require a declared conservative matrix-work bound before execution.
- Consequences: `KalmanFilter` and `RauchTungStriebelSmoother` are runnable through one consumed CLI workflow. Nonlinear filters, particle methods, spatial fields, real repeated-tissue data, and biological/evolutionary claims remain open.

## DEC-0139 — Bound nonlinear Gaussian filtering with an analytic scalar function family

- Date: 2026-08-25
- Status: accepted for LONG-NONLINEAR-01
- Decision: make the first EKF/UKF caller a scalar time-varying quadratic transition/observation family with analytic derivatives and exact three-point unscented transforms. Retain per-step derivative or sigma-spread diagnostics, component missingness, Gaussian moment likelihood, and explicit method identity. Use Joseph variance for EKF and the unscented cross-covariance update for UKF.
- Consequences: the EKF and UKF branches of `NonlinearGaussianFilter` are consumed and agree with the exact Kalman oracle in their linear limit. Multivariate models, nonlinear smoothing, non-Gaussian state distributions, tuning calibration, and biological validity remain open.

## DEC-0140 — Preserve sequential weights and explicit ancestry in the first particle workflow

- Date: 2026-08-25
- Status: accepted for LONG-PARTICLE-01
- Decision: use a scalar bootstrap proposal over the same analytic quadratic family as IC-0124, carry prior normalized weights when ESS does not trigger resampling, normalize observation weights in log space, apply systematic resampling below a declared ESS fraction, and retain post-resampling particles/weights plus their exact previous-step ancestor indices. Smooth by terminal weighted sampling and backward ancestry tracing.
- Consequences: `ParticleFilter` and the ancestry-trace branch of `ParticleSmoother` are consumed with bounded seeded replay. General proposals, backward simulation, particle Gibbs, spatial fields, particle-count calibration, and biological validity remain open.

## DEC-0141 — Begin 3-D statistics with one fully physical cuboid specialization

- Date: 2026-08-25
- Status: accepted for DIM-K3D-01
- Decision: jointly consume dimensionality validation, 3-D window validation, and homogeneous K/L through an axis-aligned cuboid specialization. Normalize nanometres/micrometres/millimetres before geometry, require positive voxel spacing and optional SPD dimensionless anisotropy, and implement separate none, finite-sample border, and translation-overlap estimators. Retain the normalized points/window and exact pair/work accounting; cap retained pairs independently of the caller limit.
- Consequences: `ValidateDimensionality`, the cuboid branch of `ValidateWindow3D`, and `KFunction3D` are live without applying any 2-D correction. General watertight/voxel/tetrahedral windows, isotropic surface-fraction correction, large indexed plans, real 3-D calibration, and non-cuboid claims remain open.

## DEC-0142 — Keep supplied-intensity and directed-type 3-D normalizations distinct

- Date: 2026-08-25
- Status: accepted for DIM-WEIGHTED-K3D-01
- Decision: reuse IC-0126 geometry/metric/correction validation while implementing inhomogeneous K as ordered inverse-intensity pair contributions and cross-K as A-reference-to-B-target contributions only. None/translation divide by full cuboid volume; border uses exact eroded volume and eligible references. Define cross-g by successive spherical-shell K increments and return unavailable, not NaN, for a zero-volume first shell.
- Consequences: `InhomogeneousK3D` and `CrossK3D` are live with supplied intensities and directed semantics. Intensity fitting, uncertainty propagation, mark interaction models, arbitrary-window corrections, and real-data calibration remain open.

## DEC-0143 — Bind 3-D graph uncertainty to explicit distance bases and canonical content

- Date: 2026-08-25
- Status: accepted for DIM-GRAPH3D-01
- Decision: build radius and undirected union-kNN graphs only after IC-0126 normalization. Treat per-point radial uncertainty by a caller-selected nominal, possible/lower-bound, or guaranteed/upper-bound distance; never silently fold it into coordinates. Canonicalize edges by point ID and digest the complete normalized graph contract with the already locked `sha2 0.10.9` dependency.
- Consequences: one sparse `Build3DSpatialGraph` specialization can be consumed and compared reproducibly. Correlated/deformation posterior uncertainty, learned weights, registration fitting, approximate indexes, and biological adjacency claims remain open.

## DEC-0144 — Restrict phylogenetic–spatial association to exact biological blocks

- Date: 2026-08-25
- Status: accepted for EVO-PHYLO-ASSOC-01
- Decision: compute one Mantel-like Pearson statistic only over within-patient/specimen clone pairs on an imported positive weighted tree and physical 3-D centroids. Canonicalize clone rows, permute clone-to-tree-node assignments independently inside those exact blocks under a named seed, and use an inclusive plus-one two-sided p-value. Retain every observed pair and null statistic.
- Consequences: `PhylogeneticSpatialAssociation` is runnable without implying migration history. Partial trees/networks, uncertain topology/assignments, longitudinal direction, ancestral-location inference, and biological validation remain open.

## DEC-0145 — Derive interference exposure probabilities from the actual randomization design

- Date: 2026-08-25
- Status: accepted for CAUSAL-INTERFERENCE-01
- Decision: make the first causal workflow complete randomization with a fixed treated count inside each independent cluster and exactly enumerate its bounded assignment state space. Compute binary-any-neighbour joint exposure probabilities from those states, not caller-supplied scores. Return unavailable per-exposure estimates when positivity or an observed Hájek denominator is absent, but require positivity for the prespecified randomization-test contrast. Use fixed-outcome rerandomization diagnostics and an inclusive plus-one test.
- Consequences: randomized `ValidateCausalDesign`, binary `ComputeExposureMapping`, HT/Hájek `EstimateExposureMean`, direct/spillover contrasts, and `InterferenceRandomizationTest` have one consumed specialization. Observational identification, general exposure maps, potential-outcome variance, and real causal effects remain open.

## DEC-0146 — Keep spatial exposure construction independent from effect estimation

- Date: 2026-08-25
- Status: accepted for CAUSAL-EXPOSURE-MAP-01
- Decision: expose each prespecified graph mapping as a standalone bounded artifact before any outcome estimator consumes it. Define weighted fractions with explicit isolate unavailability, Gaussian distance decay with a declared physical bandwidth, multiscale exposure as cumulative treated-neighbour counts at exact radii, and continuous fields as caller-declared finite values. Retain graph provenance and exact directed visits.
- Consequences: every branch of `ComputeExposureMapping` has a consumed specialization without acquiring a causal claim. Outcome inspection, adaptive scale selection, estimated fields, exposure probability models, and effect inference remain separate downstream work.

## DEC-0147 — Require an analytic oracle for the first expected-information-gain estimator

- Date: 2026-08-25
- Status: accepted for DESIGN-EIG-01
- Decision: implement the pseudocode's nested Monte Carlo estimator first for one scalar linear-Gaussian candidate where expected information gain is independently known. Retain every outer numerator/denominator/value, stable log-mean-exp, sample SE, analytic value, bias, seed namespace, and exact likelihood work. Use an internal deterministic Box–Muller normal sampler rather than adding another dependency.
- Consequences: `EstimateExpectedInformationGain` has a consumed analytically checked specialization. General simulators/likelihoods, posterior-conditioned sequential acquisition, multiple-candidate optimization, and operational recommendations remain open.

## DEC-0148 — Make the binary-confounder bias model algebra explicit

- Date: 2026-08-25
- Status: accepted for CAUSAL-BIAS-01
- Decision: define each supplied scenario by treated/control binary-confounder prevalence and an additive outcome effect, with bias equal to prevalence difference times outcome effect. Canonicalize scenario IDs and retain every adjusted effect, sign reversal, and the complete region including zero status.
- Consequences: `BiasFunctionSensitivity` has one transparent consumed model. It does not estimate confounder parameters or mechanically correct an effect.

## DEC-0149 — Preserve assumption-light bounded-outcome ATE intervals

- Date: 2026-08-25
- Status: accepted for CAUSAL-PARTIAL-ID-01
- Decision: implement Manski ATE bounds using only observed consistency and known finite outcome support, with no ignorability, monotonicity, or exclusion assumption. Retain both potential-outcome mean intervals and do not replace a zero-crossing interval with the observed mean difference.
- Consequences: `PartialIdentificationBounds` has one exact bounded-outcome specialization. Sampling uncertainty and stronger assumption sets remain separate.

## DEC-0150 — Use a matched-pair sign statistic for the first Rosenbaum curve

- Date: 2026-08-25
- Status: accepted for CAUSAL-ROSENBAUM-01
- Decision: require exactly one treated observation per matched pair, exclude/report exact ties, and bound the one-sided positive-sign probability by `1/(1+Gamma)` and `Gamma/(1+Gamma)`. Compute stable exact binomial upper tails and define critical Gamma from the worst-case upper p-value at the declared alpha.
- Consequences: `RosenbaumSensitivity` has a transparent matched-pair sign-test specialization. Ranked statistics, larger matched sets, effect intervals, and confounding correction remain open.

## DEC-0151 — Centralize stable scalar and covariance primitives without hidden parallelism

- Date: 2026-08-25
- Status: accepted for NUM-STABLE-01
- Decision: create one dedicated numerical owner for max-shift log-sum/log-mean-exp, normalized Neumaier weighted means, and two-pass Neumaier symmetric covariance with explicit effective-sample denominator. Require finite bounded inputs and output. Keep deterministic parallel reduction separate until an immediate production caller defines partition/reduction semantics.
- Consequences: four Part XIII stable primitives are live and independently adversarially tested. Existing local implementations are not silently rewritten in this milestone; caller migration and parallel reduction remain downstream.

## DEC-0152 — Make maturity a monotone downgrade with terminal claim failures

- Date: 2026-08-25
- Status: accepted for EXEC-MATURITY-01
- Decision: replace free-form policy evaluation with a closed ordered maturity enum. Preserve all applicable reasons in fixed order; provenance, convergence/severe diagnostics, and unsupported causal identification force `unsupported_for_claim`, while unvalidated approximation caps research-only and missing predictive external/Bayesian calibration caps experimental. Never upgrade the declared maturity.
- Consequences: `DetermineResultMaturity` is machine-readable and precedence-tested. Existing result schemas remain unchanged until their owning migration milestone.

## DEC-0153 — Assess every execution mode and never infer approximation approval

- Date: 2026-08-25
- Status: accepted for EXEC-MODE-01
- Decision: evaluate ordered descriptors with checked base-plus-per-item memory/runtime, exact backend identity, requested maximum error, and explicit approximation approval. Retain one assessment/reason per mode. Explicit requests evaluate only the requested mode; automatic selection may bypass unapproved approximation for a later feasible exact mode but never silently approve it.
- Consequences: `SelectExecutionMode` has a complete planning specialization. Estimates are declared planning units, not measured performance, and selection does not execute a backend.

## DEC-0154 — Require contiguous evidence for real-data validation promotion

- Date: 2026-08-25
- Status: accepted for VALIDATION-LADDER-01
- Decision: represent stages zero through five exactly once in order, require evidence for completion, and reject completion after any gap. Promote only to the highest contiguous stage and retain risks beginning at the first incomplete stage. Evidence references are identifiers, not self-authenticating proof.
- Consequences: `RealDataValidationLadder` is machine-readable and cannot skip held-out/external/prospective stages. Evidence-content verification remains outside this policy function.

## DEC-0155 — Admit bounded axis-aligned anisotropy before general 3-D fields

- Date: 2026-08-25
- Status: accepted for DIM-GP3D-01
- Decision: reuse the pinned PyMC 6.3.0 exact dense GP lifecycle for three-dimensional micrometre coordinates, with three independently inferred positive axis length scales and no inferred rotation. Require coordinate variation on every axis, explicit priors/jitter, exact conditional predictions, existing strict diagnostics, and bounded cubic work. Report the diagonal metric at posterior mean length scales without calling that nonlinear transform a posterior mean metric.
- Consequences: `FitAnisotropic3DGP` has a runnable experimental specialization. Rotated/full SPD anisotropy, nonstationarity, general 3-D windows, deformation uncertainty, sparse approximations, and real biological validation remain open.

## DEC-0156 — Keep repeated residual randomization at the subject boundary

- Date: 2026-08-25
- Status: accepted for COH-REPEAT-01
- Decision: specialize Freedman–Lane inference to Gaussian OLS with subject fixed effects in both models and one target column only in the full model. Randomize by independently sign-flipping each subject's complete reduced-model residual vector; never shuffle visits independently. Retain the assumption as a claim limitation.
- Consequences: `RepeatedMeasuresFreedmanLane` is runnable for justified balanced or unbalanced repeated scalar outcomes. General nuisance columns, non-Gaussian models, cluster permutation, mixed-model bootstrap, and real-design validation remain open.

## DEC-0157 — Use whole-patient maximum deviation for functional equivalence

- Date: 2026-08-25
- Status: accepted for COH-FUNC-EQV-01
- Decision: resample complete patient difference curves and form an unstudentized simultaneous band from the nearest-rank maximum absolute bootstrap deviation. Require one common axis and prespecified positive margin curve; never substitute pointwise intervals.
- Consequences: `FunctionalEquivalenceBand` has one transparent experimental bootstrap specialization. Studentized/BCa bands, irregular axes, and real margin calibration remain open.

## DEC-0158 — Make percentile bootstrap equivalence an explicit limited method

- Date: 2026-08-25
- Status: accepted for COH-BOOT-EQV-01
- Decision: directly consume the existing patient-first hierarchical bootstrap and decide equivalence from its strict percentile interval containment. Preserve interval-method identity and an experimental ceiling rather than implying calibrated coverage for irregular estimands.
- Consequences: `BootstrapEquivalence` is runnable without duplicating resampling. BCa, studentization, nonsmooth-estimator calibration, and method sensitivity remain open.

## DEC-0159 — Separate fixed and REML random-effects multisite summaries

- Date: 2026-08-25
- Status: accepted for COH-MULTISITE-01
- Decision: admit inverse-variance fixed pooling and intercept-only scalar REML random-effects pooling for precomputed site patient-level effects. Always report Cochran Q and leave-one-site-out refits; random effects additionally report a prediction interval. Do not infer patient-level models from site summaries.
- Consequences: `MultisiteSpatialInference` is runnable for honest site summaries. One-stage hierarchical models, multivariate effects, small-site t corrections, meta-regression, and transportability remain separate.

## DEC-0160 — Promote one bounded canonical graph through its Fourier consumer

- Date: 2026-08-25
- Status: accepted for GSP-SPECTRAL-01
- Decision: begin Part VII with exact physical-radius binary graphs, no isolates, a combinatorial Laplacian, and one immediately consumed scalar signal. Use bounded deterministic Jacobi eigendecomposition, ascending eigenvalues, canonical eigenvector signs, reconstruction verification, and declared nonoverlapping bands. Digest nodes, edges, radius, weight, and Laplacian convention.
- Consequences: four graph declarations are runnable without duplicating the earlier descriptive graph-energy input. kNN/kernels, normalized/random-walk operators, nulls, heat/wavelets/scattering, sparse scale, and biological adjacency validation remain open.

## DEC-0161 — Reuse the exact spectrum for all small-graph heat outputs

- Date: 2026-08-25
- Status: accepted for GSP-HEAT-01
- Decision: evaluate dense heat kernels, signal application, diagonal signatures, and declared-pair diffusion distances from the single canonical eigensystem. Admit zero time as an exact identity oracle and define diffusion distance as uniform-node L2 between kernel rows. Keep physical-scale calibration and matrix-free approximation separate.
- Consequences: four heat declarations are runnable for bounded graphs without a second operator owner. Chebyshev/Lanczos scale and stationary-measure variants remain open.

## DEC-0162 — Freeze one interpretable exact spectral wavelet pair

- Date: 2026-08-25
- Status: accepted for GSP-WAVELET-01
- Decision: use `g(x)=x exp(-x)` and `h(x)=exp(-x)` over declared positive scales, applied through the canonical exact Fourier basis. Retain coefficients and energies rather than introducing a general closure-based kernel registry.
- Consequences: spectral wavelet transform/energy are runnable with an analytic eigenmode oracle. Kernel catalogs, Chebyshev scale, diffusion wavelet bases, scattering, and endpoint calibration remain open.

## DEC-0163 — Extract ERL once for graph-spectrum inference

- Date: 2026-08-25
- Status: accepted for GSP-NULL-01
- Decision: move the established average-tie extreme-rank-length procedure to `marklab-numerics` and make embedding and graph envelopes consume it. Graph nulls permute complete signals only inside exact declared strata, retain every band-energy curve, and use an inclusive upper-tail scalar low-frequency p-value. Canonicalize near-zero Laplacian eigenvalues to positive zero before band membership.
- Consequences: `GraphSpectrumNullTest` is runnable without a duplicate ERL implementation. More general exchangeability plans and calibrated biological band choices remain open.

## DEC-0164 — Admit Chebyshev mechanics only through exact-heat differential validation

- Date: 2026-08-25
- Status: accepted for GSP-CHEB-01
- Decision: scale the canonical combinatorial Laplacian by its exact bounded maximum eigenvalue, compute heat-filter coefficients by deterministic Chebyshev-node quadrature, and select order using both a longer-reference coefficient tail and dense scalar grid error. Always compare the resulting signal to the existing exact spectral application at current bounded sizes. Describe these as estimates/checks, not a rigorous continuous error certificate.
- Consequences: `ChebyshevApply` and a consumed `AdaptiveChebyshevOrder` specialization are live. Lanczos bounds, sparse large-graph performance, arbitrary filters, and certified analytic tails remain open.

## DEC-0165 — Define diffusion wavelets by exact lazy-spectrum rank thresholds

- Date: 2026-08-25
- Status: accepted for GSP-DIFFWAVE-01
- Decision: reuse the canonical exact eigensystem to form `I-L/lambda_max`, evaluate dyadic powers, and retain modes whose powered magnitude exceeds a declared tolerance. Record scaling/detail bases and reconstruct the signal from the resulting orthogonal partition.
- Consequences: diffusion-wavelet construction and transform are runnable for bounded dense graphs. Sparse rank-revealing factorizations, localized bases, and calibrated physical multiscale interpretations remain open.

## DEC-0166 — Bound graph scattering to fixed kernels and declared signal perturbations

- Date: 2026-08-25
- Status: accepted for GSP-SCATTER-01
- Decision: use the existing `x exp(-x)` spectral response, strictly increasing scale paths, pointwise modulus, arithmetic node pooling, and maximum order two. Measure feature change only against explicitly supplied finite signal perturbations and reject ratios above the caller tolerance.
- Consequences: scattering and one concrete stability check are consumed without claiming a general graph-deformation theorem. Kernel learning, graph perturbations, alternative pooling, and pathology calibration remain open.

## DEC-0167 — Admit one typed spatial-near message layer

- Date: 2026-08-25
- Status: accepted for HET-GRAPH-01
- Decision: canonicalize typed nodes and directed typed spatial-near relations, require one common feature dimension, and execute exactly one relation-scaled sum or mean aggregation layer. Retain the graph digest and exact pair/edge work.
- Consequences: heterogeneous construction and message passing are runnable as a bounded synthetic specialization. Multiple relation families per receiver, learned weights, deep networks, training, and biological claims remain open.

## DEC-0168 — Use the normalized incidence hypergraph operator

- Date: 2026-08-25
- Status: accepted for HET-HYPER-01
- Decision: canonicalize weighted typed hyperedges and finite memberships, then compute `I-Dv^-1/2 H W De^-1 H' Dv^-1/2` and the signal Rayleigh quotient under explicit incidence work bounds.
- Consequences: hypergraph construction, Laplacian, and signal smoothness have a hand-incidence oracle. Alternative Laplacians, learned memberships, large sparse scale, and endpoint interpretation remain open.

## DEC-0169 — Restrict motifs to exhaustive typed triangles and stratum label nulls

- Date: 2026-08-25
- Status: accepted for HET-MOTIF-01
- Decision: enumerate every bounded node triple, match a sorted three-label multiset, accumulate pairwise motif adjacency, and permute complete labels only within exact declared strata with a seeded inclusive upper-tail p-value.
- Consequences: motif count, adjacency, and null declarations are live for typed triangles. Larger/directed/weighted motifs, automorphism catalogs, and real exchangeability designs remain open.

## DEC-0170 — Construct clique Hodge mathematics from canonical orientations

- Date: 2026-08-25
- Status: accepted for HET-HODGE-01
- Decision: orient sorted edges and clique triangles canonically, construct `B1` and `B2`, prove `B1 B2=0`, compute lower/upper first Hodge operators, solve gauge-fixed gradient and triangle-curl projections, and apply one explicit first-order polynomial filter.
- Consequences: clique construction, Hodge Laplacian, edge-flow decomposition, and simplicial filtering are runnable on bounded dimension-two complexes. General dimensions, sparse least squares, non-clique complexes, and biological flow interpretation remain open.

## DEC-0171 — Require interpretation and perturbation evidence for cellular complexes

- Date: 2026-08-25
- Status: accepted for HET-CELLULAR-01
- Decision: accept explicitly interpreted junctions, oriented interfaces, and domains; reject any domain whose signed interfaces do not close; and require at least one declared segmentation perturbation whose topology, incidence, and geometric displacement are reported against baseline.
- Consequences: `BuildCellularComplex` is live only as research-only declared-compartment mathematics. The workflow does not infer cells from images or establish robust pathology utility beyond supplied perturbations.

## DEC-0172 — Make graph-suite gaps first-class validation results

- Date: 2026-08-25
- Status: accepted for GRAPH-VALIDATE-01
- Decision: execute fixed independent exact fixtures for every live graph family and one radius-choice sensitivity check. Emit explicit `not_supported` or `not_applicable` entries for kNN/kernel/barrier/component breadth, registration perturbations, sparse-memory scale, and GPU parity.
- Consequences: `ValidateGraphMathematicsSuite` is runnable and cannot silently imply unexecuted stress evidence. Its pass state is synthetic exact validation only, not pathology performance or release-scale validation.

## DEC-0173 — Pin GUDHI exact alpha persistence with its effective license

- Date: 2026-08-25
- Status: accepted for TOP-ALPHA-01
- Decision: pin GUDHI 3.13.0/Python 3.12 and use exact alpha predicates, squared-alpha filtration values, and SimplexTree persistent cohomology. Record that alpha construction depends on CGAL and is effectively GPLv3; retain essential births separately from JSON finite deaths. Implement landscape/Euler mechanics directly and image integration through pinned SciPy 1.18.1.
- Consequences: six topology declarations are live on exact synthetic fixtures. Distribution licensing must account for the effective GPLv3 dependency; pathology interpretation and scale selection remain unvalidated.

## DEC-0174 — Bound witness approximation to deterministic farthest landmarks

- Date: 2026-08-25
- Status: accepted for TOP-WITNESS-01
- Decision: use GUDHI 3.13.0 weak Euclidean witnesses with lexicographically seeded farthest-point landmarks, report coverage radius, support nu zero only, and retain the full bounded filtration/persistence artifact.
- Consequences: witness filtration is runnable without implying approximation adequacy for tissue data.

## DEC-0175 — Pin raster morphology conventions to scikit-image

- Date: 2026-08-25
- Status: accepted for TOP-MORPH-01
- Decision: pin scikit-image 0.26.0 (BSD-3-Clause) and SciPy 1.18.1 for supplied binary rasters, pixel-count area, 2/4-direction Crofton perimeter, declared 4/8 foreground Euler connectivity, and disk dilation/erosion at integer pixel radii.
- Consequences: raster Minkowski and morphological curves are reproducible. Subpixel radii, polygon comparison, segmentation uncertainty, and biological claims remain open.

## DEC-0176 — Use exact finite pair events for connectivity transitions

- Date: 2026-08-25
- Status: accepted for TOP-CONNECT-01
- Decision: sort every bounded Euclidean pair event once, update deterministic union-find across declared radii, exclude one largest component from susceptibility, and define critical radius as the first declared boundary-spanning radius.
- Consequences: connectivity transition mechanics are live with explicit finite-size caveats; this is not asymptotic percolation or cohort inference.

## DEC-0177 — Keep topology comparison at the whole-patient diagram boundary

- Date: 2026-08-25
- Status: accepted for TOP-COMPARE-01
- Decision: compute GUDHI bottleneck distances between complete finite diagrams, form the biased distance-energy statistic, and exactly enumerate group-A counts within every declared stratum. Never permute persistence points or other within-patient features.
- Consequences: patient-level topology comparison mechanics are runnable; biological replication, metric selection, and endpoint validity remain external evidence requirements.

## DEC-0178 — Recompute every topology family under declared mask toggles

- Date: 2026-08-25
- Status: accepted for TOP-STABILITY-01
- Decision: admit a seeded, bounded declared-pixel-toggle generator and recompute signed-distance cubical persistence, a fixed landscape, dilation Euler curves, raster Minkowski functionals, and foreground connectivity radius for every repetition. Retain each selected pixel and sensitivity metric.
- Consequences: perturbation sensitivity is observable without implying a realistic segmentation-error distribution. Broader generators and patient calibration remain open.

## DEC-0179 — Separate exact topology validation from unmeasured scale

- Date: 2026-08-25
- Status: accepted for TOP-VALIDATE-01
- Decision: execute analytic/hand controls across every live topology family and record essential-death policy. Mark representative sparse-memory scaling `not_verified` because no admitted workload or measurement exists.
- Consequences: `ValidateTopologySuite` is live and honest about its scale ceiling; exact-fixture success is not performance or pathology validation.

## DEC-0180 — Admit one explicit paired Gaussian pCCA EM specialization

- Date: 2026-08-25
- Status: accepted for MM-PCCA-01
- Decision: require stable paired patient IDs, two measured Gaussian views, complete rows, and explicit train/test roles. Fit means/scales on training rows only, use deterministic SciPy linear algebra for the published EM updates with diagonal noise floors, and retain the likelihood trace and held-out cross-view prediction.
- Consequences: multimodal design validation and classical pCCA are runnable without generalizing the design schema beyond an immediate caller.

## DEC-0181 — Preserve CCA-Zoo's Bayesian model while retaining NUTS diagnostics

- Date: 2026-08-25
- Status: accepted for BAY-PCCA-01
- Decision: pin CCA-Zoo 3.0.0/NumPyro 0.21.0/JAX 0.11.1 and execute its model through a thin static adapter that retains MCMC extra fields discarded by the estimator wrapper. Restrict version one to one latent factor and align every draw by a positive first-view maximum loading.
- Consequences: Bayesian pCCA has a zero-divergence synthetic specialization. R-hat remains unavailable for the admitted one-chain fixture; multi-factor rotations and stable claims remain open.

## DEC-0182 — Use MOFA for multiview VI and structural/MAR masking

- Date: 2026-08-25
- Status: accepted for MM-MOFA-01
- Decision: pin mofapy2 0.7.4 and h5py 3.16.0, fit only observed training entries under Gaussian likelihoods, order factors by total explained variance, orient by global maximum loading, and infer held-out scores from available views before evaluating masked targets.
- Consequences: multiview factors and structural/MAR missing-modality predictions are runnable under the LGPL-3.0 backend. MNAR identification, non-Gaussian calibration, hierarchy, and real evidence remain open.

## DEC-0183 — Use one-view MOFA for bounded Gaussian matrix factorization

- Date: 2026-08-25
- Status: accepted for MM-MATRIX-01
- Decision: pin mofapy2 0.7.4/h5py 3.16.0, fit only observed entries, order factors by explained variance, orient each maximum loading positive, and evaluate caller-retained masked targets after fitting. Preserve first and second moments, noise, ARD activity, and finite ELBO history.
- Consequences: `BayesianMatrixFactorization` is live for bounded Gaussian patient matrices under synthetic experimental claims. Other likelihoods, draw-level rotation uncertainty, and real calibration remain open.

## DEC-0184 — Compile hierarchical factors with exact level ownership

- Date: 2026-08-25
- Status: accepted for MM-HIER-01
- Decision: compile only exact patient→specimen→region→cell parentage, attach observations at their declared entity level, distinguish measured and predicted modalities, and make patient the replication unit. Reject skipped levels and direct use of cells as patient replicates.
- Consequences: `AddHierarchicalFactorStructure` has a consumed typed graph workflow without pretending that compilation is a fitted posterior.

## DEC-0185 — Bound graph-spatial factors to a SciPy Laplace specialization

- Date: 2026-08-25
- Status: accepted for MM-SPATIAL-MATRIX-01
- Decision: use a declared unnormalized graph Laplacian plus positive diagonal, Gaussian likelihood, ARD-like loading precision, alternating linear solves, and pinned SciPy 1.18.1 L-BFGS MAP/inverse-Hessian approximation. Cap total latent parameters at 512 and report `approximate_only`.
- Consequences: `SpatialBayesianMatrixFactorization` is live on bounded region graphs with masked predictive uncertainty. This is not an exact posterior, GP factor model, SPDE model, or representative-scale implementation.

## DEC-0186 — Use pinned JAX/SciPy Laplace adapters for three-mode tensors

- Date: 2026-08-25
- Status: accepted for MM-TENSOR-01
- Decision: restrict version one to complete-index three-mode Gaussian tensors with held-out masks, positive Gaussian shrinkage, bounded CP rank or Tucker ranks, JAX 0.11.1 differentiation, and SciPy 1.18.1 L-BFGS inverse-Hessian uncertainty. Align CP component energy/sign and Tucker mode signs after fitting.
- Consequences: Bayesian CP and Tucker declarations are runnable as explicit approximate synthetic workflows. General tensor order, non-Gaussian likelihoods, HMC agreement, rank calibration, and real validation remain open.

## DEC-0187 — Fit one exact Matérn spatial factor through PyMC

- Date: 2026-08-25
- Status: accepted for MM-SPATIAL-LATENT-01
- Decision: admit one bounded measured-region Gaussian specialization with unique 2-D physical coordinates, one Matérn-3/2 latent factor, a half-normal range prior, noncentered field construction, two-chain PyMC 6.3.0 NUTS, and per-draw sign alignment.
- Consequences: `FitSpatialLatentFactorModel` is live for exact small GP factors with retained NUTS diagnostics. Multiple factors/likelihoods, sparse dispatch, range calibration, and real geometry remain open.

## DEC-0188 — Treat supplied physical bases as the multiresolution contract

- Date: 2026-08-25
- Status: accepted for MM-MULTIRES-01
- Decision: require two-to-eight named unique physical scales with finite supplied bases, bounded factors and shrinkage, then fit all scale contributions jointly with JAX differentiation and SciPy MAP/inverse-Hessian uncertainty.
- Consequences: `FitMultiresolutionSpatialFactors` is live as a bounded `approximate_only` workflow; basis adequacy and scale discovery are caller responsibilities.

## DEC-0189 — Train modality robustness with explicit retained-view patterns

- Date: 2026-08-25
- Status: accepted for MM-DROPOUT-01
- Decision: require normalized named dropout patterns that each retain at least one declared anchor modality. Fit training-only standardized encoders/decoders with a probability-weighted reconstruction plus full/partial latent-consistency objective and validate every pattern on held-out patients.
- Consequences: `TrainModalityRobustInference` is live for bounded Gaussian views through pinned JAX/SciPy. It is predictive-objective training, not a calibrated Bayesian posterior or MNAR identification.

## DEC-0190 — Compile and fit the joint pathology graph in one consumed workflow

- Date: 2026-08-25
- Status: accepted for MM-JOINT-01
- Decision: compile exact patient→region latent ownership and five measured morphology/IHC/omics/clone/clinical blocks, then immediately fit the graph with Gaussian, Poisson, Bernoulli, and masked clinical likelihoods using pinned JAX/SciPy Laplace approximation. Preserve the ModelIR, likelihood-specific checks, uncertainty, and claim limits together.
- Consequences: `CompileJointPathologyModel` and `FitJointPathologyModel` are live under a deliberately narrow frontier/synthetic contract. No cell/patch/registration block, NB overdispersion, exact posterior, or clinical claim is implied.

## DEC-0191 — Reuse the canonical nested M0–M5 fitter for comparison

- Date: 2026-08-25
- Status: accepted for MM-COMPARE-01
- Decision: expose the existing IC-0091 patient-held-out nested SciPy ridge workflow under `marklab multimodal compare-models`, retaining all six fold-fitted models and the six prescribed paired patient permutation increments. Keep the original complementarity command unchanged.
- Consequences: `CompareMultimodalModels` has one canonical shared fitting owner rather than a duplicate implementation. External subgroups, predictive density, and real cohorts remain evidence gaps.

## DEC-0192 — Make unavailable multimodal validation evidence explicit

- Date: 2026-08-25
- Status: accepted for MM-VALIDATE-01 and SPDE-FACTOR-01
- Decision: execute independent analytic/simulated controls for subspace, missing views, alignment, interval coverage, GP/GMRF, multiresolution, dropout, MNAR sensitivity, confounding, prior sensitivity, and tensor rank. Emit explicit unavailable rows for SPDE mesh recovery, same-model HMC/VI/Laplace comparison, and real patient-held-out validation.
- Consequences: `ValidateMultimodalBayesianSuite` is live with a partial external-evidence status. `FitSPDESpatialFactorModel` remains genuinely blocked on the promoted 2-D mesh and projection owner and is not relabelled as graph-Laplacian fitting.

## DEC-0193 — Pin SimpleITK for multiresolution B-spline registration

- Date: 2026-08-25
- Status: accepted for REG-NONRIGID-01
- Decision: pin stable SimpleITK 2.5.5 (Apache-2.0), validate physical frames/resolution/masks, and use its deterministic ITKv4 B-spline registration with declared same-stain mean squares and physical-unit pyramids. Retain the full transform/displacement, warped image, objective, stop condition, and Jacobian map.
- Consequences: `MultiResolutionNonrigidRegistration` is live on bounded synthetic same-stain images. Cross-stain metrics, inverse transforms, landmarks, and real registration validity remain separate evidence.

## DEC-0194 — Implement stationary-velocity exponentiation through JAX composition

- Date: 2026-08-25
- Status: accepted for REG-SVF-01
- Decision: optimize a bounded dense stationary velocity through pinned JAX/SciPy, exponentiate with differentiable displacement-field scaling and squaring, construct the inverse with `exp(-v)`, and gate the result on Jacobian and inverse consistency. Compose displacements rather than clipped absolute maps at image boundaries.
- Consequences: `SVFDiffeomorphicRegistration` and its consumed `ExponentiateVelocity` primitive are live for bounded same-stain synthetic images. Dense scale and general boundary models remain open.

## DEC-0195 — Admit Gaussian-kernel landmark LDDMM shooting

- Date: 2026-08-25
- Status: accepted for REG-LDDMM-01
- Decision: use pinned JAX/SciPy to optimize initial landmark momentum and integrate the canonical Hamiltonian equations with fixed-step RK4. Retain every position/momentum state, kinetic energy/drift, and target residual.
- Consequences: `LDDMMRegistration` and `Shoot` are live for paired 2-D landmark sets, not dense images or general EPDiff fields.

## DEC-0196 — Bound probabilistic diffeomorphism to a variational translation SVF

- Date: 2026-08-25
- Status: accepted for REG-PROB-SVF-01
- Decision: fit a Gaussian variational posterior over constant translation velocity with a fixed antithetic reparameterized ELBO, Gaussian velocity prior, same-stain image likelihood, and seeded transform draws. A constant SVF exponentiates exactly to a Jacobian-one translation.
- Consequences: `ProbabilisticDiffeomorphicRegistration` is live under an explicit narrow synthetic ceiling. It is not a dense deformation posterior.

## DEC-0197 — Couple Bayesian landmark deformation to uncertainty propagation and compatibility

- Date: 2026-08-25
- Status: accepted for REG-LANDMARK-UNC-01
- Decision: use the analytic independent-coordinate squared-exponential GP posterior over landmark displacement, retain correlated query draws, propagate them through a declared nonlinear centroid endpoint with localization separated, compare the delta approximation, and average spatial/feature compatibility over transform draws with source dustbins.
- Consequences: Bayesian landmark registration, Monte Carlo/delta propagation, and probabilistic correspondence are live together. Correspondence is explicitly many-to-one compatibility, never physical cell identity.

## DEC-0198 — Build a biological-similarity atlas without a physical claim

- Date: 2026-08-25
- Status: accepted for ATLAS-01
- Decision: admit frozen measured region features, patient-replicated domain prototypes, pooled shrinkage Mahalanobis covariance, probability/entropy/OOD mapping, leave-one-patient preprocessing, and declared subsampling/feature-shift validation. Version the atlas by the complete hash-bound request and state that it has no physical registration claim.
- Consequences: atlas build, query map, and validation are live on a synthetic cohort. Real training populations, scanner/stain calibration, external sites, and anatomical homology remain promotion requirements.

## DEC-0199 — Pin JAX CPU neural point-process training

- Date: 2026-08-25
- Status: accepted for NEURAL-PP-01
- Decision: use pinned JAX 0.11.1/SciPy 1.18.1 with a hash-bound tanh hidden representation, softplus intensity, softmax marks, exact rectangular windows, independent patient splits, and fixed versus doubled-grid quadrature. Train the summed Cox plus mark likelihood under deterministic CPU policy.
- Consequences: neural Cox intensity/training and static marked likelihood/training are live on synthetic patterns. Ensemble/posterior uncertainty and real multitype spatial calibration remain promotion gaps.

## DEC-0200 — Use an iid-equivariant logistic-normal flow and Gaussian score diffusion

- Date: 2026-08-25
- Status: accepted for NEURAL-SET-01
- Decision: make the point transform independently invertible/equivariant in logit-window coordinates, retain a Poisson count model and exact permutation-invariant likelihood, then fit the conditional Gaussian marginal score under a VP schedule and sample by reverse probability-flow Euler steps.
- Consequences: flow model/training and diffusion training/sampling are live with explicit iid-exchangeable semantics, boundary support, and synthetic diversity checks. General interacting equivariant flows remain open.

## DEC-0201 — Pin sbi for distinct amortized estimands and sequential rounds

- Date: 2026-08-25
- Status: accepted for NEURAL-SBI-01
- Decision: pin sbi 0.26.1 and Torch 2.13.0 under deterministic CPU policy. Train MDN NPE, MDN NLE, and MLP NRE as separate estimands; normalize each on a bounded oracle grid, serialize each state, and run two NPE proposal rounds with disjoint simulation-bank hashes.
- Consequences: `TrainNPE`, `TrainNLE`, `TrainNRE`, and `SequentialSBI` are live on a one-dimensional Gaussian-location oracle. Higher dimension, accelerator reproducibility, coverage breadth, and real simulators remain open.

## DEC-0202 — Validate actual repeated generator artifacts

- Date: 2026-08-25
- Status: accepted for NEURAL-VALIDATE-01
- Decision: require two independently executed byte-identical point-set generator artifacts and the original train/held-out patient patterns. Execute cardinality, support, pair/nearest-neighbor, conditional mode, diversity, exact memorization, membership-attack, rare-count, and simpler-baseline checks; retain unsupported graph/mark/compartment/real-data rows.
- Consequences: `ValidateGenerativeTissueModel` is live as a consumed synthetic model card and cannot promote the model beyond research-only.

## DEC-0203 — Reconstruct landmark serial stacks with a Gaussian translation posterior

- Date: 2026-08-25
- Status: accepted for DIM-STACK-01
- Decision: require ordered physical sections with paired landmarks and a declared reference, compose adjacent translations, infer per-section Gaussian translation uncertainty from landmark noise, retain cycle/landmark/gap diagnostics, and propagate every transform draw into a 3-D stack-centroid endpoint.
- Consequences: serial reconstruction, Bayesian stack fitting, and uncertainty propagation are live together for a bounded translation specialization. Nonrigid/missing-image stacks and real anatomy remain open.

## DEC-0204 — Extend pinned exact alpha construction to 3-D

- Date: 2026-08-25
- Status: accepted for DIM-ALPHA3D-01
- Decision: use GUDHI 3.13.0 exact 3-D alpha construction with squared physical-alpha convention, complete tetrahedral filtration, F2 boundary-of-boundary check, and persistence through the declared dimension. Preserve the effective CGAL/GPLv3 license.
- Consequences: `Build3DAlphaComplex` is live on bounded synthetic physical points. Representative scale and pathology filtration remain unverified.

## DEC-0205 — Require controls and independent measurements for deformation/change separation

- Date: 2026-08-25
- Status: accepted for LONG-DEFORM-BIO-01
- Decision: condition on supplied deformation posterior draws, interpolate the baseline with pinned SciPy thin-plate splines, fit a prespecified domain change for every draw, and require both a deformation-only negative control and an independent change measurement excluded from fitting.
- Consequences: `FitDeformationBiologyModel` is live on a synthetic translation/domain specialization without claiming general longitudinal identifiability.

## DEC-0206 — Integrate imported clone uncertainty in both fitted clone models

- Date: 2026-08-25
- Status: accepted for EVO-CLONE-MODELS-01 and DIM-ADV-VALIDATE-01
- Decision: draw every imported clone location from its SPD covariance and fit inverse-gamma Brownian branch diffusion; fit clone niches with probability-weighted Gaussian mixtures and patient centering plus leave-one-patient sensitivity. Validate all advanced 3-D families with explicit real-data gap.
- Consequences: clone phylogeography/niche and the umbrella suite are live under synthetic association/model-plausibility ceilings, not identified history.

## DEC-0207 — Keep spatial mediation design-gated and research-only

- Date: 2026-08-25
- Status: accepted for CAUSAL-PERTURB-01
- Decision: expose spatial mediation only inside a bounded synthetic perturbation workflow with declared treatment-before-mediator-before-outcome ordering, randomized cluster treatment, a prespecified neighbour exposure, and explicit mediator-outcome sensitivity values. Report the fitted mediator/outcome path decomposition as research-only even when the synthetic oracle is recovered; if the timing or identification declarations are absent, no causal mediation result is emitted.
- Consequences: `SpatialMediationAnalysis` can have a consumed synthetic owner without promoting cross-sectional association to identified mediation. Real perturbation replication, mediator-outcome identification, interference transport, and prospective validation remain required before any causal-mediation claim.

## DEC-0208 — Cross-fit bounded synthetic observational estimators by cluster

- Date: 2026-08-25
- Status: accepted for CAUSAL-OBS-01
- Decision: validate unique units and at least eight independent clusters, fit a regularized logistic propensity, require overlap at the declared clip, estimate a linear dose/exposure response, AIPW treatment and exposure summaries, cluster-disjoint nuisance folds, two-residual spatial DML, and an adjusted negative-control outcome through pinned SciPy/NumPy. Preserve every fold assignment and never describe synthetic recovery as identification in a real cohort.
- Consequences: the six observational declarations have one consumed numerical specialization with positivity and leakage checks. General learners, uncertainty robust to arbitrary clustering, real confounder sufficiency, and causal identification remain unproved.

## DEC-0209 — Make active acquisition a bounded prospective simulation

- Date: 2026-08-25
- Status: accepted for ACTIVE-DESIGN-01
- Decision: use scalar Gaussian posterior updates for sequential design, transparent budgeted net-utility ranking for ROI/stain/landmark candidates, explicitly favor biological over technical replication at equal cost/precision, and estimate prospective power by seeded normal endpoint simulation with Wilson intervals. Retain candidate costs, selection history, posterior history, allocation frontier, failures, and uncertainty.
- Consequences: all six active/allocation declarations are executable against declared synthetic actions. The plan cannot operate instruments or claim prospective laboratory benefit.

## DEC-0210 — Validate causal and active mechanics without hiding prospective gaps

- Date: 2026-08-25
- Status: accepted for CAUSAL-ACTIVE-VALIDATE-01
- Decision: execute fixed synthetic randomized, exposure, DR/DML, positivity, negative-control, analytic-EIG, selection, replication, and power controls, then retain prospective laboratory validation as an explicit not-verified row that prevents complete status.
- Consequences: `ValidateCausalActiveSuite` is live as a partial evidence ledger. It is not external or prospective causal/design validation.

## DEC-0211 — Extract unified execution from the proven durable single-node path

- Date: 2026-08-25
- Status: accepted for PLAT-DUR-01 and EXEC-ALGORITHM-01
- Decision: make `marklab-workflow::execute_algorithm` the narrow generic owner of the already-proven sequence: scheduler-derived identity, exact durable restore, normal scheduler validation/execute/decode, verified inline read, and durable publication/ledger/head commit on a miss. Its descriptor is the existing typed `WorkflowNode`/`NodeSpec`/cache material plus an explicit output schema and native runtime provenance. Keep dependency-bearing DAG execution, arbitrary task execution, external backends, and diagnostics policy outside this function.
- Consequences: `marklab project classical` becomes the immediate production caller and retains byte-compatible miss/hit semantics. This closes the Part XIII unified invocation declaration only for dependency-free typed native nodes under the existing durable transaction.

## DEC-0212 — Admit fixed-step HMC on a conjugate normal oracle

- Date: 2026-08-25
- Status: accepted for BAY-HMC-01
- Decision: implement diagonal-mass one-dimensional fixed-step leapfrog HMC through pinned NumPy/SciPy, retaining every posterior draw, accept decision aggregate, Hamiltonian error, fixed identity Jacobian, and analytic conjugate posterior. Keep adaptation and constrained transforms outside this narrow engine.
- Consequences: `RunHMC` has an immediate consumed model and independent analytic oracle rather than being inferred from NUTS. General dimension, mass adaptation, transforms, and real-model calibration remain separate.

## DEC-0213 — Use exact finite state for controlled exchange and bounded latent-parent inference

- Date: 2026-08-25
- Status: accepted for BAY-ADV-CLUSTER-01
- Decision: specialize exchange MCMC to a six-site binary Gibbs graph whose 64 states and auxiliary distribution are exactly enumerated for every proposal. Consume it as the symmetric two-type fit. For Thomas clusters, use label-invariant BIC-targeted birth/death parent-count moves with deterministic conditional clustering, then partially pool log kappa/offspring/scale across independent synthetic patients.
- Consequences: latent-parent, exact exchange, multitype Gibbs, and replicated-cluster declarations are runnable under explicit bounded synthetic semantics. General continuous-space perfect simulation, arbitrary type matrices, full reversible-jump parent locations, edge correction, and real calibration remain open claim limits.

## DEC-0214 — Give parallel reduction immediate calibration and benchmark callers

- Date: 2026-08-25
- Status: accepted for RUNTIME-VALIDATION-01
- Decision: partition items into fixed contiguous ranges, derive every item seed from namespace plus global index, map partitions on scoped threads, and reduce partition outputs in index order. Use this owner immediately in a Gaussian normal-mean calibration suite and an equivalent-work wrapping-integer checksum scaling smoke benchmark. Diagnose the admitted fixed-step HMC artifact only for applicable oracle/acceptance/energy checks and report nonapplicable families explicitly.
- Consequences: deterministic parallel reduction, common fitted diagnostics, universal calibration, and scaling benchmark declarations have consumed specializations. Floating calibration is tolerance-stable; integer checksums are bitwise stable. This is not a representative performance or universal diagnostic claim.

## DEC-0215 — Promote a bounded rectangular finite-element SPDE owner

- Date: 2026-08-25
- Status: accepted for SPDE-SUITE-01 and SPDE-FACTOR-01
- Decision: restrict the first promoted mesh to a hole-free physical rectangle with regular boundary vertices and pinned SciPy Delaunay triangulation. Assemble exact linear-triangle consistent mass and stiffness matrices, alpha-two lumped-mass Matérn precision, and barycentric event/quadrature/region projections. Consume the same owner in fixed-hyperparameter Laplace-MAP LGCP and one-factor Gaussian spatial-factor fits, with two mesh resolutions and synthetic controls.
- Consequences: all three SPDE declarations are live for this explicit geometry/backend. Holed or narrow-interface constrained meshing, adaptive refinement, inferred SPDE hyperparameters, sparse representative scale, posterior sampling, and real pathology validation remain unavailable rather than implied.

## DEC-0216 — Admit typed static external workers to durable project execution

- Date: 2026-08-26
- Status: accepted for BACK-01/WS-13 durable PyMC and POT workflows
- Decision: add `marklab project normal-mean` and `marklab project fused-gromov-wasserstein` as dependency-free typed nodes over the existing static PyMC 6.3.0 (Apache-2.0) and POT 0.9.7.post1 (MIT) adapters. Bind the exact raw input, canonical typed request, Python-3.12 lock digest, worker digest, backend/version/license identity, seed or deterministic solver controls, resource limits, and root executable provenance into the existing scheduler/durable execution identity. Persist only the strictly decoded typed worker result through the existing result codec, object store, ledger, head, and recovery transaction; reconstruct the unchanged one-shot scientific result on replay without starting Python.
- Consequences: the two already-runnable oracle-backed external workflows gain cross-process durable miss/hit behavior without a plugin framework, arbitrary task runner, new dependency, result-schema change, backend discovery, remote/container execution, or duplicated store/ledger/recovery/cache ownership. A shared descriptor may be extracted only from fields and process behavior both concrete callers require.

## DEC-0217 — Admit only exact produced-artifact dependency edges

- Date: 2026-08-26
- Status: accepted for WF-01/WS-12 dependency-bearing execution
- Decision: retain sequential typed node construction and the existing single-node scheduler lifecycle. A node with declared dependencies may execute only when each registered upstream specification has already produced exactly one of the dependent node's declared immutable input artifacts in the current project. Record the producing node-spec digest in new workflow-success state, restore it from the durable request, and reject missing or ambiguous produced-artifact edges before cache lookup or execution.
- Consequences: current callers can compose real typed data flow without a heterogeneous node registry, arbitrary task runner, dynamic dispatch container, parallel DAG executor, implicit latest-run selection, or cache-key duplication. Legacy direct project commits remain dependency-ineligible because they carry no workflow specification identity.

## DEC-0218 — Derive signed window distance from the canonical domain

- Date: 2026-08-26
- Status: accepted for FND-02/WS-22 signed boundary semantics
- Decision: expose signed boundary distance on `ObservationWindow2D` with positive values inside the permitted domain, zero on any exterior or hole boundary, and negative values outside the domain or inside a hole. Derive the sign from the canonical topology-aware containment result and the magnitude from the existing exact segment distance, and make classical spatial geometry consume that single owner.
- Consequences: signed distance gains an immediate production caller and consistent polygon/hole semantics without a second geometry representation, raster approximation, compartment model, coordinate-frame schema, or result-format change. Typed frames, compartments, and external differential validation remain separate outcomes.

## DEC-0219 — Grow typed marks through the existing declared-scalar caller

- Date: 2026-08-26
- Status: accepted for the current FND-04/WS-23 increment
- Decision: introduce the smallest row-aligned `MarkTable` needed by the existing declared marked-analysis path: stable `CellId` rows and typed binary, probability, and finite continuous columns with one column-wide measurement status, modality, unit, provenance artifact identity, and explicit missingness policy. Adapt the current binary/probability workflow from that table while preserving the compatibility `Pattern` and result-format 0.3; reject row, identity, probability, finite-value, provenance, status, unit, and undocumented-threshold mismatches at construction.
- Consequences: one production analysis consumes a multi-column typed mark owner instead of adding another endpoint-specific value wrapper. Categorical, ordinal, probability-simplex, vector-reference, Arrow/Parquet, arbitrary-unit, and real IHC round-trip/scale coverage remain open and must be added only with their own immediate callers.

## DEC-0220 — Admit typed global Moran inference through one concrete spatial-mark workflow

- Date: 2026-08-26
- Status: accepted for the current FND-02/FND-04/FND-06 and SIG-01A increment
- Decision: bind `ObservationWindow2D` to one installed physical `[X,Y]` micrometre frame and consume that bound domain, one finite continuous `MarkTable` column, fixed-radius `SpatialIndex2D` neighbours, an explicit binary-symmetric or row-standardized weight policy, and a method-specific random-labeling design in global Moran's I. The design names its replicate count, seed, alternative, and either no conditioning or the existing exact `histologic_compartment` row labels; permutations move whole scalar values only within declared strata. Reject frame drift, outside points, zero variance, isolated rows, empty/degenerate strata, non-finite results, and work beyond explicit point/edge/permutation limits.
- Consequences: one public scientific caller now joins typed frame, typed continuous mark, spatial weights, compartment-conditioned randomization, and deterministic inference without a generic weights framework or universal inference registry. Categorical/ordinal marks, polygon compartment partitions/interfaces, covariate residualization, local Moran maps, broader multiplicity, patient-level comparison, and external PySAL/R agreement remain separate outcomes.

## DEC-0221 — Share only the proven blocked-permutation design mechanics

- Date: 2026-08-26
- Status: accepted for FND-06 patient-label and global-Moran callers
- Decision: make `marklab-cohort::InferenceDesign` the typed owner of the fields now required by both concrete callers: analysis level, null family, permutation unit, complete exact block membership, positive replicate count, seed, method-owned seed namespace, alternative, and single-endpoint multiplicity. It emits deterministic whole-unit index permutations and rejects partial patient blocks, row-count drift, empty units, and block definitions with no exchangeable unit. Patient label permutation and continuous-mark global Moran retain their method-specific scientific validation and statistics while delegating only this schedule.
- Consequences: two production callers share one explicit randomization-unit contract without a plugin registry, universal statistic trait, arbitrary null callback, automatic unit fallback, parallel executor, or general multiplicity framework. Paired sign flips, hierarchical bootstrap, functional/Max-T schedules, cluster/interference designs, and non-single-endpoint multiplicity remain separate until they provide immediate callers for further extraction.

## DEC-0222 — Type the compartment column required by Moran conditioning

- Date: 2026-08-26
- Status: accepted for the current FND-04/WS-23 compartment-conditioned caller
- Decision: extend `MarkTable` with one dense categorical column specialization for the exact `histologic_compartment` row codes already emitted by the compatibility loader. Bind a stable mark ID, bounded ordered level labels, column-wide measurement status, histology modality, categorical unit, missingness policy, and exact provenance artifact; require codes to index the declared levels and match the compatibility row codes. Global Moran's stratified design must read this typed column and retain its mark identity/status rather than reading an untyped `Pattern` map directly.
- Consequences: categorical marks gain an immediate production caller and provenance-bearing result path without a generic dictionary/interchange format or arbitrary categorical endpoint. Ordinal, simplex, vector-reference, nullable physical encodings, multiple simultaneous categorical columns, and polygon compartment partitions remain open.

## DEC-0223 — Compose typed Moran runs through verified durable artifacts

- Date: 2026-08-26
- Status: accepted for WF-01/WS-12 and the current FND-06 increment
- Decision: expose global Moran inference as a typed source node and expose a descriptive pre/post node that consumes two exact produced Moran artifacts. Keep the source method, typed mark input, framed observation window, randomization design, resource limits, canonical private result codec, scheduler key, project output state, durable object/ledger/head transaction, and recovery owner unchanged. Add only a store-aware variant of the existing durable single-node invocation so schema-bound inputs follow the scheduler's already-existing `run_single_with_store` verification path before execution or replay.
- Consequences: a second real dependency-bearing scientific graph can reopen as three durable hits without recomputation, while provenance bytes remain verified. This does not add a heterogeneous registry, graph-wide executor, arbitrary task runner, new result-format version, inferential pre/post claim, or generalized serializer.

## DEC-0224 — Reuse the exact admitted radius weights for global Geary C

- Date: 2026-08-26
- Status: accepted for SIG-01B and the current FND-02/FND-04/FND-06 increment
- Decision: compute global Geary's C from the same exact directed radius edges, binary-symmetric or row-standardized weights, typed continuous mark, framed window, compartment-conditioned whole-value randomization, deterministic seed, and point/edge/permutation-work ceilings already admitted for global Moran's I. Keep a distinct Geary result type and hand oracle, while sharing only the concrete design, limit, alternative, weight-policy, edge-plan, and blocked-permutation mechanics that both immediate callers use.
- Consequences: Moran and Geary answer distinct formulas without hidden weight normalization or a generalized statistic/plugin registry. Local statistics, variograms, covariate residualization, external PySAL/R agreement, and patient-level comparison remain separate outcomes.

## DEC-0225 — Admit a bounded observed scalar semivariogram before its inference layer

- Date: 2026-08-26
- Status: accepted for the current SIG-01F/FND-03 increment
- Decision: compute observed scalar semivariance as one half of the mean squared mark difference in caller-declared contiguous physical-distance bins. Bind the exact typed continuous mark/status, framed window, row order and coordinates, bin edges, and a hard all-unordered-pair visit ceiling into one plan digest. Report empty bins as unavailable and state that no edge correction or randomization inference is applied in this increment.
- Consequences: the scalar curve and exact pair/bin data flow become executable with a hand oracle without claiming the full SIG-01F inferential contract. Permutation envelopes, pair edge correction where justified, directional variograms, scale evidence, and external agreement remain required before complete status.

## DEC-0226 — Add one blocked whole-value ERL envelope to the scalar semivariogram

- Date: 2026-08-26
- Status: accepted for the current SIG-01F/FND-06 increment
- Decision: reuse `marklab-cohort::InferenceDesign` to move complete scalar values across fixed locations, either globally or within exact typed `histologic_compartment` codes. Re-evaluate the existing scalar-semivariogram bins for every deterministic replicate and apply the existing `marklab-numerics` two-sided extreme-rank-length global envelope only across nonempty bins. Bind positive permutation count, seed, family-wise alpha, and a hard permutation-by-all-pair work ceiling; retain the observed result unchanged.
- Consequences: the admitted scalar curve gains one explicit curve-level family-wise test and simultaneous envelope without a new null engine, pointwise testing, generic statistic callback, or hidden scale selection. Directional variograms, edge-correction validation, external agreement, representative scale, and real cohort promotion remain open.

## DEC-0227 — Seal one closed real CellViT result bundle

- Date: 2026-08-26
- Status: accepted for RESULTS-CELLVIT-2DAY-01
- Decision: use one closed version-one bundle index for the eight lanes required by the immediate real-data caller: coordinate-only, scalar mark, vector embedding, annotation combination, patch/multiscale, patient level, pinned PyMC, and independent raw patch embedding. Every lane must be present exactly once and either name one regular result file or carry one nonempty blocker. Seal every regular non-symlink file by relative path and SHA-256, bind the exact objective and index digest, and reject missing, extra, changed, absolute, escaping, or symlinked content.
- Consequences: the real CPTAC CellViT run has one tamper-evident, deterministic provenance/diagnostic handoff without changing result-format 0.3, adding a dependency, inventing a registry, or treating a manifest as a new execution engine. The bundle records cell-aggregated patch results while explicitly leaving independent raw patch-vector tensors unavailable.

## DEC-0228 — Admit bounded floating recomputation tolerance in the classical codec

- Date: 2026-08-26
- Status: accepted for the durable classical real-data caller
- Decision: when the strict classical result decoder recomputes theoretical K, observed K, and L from already finite validated fields, accept exact equality or relative disagreement within sixteen binary64 epsilons; retain exact zero, count, denominator, radius, status, and all other semantic checks. This tolerance applies only to redundant calculated-float identities after serialization.
- Consequences: real noninteger polygon areas replay through the typed JSON codec without weakening result counts or permitting material formula drift. A regression moves the redundant calculated fields by one ULP and the existing inconsistent-count test remains rejected.

## DEC-0229 — Admit pinned NumPyro only for concrete cross-backend Bayesian agreement

- Date: 2026-08-26
- Status: accepted for BAY-01/BAY-02/WS-40 hierarchical agreement
- Decision: declare the already locked NumPyro 0.21.0/JAX 0.11.1 stack as a direct Python worker dependency and use it first for the exact typed Gaussian patient varying-intercept model already fitted by PyMC 6.3.0. The closed comparison owns both exact backend/environment/worker identities, the unchanged model/hierarchy/prior/likelihood/data and sampling controls, rank-normalized R-hat, bulk/tail ESS, divergences, tree-depth and energy diagnostics, posterior-predictive checks, and a caller-declared Monte Carlo agreement rule. A comparison that fails diagnostics or agreement remains diagnostic-only.
- Consequences: the immediate hierarchical caller can satisfy the first two-backend model gate without a plugin registry, free-form model string, implicit backend selection, new remote/container executor, or native substitute. NumPyro remains experimental and CPU-executed in current evidence despite being GPU-capable; direct GPU evidence, CmdStan integration, and field/point-process cross-backend calibration remain separate required outcomes.

## DEC-0230 — Evaluate hierarchy priors through explicit one-at-a-time fitted scenarios

- Date: 2026-08-26
- Status: accepted for BAY-02/BAY-03/WS-41/WS-44 prior sensitivity
- Decision: add one typed Gaussian-hierarchy sensitivity result and CLI that refit the existing pinned PyMC model under a baseline plus lower/upper one-at-a-time scale changes for the global-mean and between-patient standard-deviation priors. Preserve the exact patient data, likelihood, known observation scale, seed, sampling, diagnostics, and backend identities across scenarios; express posterior shifts in baseline posterior-standard-deviation units and require every scenario to converge before returning an ordinary sensitivity state.
- Consequences: the immediate hierarchy caller receives an executable, diagnostics-gated prior analysis without a generalized prior registry, hidden default grid, free-form model, or approximate substitute. Joint prior grids, prior-location changes, model-form sensitivity, SBC, and field/point-process sensitivity remain separate outcomes.

## DEC-0231 — Calibrate the exact NumPyro patient hierarchy with bounded SBC

- Date: 2026-08-26
- Status: accepted for BAY-02/BAY-03/WS-40/WS-41/WS-44 calibration
- Decision: simulate global means, positive between-patient scales, patient effects, and nested observations from the exact typed Gaussian-hierarchy prior/likelihood, then refit each replicate with the pinned NumPyro NUTS backend. Retain complete replicate/failure disposition, ranks and 90% coverage for the population mean and heterogeneity scale, rank histograms, declared uniformity/coverage gates, per-fit R-hat/bulk-tail ESS/divergence/tree-depth checks, deterministic seeds, and hard simulation/iteration/output/time bounds. Calibration is complete only when every fit passes the unchanged diagnostic policy and both parameter families pass the declared aggregate gates.
- Consequences: the immediate hierarchy caller gains real simulation-based calibration rather than inheriting the conjugate scalar SBC result. The bounded balanced nested design does not prove all real 121-patient shapes, non-Gaussian likelihoods, field/point-process models, or biological calibration; those remain separate evidence requirements.

## DEC-0232 — Compare the exact gridded LGCP across PyMC and NumPyro

- Date: 2026-08-27
- Status: accepted for BAY-FIELD-A/BAY-PP-A/WS-40 cross-backend agreement
- Decision: fit the existing typed fixed-grid, fixed-Matérn, noncentered gridded LGCP request independently with the already pinned NumPyro 0.21.0/JAX 0.11.1 backend and compare it with the existing PyMC 6.3.0 fit. Preserve the exact data, grid, window, covariance, priors, likelihood, seed, sampling and diagnostic policies; retain both exact backend/environment/worker/request identities; and gate global parameters, latent-cell effects, and expected cell counts with explicit Monte Carlo and absolute tolerances. Any nonconverged fit or failed comparison remains diagnostic-only.
- Consequences: one concrete field caller gains a second-engine check without a generalized backend registry, model language, arbitrary task runner, implicit backend selection, GPU claim, or new result-format version. LGCP simulation calibration, prior/kernel sensitivity, spatial posterior-predictive checks, CmdStan admission, and actual GPU evidence remain separate outcomes.

## DEC-0233 — Calibrate the exact fixed-grid LGCP with bounded SBC

- Date: 2026-08-27
- Status: accepted for BAY-FIELD-A/BAY-PP-A/WS-40/WS-44 calibration
- Decision: generate intercepts, coefficients, fixed-covariance latent fields, and Poisson cell counts from the exact typed gridded-LGCP prior/likelihood already consumed by the real CellViT caller, then refit each replicate with the pinned NumPyro backend. Retain complete replicate/failure disposition, rank and 90% coverage diagnostics for both global parameters and one prespecified latent-cell coordinate, unchanged per-fit convergence gates, deterministic seeds, and hard simulation/iteration/output/time bounds.
- Consequences: the concrete fixed-grid field model gains simulation calibration without a generalized calibration registry or a claim about learned kernel hyperparameters, arbitrary meshes, all latent directions, real biological calibration, or a different point-process family.

## DEC-0234 — Diagnose gridded-LGCP spatial replication from existing posterior patterns

- Date: 2026-08-27
- Status: accepted for BAY-PP-A/WS-43 posterior predictive validation
- Decision: consume the existing complete-fit-only gridded-LGCP posterior pattern artifact and report cell-count variance plus horizontal/vertical adjacent-cell mean absolute contrast for the observation and every bounded replicate, with inclusive posterior-predictive tail probabilities. Keep the original typed prediction and its exact draw/point provenance intact in a dedicated result rather than adding fields to the established fit result.
- Consequences: the current real fixed-grid caller gains spatial posterior-predictive evidence beyond total/zero counts without changing result-format 0.3, the existing fit schema, the simulator, or the artifact store. These two grid diagnostics do not establish continuous-space K-function fit or adequacy for other point-process families.

## DEC-0235 — Evaluate fixed-grid LGCP priors and kernel choices one factor at a time

- Date: 2026-08-27
- Status: accepted for BAY-FIELD-A/BAY-PP-A/WS-41 sensitivity
- Decision: refit the existing PyMC gridded LGCP under a baseline plus lower/upper one-at-a-time multipliers for intercept-prior scale, coefficient-prior scale, fixed Matérn amplitude, and fixed Matérn length scale. Preserve exact inputs, likelihood, seed, sampling, jitter, backend identity, and diagnostics across scenarios; report global shifts and cellwise latent/expected-count RMS shifts in baseline posterior-standard-deviation units; and withhold an ordinary sensitivity state unless every fit converges.
- Consequences: the concrete CellViT field caller receives a bounded nine-fit prior/kernel analysis without joint grids, learned kernel hyperparameters, a generic scenario engine, or a robustness claim outside the declared multiplier range.

## DEC-0236 — Add a genuine patient beta-binomial hierarchy as the first non-Gaussian model

- Date: 2026-08-27
- Status: accepted for BAY-HIER-A/WS-41
- Decision: add one typed patient-level successes/trials model with a Beta prior on the population probability, a half-Normal prior on positive concentration, patient probabilities drawn from the implied Beta distribution, and binomial observations. Retain exact patient order/identity, prior rationale, concentration-derived overdispersion, partial-pooling summaries, posterior-predictive total and between-patient dispersion checks, pinned PyMC identity, normalized diagnostics, deterministic seed, and hard patient/trial/iteration/output/time bounds.
- Consequences: Marklab gains a genuine non-Gaussian hierarchical likelihood without changing the existing fixed-prior independent-group beta diagnostic, adding a general model language, treating cells as independent patients, or claiming all binomial/count/ordinal/hurdle families.

## DEC-0237 — Fit robust repeated ROI observations with a Student-t patient hierarchy

- Date: 2026-08-27
- Status: accepted for BAY-03/BAY-HIER-A/WS-41
- Decision: add a typed noncentered patient varying-intercept model with Normal population-mean prior, half-Normal between-patient and observation-scale priors, and an exponential prior on degrees of freedom above two for a finite-variance Student-t likelihood. Reuse the admitted patient/observation CSV shape, retain patient partial pooling, robust residual posterior-predictive checks, exact pinned PyMC identity, normalized diagnostics, deterministic seed, and existing observation/iteration/output/time bounds.
- Consequences: the real repeated ROI caller gains a heavy-tailed alternative without inventing crossed effects or varying slopes unsupported by its two-column design, changing the Gaussian contract, or claiming that robustness removes the need for sensitivity/calibration.

## DEC-0238 — Route the exact Student-t hierarchy through durable execution

- Date: 2026-08-27
- Status: accepted for PLAT-01/WF-01/BACK-01/BAY-03
- Decision: add one static `marklab project student-t-hierarchy` node using the existing project, scheduler, content-addressed store, execution ledger, pending-intent recovery, cache-key owner, and output transaction. Its identity includes exact raw input bytes, typed model/prior/sampling/resource request, PyMC/Python/lock/worker identities, deterministic seed, native runtime, and strict result codec; a hit must decode and validate without starting Python.
- Consequences: the real robust hierarchy becomes resumable and cross-process replayable without a new task runner, artifact store, backend registry, result format, or duplicate recovery/cache machinery.

## DEC-0239 — Compare the exact Student-t hierarchy across PyMC and NumPyro

- Date: 2026-08-27
- Status: accepted for BAY-01/BAY-03/BAY-HIER-A/WS-40/WS-44
- Decision: fit the existing typed finite-variance Student-t patient hierarchy independently with pinned NumPyro 0.21.0/JAX 0.11.1 and compare population mean, between-patient scale, observation scale, degrees of freedom, and patient-mean field against PyMC 6.3.0 under explicit Monte Carlo plus minimum absolute tolerances. Preserve the exact data, priors, parameterization, likelihood, seed, sampling, diagnostics, and backend/environment/worker/request identities; any failed fit or comparison is diagnostic-only.
- Consequences: the robust real caller gains a second-engine check without a general model language, implicit backend selection, CmdStan/GPU claim, or a substitute for SBC and prior sensitivity.

## DEC-0240 — Evaluate Student-t hierarchy priors and tail weight one factor at a time

- Date: 2026-08-27
- Status: accepted for BAY-03/BAY-HIER-A/WS-41/WS-44
- Decision: refit the exact PyMC Student-t hierarchy under baseline plus lower/upper one-at-a-time multipliers for the population-mean prior scale, between-patient scale prior, observation-scale prior, and exponential rate controlling degrees of freedom above two. Preserve the exact data, likelihood, seed, sampling, backend identity, and diagnostics; report posterior shifts in baseline posterior-standard-deviation units and withhold an ordinary sensitivity state unless all nine fits converge.
- Consequences: the real robust hierarchy gains a bounded prior/tail sensitivity result without a generic scenario framework, joint grid, hidden default, or robustness claim outside the declared multipliers.

## DEC-0241 — Calibrate the exact Student-t patient hierarchy with bounded SBC

- Date: 2026-08-27
- Status: accepted for BAY-02/BAY-03/BAY-HIER-A/WS-44
- Decision: simulate population means, between-patient scales, patient effects, observation scales, degrees of freedom above two, and repeated observations from the exact typed Student-t prior/likelihood, then refit every replicate with the same dense-mass NumPyro NUTS model used for cross-backend agreement. Use a fixed calibration-only maximum tree depth of 12 for the near-zero scale geometry while retaining the zero tree-depth-hit gate, complete failure disposition, ranks and 90% coverage for all four population parameters, all other unchanged per-fit diagnostics, deterministic seeds, and hard replicate/observation/iteration/output/time bounds.
- Consequences: the robust hierarchy gains generative calibration without a general calibration registry, omitted failed fits, relaxed thresholds, or a claim about crossed/varying-slope designs and other non-Gaussian likelihoods.

## DEC-0242 — Admit CellViT neoplastic counts for the existing beta-binomial hierarchy

- Date: 2026-08-27
- Status: accepted for BAY-03/BAY-HIER-A/WS-41 real count admission
- Decision: extend the existing full-corpus CellViT adapter to emit one sorted patient successes/trials table from the already verified row-aligned hard annotations and case map. Success means the unique source `Neoplastic` class; trials mean every admitted classified CellViT cell across all admitted slides for that patient. Retain exact source and adapter digests, patient aggregation, class identity, total counts, and the limitation that classifier outputs and spatially correlated cells are not independent biological Bernoulli trials.
- Consequences: the current beta-binomial caller can execute on a truthful real count table without inventing denominators, relabeling classes, or adding a general count importer. Its result remains exploratory model-based composition evidence and cannot be described as manual pathology ground truth, cell-level independence evidence, calibration, or clinical evidence.

## DEC-0243 — Route the exact beta-binomial hierarchy through durable execution

- Date: 2026-08-27
- Status: accepted for PLAT-01/WF-01/BACK-01/BAY-03
- Decision: add one static `marklab project beta-binomial-hierarchy` node using the existing project, scheduler, artifact store, execution ledger, pending-intent recovery, cache-key owner, and output transaction. Its identity includes exact raw count-table bytes, typed prior/model/sampling/resource request, PyMC/Python/lock/worker identities, deterministic seed, native runtime, and strict result codec; a hit must decode and validate without starting Python.
- Consequences: the real CellViT count hierarchy becomes resumable and cross-process replayable without a new task runner, store, registry, result format, or duplicated recovery/cache mechanism.

## DEC-0244 — Compare the exact beta-binomial hierarchy across PyMC and NumPyro

- Date: 2026-08-27
- Status: accepted for BAY-01/BAY-03/BAY-HIER-A/WS-40/WS-44
- Decision: fit the existing typed patient beta-binomial hierarchy independently with pinned NumPyro 0.21.0/JAX 0.11.1 and compare population probability, positive concentration, and all patient probabilities against PyMC 6.3.0 under explicit Monte Carlo plus minimum absolute tolerances. Preserve exact successes/trials, priors, likelihood, seed, sampling, diagnostic policy, and backend/environment/worker/request identities; any failed fit or comparison is diagnostic-only.
- Consequences: the real classifier-derived count caller gains a second-engine check without a model language, backend registry, implicit selection, CmdStan/GPU claim, or substitute for prior sensitivity and simulation calibration.

## DEC-0245 — Evaluate beta-binomial hierarchy priors one factor at a time

- Date: 2026-08-27
- Status: accepted for BAY-03/BAY-HIER-A/WS-41/WS-44
- Decision: refit the exact PyMC beta-binomial hierarchy under a baseline plus lower/upper one-at-a-time multipliers for population Beta alpha, population Beta beta, and the positive concentration-prior scale. Preserve exact successes/trials, likelihood, seed, sampling, backend identity, and diagnostics; report population, concentration, and patient-field shifts in baseline posterior-standard-deviation units and withhold an ordinary sensitivity state unless all seven fits converge.
- Consequences: the real count caller gains a bounded prior sensitivity result without a general scenario engine, joint grid, hidden defaults, or a robustness claim outside the declared multipliers.

## DEC-0246 — Calibrate the exact beta-binomial patient hierarchy with bounded SBC

- Date: 2026-08-27
- Status: accepted for BAY-02/BAY-03/BAY-HIER-A/WS-44
- Decision: generate population probability, positive concentration, patient probabilities, and successes from the exact typed Beta/half-Normal/Beta/binomial prior and likelihood using the admitted trial-count shape, then refit every replicate with the mathematically equivalent patient-probability-collapsed beta-binomial likelihood and dense-mass NumPyro NUTS. Draw the prespecified patient probability from its exact conditional Beta posterior for rank/coverage. Use a fixed calibration-only maximum tree depth of 12 while retaining zero depth hits, unchanged per-fit diagnostics, complete failure disposition, ranks and 90% coverage for both population parameters and that patient coordinate, deterministic seeds, and hard replicate/trial/iteration/output/time bounds.
- Consequences: the real count hierarchy gains generative calibration without a calibration registry, omitted failed fits, relaxed thresholds, or a claim that classifier-derived spatial cells are independent biological trials.

## DEC-0247 — Estimate one patient-unit beta-binomial molecular-group contrast

- Date: 2026-08-27
- Status: accepted for BAY-03/BAY-HIER-A/WS-41
- Decision: extend the existing full-corpus adapter with an exact patient-ID join between admitted CellViT counts and the already pinned MSI/MSS molecular-label manifest, preserving every numerator and denominator. Add one typed two-group patient beta-binomial regression with a Normal reference log-odds prior, Normal comparison-minus-reference log-odds prior, half-Normal concentration prior, collapsed beta-binomial likelihood, and exact conditional patient-probability draws. Require exact declared reference/comparison labels and at least four patients per group; retain probability difference, odds ratio, partial pooling, group-aware posterior predictive checks, normalized diagnostics, deterministic seed, and hard patient/trial/iteration/output/time bounds.
- Consequences: the admitted MSI/MSS count intersection can estimate an exploratory patient-level classifier-composition contrast without using cells as population replicates, adding a general formula language, or claiming clinical, causal, manual-ground-truth, or independent-cell evidence.

## DEC-0248 — Cross-check the molecular-group contrast with independent NUTS implementations

- Date: 2026-08-27
- Status: accepted for BAY-01/BAY-03/BAY-HIER-A/WS-44
- Decision: execute the exact DEC-0247 collapsed beta-binomial group model in pinned PyMC and pinned dense-mass NumPyro, preserving identical priors, likelihood, patient ordering, seed, sampling controls, diagnostic policy, posterior estimands, exact conditional patient draws, and group-aware posterior predictive summaries. Gate the intercept, group effect, both group probabilities, probability difference, odds ratio, concentration, and every patient probability by interval overlap and a declared Monte Carlo-error-aware tolerance.
- Consequences: the group contrast gains an independent implementation check without creating a backend registry, changing the model, or promoting exploratory classifier composition to clinical or causal evidence.

## DEC-0249 — Measure molecular-group prior sensitivity on the scientific estimands

- Date: 2026-08-27
- Status: accepted for BAY-03/BAY-HIER-A/WS-44
- Decision: run the DEC-0247 patient beta-binomial group regression over a fixed seven-scenario one-at-a-time grid: baseline plus 0.5x and 2x intercept-scale, group-effect-scale, and concentration-scale priors. Preserve the input, seed, likelihood, sampling, and diagnostic controls, and measure every population estimand plus the full patient field against its baseline posterior standard deviation with a declared material-shift threshold.
- Consequences: the exploratory MSI/MSS contrast reports whether reasonable prior-scale changes materially alter the conclusion, while keeping prior sensitivity distinct from cross-backend agreement and calibration.

## DEC-0250 — Calibrate the patient beta-binomial group regression generatively

- Date: 2026-08-27
- Status: accepted for BAY-02/BAY-03/BAY-HIER-A/WS-44
- Decision: simulate intercept, group effect, positive concentration, group-conditioned patient probabilities, and patient successes from the exact DEC-0247 priors and likelihood using the admitted group/trial-count shape. Refit every replicate with the mathematically equivalent collapsed beta-binomial likelihood in pinned dense-mass NumPyro, draw the first patient probability from its exact conditional Beta posterior, and retain complete failure disposition, deterministic seeds, hard work bounds, unchanged per-fit diagnostic gates, rank uniformity, and 90% coverage for intercept, group effect, concentration, probability difference, and the patient coordinate.
- Consequences: the group regression gains end-to-end simulation-based calibration without omitted failures, relaxed sampler gates, a calibration registry, or any assertion that CellViT classifier outputs are independent biological trials.

## DEC-0251 — Adjust the molecular-group contrast for one complete patient covariate

- Date: 2026-08-27
- Status: accepted for BAY-03/BAY-HIER-A/WS-41
- Decision: join the exact DEC-0247 count/label table to the already hashed clinical source's `Gender` row, which is complete for all 105 labeled patients and has at least four patients in every MSI/MSS by Female/Male design cell. Fit one additive collapsed patient beta-binomial model with explicit group and gender log-odds effects, no interaction, a shared positive concentration, exact conditional patient probabilities, and marginal group probabilities standardized to the observed gender distribution. Retain the four design-cell probabilities, both conditional group differences, the standardized marginal difference, odds ratios, PPC, diagnostics, deterministic seed, and hard work bounds.
- Consequences: the MSI/MSS classifier-composition contrast can be compared with one fully observed demographic adjustment without silently dropping patients, inferring the clinical source's undeclared age units, adding a formula language, or claiming confounding control, causality, or clinical validity.

## DEC-0252 — Cross-check the gender-adjusted count model independently

- Date: 2026-08-27
- Status: accepted for BAY-01/BAY-03/BAY-HIER-A/WS-44
- Decision: execute the exact DEC-0251 additive collapsed beta-binomial model in pinned PyMC and pinned dense-mass NumPyro with identical priors, data, patient ordering, seed, sampling controls, diagnostics, conditional patient draws, standardization weights, and PPC. Gate every reported scalar estimand and all 105 patient probabilities by interval overlap and declared Monte Carlo-error-aware tolerances.
- Consequences: the adjusted MSI/MSS result gains an independent implementation check without changing the model, adding a backend registry, or implying that one demographic adjustment establishes clinical or causal validity.

## DEC-0253 — Measure adjusted-model prior sensitivity

- Date: 2026-08-27
- Status: accepted for BAY-03/BAY-HIER-A/WS-44
- Decision: run the DEC-0251 model over a fixed nine-scenario one-at-a-time grid: baseline plus 0.5x and 2x intercept, group-effect, gender-effect, and concentration prior scales. Hold the exact patient design, standardization weights, seed, likelihood, sampler, and diagnostics fixed; measure every reported scalar estimand and all patient probabilities against baseline posterior SD with a declared material threshold.
- Consequences: the adjusted contrast reports bounded prior sensitivity without a formula system, arbitrary scenario registry, or universal robustness claim.

## DEC-0254 — Calibrate the gender-adjusted count model generatively

- Date: 2026-08-27
- Status: accepted for BAY-02/BAY-03/BAY-HIER-A/WS-44
- Decision: simulate intercept, group effect, gender effect, positive concentration, group/gender-conditioned patient probabilities, and patient successes from the exact DEC-0251 additive priors and likelihood using the admitted design and trial-count shape. Refit every replicate with the mathematically equivalent collapsed beta-binomial likelihood in pinned dense-mass NumPyro, preserve the observed-gender standardization for the marginal group contrast, draw the first patient probability from its exact conditional Beta posterior, and retain complete failure disposition, deterministic seeds, hard work bounds, unchanged per-fit diagnostic gates, rank uniformity, and 90% coverage for all three regression coefficients, concentration, the marginal contrast, and the patient coordinate.
- Consequences: the adjusted count model gains end-to-end simulation-based calibration without omitted failures, relaxed sampler gates, a formula or calibration registry, or any assertion that one demographic adjustment establishes causal or clinical validity.

## DEC-0255 — Model repeated CellViT slides within patient without changing the biological unit

- Date: 2026-08-27
- Status: accepted for BAY-03/BAY-HIER-A/WS-41
- Decision: admit exact nonempty per-slide Neoplastic/all-classified-cell counts only when they reaggregate byte-exactly to the existing 105-patient group/gender table. Fit one additive group-plus-gender beta-binomial slide likelihood with a non-centered Normal patient random intercept, half-Normal patient log-odds scale, shared half-Normal slide concentration, no interaction, and the existing patient-level group/gender design. Require at least eight patients with repeated nonempty slides and four patients per design cell; keep patient as the biological inference unit, compute group/gender posterior predictive contrasts from patient-aggregated slide replicas, and report the typical-patient observed-gender-standardized group contrast.
- Consequences: the actual 217 informative slides can exercise one genuine nested repeated-measure hierarchy without treating slides or cells as independent cohort replicates, adding a formula/mixed-model framework, claiming general random slopes/crossed effects, or converting exploratory classifier composition into causal or clinical evidence. Two exact zero-cell slides remain provenance-recorded and contribute no likelihood row.

## DEC-0256 — Route the repeated-slide hierarchy through durable execution

- Date: 2026-08-27
- Status: accepted for PLAT-01/WF-01/BACK-01/BAY-03
- Decision: add one static `marklab project beta-binomial-group-gender-slide-hierarchy` node using the existing durable project, scheduler, artifact store, cache-key owner, ledger, recovery, output transaction, and typed result codec. Its cache identity includes exact slide-table bytes, typed nested-model request, seed and sampler controls, PyMC/Python/lock/worker identities, the static Apache-2.0 backend descriptor, and native runtime identity; a hit must decode and validate without starting Python.
- Consequences: the real repeated-slide hierarchy becomes cross-process resumable without a new task runner, store, registry, plugin surface, or duplicated transaction/recovery mechanism.

## DEC-0257 — Cross-check the repeated-slide hierarchy independently

- Date: 2026-08-27
- Status: accepted for BAY-01/BAY-03/BAY-HIER-A/WS-44
- Decision: execute the exact DEC-0255 non-centered patient-random-intercept and slide beta-binomial model in pinned PyMC and pinned dense-mass NumPyro with identical data, priors, patient indexing, seed, sampling controls, diagnostics, typical-patient standardization, and patient-unit/slide-deviation posterior predictive summaries. Gate every reported scalar, all patient probabilities, and all patient random log-odds effects by interval overlap and declared Monte Carlo-error-aware absolute tolerances.
- Consequences: the repeated-measure caller gains an independent implementation check without changing its biological unit, parameterization, likelihood, adding a backend registry/formula language, or substituting backend agreement for prior sensitivity and simulation calibration.

## DEC-0258 — Measure repeated-slide hierarchy prior sensitivity

- Date: 2026-08-27
- Status: accepted for BAY-03/BAY-HIER-A/WS-44
- Decision: refit the exact DEC-0255 PyMC model under baseline plus 0.5x and 2x one-at-a-time multipliers for intercept, group-effect, gender-effect, patient-log-odds-scale, and slide-concentration prior scales. Preserve exact slide/patient design, likelihood, seed, sampler, PPC, and diagnostic gates; measure every scalar estimand plus complete patient-probability and patient-random-effect fields in baseline posterior-SD units, and withhold an ordinary sensitivity state unless all 11 fits converge.
- Consequences: the nested real caller gains a bounded prior sensitivity result without a formula/scenario framework, joint grid, hidden defaults, or robustness claim outside the declared multipliers.

## DEC-0259 — Calibrate the repeated-slide hierarchy generatively

- Date: 2026-08-27
- Status: accepted for BAY-02/BAY-03/BAY-HIER-A/WS-44
- Decision: simulate fixed intercept/group/gender effects, positive patient log-odds scale, patient random intercepts, patient probabilities, positive slide concentration, slide probabilities, and successes from the exact DEC-0255 priors and beta-binomial likelihood using the admitted patient/slide/trial design. Refit every replicate with the same non-centered dense-mass NumPyro model used for backend agreement, retain complete failure disposition, deterministic seeds, hard work bounds, unchanged diagnostic gates, rank uniformity, and 90% coverage for all global parameters, the typical-patient standardized group contrast, one patient probability, and one patient random effect.
- Consequences: the nested caller gains end-to-end generative calibration without omitted failures, relaxed sampler gates, a calibration registry, or any claim that classifier-derived spatial cells are independent biological trials.

## DEC-0260 — Replay exact training-only region retrieval through durable projects

- Date: 2026-08-27
- Status: accepted for WF-01/WS-12/EMB-RETRIEVAL-01
- Decision: expose the existing exact region-retrieval algorithm as `marklab project region-retrieval` with the exact training and query CSV identities, candidate count, patient/site leakage policy, visit ceiling, native executable/runtime identity, and implementation revision in the durable cache contract. Preserve the existing user-visible `marklab.region_retrieval` version-one document. Use a private typed artifact codec that represents every finite floating-point value by its exact `u64` bits so scheduler decode/re-encode is byte-canonical; on replay, require exact identities/order/ranks, at most one ULP for the rederived training transform, and a `64 * epsilon` bound for rederived distances, contributions, and OOD.
- Consequences: patient-held-out retrieval can miss once and replay byte-identically across processes through the existing project ledger, scheduler, artifact store, and `ExecuteAlgorithm`, without a generic native runner, plugin framework, new public result format, or second algorithm execution.

## DEC-0261 — Admit the first CRC fingerprint ladder increment as fixed M0/M1 patient features

- Date: 2026-08-27
- Status: accepted for COH-01/EMB-RETRIEVAL-01/WS-34
- Decision: join exact TCGA CRC patient identities across frozen molecular labels, GDC pathologic stage, label-free CellViT spatial summaries, and annotation-based tumor-microenvironment summaries. Define M0 as stage, log cell count, tumor/inflammatory/connective fractions, and cell density; define M1 as M0 plus prespecified inflammatory/stromal neighborhood enrichment and residual-organization summaries. Construct features without molecular labels, exclude each query patient before fitting its retrieval transform, and separately exclude its complete tissue-source site for leakage sensitivity. Record the constant-zero background fraction as unavailable rather than forcing it into the fingerprint.
- Consequences: the immediate 169-patient TCGA caller gains real M0/M1 full-distance rankings, held-out retrieval, patient-bootstrap uncertainty, and patient-label MMD/energy evidence without treating cells/slides/edges as replicates, optimizing toward significance, adding a fingerprint registry, or claiming that later M2-M7 lanes are complete.

## DEC-0262 — Add one exact border-corrected F/G/J workflow on the canonical window

- Date: 2026-08-27
- Status: accepted for FND-02/FND-03/PP-04/WS-12/WS-22/WS-30
- Decision: add a separate typed nearest/empty-space result family that reuses `ObservationWindow2D`, `SpatialIndex2D`, whole-pattern conditional CSR, deterministic seeds, and the durable project transaction. Estimate G from event-to-nearest-other-event distances and F from a declared fixed cell-centred rectangular probe grid, using the standard reduced-sample border rule independently for events and probes. Define J only when both inputs exist and `1-F` exceeds an explicit positive denominator floor; otherwise persist a typed unavailable state, never infinity. Reuse the same probes across observed and null patterns, report their spacing and maximum cell-centre location error, run separate ERL envelopes for F, G, and eligible J radii, and enforce explicit point/radius/probe/query/draw/memory ceilings.
- Consequences: users gain a complete one-shot and durable `nearest-space` workflow with exact polygon/hole semantics and cross-process replay. This is not an inhomogeneous, multitype, patient-level, mechanistic, or clinical analysis, and a deterministic grid is not represented as a random Monte Carlo sample.

## DEC-0263 — Consume the typed categorical MarkTable in one shared-pair workflow

- Date: 2026-08-27
- Status: accepted for FND-03/FND-04/FND-06/PP-03/MRK-01/WF-01/WS-12/WS-23/WS-30
- Decision: add one exact two-level categorical workflow over the existing provenance-validated `histologic_compartment` MarkTable column and framed observation window. Retain one bounded directed pair plan once; report a radius-shell directed mark-connection probability and cumulative standard-border directed cross-K for one declared distinct source/target level pair. Random-labeling inference must permute complete categorical rows through the existing `InferenceDesign`, preserve all level counts and geometry, and compute separate two-sided ERL families for connection and cross-K. Bind level order, measurement status, radii, null controls, seed, and all point/pair/permutation/memory ceilings into the typed result and durable cache identity.
- Consequences: the first general categorical MarkTable caller becomes a complete scientific/durable workflow without a generic mark registry, category discovery, implicit symmetrization, inhomogeneous correction, or treating cells as patient replicates. Other categorical columns, continuous mark correlation, mark-weighted K, pair-correlation g, and patient-level comparison remain separate callers.

## DEC-0264 — Evaluate binary probability mark connection by exact expected contributions

- Date: 2026-08-27
- Status: accepted for MRK-01/MRK-01A/MRK-02C/FND-03/FND-04/FND-06/WF-01/WS-12/WS-23/WS-30
- Decision: for one declared dense binary probability column, define the shell mark-connection endpoint as `sum_(i,j) p_i p_j / number_of_eligible_directed_pairs`, using the existing standard-border directed pair plan. This is the exact conditional expectation of positive-positive pair membership under independent Bernoulli label uncertainty, not a sampled-label analysis. Random-label inference permutes complete probability rows over fixed locations, and the global random-label expectation uses the exact without-replacement ordered mass `((sum p)^2 - sum p^2) / (n(n-1))`. Require nonzero expected positive-positive pair mass and nonzero effective negative mass; bind the mark identity, expected-contribution mode, geometry, null controls, seed, and resource ceilings into the result and durable cache identity.
- Consequences: categorical and probabilistic mark connection share one concrete retained geometry owner while preserving separate formulas and outputs. This does not define probability-simplex, sampled-label uncertainty, continuous mark correlation, mark-weighted K, inhomogeneous correction, arbitrary category discovery, or a generic mark-interaction framework.

## DEC-0265 — Normalize positive continuous mark correlation by one global arithmetic mean

- Date: 2026-08-27
- Status: accepted for MRK-01/MRK-01B/FND-03/FND-04/FND-06/WF-01/WS-12/WS-23/WS-30
- Decision: for the existing positive finite `nucleus_area_um2` MarkTable column, define the shell mark-correlation curve as `mean_(eligible directed pairs in shell)(m_i m_j) / global_mean(m)^2`. Compute the global arithmetic mean once over every admitted row in fixed order with compensated `f64` accumulation; never recompute it by radius, edge eligibility, or permutation. Report the exact finite-row random-label expectation `(((sum m)^2 - sum m^2) / (n(n-1))) / global_mean(m)^2`, reject zero mark variance, and permute complete continuous rows over the shared fixed standard-border pair plan for one two-sided ERL family.
- Consequences: the endpoint matches the named normalized positive-mark product family and remains distinct from centered covariance, Pearson correlation, variograms, and mark-weighted K. The first caller is fixed to the existing square-micrometre nucleus-area declaration; arbitrary units/continuous columns, covariate residualization, stratified nulls, and weighted K remain separate milestones.

## DEC-0266 — Define cumulative mark-weighted K against the unweighted standard-border baseline

- Date: 2026-08-27
- Status: accepted for MRK-01/MRK-01C/FND-03/FND-04/FND-06/PP-01/WF-01/WS-12/WS-23/WS-30
- Decision: for the same admitted positive `nucleus_area_um2` mark, use pair weight `m_i m_j / global_mean(m)^2` and define cumulative weighted K as `area * sum_(eligible ordered pairs within r)(weight_ij) / (n * eligible_centers(r))`. Report the existing unweighted standard-border K with the identical pair and denominator as an explicit baseline, plus its exact finite-row random-label weighted expectation. Keep the global mean fixed over all rows and permute complete continuous marks over fixed locations for one two-sided weighted-K ERL family.
- Consequences: weighted K answers cumulative weighted-neighbor mass and is not renamed mark correlation. The workflow reuses the concrete pair, arithmetic, edge, null, resource, codec, and durable owners without adding a weight-function registry. Other weights, inhomogeneous correction, arbitrary continuous columns, and covariate-conditional nulls remain separate callers.

## DEC-0267 — Freeze homogeneous pair-correlation to one compact-support border estimator

- Date: 2026-08-27
- Status: accepted for PP-03/PP-03A/PP-03B/FND-03/FND-06/WF-01/WS-12/WS-30
- Decision: add a distinct homogeneous pair-correlation result family using the symmetric Epanechnikov kernel `3(1-u^2)/(4h)` on `|u| <= 1`, one explicit positive physical bandwidth `h`, and standard-border center eligibility at `r+h`. Define unmarked `g(r)` as `area * sum k_h(r-d_ij) / (2*pi*r*n*eligible_centers(r+h))`; define directed categorical cross-g analogously with the target count in place of `n`. Radii at or below the bandwidth, non-finite normalization, no eligible centers, and no directed pairs in kernel support are unavailable or rejected explicitly. Use whole-pattern conditional CSR for unmarked inference and complete-row random labeling for cross-g, each with a separate two-sided ERL family and bounded pair/kernel/null work.
- Consequences: users gain a density-normalized kernel statistic that is not an alias of shell counts, centered mark covariance, K, or cross-K. The two immediate callers may share only the frozen kernel-support calculation after both concrete estimators use it; bandwidth selection, kernel registries, inhomogeneous correction, intensity estimation, translation/isotropic correction, and automatic type-pair expansion remain outside this decision.

## DEC-0268 — Consume one explicit leave-one-out intensity in standard-border inhomogeneous K/L

- Date: 2026-08-27
- Status: accepted for PP-02/PP-05/FND-02/FND-03/FND-06/WF-01/WS-12/WS-30
- Decision: add one end-to-end unmarked workflow with a caller-supplied physical bandwidth and the normalized isotropic two-dimensional Gaussian kernel. At each observed event, exclude that event, multiply the remaining kernel sum by `n/(n-1)`, and divide by deterministic cell-centred quadrature of the kernel mass over the exact observation window. Persist every row intensity, boundary mass, training count, bandwidth, grid spacing/discretization, and leave-one-out policy; reject nonpositive, non-finite, or below-floor intensity. Define standard-border inhomogeneous K as `sum_(eligible ordered pairs) 1/(lambda_i lambda_j) / sum_(eligible centers) 1/lambda_i`, which reduces exactly to the existing homogeneous reduced-sample estimator when intensity is constant, and define `L=sqrt(K/pi)`. For within-pattern inference, freeze the observed all-point pilot on the same quadrature grid, draw the conditioned event count from its declared piecewise-cell mass with uniform jitter restricted to the exact window, evaluate that fixed pilot at null events, and form one two-sided ERL family. Bind all draw, intensity-evaluation, pair, probe, radius, point, and memory ceilings.
- Consequences: PP-05 gains one concrete estimator only because PP-02 consumes it immediately. The result is descriptive per specimen and does not claim density correction without the persisted pilot and diagnostics. This does not add automatic bandwidth selection, a general intensity artifact registry, compartment estimators, translation/isotropic correction, inhomogeneous g, type-specific rare-mark intensity, continuous exact polygon-kernel integration, or patient-level population inference.

## DEC-0269 — Reuse the persisted intensity pilot in inhomogeneous pair-correlation g

- Date: 2026-08-27
- Status: accepted for PP-02/PP-03/PP-03A/PP-05/FND-03/FND-06/WF-01/WS-12/WS-30
- Decision: add one inhomogeneous pair-correlation workflow whose base configuration is the exact DEC-0268 Gaussian leave-one-out intensity/null contract and whose separate caller-supplied pair bandwidth uses the positive-support Epanechnikov kernel. Require every radius to exceed the pair bandwidth and use standard-border center eligibility at `r+h_pair`. Define `g_inhom(r)` as `sum k_h(r-d_ij)/(lambda_i lambda_j) / (2*pi*r*sum_eligible 1/lambda_i)`, which reduces exactly to the homogeneous pair-correlation normalization under constant intensity. Fit the intensity pilot once per execution, persist the identical event/fixed-grid artifact emitted by inhomogeneous K/L for the same base configuration, and freeze it across the same conditioned gridded null and one two-sided ERL family.
- Consequences: intensity bandwidth and pair-smoothing bandwidth are distinct typed identities and cannot be silently conflated or tuned from the output. The two workflows share only the fitted pilot/null machinery they both consume; cumulative K/L and kernel-smoothed g retain separate result types, formulas, unavailable states, pair work, codecs, and cache identities. No kernel registry, automatic bandwidth selector, new intensity estimator, inhomogeneous multitype expansion, or broader PP-family completion is introduced.

## DEC-0270 — Admit exact aligned binary compartment partitions for interface distance

- Date: 2026-08-27
- Status: accepted for FND-02/GEO-01/GEO-01A/GEO-01C/WS-22
- Decision: add one coordinate-frame-bound binary compartment partition over three existing canonical `ObservationWindow2D` values: the analyzed domain, a declared negative compartment, and a declared positive compartment. Require the two compartment areas to equal the analyzed area within sixteen positive-f64 ULPs and require their canonical boundary segments to tessellate exactly: every analyzed outer segment belongs to exactly one compartment and every non-outer compartment segment has one exact reversed-or-equal mate in the other compartment. Derive the oriented shared-interface index from those matched internal segments. Signed interface distance is positive in the positive compartment, negative in the negative compartment, exact zero on the shared interface, and unavailable outside the analyzed domain. Bind compartment order, frame, all three window identities, interface segments, and a caller boundary-segment ceiling into one digest.
- Consequences: signed tumor/stroma-style interface distance no longer aliases signed distance to the tissue edge, and gaps, overlaps, frame drift, segment-representation drift, ambiguous membership, and unbounded validation fail explicitly. The bounded specialization requires aligned polygon vertices and does not repair, snap, overlay, rasterize, infer, or probabilistically register boundaries. Multiclass partitions, incomplete/residual compartments, uncertain segmentations, contact fractions, patient inference, interchange schemas, and result-format changes remain separate immediate-caller outcomes.

## DEC-0271 — Consume exact compartment geometry through typed cell interface profiles

- Date: 2026-08-27
- Status: accepted for FND-02/FND-04/GEO-01/GEO-01A/GEO-01C/WS-22/WS-23
- Decision: make the immediate caller of DEC-0270 one bounded per-cell interface profile over an existing `DeclaredScalarPatternInput` and its exact `histologic_compartment` `MarkTable` column. Resolve the partition's declared negative and positive IDs against the categorical codebook, require every row label to agree with the polygon membership sign except that either adjacent label may own an exact-zero interface point, and retain stable `CellId`, row, compartment ID, signed micrometre distance, per-compartment count/range/mean-absolute-distance, measurement status, partition digest, table identity, and hard point/query/memory limits. Use the cell as the descriptive row only; do not add a permutation or treat cells as biological replicates.
- Consequences: the geometry owner has a complete typed scientific data flow and catches annotation/geometry drift before reporting infiltration-style distances. This is a descriptive per-specimen profile, not patient inference, boundary uncertainty, contact fraction, multiclass interface, clinical evidence, or result-format 0.3 change. A durable node may reuse the existing project/store/codec owners after the typed behavior is correct; no generic geometry-task or profile registry is introduced.

## DEC-0272 — Define binary compartment contact against the complete compartment boundary

- Date: 2026-08-27
- Status: accepted for GEO-01/GEO-01A/GEO-01B/WS-22
- Decision: derive one contact fraction directly from the exact DEC-0270 tessellation. For each oriented compartment, retain shared internal-interface length, analyzed-tissue outer-boundary length, and their sum as the complete compartment-boundary denominator; define contact fraction as `shared_interface_length / complete_compartment_boundary_length`. Verify the segment-classified shared-plus-outer length against the canonical compartment perimeter within sixteen positive-f64 ULPs. Bind the existing partition digest and role order into a separate strict durable result.
- Consequences: the reported fraction has one explicit geometric denominator and cannot silently substitute cell counts, tissue-window perimeter, convex-hull perimeter, or shared length alone. The two role-specific rows may be numerically equal but retain distinct compartment identities. This does not define phenotype contact, pairwise cell contact, multiclass contact matrices, interface uncertainty, morphology, patient inference, or a generic contact registry.

## DEC-0273 — Define polygon fragmentation from exact component areas

- Date: 2026-08-27
- Status: accepted for FND-02/GEO-01/GEO-01A/GEO-01D/WS-22
- Decision: derive one role-preserving fragmentation result from each exact binary compartment's canonical polygon components. Retain sorted component areas, component and hole counts, total area/perimeter, largest-component area fraction, Shannon entropy of component-area proportions in natural-log units, entropy normalized by `ln(component_count)` with exact zero for a singleton, and perimeter/area in inverse micrometres. Bind the unchanged exact partition identity into a separate durable result. Report cell mixing as unavailable until a typed cell table and physical adjacency scale are both supplied.
- Consequences: fragmentation has an explicit vector-polygon basis and scale/unit contract rather than DBSCAN, raster pixels, inferred repairs, or cell counts. Component-area entropy is a shape-fragmentation measure and is not mislabeled as cell mixing. Cell adjacency mixing, morphology perturbation, multiclass partitions, uncertain segmentations, patient inference, and real-mask validation remain separate callers.

## DEC-0274 — Define cell mixing on one declared physical radius graph

- Date: 2026-08-27
- Status: accepted for FND-02/FND-03/FND-04/GEO-01/GEO-01D/WS-22/WS-23
- Decision: consume the exact typed compartment-interface input through one undirected cell graph whose edge exists exactly when Euclidean distance is at most a caller-supplied positive micrometre radius. Reuse the canonical `SpatialGeometryPlan2D`/`SpatialIndex2D`, charge every directed neighbor visit, and retain total/cross edges, cross-edge fraction, its exact complete-random-label expectation `2*n_negative*n_positive/(n*(n-1))`, observed-minus-expected, binary same/cross edge entropy, and role-specific same/cross neighbor incidences and entropy. Bind typed rows/provenance, partition, radius, formulas, and point/query/pair/memory ceilings into durable identity.
- Consequences: GEO-01D cell mixing now has an explicit graph basis and physical scale and remains distinct from DEC-0273 polygon component-area entropy. The result is descriptive per specimen; it adds no cell-level p-value, treats no edge as a biological replicate, and does not select the radius from the output. Multiscale sensitivity, patient inference, multiclass matrices, uncertainty, and real-mask validation remain separate callers.

## DEC-0275 — Preserve CellViT class probabilities as complete simplex rows

- Date: 2026-08-27
- Status: accepted for FND-04/FND-05/WS-23/WS-24
- Decision: add one contiguous `ProbabilitySimplex` MarkTable column with an ordered unique class codebook, per-cell measurement status/provenance, and complete dense f32 rows. Require at least two classes, finite values in `[0,1]`, exact width on every row, and absolute row-sum error at most `1e-5`; preserve supplied probabilities without renormalizing, thresholding, sampling, or moving coordinates independently. Make its immediate caller a bounded soft class-composition result retaining class means, row and aggregate Shannon entropy in nats, effective class count, maximum row-sum error, exact table identity, and point/class/value/memory ceilings.
- Consequences: CellViT annotation probabilities gain a typed contiguous owner and a complete-row scientific consumer without becoming hard labels or independent scalar marks. The result is descriptive per specimen and does not use cells as patient replicates. Calibration, patient inference, simplex random labeling, neighborhood composition, interchange, multiple simultaneous simplex columns, and a generic mark registry remain separate callers.

## DEC-0276 — Aggregate complete simplex rows on one fixed physical neighborhood graph

- Date: 2026-08-27
- Status: accepted for FND-03/FND-04/NIC-01/NIC-01A/WS-23/WS-30
- Decision: make the immediate spatial caller of DEC-0275 an exact undirected cell graph at one caller-supplied positive micrometre radius over the canonical physical window/index. For every focal cell, retain stable CellId, exact neighbor count, and the arithmetic mean of complete target-neighbor simplex rows; retain a typed unavailable vector for zero-neighbor cells and the directed-incidence-weighted mean class mass across all nonempty neighborhoods. Traverse deterministically by sorted neighbor row, charge every directed pair visit, and bind table/provenance/window/radius and point/class/value/pair/memory ceilings into identity.
- Consequences: soft neighborhood composition preserves probability mass and complete rows without hard labels, independent coordinate-wise permutations, niche clustering, or radius selection from the result. Cells and edges remain descriptive within a specimen, not independent population replicates. Multiscale stability, patient inference, spatial nulls, neighborhood clustering, and real-data promotion remain separate callers.

## DEC-0277 — Reuse one geometry plan across prespecified soft-neighborhood scales

- Date: 2026-08-27
- Status: accepted for FND-03/FND-04/NIC-01/NIC-01A/WS-23/WS-30
- Decision: extend DEC-0276 only through a strictly increasing caller-supplied list of positive micrometre radii. Build the canonical geometry/index once, evaluate each radius with deterministic sorted neighbors and complete target simplex rows, retain every scale's per-cell zero/nonzero state and aggregate class mass, charge total directed visits across scales, and report total-variation distance between adjacent available aggregate class-mass vectors. Bind the exact radius list and point/class/radius/value/pair/memory ceilings into durable identity.
- Consequences: NIC-01A gains a prespecified multiscale result and transparent adjacent-scale sensitivity without choosing a favorable radius or collapsing unavailable scales. This does not add automatic scale selection, niche clustering, patient inference, stability thresholds, or a generic neighborhood registry.

## DEC-0278 — Reference verified cell-embedding artifacts from typed mark rows

- Date: 2026-08-28
- Status: accepted for FND-04/FND-05/MRK-02D/WS-23/WS-24
- Decision: add one `VectorArtifactRef` MarkTable column whose value is the existing compact `CellEmbeddingArtifact`, never a copied vector matrix. Construction consumes the already verified materialized table only to prove artifact/QC agreement and exact ordered CellId identity, then retains the mark ID/label/status, artifact/row-link/expected-cell/provenance identities, logical table digest, shape/QC, row-identity digest, modality, vector unit, and missingness policy. `NotPermitted` requires every vector present; `Allowed` preserves the artifact's explicit missing/extraction/QC states. The existing binary-centroid caller must reject a typed vector reference that differs from the separately supplied verified artifact while preserving the older no-reference path.
- Consequences: one current formal vector statistic gains a canonical row-bound typed mark without duplicating the embedding store, matrix, row link, provenance graph, or physical codecs. This is a bounded CellViT specialization, not a general vector registry, arbitrary external reference, patch-vector claim, new physical format, or result-format change; remaining vector callers and real source-import promotion remain separate work.

## DEC-0279 — Route patient MMD and energy nulls through population independence

- Date: 2026-08-28
- Status: accepted for FND-06/CMP-01B/COH-01/WS-31/WS-34
- Decision: extend the existing `marklab-cohort::InferenceDesign` only with the population-independence null now shared by the patient-level MMD and energy-distance callers. One whole patient group label is the atomic unit, the common unblocked patient set is the exact exchangeability block, the alternative remains one-sided high, and each method retains its existing private seed namespace. The shared schedule must reproduce the current Fisher–Yates label stream bit-for-bit so observed statistics, null replicates, p-values, seeds, work bounds, and public result formats do not change.
- Consequences: the two fingerprint distribution tests stop owning duplicate permutation mechanics while preserving deterministic evidence already sealed in the CRC bundle. This does not add stratified MMD/energy, pairing, repeated/multisite exchangeability, a universal null registry, new multiplicity policy, or a result/schema change.

## DEC-0280 — Preserve ordinal IHC levels without interval arithmetic

- Date: 2026-08-28
- Status: accepted for FND-04/IHC-01/WF-01/WS-12/WS-23
- Decision: add one dense non-nullable ordinal MarkTable specialization with an ordered unique level codebook, measured IHC status/provenance, and exact zero-based row codes. Its immediate bounded composition caller reports counts, proportions, cumulative proportions, lower/upper empirical median levels, Shannon entropy, normalized entropy, and effective level count. It must not report a mean, variance, distance, or linear contrast of ordinal codes. Bind exact levels/rows/provenance/limits into the existing durable project path.
- Consequences: FND-04 gains a scientifically valid ordinal data flow without pretending unequal category spacing is quantitative. This does not add threshold inference, proportional-odds modeling, missing/partial ordering, general categorical interchange, real IHC evidence, or a generic mark registry.

## DEC-0282 — Admit exact patient blocks for fingerprint population tests

- Date: 2026-08-28
- Status: accepted for FND-06/CMP-01/CMP-01B/COH-01/WS-31/WS-34
- Decision: extend the existing MMD and energy-distance patient workflows with an optional exact `patient_id`→block assignment table. Every admitted patient must occur exactly once, every block label must be nonempty/bounded, and at least one block must contain both declared groups; reject missing, duplicate, foreign, singleton-only, and group-confounded block designs. Reuse each method's existing kernel/distance matrix, statistic, private seed namespace, work limits, and `InferenceDesign` Fisher–Yates schedule, moving whole patient labels only within blocks. Preserve the unblocked APIs and their output bytes; blocked CLI inputs use one optional trailing `block` CSV column and report the null family and block count.
- Consequences: patient/site-restricted fingerprint tests become user-visible without changing unblocked results or treating cells/features as exchangeable units. This does not add automatic site selection, paired/repeated fingerprints, covariate residualization, multiplicity expansion, or a general permutation registry.

## DEC-0283 — Admit exact patient blocks for functional curve inference

- Date: 2026-08-28
- Status: accepted for FND-06/COH-01/WS-31/WS-34
- Decision: extend the existing common-axis functional two-sample permutation with the same exact patient-ID→block assignments already admitted by MMD and energy distance. Preserve each complete curve and patient label as the atomic unit, the existing L2 statistic, private seed namespace, work bound, one-sided-high plus-one p-value, and unblocked output bytes. Blocked CLI input uses one optional trailing `block` column and reports the population-independence null and exact block count.
- Consequences: prespecified multiscale patient curves can be compared within site/batch restrictions without splitting axis values or inferring strata. This does not add curve registration, smoothing, axis selection, covariate residualization, paired/repeated curves, multiplicity expansion, or a general randomization registry.

## DEC-0284 — Own repeated-subject residual signs in the inference design

- Date: 2026-08-28
- Status: accepted for FND-06/COH-01/WS-31/WS-34
- Decision: add one explicit subject-residual sign-symmetry null and complete-subject-residual-vector permutation unit to `InferenceDesign`, immediately consumed by the existing repeated-measures Freedman–Lane workflow. Preserve the current reduced/full subject-fixed-effect models, private seed namespace, independent Rademacher sign stream, two-sided absolute studentized statistic, work bound, and plus-one p-value exactly. Persist the typed design in the library result and expose its null family and permutation unit in the existing version-1 CLI design summary.
- Consequences: visits and residual coordinates cannot be mistaken for independently randomized population units, while the established repeated workflow remains numerically unchanged. This does not claim residual exchangeability, add visit alignment or covariates, substitute a paired endpoint test, add cluster bootstrap weights, or create a generic resampling registry.

## DEC-0285 — Own complete paired differences in the inference design

- Date: 2026-08-28
- Status: accepted for FND-06/COH-01/WS-31/WS-34
- Decision: add the master-plan `PairedSignFlip` null and a complete patient-pair-difference permutation unit to `InferenceDesign`, immediately consumed by the existing paired scalar permutation workflow. Preserve exact condition-B-minus-condition-A differences, current validation, private seed namespace, independent Rademacher stream, studentized statistic, less/greater/equal-tail two-sided alternatives, work bounds, and plus-one p-values. Persist the typed design in the library result and expose its null family and permutation unit in the existing version-1 CLI design summary.
- Consequences: condition rows cannot be separated or permuted as independent patients, and existing paired numerical evidence remains exact. This does not add incomplete-pair imputation, repeated visits, covariates, cluster weights, pair matching, or a general sign-flip registry.

## DEC-0286 — Estimate multisite contrasts from independent patient rows

- Date: 2026-08-28
- Status: accepted for COH-01/FND-06/WS-31/WS-34
- Decision: add one patient-level multisite two-group workflow that requires globally unique exact patient IDs, exact site/group labels, finite scalar endpoints, and at least two patients in each declared group at every site. Compute each site's group-A-minus-group-B mean difference and Welch independent-groups standard error, then pass those typed site summaries unchanged to the existing fixed-effect or REML pooling and leave-one-site-out owner. Bound rows/sites with existing cohort limits and reject site/group confounding; expose one version-1 CLI result with per-site contrasts and nested pooled output.
- Consequences: multisite inference becomes executable from patient endpoints without asking users to precompute standard errors or treating cells/specimens as independent units. This does not pool raw features across sites, residualize covariates, infer site labels, add patient overlap, claim exchangeability, or introduce a meta-analysis framework beyond the maintained existing models.

## DEC-0287 — Own patient-then-specimen bootstrap draws in the inference design

- Date: 2026-08-28
- Status: accepted for COH-01/FND-06/WS-31/WS-34
- Decision: add the master-plan `HierarchicalBootstrap` null and `PatientThenNestedSpecimen` unit to `InferenceDesign`, immediately consumed by both existing hierarchical-bootstrap and bootstrap-equivalence workflows. Bind exact canonical patient blocks and specimen counts; sample patient occurrences first and then the selected patient's specimen indices using the unchanged private namespace, SplitMix draw order, replicate count, and patient-first algorithm. Persist the typed design in the library result and expose its null family/unit in both existing version-1 CLI design summaries.
- Consequences: nested draws become a typed, inspectable schedule and cannot be replaced by flat specimen resampling, while all established replicate means and percentile intervals remain exact. This does not change the specimen-row-mean estimand, add cluster weights or covariates, infer hierarchy, or create a generic bootstrap framework.

## DEC-0288 — Permute equal-weight endpoint summaries as whole clusters

- Date: 2026-08-28
- Status: accepted for COH-01/FND-06/WS-31/WS-34
- Decision: add one cluster-randomized scalar workflow over globally unique patient rows nested in exact cluster IDs. Require one consistent declared group per cluster and at least two clusters per group; reduce each cluster to its equal-weight finite patient-endpoint mean, then compute the group-A-minus-group-B Welch contrast and permute whole cluster labels through an explicit `ClusterLabelPermutation`/`CompleteClusterEndpoint` inference design with a private seed namespace and existing permutation work ceiling. Expose one version-1 CLI result with patient/cluster counts, design, effect, statistic, alternative, and exact replicate accounting.
- Consequences: patients within an assigned cluster cannot masquerade as independently randomized units, and unequal cluster sizes do not silently patient-weight the cluster-level estimand. This does not infer clusters, model intracluster correlation, adjust covariates, handle cluster crossover/repeated time, add small-sample corrections, or create a generic cluster framework.

## DEC-0289 — Expose the existing randomized-interference design explicitly

- Date: 2026-08-28
- Status: accepted for FND-06/COH-01/WS-31/WS-82
- Decision: extend the existing version-1 randomized binary interference result with the exact design facts already enforced by production: clustered-unit analysis level, randomized-interference fixed-outcome null family, complete cluster assignment state as the atomic randomization unit, and exact unit/cluster counts. Preserve assignment enumeration, within-cluster treated counts, graph/exposure mapping, HT/Hájek estimands, positivity rules, test contrast, ChaCha20 namespace/stream, work limits, p-value, and all prior fields exactly.
- Consequences: the interference workflow now satisfies the explicit FND-06 design-summary requirement without depending on the cohort crate or inventing another scheduler. This does not claim interference identification from observational data, add graph uncertainty, change the exposure mapping, calibrate type-I error, or generalize a randomization registry.

## DEC-0290 — Restrict single-step Max-T within exact patient blocks

- Date: 2026-08-28
- Status: accepted for FND-06/COH-01/CMP-01B/WS-31/WS-34
- Decision: extend the existing complete-endpoint-family single-step Max-T workflow with the exact patient-ID→block assignments already used by MMD, energy, and functional inference. Move one whole patient label and complete endpoint vector within each block; retain the current two-sided absolute Welch maximum, private seed namespace, endpoint family, critical-value rule, inclusive-plus-one adjusted p-values, alpha, and work ceiling. Persist the typed population-independence/two-sided design in the library result. Accept one optional trailing CLI `block` column and report blocked design fields only when supplied, preserving legacy unblocked JSON bytes.
- Consequences: family-wise endpoint inference can respect prespecified site/batch restrictions without splitting endpoint vectors or silently using unblocked labels. This does not select endpoints, infer blocks, residualize covariates, add step-down procedures, broaden multiplicity families, or create a generic test registry.

## DEC-0291 — Add step-down Max-T to the existing patient endpoint family

- Date: 2026-08-28
- Status: accepted for INF-01C/FND-06/COH-01/CMP-01B/WS-31/WS-34
- Decision: add an explicit step-down option to the existing complete-endpoint Max-T workflow, for both unrestricted and exact-block population-independence designs. Order hypotheses by decreasing absolute observed Welch statistic, use each existing whole-patient permutation once, compare each hypothesis with the maximum over its remaining less-extreme set, treat exact observed-statistic ties as one step, and enforce monotone inclusive-plus-one adjusted p-values. Preserve the endpoint order, seed namespace, initial complete-family critical value, alpha, work ceiling, block compiler, and legacy single-step output bytes. Stream one null endpoint vector at a time rather than retaining the full permutation-by-endpoint matrix.
- Consequences: callers can gain the standard step-down power improvement without changing the scientific endpoint family or randomization unit. This does not select endpoints, infer blocks, add hierarchical families, perform covariate adjustment, provide local-map release policy, or create a multiplicity registry.

## DEC-0292 — Sign complete paired endpoint vectors for Max-T

- Date: 2026-08-28
- Status: accepted for INF-01C/FND-06/COH-01/CMP-01B/WS-31/WS-34
- Decision: add `paired_max_t_permutation` and `marklab cohort paired-max-t` for one exact complete endpoint vector under each of two declared conditions per independent patient pair. Construct condition-B-minus-condition-A vectors, draw one deterministic sign per pair and replicate through a distinct `CompletePatientPairDifferenceVector` unit, apply that sign to every endpoint together, and control the prespecified complete family with the existing two-sided single-step or step-down Max-T mechanics. Persist the exact condition labels, complete-family multiplicity identity, correction, alpha, initial critical value, private namespace, seed, counts, and 100-million pair-by-endpoint-by-replicate ceiling in a strict version-one result.
- Consequences: paired multivariate endpoints can receive family-wise inference without splitting endpoints or falling back to unpaired labels. This does not accept incomplete pairs/families, impute missing endpoints, select endpoints, add one-sided family tests, infer pairing, adjust covariates, or create a multiplicity registry.

## DEC-0293 — Permute whole patient residuals around one fixed nuisance covariate

- Date: 2026-08-28
- Status: accepted for FND-06/COH-01/WS-31/WS-34
- Decision: add `patient_covariate_freedman_lane` and `marklab cohort covariate-permutation` for one scalar outcome, two declared groups, and one prespecified finite nuisance covariate per independent patient. Fit the reduced Gaussian OLS model `outcome ~ intercept + covariate`, permute each complete reduced-model patient residual through a private deterministic namespace, reconstruct outcomes around fixed reduced fits, and test the group-A indicator in `outcome ~ intercept + covariate + group`. Deterministically center and max-absolute-deviation scale the nuisance covariate, persist that transform, exact model dimensions, residual degrees of freedom, design, alternative, seed, and replicate counts, and retain the existing 100-million patient-by-replicate ceiling.
- Consequences: one concrete covariate-adjusted patient comparison becomes executable without treating observations below patient as exchangeable or creating a formula language. Validity still requires reduced-model residual exchangeability conditional on the fixed covariate. This does not infer/select covariates, support multiple nuisance columns, blocks, weights, robust/clustered errors, nonlinear outcomes, cross-fitting, causal adjustment, or automatic fallback to label permutation.

## DEC-0294 — Restrict one-covariate patient residuals within exact blocks

- Date: 2026-08-28
- Status: accepted for FND-06/COH-01/WS-31/WS-34
- Decision: extend the one-covariate Freedman-Lane workflow with exact `patient_id`→block assignments through the existing `PatientExchangeabilityBlock` owner. Canonically align assignments to sorted patient rows, require one complete valid assignment per patient and at least one exchangeable block, and permute each whole reduced-model residual only within its block using the unchanged covariate-method namespace, OLS models, alternative, statistic, work ceiling, and plus-one rule. Accept one optional trailing CLI `block` column and emit blocked fields only on that path, preserving the unblocked version-one JSON shape.
- Consequences: prespecified site/batch restrictions can condition residual exchangeability without moving patients across blocks. This does not infer blocks, require group mixing within every block, support multiple covariates, fit block effects, add cluster-robust errors, or turn the patient-block alignment helper into a general registry.

## DEC-0295 — Admit a fixed named nuisance-covariate matrix without formulas

- Date: 2026-08-28
- Status: accepted for FND-06/COH-01/WS-31/WS-34
- Decision: add `patient_covariate_matrix_freedman_lane`, its exact-block counterpart, and `marklab cohort covariate-matrix-permutation` for 1–32 exact ordered finite nuisance columns per independent patient. Fit a fixed reduced OLS model containing intercept plus every declared nuisance column and a full model adding only the declared group-A indicator. Center and max-absolute-deviation scale every column independently, persist ordered names/transforms/model dimensions, and permute complete reduced-model patient residuals globally or within exact blocks through a distinct private namespace. Bound each run by 100 million declared OLS work units including patient rows, full-model squared/cubic work, and every requested fit.
- Consequences: multiple prespecified technical/clinical covariates become executable without a formula parser, column selection, or hidden preprocessing. This does not support interactions, splines, categorical expansion, missing-data imputation, weights, robust/clustered errors, cross-fitting, causal interpretation, or automatic model selection.

## DEC-0296 — Pool adjusted patient effects only after within-site OLS

- Date: 2026-08-28
- Status: accepted for FND-06/COH-01/WS-31/WS-34
- Decision: add `multisite_covariate_patient_contrast` and `marklab cohort multisite-covariate-contrast` for one exact 1–32-column nuisance matrix shared by every patient and site. Within each canonical site, center/scale nuisance columns using only that site's patients, fit `endpoint ~ intercept + nuisance matrix + group-A indicator`, and retain the adjusted group coefficient, OLS standard error, residual degrees of freedom, patient counts, and transforms. Feed only those site effects and patient counts into the unchanged fixed-effect/REML pooling, heterogeneity, confidence/prediction interval, and leave-one-site-out owner. Bound total site OLS row/squared/cubic work at 100 million units.
- Consequences: adjusted multisite inference is executable without pooling raw patients across sites or treating site as a nuisance dummy. Every site must identify the same prespecified design. This does not add site-by-covariate interactions, random slopes, cluster-robust errors, cross-site normalization, missing-data handling, causal adjustment, or replace the existing pooling model.

## DEC-0297 — Residualize only after equal-weight cluster reduction

- Date: 2026-08-28
- Status: accepted for FND-06/COH-01/WS-31/WS-34
- Decision: add `cluster_covariate_matrix_freedman_lane` and `marklab cohort cluster-covariate-permutation` for globally unique patients nested in exact consistently labeled clusters with 1–32 fixed nuisance columns. Reduce each cluster to equal-weight patient outcome and nuisance means; fit the reduced cluster-level OLS model containing intercept plus every centered/scaled nuisance column; permute each complete cluster residual through a distinct private namespace; reconstruct around fixed reduced fits; and test the group-A indicator in the full cluster-level model. Persist exact patient/cluster/group counts, nuisance identities/transforms, model dimensions, residual degrees of freedom, alternative, seed, and replicate counts under a 100-million OLS-work ceiling.
- Consequences: covariate adjustment cannot silently restore patient-level pseudoreplication or patient-weight unequal clusters. Validity requires reduced-model residual exchangeability across independent clusters conditional on the fixed nuisance matrix. This does not add a formula language, interactions, weights, cluster-robust errors, small-sample corrections, observational causal interpretation, automatic covariate selection, or real-effect evidence without an admitted cluster-randomized dataset.

## DEC-0298 — Open endpoint families only through serial complete-family rejection

- Date: 2026-08-28
- Status: accepted for INF-01C/FND-06/COH-01/WS-31/WS-34
- Decision: add `hierarchical_gatekeeping_max_t` and `marklab cohort hierarchical-max-t` for an exact ordered partition of one complete patient endpoint vector into at least two named families. Reuse one whole-patient population-independence label schedule under a distinct private namespace. Apply the declared single-step or step-down Max-T correction locally within every family at the same family-wise alpha; open the first family; open a successor only if every endpoint in its currently opened predecessor is rejected; and force every endpoint decision in a closed family to false while retaining its local diagnostic. Persist the exact opening rule, order, family membership, local critical values and p-values, decisions, patient/group counts, alpha, seed, and replicate counts under the existing 100-million complete-vector work ceiling.
- Consequences: one concrete strong-FWER serial gatekeeper becomes executable without an endpoint registry or post-hoc family discovery. Family order and membership must be fixed before outcome inspection. This does not add alpha recycling, fallback testing, DAG/graphical procedures, selective inference, automatic family construction, blocked/paired gatekeeping, or a real CRC claim without a prospectively prespecified hierarchy.

## DEC-0299 — Retain imported categorical codebooks for multiclass radius mixing

- Date: 2026-08-28
- Status: accepted for FND-03/FND-04/GEO-01D/WF-01/WS-23/WS-25
- Decision: add `categorical_neighborhood_mixing` and its durable node for a complete categorical MarkTable column with at least three declared classes. On one exact physical-radius graph, persist the symmetric directed class-pair count matrix, observed fractions, fixed-count without-replacement random-label expectations and excesses, cross-class edge summary, zero-neighbor states, per-source-class neighbor distributions/entropy, exact identities, and hard point/class/pair/memory bounds. Extend the compatibility `Pattern` with a serde-defaulted ordered categorical-codebook map produced by both CSV and Parquet loaders; require any retained `histologic_compartment` codebook to agree exactly with its MarkTable declaration. Use the existing exact-f64 codec for durable output.
- Consequences: hard multiclass CellViT annotations can cross the existing physical-format boundary without losing code-to-label meaning, and their descriptive mixing no longer collapses to a binary partition. The new Pattern field is backwards compatible for older serialized inputs and does not change result-format 0.3. This does not create a generic mark registry, infer missing class probabilities, treat cells/edges as cohort replicates, add polygonal multiclass partitions, or claim a durable real result when the derived source lacks stable cell IDs.

## DEC-0300 — Retain exact source cell identity through compatibility Pattern import

- Date: 2026-08-28
- Status: accepted for FND-04/GEO-01D/WF-01/WS-12/WS-23/WS-25
- Decision: add one optional serde-defaulted, row-aligned `cell_ids` field to `Pattern` plus `Pattern::typed_cell_ids`. CSV and Parquet loaders accept the same optional `cell_id` column, require it on every retained row or none, validate every value through the existing bounded `CellId` constructor, and require strict increasing order. The CPTAC CellViT adapter emits `<exact slide_id>:<zero-padded source row>` for both coordinate `cell_id` and vector `object_id`. Its high-confidence binary is thresholded from the exact serialized value that Rust imports as `f32`, not the pre-serialization Python float.
- Consequences: the existing MarkTable, hierarchy, scheduler, durable project, ledger, and artifact store can execute the admitted real multiclass workflow without fabricated row identity or a second identity registry. Older Pattern payloads remain valid, result-format 0.3 is unchanged, and full class-probability simplexes are still not inferred from winning-class confidence. This does not add generic source adapters, direct MarkTable export, arbitrary identity synthesis, or a plugin surface.

## DEC-0301 — Execute the first bounded DAG as fixed typed durable node shards

- Date: 2026-08-28
- Status: accepted for PLAT-01/WF-01/WS-12
- Decision: add one concrete `execute_marked_prepost_dag` path over the existing two independent `MarkedAnalysisNode` roots and typed `MarkedPrePostNode` dependency. Plan only explicit positive per-root thread counts, declared per-root memory, total Pattern rows, and host-wide node/thread/memory ceilings. Run the two roots concurrently only when both fit; otherwise use deterministic pre-then-post waves. Persist each fixed node under its own child `DurableProject`, expose a roots-only target for bounded interruption/resume, reconstruct exact root successes in memory, and let the existing scheduler verify dependency edges before the comparison. Hardcode only the existing result-format 0.3 marked and pre/post schemas.
- Consequences: a current scientific workflow gains actual parallel ready-node execution, process-boundary resume, pending-intent/ledger/head recovery, cache replay, and exact direct-path parity without weakening `execute_algorithm` or introducing an untyped task runner. The layout reuses the existing artifact store and ledger implementation once per fixed node; it does not add a second persistence format, arbitrary node factories, YAML/JSON task execution, remote workers, or a plugin framework. Broader heterogeneous DAG construction remains active and must be justified by another immediate caller.

## DEC-0302 — Model complete patient CellViT compositions without binary collapse

- Date: 2026-08-28
- Status: accepted for BAY-01/BAY-02/BACK-01/WF-01/WS-13/WS-40/WS-41
- Decision: add `marklab bayes dirichlet-multinomial-group` and its durable `marklab project` path for one complete ordered hard-class count vector per independent patient in two declared molecular groups. Use a reference-class-zero softmax with normal baseline logits and group log-ratio effects, one positive shared Dirichlet concentration, and a patient-level collapsed Dirichlet-multinomial likelihood in the existing pinned PyMC 6.3.0/Python 3.12 environment. Retain zero class counts, reject incomplete patient vectors, require four patients per group, and bound patients, classes, total cells, NUTS iterations, output bytes, and process time. Publish exact backend/lock/worker/request/input identities, class-wise posterior composition and differences, patient-level posterior predictive summaries, standard diagnostics, and one version-1 typed result; include the same identities in the existing static durable descriptor and cache key.
- Consequences: admitted CellViT molecular-group analysis retains all five hard annotation classes and the patient as population unit instead of reducing the data to Neoplastic versus all other cells. Result-format 0.3 and existing binary beta-binomial paths are unchanged. This does not treat cells as independent replicates, recover absent class-probability simplexes, model spatial arrangement, add covariates or patient/cohort random effects, claim causality or clinical validity, introduce a generic likelihood registry, or replace agreement/sensitivity/calibration work for broader Bayesian promotion.

## DEC-0303 — Promote the patient multiclass likelihood with the existing Bayesian checks

- Date: 2026-08-28
- Status: accepted for BAY-01/BAY-02/WS-40/WS-44
- Decision: add three concrete CLI callers around the DEC-0302 model rather than a calibration framework. `dirichlet-multinomial-group-agreement` fits the identical typed request independently in pinned PyMC and NumPyro and compares every class's reference/comparison probability and group difference plus concentration under explicit Monte Carlo/absolute tolerances. `dirichlet-multinomial-group-sensitivity` reruns the PyMC fit over the fixed seven-scenario one-at-a-time half/base/double grid for baseline-logit, group-effect, and concentration prior scales and reports posterior-SD-standardized shifts. `dirichlet-multinomial-group-sbc` uses 20–100 bounded prior-generative NumPyro replicates and checks ranks and 90% coverage for every free baseline logit, every group log-ratio effect, concentration, and every derived class-probability difference, with explicit failure dispositions and cell/iteration/output/time ceilings.
- Consequences: the multiclass likelihood receives the same independent-backend, sensitivity, and calibration evidence required of the repository's promoted Bayesian families without inventing a model registry or claiming one real fit proves calibration. Result-format 0.3 is unchanged. The real CPTAC result is backend-consistent but materially sensitive to the prespecified half-scale baseline-logit prior, which remains a reported limitation; this does not add covariates, spatial effects, patient/cohort hierarchy, clinical validation, CmdStan, GPU evidence, or exhaustive field diagnostics.

## DEC-0304 — Scale one exact-radius CellViT graph signal without dense spectra

- Date: 2026-08-28
- Status: accepted for GSP-01/FR-01B/WF-01/WS-12/WS-61/WS-62
- Decision: add `graph_sparse_radius_heat_workflow`, `marklab graph sparse-radius-heat`, and its durable `marklab project` path for one concrete large CellViT graph-signal caller. Canonically sort exact node IDs; construct every binary undirected edge within one physical radius through deterministic uniform-cell neighbor search; preserve isolated nodes; and apply the combinatorial-Laplacian heat kernel with an adaptive Chebyshev recurrence over the conservative Gershgorin interval `[0,2*d_max]`. Persist graph/radius identity, selected coefficients/order, tail and dense-grid scalar approximation diagnostics, filtered signal, isolated count, candidate/edge/matrix-vector work, conservative working bytes, and explicit caller ceilings. Reuse the existing durable project, scheduler, ledger, store, runtime provenance, and exact-f64 result codec; expose the codec module only as a hidden public bridge required by this binary caller.
- Consequences: graph heat moves from the existing 128-node dense eigendecomposition ceiling to a real 2,000-cell pathology workload while retaining exact small-graph differential evidence and deterministic durable replay. This does not claim an exact eigenspectrum, a formal continuous spectral error proof beyond the recorded tail/grid diagnostic, GPU acceleration, graph-perturbation calibration, population inference from one slide, a generic sparse matrix framework, or topology scaling; arbitrary-window point processes remain open.
