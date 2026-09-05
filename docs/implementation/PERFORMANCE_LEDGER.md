# Performance ledger

Implementation base: `55fce12f10684a9081ca1f744f87d6f5feedcb24`

## Environment

- Date: 2026-08-22
- OS/hardware: macOS 26.5.2 arm64; Apple M4 Pro; 48 GiB memory; 12 logical CPUs
- Rust: `rustc 1.96.0 (ac68faa20 2026-05-25)`
- Cargo: `cargo 1.96.0 (30a34c682 2026-05-25)`
- LLVM: 22.1.2
- Benchmark command features: all features; smoke profile; Criterion `--quick`; benchmark-specific thread policies unchanged

## Baseline workloads

| Workload | Command | Shape/output | Time | Memory/allocations | Status |
|---|---|---|---|---|---|
| Multiscale residual | `env MARKLAB_BENCH_PROFILE=smoke cargo +1.96.0 bench --locked --all-features -- --quick` | Existing smoke fixture/equivalent output | 45.703–45.746 ms | Criterion smoke only | pass |
| CSV complete load | same suite | Existing smoke fixture | 9.0462–9.2575 ms | Criterion smoke only | pass |
| CSV decode/filter | same suite | Existing smoke fixture | 2.7986–2.8557 ms | Criterion smoke only | pass |
| Indexed nearest neighbor | same suite | Existing smoke fixture | 6.2113–6.2404 ms | Criterion smoke only | pass |
| Periodogram | same suite | Existing smoke fixture | 11.657–11.775 ms | Criterion smoke only | pass |
| Permutation engine | same suite | Existing smoke fixture | 15.140–15.168 ms | Criterion smoke only | pass |
| ERL envelope | same suite | Existing smoke fixture | 15.300–15.313 ms | Criterion smoke only | pass |
| Structure factor | same suite | Existing smoke fixture | 56.196–56.479 ms | Criterion smoke only | pass |

## Classical K/L checkpoint — 2026-08-24

- No timing benchmark was run or added. WS-30-KL-01 establishes a correct exact baseline and makes no optimization or throughput claim; the active contract did not require specialized performance evidence.
- The implementation retains one point R-tree, the window's one boundary R-tree, one boundary-distance vector, radius-difference counters, one CSR coordinate set at a time, and the bounded simulation-by-radius L matrix. It retains no all-pairs matrix.
- Exact point, radius, topology-candidate, pair-visit, CSR-draw, scheduler-output, and conservative retained/working-byte limits are exercised by focused one-short tests. Scale benchmarks remain required before stable PP-01 promotion and are still represented in `PROGRAM_TRACKER.md`.
| DHAT heap regression | `cargo +1.96.0 test --locked --no-default-features --features dhat-heap --lib dhat_ -- --test-threads=1` | Three existing allocation regressions; 180 other tests filtered | 11.944 s wall including build | Test assertions passed; 14 narrow-feature compile warnings | pass |

Criterion release compilation took 2m42s and the full command about 2m44s. Gnuplot was unavailable, so Criterion used Plotters; numeric timing output completed. These results reproduce the smoke workload only and do not replace scheduled full or million-row measurements.

No performance claim is approved until workload equivalence, output size, hardware, compiler, features, threads, and exact command are recorded.

## WS-B exit reproduction

Command: `env MARKLAB_BENCH_PROFILE=smoke cargo +1.96.0 bench --locked --workspace --all-features -- --quick`

Environment and workload definitions are unchanged from the baseline above. The final run at `ac7da28da082f795381da0a03e8437fc4a774262` rebuilt the release profile in 2m00s. Gnuplot remained unavailable, so Criterion used Plotters.

| Workload | Final quick interval | Criterion comparison to prior local sample | Status |
|---|---:|---|---|
| Multiscale residual grid64 | 46.195–46.471 ms | no detected change, p = 0.08 | pass |
| CSV complete load, 10,000 cells | 9.0747–9.0989 ms | no detected change, p = 0.21 | pass |
| CSV decode/filter, 10,000 cells | 2.8807–2.9672 ms | no detected change, p = 0.08 | pass |
| Indexed nearest neighbor, 10,000 cells | 6.1204–6.1721 ms | no detected change, p = 0.50 | pass |
| Periodogram grid64 | 11.877–11.896 ms | no detected change, p = 0.68 | pass |
| Permutations n500/k16/B7 | 15.179–15.486 ms | no detected change, p = 0.56 | pass |
| ERL B7 | 15.256–15.304 ms | no detected change, p = 0.15 | pass |
| Structure factor n1000/k32 | 56.784–57.180 ms | no detected change, p = 0.86 | pass |

The first workspace attempt measured these same Criterion targets but was not a passing gate: Cargo subsequently forwarded `--quick` to the new child crates' implicit libtest benchmark harnesses, which rejected the flag. The red manifest regression and `bench = false` fix in `ac7da28` restrict the command to real Criterion targets; the table records the final passing rerun. These quick intervals are noisy smoke measurements, not an optimization claim.

The phase-boundary DHAT command passed 3/3 assertions (180 filtered) after a 7.12 s test-profile build, with the same 14 narrow-feature warnings. The WSI-enabled release synthetic smoke rebuilt in 1m07s and completed 12/12 scenarios and 120/120 replicates with zero failed replicates; it is validation smoke rather than a timing benchmark.

## C-01 hierarchy construction

Commands:

```text
env MARKLAB_BENCH_PROFILE=smoke cargo +1.96.0 bench --locked --workspace --all-features --bench cohort_hierarchy -- --quick
env MARKLAB_BENCH_PROFILE=full cargo +1.96.0 bench --locked --workspace --all-features --bench cohort_hierarchy -- --quick
```

The fixture is built outside the timed closure. Each Criterion iteration clones the declarative input using `BatchSize::LargeInput`, constructs and validates a new hierarchy, and asserts exact patient/specimen/block/slide/cell and pair/repeated counts. `sample_size(10)` and cell-count throughput are declared. The smoke shape is 100 patients/specimens × 100 cells = 10,000 cells; the permitted full fallback is 10,000 patients/specimens × 100 cells = 1,000,000 cells. Both include one block and slide per specimen. This measures hierarchy construction/validation, not embedding storage or a 10-million-cell claim.

| Shape | Final quick interval | Throughput | Status |
|---|---:|---:|---|
| 10,000 cells / 100 specimens | 1.1645–1.1726 ms | 8.5282–8.5873 million cells/s | pass |
| 1,000,000 cells / 10,000 specimens | 147.13–147.27 ms | 6.7900–6.7969 million cells/s | pass |

Environment matches the ledger header. Gnuplot remained unavailable and Criterion used Plotters. The implementation uses shared ID storage, indexed vectors/maps, iterative parent traversal, and a role-matrix-bounded specimen → patient biological lineage. The benchmark verifies equivalent output but does not measure peak memory, serialization, persistence, remote I/O, or inferential workloads; no broader performance claim is made.

