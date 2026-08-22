# Marklab Refactor Closure Report

Plan version: 1.0  
Base production SHA: `a642fbcdd80b5baf784cd633b707dc0283a24d11`  
Refactor branch: `refactor/audit-remediation`  
Closure source SHA: `9a11e11d06a64f3f479564082d5f82186c450980`  
Date: 2026-08-22

## Executive summary

The remediation converted Marklab from a mixed prototype/CLI workflow into a
library-first analysis system with explicit scientific, application, adapter,
and output responsibilities. Every one of the 59 registered findings is
closed: 58 were fixed and BOUND-05 was disproved with exact dependency and WSI
integration evidence.

The principal repairs are:

- true orientation-preserving two-dimensional rigid registration;
- distinct stratified and unstratified confounding inference;
- typed unavailable and undefined statistics with a finite serialization
  boundary;
- honest multiscale-residual, tapered-periodogram, mark-pair-covariance,
  descriptive-margin, and beta-posterior naming;
- application-owned marked and multimodal run objects, with one transform,
  spatial index, graph, telemetry history, and result assembly path;
- one normalized CSV/Parquet row contract and one `PatternBuilder`;
- transactional output, safe batch identifiers, versioned pre/post results,
  and a deliberately narrow 0.2-to-0.3 converter;
- one deterministic `rstar`-backed two-dimensional spatial index reused by
  nearest-neighbor, graph, covariance, territory, and profile calculations;
- shell-level contiguous spectral permutation storage with operational mode
  chunking and reusable scratch;
- validation scenarios that generate inputs and observe production engine
  results instead of synthesizing outcomes.

Deleted or removed surfaces include the false scale-plus-translation rigid
fit, `Pattern::from_paths`, fake MODWT/DoG/Bartlett names, multimodal
placeholders in marked results, unimplemented territory/profile fields,
sentinel-zero comparison results, the mixed pre/post god file, duplicate CLI
scientific computations, and compatibility aliases that would conceal the
0.3 contract.

The remaining risks are operational and compatibility limitations rather than
open registered defects. They are listed explicitly below.

## Findings closure matrix

The paths below are representative owners, not exhaustive per-commit file
lists. The full reproduction, resolution, test, and benchmark evidence remains
in [FINDINGS_MATRIX.md](FINDINGS_MATRIX.md); raw performance data and methods
are in [PERFORMANCE_FINAL.md](PERFORMANCE_FINAL.md).

