# Multiplex panels to patient-level spatial inference

This experimental workflow connects the shared mark table, exact spatial geometry, existing Moran
and Geary engines, patient Max-T inference, durable execution and a claim-bounded report. It is a
bounded research profile, not evidence of clinical validity or universal whole-slide capacity.

## Run the complete example

The shipped fixture is synthetic: six patients, two slides per patient, two quantitative channels,
a nullable binary phenotype and nullable categorical labels. No patient data is included.

```sh
cargo +1.96.0 build --locked --package marklab --features cli --bin marklab
target/debug/marklab study describe
target/debug/marklab study run \
  --recipe examples/multiplex-study/recipe.json \
  --project target/multiplex-example-project \
  --out target/multiplex-example-report
```

These build commands use a source checkout. In an extracted release bundle, skip the Cargo build
and use `./marklab` (`marklab.exe` on Windows) with the same study arguments.

The new/empty output directory receives `result.json`, `report.md` and a content manifest. A
nonempty destination is never overwritten. The durable project retains exact source/design/execution
identities, cached outputs and an append-only execution ledger. This workflow uses native Rust;
it does not require a Python fitting environment.

To stop after three canonically ordered slides, add `--through-slides 3`. No report is published
at that checkpoint. Run the same recipe/project again without that option to finish. Use a new
output directory when repeating a complete run. Unchanged slide calculations and patient inference
are restored; changing one slide invalidates that slide and its dependent inference. Collections
of up to 64 slide dependencies support the profile's larger studies without raising the existing
durable-record input limit.

## Data and statistical contract

The strict version-1 recipe has `format`, `version`, `study_id`, `channels`, `design`, `slides`, and
optional `limits`. Unknown fields fail. The example JSON is an executable template.

Each channel declares a stable ID, label, kind, exact assay unit, measured/imported/morphology
prediction status, and acquisition/processing provenance. Continuous values are finite signed
numbers; binary observations are `0`/`1` in `unitless` units; categorical observations are integer
indices into an explicit ordered label codebook with unit `categorical`. `null` is unavailable,
never a zero. Every slide carries all declared channels, including explicit nulls for absent data.
Nominal categories are preserved but cannot be selected as numeric Moran/Geary endpoints.

Each slide declares its patient and group, physical coordinate frame, exact GeoJSON MultiPolygon
window (including supported holes/components), canonical unique Cell IDs, row-aligned XY
coordinates in micrometres, and named observation arrays. Cell IDs must be unique across the
whole study. Qualify source-local IDs by slide; do not accidentally identify two different cells
as the same object. No scale, registration, phenotype, patient identity or independent sampling
unit is inferred from a filename or coordinate range.

The design freezes selected channels, neighbour radius, binary-symmetric or row-standardized
weights, per-channel complete-case selection, equal-slide patient means, independent-patient group
exchangeability, seed, permutations and family-wise alpha. At least two declared independent
patients per group are required. Channel observations are not biological replicates.

Both statistics use the same observed-cell graph for each channel. Missingness can change that
graph and therefore the estimand. Fewer than three observations, an isolated observed point,
constant values or numerical failure produce an explicit unavailable slide endpoint. A required
unavailable endpoint blocks complete-family patient inference: no slide, patient or endpoint is
silently omitted. Patient Max-T failures retain diagnostics without fabricated replicate counts.
The report derives its claim ceiling from these outcomes and remains experimental even on success.

## Bounds and evidence

The profile admits at most 16 MiB of recipe JSON, 64 declared/32 selected channels and 1,024 slides.
Default ceilings are 100,000 cells per slide, 200,000 total cells, one million retained directed
edges, 50 million complete-study statistic edge evaluations and a 512 MiB conservative retained-work
memory estimate. Explicit limits may be lowered or raised within the fixed admission ceilings.
These are admission limits, not measured peak RSS or validated whole-slide fitting capacity.

Coverage includes independent small-graph Moran/Geary oracles, canonical patient-inference parity,
nullable/multichannel data, missing-slide claim suppression, identity/unit/resource errors,
65-slide dependency fan-in, changed-source invalidation, process-boundary replay, and atomic output.
No external biological calibration or clinical/causal promotion is implied. Other mark families,
ROI weighting, covariates, spatial null permutations and arbitrary graph recipes remain separate
existing methods or future caller-driven extensions.

For a larger reproducible synthetic workload, use `examples/multiplex-study/generate.py` with
explicit `--rows` and `--columns`. Its `--verify path/to/result.json` mode checks the exact grid-edge
count, patient/slide denominators and available result before a timing is accepted. Cold and replay
runs must produce identical scientific JSON. Record build profile, hardware, repeated timings and
RSS; do not extrapolate a small grid to irregular whole-slide tissue or a different estimator.
The [executed workload record](multiplex-study-measurements.md) reports cold/replay checks at
72 and 60,000 cells, with all samples, RSS, input hashes and measurement limitations.

## Python and AnnData

Add `clients/python` to your Python import path and import the standalone module:

```python
from pathlib import Path
from marklab_client import anndata_slide, run_study

# channels and design are the same explicit declarations used in the JSON recipe.
slide = anndata_slide(
    adata, slide_id="slide-01", patient_id="patient-01", group="reference",
    coordinate_frame_id="slide-01-um", coordinate_unit="micrometer",
    spatial_key="spatial_um", window=window_geojson, channels=channels,
)
# Assemble every independently identified patient/slide and freeze the design before running.
recipe = {"format": "marklab.multiplex_study_recipe", "version": 1,
          "study_id": "my-study", "channels": channels,
          "design": design, "slides": all_slides}
result = run_study(recipe, project=Path("study-project"), out=Path("study-report"))
print(result["maturity"], result["inference"]["status"])
```

The module uses only the standard library and calls the native service. `anndata_slide` takes
continuous channels from named `var_names` in X (or an explicitly selected layer) and binary or
categorical channels from same-named `obs` columns. It preserves observation alignment, missingness,
category labels, measurement declarations, exact geometry and physical coordinates. Source Cell IDs
are losslessly slide-qualified and all arrays are sorted together. It checks the matrix bound
before calling `to_df`, which [densifies sparse input and drops annotations](https://anndata.readthedocs.io/en/stable/generated/anndata.AnnData.to_df.html).
The adapter carries annotations separately and does not change the source AnnData.

The real H5AD/sparse-matrix profile is tested with AnnData 0.12.4. Backed/lazy/dask arrays,
SpatialData transformations, OME-NGFF, R bindings and zero-copy transport are not claimed. Supply a
bounded in-memory ROI and already calibrated physical coordinates. The separately locked client
**test** environment can be reproduced from a source checkout without changing the scientific workers:

```sh
UV_PROJECT_ENVIRONMENT="$PWD/target/client-venv" \
  uv sync --project clients/python --locked --group test --python 3.12
target/client-venv/bin/python -m unittest discover -s clients/python/tests -p 'test_*.py'
```