## C-04 cell-embedding storage workload

Date: 2026-08-23. The OS, Apple M4 Pro host, 48-GiB memory, 12 logical CPUs, Rust 1.96.0, Cargo 1.96.0, and LLVM 22.1.2 match the ledger environment above. The benchmark uses root features `csv,parquet`, Criterion 0.7 with 10 flat samples, a one-second warm-up, a ten-second requested measurement window, and one calling thread. Criterion extended collection to one iteration per sample because both workloads exceed the requested window. Gnuplot was unavailable, so Criterion used Plotters.

Commands:

```text
cargo +1.96.0 test --locked --features csv,parquet,dhat-heap --test cellvit_embedding_heap -- --exact embedding_10k_1280_peak
cargo +1.96.0 bench --locked --bench cell_embeddings --features csv,parquet -- '10k_x_1280' --noplot
cargo +1.96.0 bench --locked --bench cell_embeddings --features csv,parquet --no-run
/usr/bin/time -l cargo +1.96.0 bench --locked --bench cell_embeddings --features csv,parquet -- '1m_x_256' --noplot
```

The synthetic fixture, arithmetic order, random-access sequence, digest framing, and RSS ownership are frozen by DEC-0030/DEC-0031. Both profiles check row count, dimension, canonical logical digest, and numeric digest on every iteration. The 10,000 × 1,280 profile pins logical/numeric digests `df74ee3588f3c5dbf4fa81ffc285dd9f84daf8bb1101e7294fba6536785931de` / `0396c2d78ff98c7307e7dcf383d8579cf1a06c2501f9fc67c91a285308de08b4`; the 1,000,000 × 256 profile pins `d5dc753238330e9be60fdee94c6c24d7151f26660527689b15e8895e983c5633` / `133dc720d9775d8d2a3c4140a36529a750ab844acd6970eb6ca2ff86d8e7d49a`.

| Profile | Checked work | Final interval | Throughput | Memory | Status |
|---|---|---:|---:|---:|---|
| 10,000 × 1,280 | Source import/finalization, sequential table/QC workload, prepublished Arrow/Parquet scans and materialized round trips | 4.1653–4.1803 s | 3.0619–3.0730 million values/s | Criterion timing; DHAT below | pass |
| 1,000,000 × 256 | Sequential QC/means/sum-of-squares/norm, 4,096 random row accesses, 16 × 16 population covariance, 64 × 64 linear kernel | 10.446–10.488 s | 24.408–24.506 million values/s | 1,117,552,640-byte maximum RSS | pass |

The timed full command completed in 126.19 s wall time and reported no compilation after the separate locked `--no-run` build. Its maximum RSS is 41.6% of the frozen 2.5-GiB host threshold. An earlier cold invocation that compiled and linked thin-LTO code inside `/usr/bin/time` reached 4,904,665,088 bytes; the immediately repeated current-binary calibration reached 1,098,268,672 bytes. DEC-0031 treats the former as disclosed build-resource usage and requires the current-binary timed command for embedding-runtime RSS.

The exact DHAT gate completed in 137.51 s. A diagnostic run of the same test reported zero current tracked bytes, a 324,194,945-byte peak, and a 603,979,776-byte cap. Unlike Criterion, its measured path imports the source table, publishes fresh Arrow and Parquet objects into a disk-backed temporary store, validates both scans and materialized reads, then drops every operation-owned value before sampling heap statistics.

These are storage/import/integrity workload checks, not embedding inference, biological analysis, a scientific estimand, or an optimization claim. The full profile does not exercise source-bundle import or physical columnar round trips; the smoke and authorized-corpus reconciliation own those complementary dimensions. Ten-million-row out-of-core behavior remains planned and unmeasured. C-04 remains open until the authorized 32-bundle Rust reconciliation and phase-boundary gates pass.

## C-04 closure disposition

The final workspace smoke at implementation SHA `55d1c8b8a07b2dbf40f77b5590127456f9042295` executed eight Criterion targets and ten intervals. The embedding interval was 4.1952–4.2284 s at 3.0271–3.0511 million values/s; every existing target reported no detected change. The final embedding DHAT phase rerun passed in 138.51 s, and the prior diagnostic values (current 0, peak 324,194,945, cap 603,979,776 bytes) remain the allocation evidence.

The authorized 32-bundle reconciliation and every canonical phase-boundary gate subsequently passed or, for exact registry packaging, reproduced the explicit DEC-0018 non-green blocker. C-04 is therefore closed as storage/import infrastructure. The preceding “remains open” sentence is retained as historical checkpoint state; it is superseded by this disposition. No optimization, 10-million-row, mmap/zero-copy, real-corpus promotion, inference, or biological-performance claim is added.

## C-05 logical cell-link work boundary

Date: 2026-08-23. No timing, throughput, heap, or RSS benchmark is claimed at this intermediate checkpoint. The contained-shared implementation uses two identical sorted four-bucket passes with runtime `O(p log p + c log p + k + e log e)`, retained output `O(c + e)`, and `O(p)` index scratch. Dense bucket occupancy can make candidate count `k` approach `c * p` even with zero edges; DEC-0035 therefore requires an explicit per-pass candidate-check limit enforced before comparison. The focused pathological fixture accepts exactly 12 candidate checks with zero edges and rejects an 11-check budget. This is resource-safety evidence, not performance evidence. The frozen 10,000-patch/100,000-link smoke and 100,000-patch/1,000,000-link closure workloads remain required before any optimization or throughput claim.

## C-05 logical patch-region work boundary

Date: 2026-08-23. No timing, throughput, heap, or RSS benchmark is claimed at this intermediate checkpoint. For `z` stored nonzero declarations over `p` expected patches and `r` expected regions, validation is `O(z * (log p + log r))`, digest/copy work is `O(z)`, and retained output is `O(z)` plus typed-ID text. Assessment construction charges the caller input capacity/text plus the exact-capacity retained clone; link construction charges the live assessment plus the new retained link before allocation. The 400,000,000-row and checked `u64` product boundaries are tested without giant allocation. Frozen link-scale Criterion and DHAT evidence remains required before C-05 closure.

## C-05 strict source-record work boundary

Date: 2026-08-23. No timing, throughput, heap, or RSS benchmark is claimed at this intermediate checkpoint. For `n` source rows, validation is `O(n log n)` because source-key, source-row, expected-patch, and non-null vector-row uniqueness/order proofs use bounded sorting; canonical parsing is `O(encoded bytes + n log n)` across a structural/count pass, exact-capacity typed collection, and streaming fixed-point comparison. Retained storage is `O(n + source-key text)`; the identity map additionally retains one `u64` source-row index per expected patch so source-row links validate in `O(n)`. Constructors and decoders charge caller capacity, typed rows/text, exact retained output, encoded/decoded ceilings, and allocation failure. The 100,000,000-row and 1-GiB encoded limits are boundary-tested without giant allocation. Frozen source-profile and scale evidence remains required before C-05 closure.

