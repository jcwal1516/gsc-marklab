# Reviewed dependency advisories

Review date: 2026-08-22; the `wsi-rs` path was re-reviewed on 2026-10-30. Owner: marklab maintainers. These exceptions expire on
2027-01-15 and must be removed or renewed after reviewing the then-current resolved graph.

| Advisory | Dependency path | Impact | Mitigation |
|---|---|---|---|
| RUSTSEC-2021-0153 (`encoding` 0.2.33, unmaintained) | `marklab -> wsi-rs 0.9.0 -> dicom-* 0.9.1 -> encoding` | Maintenance risk in optional DICOM WSI parsing; the advisory does not report a vulnerability. | WSI is opt-in, all slide inputs are treated as untrusted, reads are bounded before decode, corrupt inputs return errors, and the exception is limited to the exact reviewed `wsi-rs` graph. Track an upstream DICOM replacement or update. |
| RUSTSEC-2024-0436 (`paste` 1.0.15, unmaintained) | `marklab -> statrs 0.18.0 -> nalgebra 0.33.3 -> simba 0.9.1 -> paste` | Maintenance risk in a compile-time macro crate; the advisory reports no runtime vulnerability and offers no safe upgrade. | No direct API use. Keep the statistics dependencies patched, and remove the exception when `simba` stops resolving `paste`. Parquet 60 no longer depends on `paste`. |

`RUSTSEC-2026-0190` was resolved by updating transitive `anyhow` from 1.0.102 to
1.0.103. Production code does not depend directly on `anyhow`.

`RUSTSEC-2026-0253` was resolved by updating the locked transitive `lru` version
from 0.18.1 to 0.18.2 without changing any manifest constraint.

`RUSTSEC-2026-0418` was resolved by updating the locked transitive `libz-sys` from 1.1.29 to
1.1.31. `RUSTSEC-2026-0391`, `-0398` and `-0403` were resolved by updating `wsi-rs` from 0.5.0
to 0.9.0, which moves the WSI stack to `zstd` 0.14.3 and drops `czi-rs`; no `zstd` 0.13 or
`zstd-safe` 7 remains. The dependency policy check now reports only the two maintenance warnings
above.