| ID | Resolution | Representative files | Tests | Benchmark | Commit |
| --- | --- | --- | --- | --- | --- |
| COR-01 | Validation invokes production engines and comparisons; no synthesized outcomes or hidden failures | `src/synthetic_smoke/`, `.github/workflows/calibration.yml` | Engine-call, scenario, no-injection, denominator, smoke, calibration | 1,000-replicate controls: 2.4% multimodal and 3.3% marked false positives | `b71b2f6`, `f47ab43`, `b660cde`, `38cf427`, `c0b6bf8`, `20803d3` |
| COR-02 | Implemented true orientation-preserving rigid fit; removed scale absorption | `src/registration/`, `src/multimodal/engine.rs` | Identity, translation, rotation, distance, scale, reflection, noise, degeneracy | N/A | `53e2348` |
| COR-03 | Persisted distinct stratified primary and unstratified sensitivity results | `src/api/qc_pipeline.rs`, `src/output/result_types/marked.rs` | Distinct execution, all conclusion states, degenerate/missing strata, round trip | Shared observed modes/powers; spectrum results in performance report | `aecc554`, `2a7fa2b` |
| COR-04 | Optional ratio/z-score plus typed undefined reasons; finite persistence boundary | `src/neighborhood/enrichment.rs`, `src/output/` | Sparse/zero-variance JSON, CSV, Parquet, and report tests | N/A | `4bf20e8` |
| COR-05 | Replaced unavailable zero sentinels and empty-bin zeros with typed absence | `src/comparison/result.rs`, `src/spectra/mark_pair_covariance.rs` | Empty bins, unavailable pre/post/profile, all serializers | N/A | `e7447c0` |
| COR-06 | One absolute-plus-relative axis comparison with typed mismatch diagnostics | `src/prepost/axes.rs` | Harmless reconstruction and material mismatch for every curve family | N/A | `e7f91ca` |
| COR-07 | Separate in-mask/QC/exclusion/retained counters and denominators | `src/io/pattern_builder.rs`, `src/io/row.rs` | Internal control, tumor, IHC, artifact, nonviable, zero denominator, CSV/Parquet parity | Loader results in performance report | `6000cc8` |
| SCI-01 | Renamed fake MODWT subsystem to multiscale residual/scale energy | `src/multiscale_residual/`, config/result/docs | Obsolete-alias rejection, checkerboard/gradient, engine/output/CLI | Marked territory and complete-run benchmarks | `ada6159` |
| SCI-02 | Replaced fake DoG name with literal scale-to-neighborhood-radius helper | `src/multiscale_residual/` | Conversion and obsolete-term source tests | N/A | `ada6159` |
| SCI-03 | Renamed wavelet territories to residual neighborhoods with meaningful fields | `src/multiscale_residual/territories.rs`, result/artifact modules | Oracle, field, GeoJSON, output, pre/post | Residual plan 69–84% faster for observed plus nulls | `ada6159` |
| SCI-04 | Renamed false Bartlett estimator to Hann-tapered raster periodogram; added real radial shells | `src/periodogram/` | Independent shell oracle and finite diagnostics | Grid-64 median 12.703 ms; full comparison recorded | `35857b4` |
| SCI-05 | Removed MMR-specific prose from the generic marked engine/report | `src/api/interpretation.rs`, `src/io/report.rs` | Generic and multimodal wording tests | N/A | `ada6159` |
| SCI-06 | Renamed pair correlation to centered mark-pair covariance | `src/spectra/mark_pair_covariance.rs`, config/result/docs | Independent centered-product, empty-bin, alias rejection | Indexed fixed-plan results in performance report | `2449e3f` |
| SCI-07 | Renamed equivalence claim to descriptive margin assessment | `src/comparison/margin_assessment.rs`, `src/prepost/` | Margin boundaries, absent/insufficient states, schema/report | N/A | `2191974` |
| SCI-08 | Renamed beta-binomial claim to fixed-prior beta posterior group summary | `src/diagnostics/beta_posterior.rs` | Posterior fixtures, grouping, schema/config/CLI/report | N/A | `1e8fbbd` |
| SCI-09 | Renamed curve test to pooled-bin non-spatial diagnostic | `src/comparison/pooled_bin_difference.rs` | Determinism, statistic, zero permutations, output language | N/A | `69005bb` |
| ARCH-01 | Library and CLI consume one complete multimodal application run | `src/multimodal/engine.rs`, `src/cli/multimodal/` | One fit/index/graph, all nulls, library/CLI equality | Complete corrected run 0.520/1.278/3.445 ms | `698226c`, `b97dfb3` |
| ARCH-02 | Split marked planning, computation, policy, diagnostics, and assembly | `src/api/` | Public run and 20-test spectrum integration suite | Complete marked run results recorded | `82751fe`, `e29bd2c` |
| ARCH-03 | Decomposed Fourier kernel, modes, shells, permutations, summaries | `src/spectra/structure_factor/` | Numerical differentials, chunk equality, deterministic parallelism | 99% retained-matrix reduction; final spectrum timings | `38700da`, `89c7421`, `3b790b2`, `8681338`, `efb8187`, `85ed0d1`, `efa1089` |
| ARCH-04 | CLI now parses/loads/invokes/writes; scientific work is application-owned | `src/cli/multimodal/analyze.rs`, `src/output/multimodal_*` | 21 multimodal CLI tests and library parity | Analysis and output measured separately | `698226c`, `b97dfb3`, `a29885b`, `99f6d78` |
| ARCH-05 | Split configuration model, defaults, decoding, and validation behind facade | `src/config.rs`, `src/config/` | Eight strict configuration contracts | N/A | `5de8304` |
| ARCH-06 | Split marked/multimodal pre/post services and shared only true semantics | `src/prepost/` | 13 pre/post tests and both CLI flows | N/A | `3ed6914` |
| ARCH-07 | Split result semantics, projections, transaction, manifest, and artifact I/O | `src/output/` | 16 output tests and CLI transaction suites | Transaction 0.874/0.859/0.939 ms | `a0dee6d`, `99f6d78` |
| ARCH-08 | Split common, marked, multimodal, pre/post, diagnostic, artifact result modules | `src/output/result_types/` | Strict 0.3 round trip/unknown-field tests | N/A | `51564c8`, `adda397` |
| ARCH-09 | Removed wildcard-coupled cosmetic modules; established cohesive owners | `src/api/`, `src/config/`, `src/output/`, `src/prepost/` | Source contracts, Clippy, full suite | Code-shape audit; N/A runtime | `1f64d03`, `b724013`, `87a5e4f`, `4aaed57`, `2ded57a`, `e5cf7e0` |
| BOUND-01 | Filesystem loading moved from `Pattern` to `PatternLoader` adapter | `src/data/pattern.rs`, `src/io/mod.rs` | Public API/streaming/parity/CLI tests | Million-row load 2.569–2.613 s | `eb4f5b0` |
| BOUND-02 | Split domain cells, labels, generic CSV, CellViT, and validation | `src/multimodal/cells.rs`, `labels.rs`, `csv_input.rs`, `cellvit.rs` | Domain, adapter, validation, allocation tests | Metadata/label memory results recorded | `dc9ffeb` |
| BOUND-03 | Domain enrichment is independent of the CLI feature | `src/neighborhood/enrichment.rs`, feature wiring | No-default and library configured-null tests | N/A | `698226c` |
| BOUND-04 | Output receives complete result/artifact models and computes no science | `src/output/`, `src/multimodal/engine.rs` | Source boundary, parity, sidecar suites | Separate output benchmark | `698226c`, `b97dfb3`, `a29885b` |
| BOUND-05 | Disproved: WSI adapter is cohesive and dependency-clean; preserved unchanged | `src/wsi.rs`, `src/cli/slide.rs` | 10 local WSI oracle tests; external scheduled test | N/A | audit at `085ed40` |
| DUP-01 | One average-even median implementation | `src/common/stats.rs` | Odd/even/non-finite median contracts | N/A | `8508671` |
| DUP-02 | Consolidated only identical mean/variance/extrema semantics with explicit names | `src/common/stats.rs` | Denominator and finite-policy unit tests | N/A | `8508671` |
| DUP-03 | One scalar permutation p-value implementation with explicit tail/ties/+1 | `src/inference/scalar_pvalues.rs` | High/low/two-sided/ties/non-finite/minimum tests | N/A | `8508671` |
| DUP-04 | One enrichment core parameterized by permutation strategy | `src/neighborhood/enrichment.rs`, `label_permutation.rs` | Stratified/unstratified numerical preservation and all-null suites | Final multimodal benchmark | `b233104` |
| DUP-05 | CSV and Parquet decode to one row type and one builder | `src/io/row.rs`, `pattern_builder.rs`, `csv/`, `parquet/` | Logical parity, dense-option, QC and metadata contracts | 1,024-row loads improve 48.9%/51.4% | `f4243cd` |
| DUP-06 | Transform and graph are constructed once in the application run | `src/multimodal/engine.rs` | Exact call-count and CLI/library parity | Complete multimodal run | `698226c` |
| DUP-07 | One telemetry history and one run-manifest builder | `src/output/manifest.rs`, result/application telemetry | Result/sidecar/trace/manifest equality | Output benchmark separated from analysis | `756ecbc`, `3d8ad46`, `5d5f8d2` |
| DUP-08 | One typed curve-analysis-to-output conversion | `src/comparison/result.rs`, pre/post/profile callers | Conversion invariant and source-literal assertion | N/A | `d5cf6af` |
| DUP-09 | Canonical named geometry lengths and maximum interpretable scale | `src/geom/length_scales.rs`, `src/data/pattern.rs` | Definition, invalid input, alias rejection, endpoint tests | No hot-loop change; Phase 12 workloads remain valid | `085ed40` |
| PERF-01 | One deterministic finite-coordinate `rstar` spatial index | `src/geom/spatial_index.rs` | Nearest/radius/kNN brute-force differential and order tests | Nearest 47–79% faster; scaling recorded | `3c4a255`, `10e4932`, `968d014`, `9d01b04` |
| PERF-02 | Radius graph uses indexed queries and creates each edge once | `src/neighborhood/graph.rs` | Graph brute-force differential and boundary cases | Up to 54% faster; subquadratic fixed-density ratios | `3c4a255` |
| PERF-03 | kNN graph uses indexed nearest iteration with deterministic cutoff ties | `src/neighborhood/graph.rs` | kNN/graph brute-force and tie tests | 85–95% faster | `3c4a255` |
| PERF-04 | Fixed pair/bin plan reused for observed and permutations | `src/spectra/mark_pair_covariance.rs`, `src/api/spatial_stage.rs` | Pair-plan differential and one-build contract | Observed+19 nulls 26–72% faster | `10e4932` |
| PERF-05 | Indexed/precomputed territory neighborhoods reused across labels | `src/multiscale_residual/territories.rs`, `src/neighborhood/territories.rs` | Marked/multimodal brute-force and one-plan tests | Observed+19 nulls 69–84% faster | `10e4932`, `968d014` |
| PERF-06 | Territory profiles use shared-index radius visitors | `src/neighborhood/profiles.rs` | Membership brute-force differential | Proportional-profile doubling ratios 1.33–3.67× | `3c4a255`, `10e4932`, `968d014` |
| PERF-07 | Million-row fixture and CSV ingestion stream; NN is indexed | `benches/pattern_load.rs`, `src/io/csv/decoder.rs` | Streaming visitor, loader, workflow contracts | One million rows 2.569–2.613 s, 430.13 MiB RSS | `eb4f5b0` |
| PERF-08 | Shell-level contiguous matrices, real chunking, reusable permutation scratch | `src/spectra/structure_factor/`, `src/common/matrix.rs` | Chunk/determinism/oracle/DHAT contracts | 99% matrix and 46.1% measured binary RSS reduction | `efb8187`, `67f83f7`, `85ed0d1`, `43e5d74`, `93cdda9`, `c183942`, `efa1089` |
| PERF-09 | Shared run metadata and borrowed/compact hot-path labels | `src/multimodal/cells.rs`, `labels.rs`, output adapters | Pointer identity, serialization, flattened exports | Complete multimodal RSS +7.5% while doing more work | `dc9ffeb` |
| PERF-10 | Output consumes runs and borrows projections; no complete-result/table clone | application and `src/output/` | Public run, CLI, output, source contracts | Transaction benchmark and complete-run RSS | `a29885b`, `82751fe`, `99f6d78` |
| MODEL-01 | Distinct residual and neighborhood territory DTOs | `src/output/result_types/marked.rs`, `multimodal.rs` | Schema, DBSCAN, profile, pre/post, GeoJSON | Territory/profile benchmarks | `51564c8` |
| MODEL-02 | Removed placeholders and populated multimodal telemetry | result types, `src/multimodal/engine.rs` | Schema absence, stage order, sidecar equality, parity | Complete multimodal and output benchmarks | `2191974`, `b71b2f6`, `51564c8`, `8ba7c97` |
| MODEL-03 | Closed serde enums replace machine-facing status/class strings | `src/output/result_types/` | Unknown enum/nested-field rejection and round trips | N/A | `f7a930f`, `adda397` |
| MODEL-04 | Pooled, Separate, Both, and Auto have distinct plans and recorded reasons | `src/api/components.rs`, result config/types | All six mandatory component-mode contracts | Separate avoids pooled work; no isolated timing claim | `b56cc60` |
| OUT-01 | Result and timing sidecar derive from one authoritative telemetry vector | `src/output/manifest.rs`, `src/output/writer.rs` | Exact result/sidecar/trace equality | Output time reported separately | `756ecbc`, `3d8ad46`, `99f6d78` |
| OUT-02 | Marked and multimodal pre/post use versioned 0.3 envelopes; safe 0.2 subset converts | `src/output/document.rs`, `migrate_v02.rs`, CLI pre/post | Round trip, file/directory equality, converter/rejection | N/A | `12d7c4c`, `a2f7132` |
| OUT-03 | Same-filesystem staged output validates then atomically renames | `src/output/transaction.rs`, `artifact_plan.rs`, `writer.rs` | Failure cleanup, existing target preservation, manifest | 0.874/0.859/0.939 ms; 12.27 MiB RSS | `e9e87b0`, `99f6d78` |
| OUT-04 | Explicit filtered canonical Parquet export preserves absence | `src/io/parquet/pattern_writer.rs` | Optional absence, supported fields, invalid metrics | Loader/export results recorded | `f4243cd` |
| OUT-05 | One logical cell schema prevents CSV/Parquet semantic drift | `src/io/row.rs`, adapters, builder | Complete Pattern equality and parity contracts | CSV/Parquet improvements recorded | `f4243cd` |
| OUT-06 | Batch IDs are validated single components contained by output root | `src/cli/batch.rs`, shared batch path resolver | Traversal, absolute, separator, symlink, valid batch | N/A | `a8d38c5` |

