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