## C-05 small canonical-record work boundary

Date: 2026-08-23. No timing, throughput, heap, or RSS benchmark is claimed at this intermediate checkpoint. Derivation, support, producer, and assessment descriptors have fixed field counts and validate in `O(encoded bytes)` with constant-sized stack state; dependency sorting is over at most five IDs. Retained storage is constant except for one owning-slide ID and bounded algorithm/version tokens. Borrowed decoding performs structural and decoded/retained preflight before allocating those strings, and every surface independently clamps encoded input to 256 KiB. This is resource-safety evidence, not a throughput claim; artifact-graph and physical-profile scale evidence remains required before C-05 closure.

## C-05 multiscale provenance-record work boundary

Date: 2026-08-23. No timing, throughput, heap, or RSS benchmark is claimed at this intermediate checkpoint. Every provenance variant has a fixed field count, validates in `O(encoded bytes)`, and sorts at most fourteen inline artifact IDs. Retained storage is constant except for one bounded owning-slide ID, direct pooling/model/license/citation/extraction text, and converter identity tokens. Before owned Serde parsing, allocation-free preflight clamps the document to 256 KiB and charges inline parsed storage, one input-sized aggregate bound for decoded strings, and `max(8, 2 * raw escaped-string bytes)` for the persistent reusable JSON scratch `Vec`; exact/one-short tests include a minimally escaped 4,096-byte citation and escaped typed slide ID. This is resource-safety evidence, not a throughput or real-source claim; artifact graph, physical profile, and frozen scale workloads remain required before C-05 closure.

## C-05 direct-patch structural-graph work boundary

Date: 2026-08-23. No timing, throughput, heap, or RSS benchmark is claimed at this intermediate checkpoint. Structural validation performs fixed-count record/profile and cross-binding work plus `O(total owned canonical JSON bytes)` serialization/comparison. Canonical comparison writes directly against the supplied store's preverified reader with one fixed 8,192-byte stack buffer, exact emitted-length accounting, and a one-byte EOF probe; it never constructs a second payload `Vec`. Dependency checks use fixed stack arrays over the seven admitted dependency shapes. The managed store retains its existing pre/post content verification and integrity precedence, so descriptor-consistent truncation/suffix is a payload mismatch while missing or corrupt managed bytes remain an availability/integrity failure. A 4,096-row source/entity/map/link fixture exercises the full managed graph path, but it is functional resource evidence rather than a benchmark. Physical decode, physical receipts, the frozen 10,000-patch/100,000-edge smoke, and closure-scale evidence remain pending.

## C-05 footprint/overlap physical work boundary

Date: 2026-08-23. No timing, throughput, heap, or RSS benchmark is claimed at this intermediate checkpoint. Writers perform allocation-free domain validation and exact decoded/file/group/retained budget checks before schema construction or batch allocation. Arrow raw preflight retains the bounded footer plus at most one encoded block and fixed verifier workspace; full decode additionally charges the maximum decoded batch and 64-KiB reader workspace. Parquet raw preflight charges bounded compact-Thrift state, footer replay/stock metadata reserve, and at most one column chunk; full decode charges cached metadata, one copied row-group window, conservative Arrow output, and fixed reader workspace. Independent integration oracles reproduce both format formulas and prove exact/one-short full-reader group and retained limits for footprint and overlap. A 16,386-footprint/16,385-edge fixture fully decodes three batches/groups, including a middle group, but this is functional resource evidence rather than a benchmark. The six remaining physical families, C-05 fuzz routes, frozen 10,000-patch/100,000-edge smoke, DHAT, host RSS, and closure-scale evidence remain pending.

## C-05 cell-patch assignment/edge physical work boundary

Date: 2026-08-23. No timing, throughput, heap, or RSS benchmark is claimed at this intermediate checkpoint. Writers first perform allocation-free logical-domain validation and exact decoded/file/group/retained checks. The independent Arrow oracle charges assignment and edge decoded arrays from their exact value, offset, validity, and all-ones unused-tail buffers: fourteen buffers for assignment batches and nine for edge batches. The independent Parquet oracle derives plain-value bytes as `40 * rows + cell_text + status_text` for assignments and as either `12 * rows + patch_text` or `28 * rows + patch_text` for edges, adds `4 * ceil(rows / 8)` bytes for each optional definition-level stream, and separately charges page headers, encoded row groups, decode output, workspace, column-aware cached metadata, and footer replay. Production metadata estimation now takes the exact six-column assignment or four-column edge schema while the existing three-column footprint/overlap wrapper remains unchanged.

Raw Arrow/Parquet preflight is bounded before stock decode, and full Parquet reads retain only one copied row-group window. Exact and one-byte-short writer/reader budgets cover file, decoded, group, and retained limits in both modes and both formats. Fixtures at 8,192, 8,193, and 16,386 rows exercise boundary rollover and complete three-batch/row-group decode. These are functional resource-safety checks, not benchmarks or optimization evidence. The three matrix families, patch-region family, C-05 fuzz routes, frozen 10,000-patch/100,000-link smoke, DHAT, host RSS, and closure-scale evidence remain pending.

## C-05 patch-region physical work boundary

Date: 2026-08-23. No timing, throughput, heap, or RSS benchmark is claimed at this intermediate checkpoint. For each Arrow batch of `r` sparse declarations, the independent exact decoded oracle charges `5 * ceil(r / 8) + 12 * (r + 1) + text_bytes + 16 * r`: five all-ones validity buffers, three UTF-8 offset buffers, three text payloads, and two full-width unsigned fraction buffers. The Parquet oracle independently charges `28 * r + text_bytes` of plain values plus five-column page/header, encoder, metadata, footer, and fixed-workspace bounds. Both formats enforce exact/one-byte-short file, decoded, group, and retained budgets before stock decode, and Parquet full reads retain one copied row-group window.

Fixtures at 8,192, 8,193, and 16,386 rows exercise rollover and complete three-batch/row-group decode. The six-role prerequisite graph uses fixed dependency arrays and fixed-size canonical streaming comparison for expected patches, expected regions, context, and assessment; it performs one managed footprint decode and retains no source vector or region geometry. These are functional resource-safety checks, not benchmark or geometric-overlap evidence. The three matrix families, C-05 fuzz routes, frozen 10,000-patch/100,000-link smoke, DHAT, host RSS, and closure-scale evidence remain pending.

## C-05 patch/region/slide matrix physical work boundary