## Architecture before and after

Before remediation, domain values opened files, the marked analysis was a
distributed coordinator coupled through parent wildcard imports, spectrum and
output were god files, and the multimodal CLI repeated transform fitting,
graph construction, inference, geometry, and diagnostics after the library
returned an incomplete result. CSV and Parquet owned parallel scientific
filtering state machines. Writers calculated or mutated result policy while
emitting non-transactional files.

The final dependency direction is:

```text
common data, finite/statistical/geometry primitives
    -> marked and multimodal domain algorithms
    -> marked and multimodal application runs
    -> input/output/WSI adapters
    -> CLI commands
```

The key ownership boundaries are:

- `AnalysisEngine` returns a `MarkedAnalysisRun` with its result, telemetry,
  reusable context, and artifact data.
- `MultimodalEngine` returns one `MultimodalAnalysisRun` containing the fitted
  transform, shared index/graph, all configured null analyses, diagnostics,
  telemetry, and artifact projections.
- `Pattern` is validated data. `PatternLoader`, decoded rows, and
  `PatternBuilder` own filesystem and ingestion behavior.
- Result-document semantics, family DTOs, artifact planning, atomic
  transactions, manifests, and physical projections have distinct output
  owners.
- Marked and multimodal pre/post services are separate; only tolerant axes,
  scalar diagnostics, typed errors, and identical territory-summary semantics
  are shared.

