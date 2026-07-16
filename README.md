# Marklab

`marklab` is a Rust library and CLI for section-level spatial analysis of marked
cell patterns in pathology. It reports organization, dispersion, anisotropy,
wavelet diagnostics, and descriptive pre/post differences relative to
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

Result documents use format 0.2:

```json
{
  "format_version": "0.2",
  "provenance": {},
  "analysis": {
    "kind": "marked_pattern",
    "result": {}
  }
}
```

Format 0.1 inputs are rejected by `prepost`; re-run the original inputs.

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
and WSI contracts. Reviewed dependency exceptions are recorded in
[docs/dependency_advisories.md](docs/dependency_advisories.md).

Licensed under MIT or Apache-2.0.