Date: 2026-08-23. No timing, throughput, heap, or RSS benchmark is claimed at this intermediate checkpoint. For a matrix batch with `r` rows, dimension `D`, identifier bytes `i`, and status bytes `s`, the exact decoded payload is `3 * ceil(r / 8) + ceil(r * D / 8) + 8 * (r + 1) + i + s + 4 * r * D`. Arrow writer bounds add nine-buffer alignment, one 64-KiB message workspace, footer blocks, and replay capacity; raw preflight retains the bounded footer plus at most one encoded block and the message workspace, while full stock decode adds one maximum decoded batch and one reader workspace. Schema and RecordBatch message-padding reconstruction uses only the fixed three-field/nine-metadata schema and fixed four-node/nine-buffer vectors and remains within that workspace.

The independent Parquet writer oracle adds exact plain ID/status/value bytes, four fixed-list level-stream bounds, three-column page overhead, component-level workspace, cached metadata, footer replay, and fixed workspace. Raw compact-Thrift preflight retains bounded metadata plus at most one column chunk; full decode retains cached metadata and one copied row-group window. Exact and one-byte-short tests cover file, aggregate decoded, row-group/batch, and retained limits for both formats. Fixtures fully decode 8,192, 8,193, and 16,386 rows, and one 65,536-dimensional row reaches both stock decoders. These are functional resource-safety checks, not throughput or optimization evidence. Derived matrix graph/receipt work, the frozen 10,000-patch/100,000-link smoke, closure-scale 100,000-patch/1,000,000-edge workload, DHAT, and host RSS evidence remain pending.

## C-05 direct-patch support/table receipt work boundary

Date: 2026-08-23. No timing, throughput, heap, or RSS benchmark is claimed at this intermediate checkpoint. Patch-support composition compares fixed-size graph and physical-receipt fields and allocates nothing. After the existing bounded raw-before-stock Arrow/Parquet matrix validation completes, direct table finalization makes one allocation-free `O(p)` pass over `p` patch rows and source-row entries to require exact expected-order PatchId/status equality, then performs constant-size expected/support/provenance/dimension/logical binding checks. The compact receipt retains only fixed-size identities, QC counters, and dimension; it retains no vector, source key, or physical bytes. Functional tests cover zero rows, all four statuses, 8,193 rows across the public Arrow batch boundary, one 65,536-dimensional row, both physical table formats, all footprint/overlap format combinations, malformed physical precedence, and exact managed `ContentIntegrity` precedence. Frozen Criterion, DHAT, host RSS, fuzz, and closure-scale evidence remains required after derived region/slide paths exist.

## C-05 derived-region finalization work boundary

Date: 2026-08-23. No timing, throughput, heap, or RSS benchmark is claimed at this milestone. For `p` source patches, `e` nonzero declared relations, `e_present` relations whose source row is present, `r` expected regions, and dimension `D`, binding and status preflight is `O(p + e)`, region lookup is `O(e log r)`, fixed-order arithmetic is `O(e_present * D)`, and output materialization is `O(r * D)`. The candidate retains one canonical `r × D` `f32` table plus fixed lineage authority. Peak operation-owned scratch is one `r × (D + 1)` `f64` accumulator followed by canonical row/vector construction; explicit sparse-relation, component-operation, retained-byte, and peak-working-byte maxima reject work before the affected allocation or arithmetic pass. Exact-edge tests accept 3 relations/9 component operations and reject 2/8, then discover and enforce exact retained and peak-working byte thresholds including one-short rejection. This is deterministic resource-safety evidence, not an optimization or scale claim. The frozen C-05 Criterion, DHAT, host-RSS, and 100,000-patch/1,000,000-link closure workloads remain required after both slide paths exist.

## C-05 derived-slide finalization work boundary

Date: 2026-08-23. No timing, throughput, heap, or RSS benchmark is claimed at this milestone. For `n` lower patch or region rows, `n_present` present rows, and dimension `D`, exact binding/status preflight is `O(n)`, fixed-order arithmetic is `O(n_present * D)`, and singleton output materialization is `O(D)`. The candidate retains one canonical slide row plus fixed lineage authority; operation-owned arithmetic scratch is one `D`-component `f64` accumulator, followed by singleton row/vector and table-construction coexistence. Contributor work counts all `n` rows and component work counts `n_present * D`; exact-edge tests accept 2 contributors/6 component operations and reject 1/5 for both paths, then discover exact retained and peak-working boundaries and reject one short. Empty/all-non-present sources produce one `missing_vector` row without component work; dimensions 1 and 65,536 pass. This is deterministic resource-safety evidence, not an optimization, biological pooling, or scale claim. The frozen C-05 fuzz, Criterion, DHAT, host-RSS, and 100,000-patch/1,000,000-link closure evidence remains required.

## C-05 shared-vector storage/link closure

Date: 2026-08-24. Host and toolchain match the C-04 Apple M4 Pro / 48-GiB / Rust 1.96.0 environment. DEC-0038 freezes the two profiles, exact grid/IDs, DEC-0031 component generator, fixed-order arithmetic, 128-bit framing, checksums, one-thread Criterion configuration, and separate compile/runtime RSS ownership. Fixture construction plus initial Arrow/Parquet publication/validation is outside Criterion iterations but inside host RSS; timed iterations traverse every component and link. This is storage/link integrity work, not embedding science or an optimization claim.

| Profile/gate | Checked shape | Final evidence | Memory | Status |
|---|---|---|---:|---|
| Criterion smoke | 10,000 patch rows × 1,024; 100,000 assignments/edges; 391,510 candidate checks per link pass | 29.961–30.261 ms focused; 30.191–30.329 ms all-workspace quick | Initial physical publication outside timed loop | pass |
| Fresh DHAT publication | Same smoke table; reconstruct link and publish patch plus assignment/edge Arrow/Parquet inside profile | 309.77 s debug/instrumented gate; all four receipts match | current 0; peak 180,729,962; cap 335,544,320; forbidden copied-vector payload 409,600,000 | pass |
| Closure Criterion | 100,000 patch rows × 1,024; 1,000,000 assignments/edges; 3,987,010 candidate checks per link pass | 283.76–284.82 ms; 363.03–364.39 million visited values+edges/s | 1,311,342,592-byte maximum RSS; cap 2,684,354,560 | pass |
| Compile/link disclosure | Same locked bench/features, `--no-run` before final runtime | 29.88 s wall | 5,221,138,432-byte maximum RSS, recorded separately | pass; not runtime evidence |