The principal god-file reduction and dependency review is recorded in
[CODE_SHAPE_AUDIT.md](CODE_SHAPE_AUDIT.md). No refactored production module
uses `use super::*`, and no task/MVP scaffolding comments remain in production.

## Scientific changes

- `Rigid` now means a determinant-positive rotation plus translation with no
  scale. Affine remains distinct; the unused scale-translation implementation
  was deleted.
- The former MODWT/wavelet/DoG names were removed. The retained method is
  documented as multiscale neighborhood residual and scale-energy analysis.
- The former Bartlett name was removed. The implemented diagnostic is a
  Hann-tapered raster periodogram with actual radial-shell grouping.
- “Pair correlation” was renamed to centered mark-pair covariance because it
  is not a density-normalized point-process pair-correlation function.
- “Equivalence” and generic “curve test” claims were narrowed to descriptive
  margin assessment and pooled-bin non-spatial diagnostics.
- The beta-binomial name was replaced by fixed-prior beta posterior group
  summaries; no shared overdispersion model is claimed.
- Generic marked outputs use neutral spatial classes and prose. MMR language is
  confined to explicit multimodal/MMR presentation surfaces.
- Stratified confounding now compares distinct nulls and persists both
  inferences. Degenerate strata are typed, not converted to a p-value or zero.
