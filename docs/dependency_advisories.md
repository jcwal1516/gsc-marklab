# Reviewed dependency advisories

Review date: 2026-07-15. Owner: marklab maintainers. These exceptions expire on
2027-01-15 and must be removed or renewed after reviewing the then-current resolved graph.

| Advisory | Dependency path | Impact | Mitigation |
|---|---|---|---|
| RUSTSEC-2021-0153 (`encoding` 0.2.33, unmaintained) | `marklab -> wsi-rs 0.5.0 -> dicom-* 0.9.1 -> encoding` | Maintenance risk in optional DICOM WSI parsing; the advisory does not report a vulnerability. | WSI is opt-in, all slide inputs are treated as untrusted, reads are bounded before decode, corrupt inputs return errors, and the exception is limited to the exact reviewed `wsi-rs` graph. Track an upstream DICOM replacement or update. |
| RUSTSEC-2024-0436 (`paste` 1.0.15, unmaintained) | `marklab -> parquet 56.2.1 -> paste`; `marklab -> statrs 0.18.0 -> nalgebra 0.33.3 -> simba 0.9.1 -> paste` | Maintenance risk in a compile-time macro crate; the advisory reports no runtime vulnerability and offers no safe upgrade. | No direct API use. Keep Parquet and statistics dependencies patched, and remove the exception when both upstream paths stop resolving `paste`. |

`RUSTSEC-2026-0190` was resolved by updating transitive `anyhow` from 1.0.102 to
1.0.103. Production code does not depend directly on `anyhow`.