Smoke table/link/numeric/link-checksum identities are `6aa1ef12bf4955b4f58460b01f3a35975ddf147a57803a42d83359c9d4eb11c0`, `c447edd055d8f6e44cecd739a31ac157c1c3b95c690eaf64469ab53806fc9a47`, `ccb9ab3ea4ca78f26c39b7b97028757feace6b746065d8e11abbdc74df91ce65`, and `b7472ac57b761db386a4cbd19470bc7b1006fbf69a1fbcf4869db5cc5ebe0ce9`. Closure identities are `113b3d67889d901bc987fd99a20e0fd94c9c1fd5d1f5179bd5b8ae3f995e9033`, `1972def442ed8c5d980f86f2f6a28b58f54a8fcec979fd2e579ad8fe281eefee`, `9520cc35c365025f630dd0d31d7bf0b77111ff347062b070a11064aeca8acc65`, and `c081e62e6c108de5645df53d4f7c87c3f4a2671cc35ecab34e200c056f22a6cd`.

The full command completed in 111.33 s wall time and performed no compilation after the current-binary prebuild. Runtime RSS is 48.8% of the threshold and 32.0% of the forbidden 4,096,000,000-byte edge-vector payload alone. These results close C-05's declared storage/link scale boundary. They do not measure model execution, source import, out-of-core behavior, statistical estimands, patient-held-out prediction, or biological performance.

## C-06 patch-overlap dispersion work boundary

Date: 2026-08-24. No throughput, optimization, heap, or RSS benchmark is claimed. For `E` canonical overlap edges, `P` canonical patch rows, and dimension `D`, status and fixed-size binding checks precede all work. The caller must authorize checked `E * D` component operations before row traversal. Endpoint preflight and lookup are `O(E log P)` under the existing C-05 row cap; arithmetic is fixed-order `O(E * D)` and retains `O(1)` additional storage. Tests prove the exact 6-operation edge and one-short rejection, including an all-non-present table so missingness cannot evade the cap. This is functional deterministic resource evidence, not a performance or formal embedding-science claim.

## C-06 declared scalar-pattern workflow boundary

Date: 2026-08-24. No throughput, optimization, heap, or RSS benchmark is claimed. Construction borrows all compatibility numeric arrays and one CellId slice; it copies no row values or coordinates. Caller-positive row and aggregate CellId-text maxima are checked before hierarchy traversal. Validation is `O(N * H)` for `N` rows and bounded hierarchy depth `H`, plus `O(N)` probability/threshold and deterministic digest passes. Retained incremental state is fixed declarations, at most three semantic artifact IDs, compact digests, slide/frame IDs, and the runtime routing summary. The scheduler reuses the existing result-0.3 codec and project store verification; no physical mark payload, general index, receipt, or benchmark infrastructure is introduced.

## C-06 patch-to-derived-region dispersion work boundary

Date: 2026-08-24. No throughput, optimization, heap, or RSS benchmark is claimed. For `R` canonical nonzero relations, `P` patch rows, `G` region rows, and dimension `D`, fixed-size graph/table/link/provenance checks precede the checked caller cap `R * D`. Arithmetic is fixed-order `O(R * D)`; existing typed binary searches add `O(R(log P + log G))`; incremental retained state is `O(1)` and no relation/vector copy is allocated. Tests prove the exact 9-operation default edge, one-short rejection, status exclusion, zero-contributor availability, and bitwise repeat/sign-flip behavior. Arbitrary component reordering is not bitwise invariant under the deliberately frozen sequential `f64` sum and is not claimed. This is functional resource evidence, not a performance, geometry, spatial-dependence, or EMB-01 claim.

## C-06 runtime-only declared marked pre/post boundary

Date: 2026-08-24. No throughput, optimization, heap, or RSS benchmark is claimed. Each input performs constant-count row/label checks and recomputes one declaration digest over fixed-size identities plus bounded declaration/provenance tokens; cross-output semantic compatibility is fixed work. The wrapper then delegates to the unchanged legacy `compare_marked_prepost`, which remains the dominant numerical/allocation path. It clones two bounded `DeclaredMarkUse` values and two compact `DeclaredScalarIdentity` values, while both potentially unbounded compatibility timepoint strings are borrowed. No result payload, CellId vector, coordinate array, serializer, cache entry, physical bytes, receipt, index, or general validation structure is retained. This is a runtime compatibility/resource characterization, not durable provenance or benchmark evidence.

## C-06 declared binary prevalence-change boundary

Date: 2026-08-24. No throughput, optimization, heap, or RSS benchmark is claimed. The caller reuses the fixed-work declared semantic gate, checks two count bounds and two canonical prevalence bit patterns, and performs exactly one `f64` subtraction only when both row counts are positive. Empty-side availability executes no delta arithmetic. The result contains fixed-size counts/scalars/status plus borrowed mark-use, scalar-identity, and timepoint references; it allocates and clones nothing and retains no row, CellId, coordinate, payload, serializer, cache, or physical bytes. This is deterministic functional/resource evidence, not performance, inference, or durable provenance evidence.

## C-06 slide aggregation-path discrepancy work boundary

Date: 2026-08-24. No throughput, optimization, heap, or RSS benchmark is claimed. Candidate, graph, provenance, common-slide, singleton-row, and receipt-bridged patch/region lineage checks precede the caller's exact dimension cap `D`. When both status-aware singleton vectors are present, arithmetic visits `D` components in stored order, promotes to `f64`, accumulates squared differences sequentially, and divides once; otherwise no component arithmetic occurs after the same cap. Runtime is `O(D)`, incremental retained state is fixed-size `O(1)`, and no vector, row, lineage, or physical payload is allocated or cloned. Tests prove exact `D`/one-short admission, including an unavailable path, fixed repeat bits, positive zero, and a foreign same-slide lineage rejection. This is functional resource evidence, not performance, agreement, geometry, spatial-dependence, inference, or embedding-quality evidence.

## C-06 declared binary cell-embedding centroid work boundary

Date: 2026-08-24. No throughput, optimization, heap, or RSS benchmark is claimed. For `N` exact declared/table rows, `N_present` present embedding rows, and positive dimension `D`, table/artifact and row-count checks precede an `O(N)` ordered CellId/status/grouping-digest pass. The caller then admits conservative checked component work `N_present * D + D` and exact arithmetic storage `2 * D * size_of::<f64>()`, including when either group is unavailable. Available execution performs fixed-order `O(N*D)` vector accumulation plus one `O(D)` centroid-difference pass; it allocates exactly two fallible `D`-component `f64` accumulators. The fixed result retains only declarations, identities, QC, two status-count summaries, and one digest. Tests prove exact/one-short row, component, and byte limits, unavailable-path admission without allocation/arithmetic, fixed repeat bits, and positive zero. This is functional deterministic resource evidence, not performance, spatial dependence, classification, embedding-quality, inference, or real-source evidence.

## C-06 declared probability–cell-embedding cross-covariance work boundary