- Validation judgments come exclusively from production results. Smoke and
  formal calibration are separate workflows and claims.

## Performance

All values below use the same Apple M4 Pro host, Rust 1.96.0, release profile,
and reproducible workload definitions documented in
[PERFORMANCE_FINAL.md](PERFORMANCE_FINAL.md).

- nearest-neighbor time improved 47–79%; kNN graph time improved 85–95%; radius
  graph time improved up to 54%;
- fixed-density indexed radius workloads had adjacent doubling ratios below
  4×, with output edge/membership counts recorded;
- observed plus 19 null evaluations improved 26–72% for mark-pair covariance
  and 69–84% for residual territories because geometry is built once;
- spectral permutation retention fell from 6,361,632 to 63,936 bytes at 999
  permutations (99.0%); measured binary RSS fell 46.1%;
- the million-row CSV load completed in 2.569–2.613 seconds at 430.13 MiB peak
  RSS, with streamed fixture generation and separate decode/nearest timings;
- transactional output for the measured marked run took
  0.874/0.859/0.939 ms at 64/128/256 cells and 12.27 MiB peak RSS;
- no equivalent default workload regressed by more than 20%. The final
  multimodal run is 11–35% slower than the incomplete baseline because it now
  performs every configured analysis once for library and CLI users; peak RSS
  increased 7.5%.

