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