Date: 2026-08-24. No throughput, optimization, heap, or RSS benchmark is claimed. For `N` exact declared/table rows, `P` present embedding rows, and positive dimension `D`, required-probability and table/artifact checks precede row admission, then an `O(N)` ordered CellId/raw-probability-digest/status pass performs only scalar probability accumulation and variation detection. The caller admits conservative checked component work `2 * P * D + 2 * D` and exact arithmetic storage `2 * D * size_of::<f64>()`, including for both unavailable states. Available execution performs one stored-order embedding-mean pass, one centered cross-product pass, and two dimension passes using exactly two fallible `D`-component `f64` accumulators. The fixed result retains declarations, identities, QC, present count/mean, availability, and one digest. Tests prove exact/one-short row, component, and byte limits, unavailable-path admission without allocation/component arithmetic, fixed repeat bits, binary non-use, probability identity, and positive zero. This is functional deterministic resource evidence, not performance, correlation, spatial dependence, classification, calibration, embedding-quality, inference, or real-source evidence.

## C-06 declared binary cell-centroid project-workflow boundary

Date: 2026-08-24. No throughput, optimization, heap, RSS, or cross-project cache benchmark is claimed. Node construction and every pre-lookup input verification recompute the existing S7 `O(N)` ordered CellId/status/grouping binding without component arithmetic or a `D`-component allocation. Cache misses then delegate unchanged to S7's conservative `N_present * D + D` component work and exact `2 * D * size_of::<f64>()` arithmetic storage; hits perform no component arithmetic. The node borrows Pattern/table inputs, retains two ordinary references, four-to-six semantic ArtifactIds, three limits, and one fixed metadata binding; it retains no row, mark, or vector array. Both availability states encode through one exact fallible 18-byte reservation. Tests prove exact 18-byte scheduler/project admission, one-short failure atomicity, strict codec rejection, all scientific-limit cache identities, and semantic-store verification before miss and would-be-hit replay. This is bounded cache-lifecycle evidence inside the existing project trust boundary, not a performance optimization, durable receipt, external producer authentication, portable cache format, or new scientific claim.

## C-06 declared nucleus-area–cell-embedding cross-covariance work boundary

Date: 2026-08-24. No throughput, optimization, heap, or RSS benchmark is claimed. For `N` exact declared/table rows, `P` present embedding rows, and positive dimension `D`, project/declaration/provenance plus required-area and table/artifact checks precede row admission, then an `O(N)` ordered CellId/raw-area-digest/status pass performs only scalar area accumulation and variation detection. The caller admits conservative checked component work `2 * P * D + 2 * D` and exact arithmetic storage `2 * D * size_of::<f64>()`, including for both unavailable states. Available execution performs one stored-order embedding-mean pass, one centered area/embedding cross-product pass, and two dimension passes using exactly two fallible `D`-component `f64` accumulators. The fixed result retains declarations, identities, QC, present count/mean, availability, and one digest. Tests prove exact/one-short row, component, and byte limits, unavailable-path admission without allocation/component arithmetic, fixed repeat bits, binary non-use, raw-area identity, and positive zero; a local unit covers checked overflow and impossible-capacity allocation failure. This is functional deterministic resource evidence, not performance, correlation, size normalization, segmentation accuracy, spatial dependence, classification, embedding-quality, inference, or real-source evidence.

## C-06 contained-patch cell-embedding local-dispersion work boundary

Date: 2026-08-24. No throughput, optimization, heap, or RSS benchmark is claimed. For `A` exact assignment rows, `E` contained-shared incidences, and positive cell-embedding dimension `D`, table/artifact, expected-cell, ordered CellId, graph, and paired physical receipt checks precede caller limits and allocation. The caller admits `A` and `E`, conservative checked component work `3 * E * D`, and exact incremental storage `E * size_of::<usize>() + D * size_of::<f64>()`. It allocates one fallible edge-index vector, sorts it in place by patch/assignment/original edge, performs scalar eligibility scans, and allocates one reusable `f64[D]` centroid only when an eligible patch exists. Available arithmetic visits eligible incidence components twice and centroid components once per eligible patch, bounded by `3ED`; runtime is `O(A + E log E + E*D)`. The fixed result retains only counts and compact artifact/digest/QC identities. Tests prove exact/one-short assignment, edge, work, and byte limits, checked overflow/allocation failure, repeated bits, positive zero, every non-present status, singleton exclusion, and overlapping-incidence accounting. This is functional deterministic resource evidence, not a throughput result, spatial-dependence measure, independent-patch design, embedding-quality result, inference, or real-source evidence.

## C-06 declared binary-group nucleus-area contrast boundary

Date: 2026-08-24. No throughput, optimization, heap, or RSS benchmark is claimed. For `N` exact declared rows, target-project and nucleus-provenance validation plus required-column/length checks precede the caller's exact row cap; only then does one stored-order pass validate every finite strictly positive area, update two `f64` sums and counts, and frame the ordered binary/area paired-value digest. Runtime is `O(N)` with fixed incremental storage: one digest state, two sums, and two counts. Optional probabilities are neither traversed nor used for grouping or arithmetic. Tests prove exact/one-short row admission, unavailable one-group paths, repeat identity, changed assignments, probability non-use, and canonical positive zero. This is functional deterministic resource evidence, not a performance, inferential, segmentation-validation, patient/specimen, independence, or real-source claim.

## C-06 contained-patch binary-group nucleus-area contrast boundary

Date: 2026-08-24. No throughput, optimization, heap, or RSS benchmark is claimed. S12 first performs its bounded `O(N)` validation and paired-value pass. The caller then binds `N` ordered CellIds to `A` assignment rows, verifies fixed-size graph/receipt identities, and admits exact assignment/edge maxima plus operation-owned heap storage `E * size_of::<usize>()`. One fallible `E`-index vector is sorted in place by patch/assignment/original edge; scalar patch grouping and arithmetic are `O(E)`, giving total runtime `O(N + E log E)` and no retained row/value/edge array in the result. Tests prove exact 3-assignment/5-edge/`5*sizeof(usize)` admission and every one-short edge, assignment, and byte rejection, alongside unavailable paths, repeat bits, positive zero, and unequal-patch weighting. This is functional deterministic resource evidence, not performance, spatial-dependence, independent-patch, segmentation-quality, inferential, or real-source evidence.
## Full-program simulation checkpoint 25 work boundaries — 2026-08-25

No throughput, latency, optimization, heap, or RSS claim is made. Reaction–diffusion performs
bounded `O(S*N + N log N)` work for `S` steps and `N` cells, with the FFT applied once to the final
field. Level-set stepping is `O(S*N)`; each requested reinitialization performs an explicitly
declared and reported `N*C` distance visits for `C` sampled contour points. Vascular transport is
`O(S*N + V + U + N)` for bounded vessel sources `V`, uptake cells `U`, transport steps, gradients,
and final four-neighbor hypoxic components. All three validate step-derived cell work before
simulation; the level-set workflow separately enforces its runtime distance-visit declaration.
These are deterministic resource bounds, not scale or performance evidence.
## Full-program simulation/SBI-summary checkpoint 26 work boundaries — 2026-08-25

