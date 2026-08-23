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
