# Phase 0 Baseline Verification

Baseline SHA: `a642fbcdd80b5baf784cd633b707dc0283a24d11`
Branch: `refactor/audit-remediation`
Recorded: 2026-08-21

| Command | Exit | Actual result |
| --- | ---: | --- |
| `cargo +1.96.0 fmt --check` | 0 | Passed with no output. |
| `cargo +1.96.0 clippy --locked --all-targets --all-features -- -D warnings` | 0 | Passed; all targets and features checked with warnings denied. |
| `cargo +1.96.0 nextest run --locked --all-features` | 0 | 249 tests passed; 1 test skipped. Nextest run ID `0c7cf003-55f6-4eb4-885b-3ad9194e24d5`; 58.766 s test execution. |
| `cargo +1.96.0 test --locked --all-features` | 0 | All unit, integration, and doc-test binaries passed. The WSI integration binary reported 10 passed and 1 ignored public-fixture test; doc tests reported 0 tests. |
| `cargo +1.96.0 test --locked --doc --all-features` | 0 | Passed; 0 documentation tests were present. |
| `cargo +1.96.0 check --locked --no-default-features` | 0 | Passed. |
| `cargo +1.96.0 test --locked --features wsi,cli --test wsi_integration` | 0 | 10 passed, 1 ignored. The ignored test requires the checksummed public Aperio SVS and independent OpenSlide oracle. |
| `cargo audit` | 0 | Scanned 330 dependencies and reported three allowed warnings: `RUSTSEC-2021-0153` (`encoding`, unmaintained), `RUSTSEC-2024-0436` (`paste`, unmaintained), and `RUSTSEC-2026-0253` (`lru 0.18.1`, unsound use-after-free in panic-unsafe `LruCache::pop()`). |
| `cargo deny check advisories licenses bans sources` | 0 | Advisories, bans, licenses, and sources passed. Warnings reported duplicate versions of `getrandom`, `hashbrown`, `r-efi`, `thiserror`, `thiserror-impl`, and `wit-bindgen`. |
| `cargo machete` | 0 | No unused dependencies found. |
| `cargo +nightly fuzz check` | 0 | All existing fuzz targets built successfully in release profile. |

## Baseline limitations

- The all-feature suites intentionally do not execute the public Aperio/OpenSlide oracle test locally.
- The project currently has no documentation tests, so the passing doc-test command provides build coverage only.
- Dependency-policy exit codes are green but do not mean warning-free. In particular, the new `lru` unsoundness advisory is not documented in `docs/dependency_advisories.md`.
- Benchmark and peak-memory baselines remain to be measured.

## RUSTSEC-2026-0253 reachability review

`cargo tree --locked --all-features -i lru` resolves `marklab -> wsi-rs 0.5.0 -> lru 0.18.1`. Source inspection found reachable `LruCache::pop()` calls in the DICOM, TIFF-family, Mirax, and Hamamatsu probe-to-open paths of `wsi-rs`. The advisory requires unwinding through a stored key's panicking `Drop` implementation and later cache use; the WSI probe-cache key is `FileIdentity`, composed only of `PathBuf`, integers, `Option<u128>`, and `bool`, and neither Marklab nor `wsi-rs` uses `catch_unwind` in these paths. The presently reviewed path therefore does not expose the advisory's panic trigger, but retaining an unsound transitive crate is a dependency risk. The patched version is `lru >= 0.18.2`; remediation must be evaluated without silently changing the lockfile during baseline capture.