Post-report Phase 13 changes affect result migration, fuzz preflight, CI,
documentation, and a locked WSI transitive patch. DUP-09 centralized names and
removed endpoint-specific scale floors but did not change the index, plan, or
spectral storage hot loops; the benchmark fixtures already supplied a positive
analysis effective length. The recorded performance conclusions therefore
remain applicable to the final source.

Remaining performance cliffs are explicit: broad-radius/high-density plans are
output-sensitive and may be rejected by the configured geometry budget, and a
tight spectral chunk trades 20–41% CPU for lower scratch memory.

## Result-format migration

Format 0.3 is intentionally breaking and rejects unknown fields. Major changes
include:

- nullable/typed enrichment ratio and z-score states;
- optional empty-bin covariance and typed comparison availability;
- distinct marked, multimodal, marked-pre/post, and multimodal/pre-post
  variants;
- distinct residual and neighborhood territories;
- `analysis_effective_length_um` instead of `l_eff_um`;
- multiscale-residual, scale-energy, tapered-periodogram,
  mark-pair-covariance, descriptive-margin, pooled-bin-diagnostic, and
  beta-posterior names;
- removal of marked multimodal placeholders, compatibility aliases,
  `p_equivalence`, `z_or_power`, empty future profile vectors, and constant QC
  overlap fields;
- closed machine-facing enums and consistent analysis availability states.

`ResultDocument::from_json` converts only the unambiguous 0.2 marked subset. It
renames safe fields, converts zero-pair bins to unavailable covariance, removes
only empty placeholders, and infers the component selection when evidence is
unambiguous. It rejects 0.2 multimodal documents, populated legacy
placeholders/comparison data, unsupported nulls, malformed shapes, and unknown
fields. Such inputs must be rerun rather than guessed into a scientifically
different result. See [migration-0.2-to-0.3.md](../migration-0.2-to-0.3.md) and
[result-format-0.3.md](../result-format-0.3.md).

## Final verification commands

All commands below ran in `/Users/user/Bench/marklab-refactor` against source
SHA `9a11e11d06a64f3f479564082d5f82186c450980` and exited 0 unless a limitation
is stated.