No performance optimization or throughput claim is made. Exact resource-distance fitting uses
`O(NR + NP^2 + P^3)` work and `O(NP + P^2)` retained design/posterior storage for at most 5,000
observations, 1,000 segments, and 128 coefficients. Mechanistic coupling delegates bounded module
work and additionally caps three field-module cell-step declarations plus agent events/pair visits
across all intervals at 250 million. Soft summary matching performs exactly
`(choose(N_obs,2)+choose(N_gen,2))*B` visits, capped at 250 million, with `O(B)` arithmetic storage.
These are deterministic functional bounds, not scale benchmarks.
## Full-program SBI checkpoint 27 work boundaries — 2026-08-25

No throughput benchmark or optimization claim is made. Rejection ABC bounds maximum proposals times
per-proposal simulator cell steps. SMC-ABC bounds stages times maximum proposals per stage times
per-proposal work and additionally performs `O(T*N^2)` full-mixture importance evaluation for `N`
particles. Synthetic likelihood bounds `(iterations+1)*replicates*per-simulation cell steps` and
uses fixed two-dimensional `O(R)` covariance work per evaluation. Each workflow rejects aggregate
declared work above 250 million before inference.
## Full-program SBI reliability checkpoint 28 work boundaries — 2026-08-25

No performance benchmark or optimization claim is made. SBC bounds replicates times observed plus
worst-case rejection-ABC simulator work. Simulation OOD performs exactly
`(calibration_count+1)*reference_count*dimension` feature visits plus deterministic distance sorts.
The posterior-predictive lab bounds replicates times per-replicate simulator cell steps. Each rejects
declared work above 250 million before execution.

## Full-program longitudinal milestone 29a work boundaries — 2026-08-25

No throughput or optimization claim is made. The dense filter/smoother validates a conservative per-step matrix-operation upper bound derived from state and observation dimensions before allocating state histories or executing. Runtime is cubic in dense state/observation dimensions and retained output is quadratic per time step; the caller supplies explicit time, dimension, and operation ceilings.

The scalar EKF/UKF specialization performs constant arithmetic per declared time step and retains one predicted state, filtered state, and diagnostics row per step. It rejects a sequence beyond the caller's explicit time-step ceiling. This is deterministic complexity accounting, not benchmark or scale evidence.

The bootstrap particle workflow performs `T*N` transition/weight work plus `O(T*N)` systematic-resampling scans and retains `O(T*N)` particles, weights, and ancestry. Backward ancestry tracing costs `O(T*M)` for `M` requested trajectories. The exact `T*N` particle-step count is checked against a caller maximum before allocation; no throughput or particle-efficiency claim is made.

## Full-program 3-D statistics milestone 30a work boundaries — 2026-08-25

No throughput or optimization claim is made. The cuboid workflow constructs exactly `N(N-1)/2` retained pair records, rejects above both the caller maximum and a one-million-pair hard cap, and performs exactly pair-count times radius-count eligibility evaluations capped at 250 million. Window/point normalization is linear; returned curves are linear in radius count. A future indexed/streaming owner is required beyond this bounded exact specialization.

Supplied-intensity K reuses the same bounded unordered plan; directed cross-K performs exactly `N_A*N_B` pair visits per radius, checked against the caller cross-pair maximum and the shared 250-million pair-radius ceiling. Both retain only normalized inputs and output curves beyond the validated geometry. These are functional work bounds, not scale or throughput evidence.

## Full-program 3-D graph milestone 31a work boundaries — 2026-08-25

The exact graph builder evaluates and retains `N(N-1)/2` candidates under the existing caller and one-million-pair hard caps. Radius selection is linear in candidates; union-kNN sorts each node's incident candidates and is bounded exact work rather than an indexed scale path. Final storage is `O(N+E)` for canonical edges and symmetric CSR, with a caller edge maximum. No performance or scalability claim is made.

## Full-program evolutionary association milestone 31b work boundaries — 2026-08-25

Tree compilation/traversal is bounded by clone count times nodes plus directed edge visits. The association retains only within-block clone pairs and performs exactly pair-count times `(permutations+1)` correlation evaluations, checked against a caller maximum before null generation. This is deterministic resource accounting, not a scale or optimization claim.

## Full-program randomized interference milestone 32a work boundaries — 2026-08-25

The workflow exactly materializes the Cartesian assignment state space, capped at both the caller maximum and one million states. Before enumeration it validates a conservative upper bound covering state exposure mapping/probability/exact-SD scans and observed/null contrast scans against the caller's unit-operation maximum. Storage is `O(S*N + B)` for `S` states, `N` units, and `B` retained null values. No performance or large-design claim is made.

Exposure construction visits each directed edge once for scalar graph mappings and once per radius for multiscale counts; continuous fields visit no edges. The exact planned visit count is checked against a caller maximum before mapping. Storage is linear in units, edges, and returned unit-by-radius values. No throughput claim is made.

Nested Gaussian EIG performs exactly `N_outer*(N_inner+1)` scalar likelihood evaluations, checked against caller and 250-million hard maxima before allocation. It retains `O(N_outer)` outer artifacts and `O(N_inner)` temporary log likelihoods. This is deterministic work accounting, not a performance or general-design scale claim.

## Full-program causal sensitivity checkpoint 33 work boundaries — 2026-08-25

Bias sensitivity and Manski bounds are linear in scenario and observation count under explicit caller and one-million hard caps. Rosenbaum sensitivity performs bounded set-by-Gamma validation plus `O(N_non_tied*G)` stable binomial-tail recurrence work and retains one curve row per Gamma. These are deterministic functional bounds, not performance claims.

## Full-program stable numerics milestone 34a work boundaries — 2026-08-25

Log and weighted-mean primitives are linear in input length. Covariance performs exactly `rows*columns*(columns+1)/2` cross products and retains `O(columns^2)` output, checked against caller and hard element/work maxima before calculation. No throughput or optimization claim is made.

Result maturity is constant work over a fixed policy catalogue. Execution-mode selection is linear in descriptor count and uses checked integer base-plus-per-item estimates; it retains one assessment per mode. These are planning-policy complexity statements, not measured performance evidence.

Validation-ladder evaluation is constant work over exactly six stages plus linear evidence/risk identifiers. It is policy bookkeeping, not a performance benchmark.

## Full-program Part VII completion checkpoint 39 work boundaries — 2026-08-25

