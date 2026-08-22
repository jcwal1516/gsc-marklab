# Marklab

`marklab` is a Rust library and CLI for section-level spatial analysis of marked
cell patterns in pathology. It reports organization, dispersion, anisotropy,
multiscale residual diagnostics, and descriptive pre/post differences relative to
fixed-position random labeling. The current multimodal workflow has first-class
MMR-IHC inputs, but the core marked-pattern analysis is not tied to one marker.

It does not prove clonality, track the same cells between sections, infer MMR
gain or loss, perform segmentation, or determine molecular MMR status.

## Requirements

- Rust 1.96
- The committed lockfile for official CLI and release-archive builds
- Optional `wsi` feature for slide inspection and bounded RGBA region extraction

## Build and test

```bash
cargo +1.96.0 build --locked --features wsi
cargo +1.96.0 test --all-features
```

The default feature set includes the CLI, parallel execution, CSV, and Parquet.
WSI is intentionally default-off for library users. Official release binaries
enable it with `--features wsi --locked`.

## Analyze a marked pattern

```bash
marklab analyze \
  --cells cells.parquet \
  --mask tumor_mask.geojson \
  --config examples/config.toml \
  --out out/case_001_post
```

The supported Rust entry point is:

```rust
use marklab::{AnalysisConfig, AnalysisEngine, Pattern};

# fn run(pattern: Pattern) -> marklab::Result<()> {
let engine = AnalysisEngine::new(AnalysisConfig::default())?;
let result = engine.analyze_pattern(&pattern)?;
# let _ = result;
# Ok(())
# }
```

Result documents use format 0.3:

```json
{
  "format_version": "0.3",
  "provenance": {},
  "analysis": {
    "kind": "marked_pattern | multimodal | marked_prepost | multimodal_prepost",
    "result": {}
  }
}
```

Older result formats are rejected by `prepost` until the documented 0.2 to
0.3 migration path is completed.

Both pre/post commands accept either a `result.json` file or its containing
result directory. Their `prepost.json` output is itself a format 0.3 result
document with a distinct marked or multimodal comparison kind.

For multimodal serial-section analysis, `registration.transform = "rigid"`
fits a two-dimensional rotation and translation without scale or reflection.
Use `"affine"` only when scale or shear is part of the intended registration
model.

The multiscale residual diagnostic is a documented heuristic, not a wavelet or
Difference-of-Gaussians transform. It combines local neighbor-difference energy,
variance of 2x2 block means, and a residual share, then evaluates the resulting
three-point scale-energy curve under label permutations.

The optional raster spectral cross-check is a single Hann-tapered 2-D
periodogram, not a Bartlett averaged-periodogram estimator. It averages mode
power within physical radial-frequency shells before summarizing the requested
lowest shells.

`marklab smoke` runs synthetic-generator smoke checks. It is not a formal
calibration or validation suite: the current multimodal generators synthesize
scenario outcomes rather than exercising the production multimodal engine.
Their output is written as `smoke.json` and labeled accordingly until the
production-pipeline validation rewrite is complete.

## WSI commands

```bash
marklab inspect-slide slide.svs
marklab inspect-slide slide.svs --output metadata.json
marklab extract-region slide.svs \
  --scene 0 --series 0 --level 0 --z 0 --c 0 --t 0 \
  --x 0 --y 0 --width 1024 --height 1024 \
  --output region.png
```

Coordinates are unsigned, level-relative pixels. Extraction rejects invalid
indices, zero dimensions, overflow, out-of-bounds regions, unsupported non-U8
sample types, and requests above 16,777,216 pixels before decode. Output is
straight interleaved RGBA8. Existing PNGs require `--force` to overwrite.

## Contracts and non-goals

See [SPEC.md](SPEC.md) for the implemented configuration, inference, result,
and WSI contracts. Result changes are tracked in
[the 0.3 format reference](docs/result-format-0.3.md) and
[the 0.2 migration guide](docs/migration-0.2-to-0.3.md). Reviewed dependency exceptions are recorded in
[docs/dependency_advisories.md](docs/dependency_advisories.md).

Licensed under MIT or Apache-2.0.