| Command | Actual result |
| --- | --- |
| `cargo +1.96.0 fmt --all --check` | Exit 0; no formatting diff |
| `cargo +1.96.0 clippy --locked --all-targets --all-features -- -D warnings` | Exit 0 |
| `cargo +1.96.0 nextest run --locked --all-features` | 380 passed; 21 intentional skips; 1 slow production smoke |
| `cargo +1.96.0 test --locked --all-features` | 273 library tests passed, 20 ignored; every integration passed; WSI 10 passed/1 external ignored; 0 doctests |
| `cargo +1.96.0 test --locked --doc --all-features` | Exit 0; 0 doctests defined |
| `cargo +1.96.0 check --locked --no-default-features` | Exit 0 |
| `cargo +1.96.0 test --locked --features wsi,cli --test wsi_integration` | 10 passed; 1 checksummed public Aperio/OpenSlide oracle ignored locally |
| `cargo +1.96.0 test --locked --all-features --test cli` | 16 passed |
| `cargo +1.96.0 test --locked --all-features --test multimodal_cli` | 21 passed |
| `cargo +1.96.0 test --locked --all-features --examples` | Exit 0; both example targets compiled |
| `cargo audit` | Exit 0; no known vulnerability; 2 allowed unmaintained-package warnings |
| `cargo deny check advisories licenses bans sources` | Exit 0; policy warnings only for reviewed transitive duplicate versions |
| `cargo machete` | Exit 0; no unused dependencies |
| `cargo +nightly fuzz check` | Exit 0; all five targets compiled |
| `cargo +1.96.0 package --locked` | Exit 0; 235 files, 1.6 MiB unpacked/376.0 KiB compressed; archive verification compiled |
| `cargo +1.96.0 run --release --locked --all-features --bin marklab -- --help` | Exit 0; all command families listed |
| `target/release/marklab smoke --suite synthetic --replicates 1 --out <temporary-run>` | Exit 0; completed summary, zero failed replicates; temporary output moved to Trash afterward |
| `cargo +1.96.0 test --release --locked --features dhat-heap --lib dhat_ -- --nocapture --test-threads=1` | Exit 0; all three allocation contracts passed during Phase 12 |

Dependency-policy warnings are not hidden:

- `cargo audit` allows `encoding 0.2.33` (`RUSTSEC-2021-0153`) through optional
  DICOM WSI parsing and `paste 1.0.15` (`RUSTSEC-2024-0436`) through Parquet and
  statistics dependencies. Both are maintenance advisories, not reported
  vulnerabilities, and both have documented upstream removal plans.
- Cargo Deny reports duplicate transitive `getrandom`, `hashbrown`, `r-efi`,
  `thiserror`, `thiserror-impl`, and `wit-bindgen` versions while still passing
  advisories, licenses, bans, and sources.

## Unresolved issues and remaining risk

There are no undocumented open critical findings in the findings matrix. The
remaining limitations are:

1. Formal calibration currently covers one deterministic geometry,
   prevalence, null-model, and seed family for each 1,000-replicate negative
   control. The scheduled workflow prevents smoke results from being presented
   as broad calibration, but broader calibration remains future scientific
   validation work.
2. The independent public Aperio/OpenSlide oracle requires a checksummed
   external fixture and was not available locally. Ten local WSI fixture/oracle
   tests pass; the external case remains in scheduled CI.
3. Two transitive packages are unmaintained as described above. No patched
   dependency path is currently available without changing upstream WSI,
   Parquet, or statistics dependencies.
4. Format 0.2 multimodal and ambiguous marked documents cannot be converted
   honestly. The converter rejects them and requires rerunning the analysis.
5. Very broad spatial radii or high-density plans can exceed the configured
   memory budget. Marklab returns a contextual error instead of exhausting
   memory; no streaming fallback exists for those deliberately rejected
   configurations.
6. The crate currently defines no Rust doctests. The documented Rust example
   target compiles and all public API contracts are integration-tested, but
   inline documentation examples do not add an independent executable layer.

These risks do not change the closure status of any registered finding and are
not downgraded or concealed by the green verification suite.