The new graph workflows remain deliberately bounded dense implementations. Diffusion wavelets and
scattering reuse cubic exact eigendecomposition and retain dense bases/features; heterogeneous
construction evaluates bounded pairs; hypergraph and cellular workflows check explicit incidence
budgets; motif/Hodge clique construction checks all bounded triples; Hodge solves dense systems.
The validation suite runs fixed tiny fixtures only. These are functional resource bounds and exact
oracles, not throughput, sparse-scale, memory-scaling, or accelerator evidence.

## Full-program Part VIII completion checkpoint 41 work boundaries — 2026-08-25

Alpha/witness and cubical persistence are bounded by caller simplex/mask limits and pinned-library
execution; patient comparison retains a quadratic distance matrix and exactly enumerates a capped
assignment product; connectivity evaluates a capped quadratic pair set; raster operations retain
bounded masks per radius/repetition. No representative sparse-memory measurement was executed, and
the validation ledger records that gap rather than making a scaling claim.

## Full-program Part IX checkpoint 42 work boundaries — 2026-08-25

EM pCCA retains dense feature covariance/latent operations under 32-feature and 10,000-row caps;
Bayesian pCCA uses bounded one-factor NUTS draws; MOFA uses bounded views/features/rows/factors and
iterations with CPU-only execution. These are functional limits on synthetic fixtures, not runtime,
memory-scaling, GPU, or representative multimodal-performance evidence.

## Full-program Part IX checkpoint 43 work boundaries — 2026-08-25

Matrix MOFA retains bounded dense Gaussian data. Hierarchy compilation is linear in supplied nodes
and attachments. Graph-spatial fitting caps the latent parameter vector and inverse-Hessian at 512;
the current exact graph/linear-algebra path is not sparse-scale. CP/Tucker fitting caps tensor
dimensions, entries, and parameters and runs CPU JAX/SciPy. No throughput, memory-scaling,
accelerator, representative pathology, or optimization claim was measured.

## Full-program Part IX completion checkpoint 44 work boundaries — 2026-08-25

Exact GP NUTS is capped at 64 regions and one factor. Multiresolution, dropout, and joint fits cap
rows/features/latent parameters and use CPU JAX/SciPy dense optimization; the joint approximation
retains a dense inverse-Hessian. M0–M5 comparison retains the existing bounded nested-fit work cap.
The validation suite uses fixed tiny analytic/simulated controls. No representative throughput,
memory scaling, accelerator, sparse-field, or real-cohort performance evidence was measured.

## Full-program Part VI registration/atlas checkpoint 45 work boundaries — 2026-08-25

SimpleITK images are capped at 256x256; dense SVF/probabilistic images at 64x64; LDDMM at 64
landmarks; GP correspondence at 128 landmarks/256 cells; atlas inputs at 10,000 region rows and 128
features. Dense fields, kernel matrices, posterior draws, and LOPO fits are retained under these
functional bounds. No representative WSI, GPU, memory-scaling, or throughput measurement was run.

## Full-program Part X neural/generative checkpoint 46 work boundaries — 2026-08-25

Neural point-process patterns/points, hidden width, and quadrature grids are bounded; point-set
patterns cap cardinality and reverse steps; neural SBI caps simulations/epochs/grid; model-card work
uses small dense pair/summary matrices. Torch/JAX run single-device deterministic CPU only. No GPU,
large simulation bank, WSI-scale pattern, memory-scaling, or throughput evidence was measured.

## Full-program Part XI advanced 3-D checkpoint 47 work boundaries — 2026-08-25

Serial stacks cap sections/landmarks/draws; exact alpha caps points; deformation interpolation caps
paired points/draws; clone models cap tree/cells/features/draws; validation uses fixed tiny controls.
All new paths retain dense arrays/kernel/interpolation work. No WSI stack, large tetrahedralization,
memory scaling, accelerator, or throughput evidence was measured.

## Full-program Part XII causal/active checkpoint 48 work boundaries — 2026-08-25

Observational and perturbation fits retain bounded dense rows/design matrices and cluster-fold
vectors. Active selection scans a bounded candidate list; sequential updates are scalar; power
retains one bounded simulation vector per design; validation uses fixed tiny controls. No large
cohort, combinatorial optimizer, acquisition latency, memory scaling, or throughput was measured.

## Full-program runtime/Bayesian checkpoint 49 work boundaries — 2026-08-25

Fixed HMC is one-dimensional with bounded leapfrog/draw counts. Cluster inference caps patterns,
points, parent candidates, latent iterations, finite Gibbs sites/states, and posterior draws.
Calibration caps scenarios/repetitions/sample size and uses fixed scoped-thread partitions. The
scaling smoke measures equivalent wrapping-integer sums at 1k/2k/4k items with checksums and phase
medians; CLI artifact persistence is transactional but explicitly not self-timed. This is functional
smoke evidence, not a throughput baseline or representative performance claim.

## Full-program terminal SPDE checkpoint 50 work boundaries — 2026-08-25

The rectangular mesh caps resolution at 16 per axis and uses dense matrix assembly/optimization even
though precision/projections are serialized sparsely. Event/region counts and iterations are bounded;
two declared resolutions are fitted for sensitivity. No adaptive/holed mesh, sparse factorization,
posterior sampling, memory scaling, WSI-scale workload, or throughput measurement was run.


## ARCH-INTEGRATION-01 multiplex application — 2026-09-04

A complete native six-patient/twelve-slide study is measured, including strict panel admission,
shared radius Moran/Geary graphs, canonical patient Max-T, durable persistence/replay and atomic
publication. The canonical owners remain; no optimization or new benchmark framework was added.
The fixed generator checks exact graph counts, all patient/slide denominators and available
inference; every cold/replay result is byte-identical and all project ledgers remain at thirteen
executions. Repeated workloads cover 72 and 60,000 cells with two selected quantitative channels
plus nullable binary/categorical annotations.

Build: `cargo +1.96.0 build --locked --release --features cli --bin marklab`, default features and
system allocator, Apple M4 Pro/48 GiB/macOS 26.5.2. `/usr/bin/time -l` measures three independent
empty-project cold runs and three replays for each workload. Median cold/replay wall time is
0.75/0.10 s for 72 cells and 0.86/0.17 s for 60,000 cells; maximum large-workload RSS is 49.766 MiB.
Concurrent full-suite/fuzz work, unflushed filesystem caches and startup noise limit interpretation.
Cold/replay are different execution paths with the same scientific output, not an equivalent-work
algorithm speedup comparison. The large grid requires 19,700 directed edges per slide/channel and
945,600 reserved statistic edge evaluations over the study.

All raw samples, commands, input/binary hashes, exact verification and limits are in
[`docs/multiplex-study-measurements.md`](../multiplex-study-measurements.md). Irregular geometry,
dense graphs, selected-channel missingness, large panels, millions of cells and other inference
engines require their own workloads. The strict 16 MiB recipe limit and fixed point/edge/memory/work
admission remain. No clinical, biological or universal whole-slide capacity claim is made.
