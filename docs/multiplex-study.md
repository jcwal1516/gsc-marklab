# Multiplex studies

`marklab study` takes a multiplex panel, with several markers measured on every cell, across
many slides and patients, and produces a patient-level report.

For each marker and slide it measures spatial autocorrelation with Moran's I and Geary's C,
which ask whether cells with high values sit next to other cells with high values. It
averages the slides of each patient, so every patient counts once, then tests every marker
across patients with a Max-T correction for the number of markers tested. A run saves its
progress and can resume after an interruption.

This workflow is experimental: it is tested on synthetic data but not yet validated on real
studies. It runs entirely in Rust; no Python environment is needed.

## Run the example

The example is synthetic: six patients with two slides each, two numeric markers, a yes/no
marker that is missing for some cells, and a categorical label. It contains no patient
data.

```sh
cargo +1.99.0 build --locked --package marklab --features cli --bin marklab
target/debug/marklab study describe
target/debug/marklab study run \
  --recipe examples/multiplex-study/recipe.json \
  --project target/multiplex-example-project \
  --out target/multiplex-example-report
```

From a release download, skip the build and run `./marklab` (`marklab.exe` on Windows) with
the same arguments.

The output directory must be new or empty and receives `result.json`, `report.md` and a
manifest listing the files. An existing non-empty directory is never overwritten.

The `--project` directory remembers finished work: the inputs, the design, cached slide
results and a log of every run. To stop after the first three slides, add
`--through-slides 3` (no report is written at that point); run the same command again
without it to finish. On a re-run, unchanged slides and patient results are reused, and a
changed slide is recomputed together with everything that depends on it. Use a new output
directory each time you repeat a complete run.

## Recipe

The recipe (version 1) has `format`, `version`, `study_id`, `channels`, `design`, `slides`
and optional `limits`. Unknown fields are errors. The example recipe is a working template.

**Channels** are the markers. Each has a stable ID, a label, a kind, its unit, whether it
was measured or predicted (`measured`, imported prediction or morphology prediction), and
how it was acquired and processed.

- Numeric channels hold finite numbers.
- Yes/no channels hold `0` or `1`, with unit `unitless`.
- Categorical channels hold integer indices into an ordered list of category names, with
  unit `categorical`. They are kept in the data but cannot be tested with Moran's I or
  Geary's C.
- `null` means not measured. It is never treated as zero. Every slide lists every channel,
  with explicit `null` values where data is missing.

**Slides** each name their patient and group, coordinate frame, observation window (a
GeoJSON MultiPolygon, holes and separate pieces allowed), cell IDs, cell X/Y positions in
micrometres, and the value of each channel for each cell. Cell IDs must be unique across
the whole study, so prefix IDs from different slides with the slide name. Marklab never
guesses scale, alignment, cell type, patient or sampling unit from a file name or from the
coordinates.

**Design** fixes, before the run: which channels to test, the neighbor radius, the neighbor
weights (binary or row-standardized), the seed, the number of permutations and the
family-wise significance level. Each channel uses the cells that have a value for it. Each
patient's value is the mean of their slides, and patients are shuffled between groups for
the test. Each group needs at least two patients. Markers measured on the same cells are
not independent samples.

## How the statistics work

For each channel on each slide, Moran's I and Geary's C use the same neighbor graph built
from the cells that have that marker. Missing values change the graph, and so change what
is being measured.

A slide result is unavailable if there are fewer than three observed cells, an observed
cell has no neighbors, all values are the same, or the calculation fails. If a slide result
needed for the patient test is unavailable, the patient test for that marker does not run;
no slide, patient or marker is dropped silently. If the patient-level Max-T test fails, its
diagnostics are kept and no replicate counts are invented. The report states how strong a
conclusion the results support, and calls the workflow experimental even when every test
succeeds.

## Limits

A recipe can be at most 16 MiB, with up to 64 declared channels, 32 tested channels and
1,024 slides. Defaults: 100,000 cells per slide, 200,000 cells in total, one million stored
neighbor edges, 50 million edge evaluations for the whole study, and 512 MiB of estimated
memory. The `limits` section can lower or raise these within fixed maximums. They protect
the machine; they are not measured memory use or proof that whole slides fit.

## Testing

Tests compare Moran's I and Geary's C with independent calculations on small graphs and
cover the patient-level test, missing data across several channels, suppression of
conclusions when a slide is missing, ID, unit and limit errors, studies with 65 slides,
recomputation after a slide changes, resuming in a new process, and all-or-nothing output.
Nothing here shows biological or clinical validity.

Other label types, ROI weighting, covariates, spatial permutation nulls and custom graphs
are available in other Marklab workflows or not yet supported.

For a larger synthetic workload, run `examples/multiplex-study/generate.py` with `--rows`
and `--columns`. Its `--verify path/to/result.json` option checks the edge count, the
number of patients and slides, and that the result is available before you trust a timing.
A fresh run and a resumed run must give identical scientific JSON. Record the build
profile, hardware, repeated timings and memory, and do not extrapolate from a regular grid
to irregular whole-slide tissue. [Measured workloads](multiplex-study-measurements.md)
reports runs at 72 and 60,000 cells.

## Python and AnnData

Add `clients/python` to your Python path and import the module:

```python
from pathlib import Path
from marklab_client import anndata_slide, run_study

# channels and design are the same declarations used in the JSON recipe.
slide = anndata_slide(
    adata, slide_id="slide-01", patient_id="patient-01", group="reference",
    coordinate_frame_id="slide-01-um", coordinate_unit="micrometer",
    spatial_key="spatial_um", window=window_geojson, channels=channels,
)
# Collect every slide for every patient and fix the design before running.
recipe = {"format": "marklab.multiplex_study_recipe", "version": 1,
          "study_id": "my-study", "channels": channels,
          "design": design, "slides": all_slides}
result = run_study(recipe, project=Path("study-project"), out=Path("study-report"))
print(result["maturity"], result["inference"]["status"])
```

The module uses only the Python standard library and calls the `marklab` program.
`anndata_slide` reads numeric channels from the named `var_names` columns of `X` (or a layer
you choose) and yes/no or categorical channels from `obs` columns with the same name. It
keeps cells aligned, preserves missing values, category names, measurement details, the
window and physical coordinates, prefixes cell IDs with the slide ID, and sorts all arrays
together. It checks the size limit before calling `to_df`, which
[makes sparse data dense and drops annotations](https://anndata.readthedocs.io/en/stable/generated/anndata.AnnData.to_df.html),
and carries the annotations separately without changing your AnnData object.

Tested with AnnData 0.12.4 on real H5AD files with sparse matrices. Backed, lazy or dask
arrays, SpatialData transformations, OME-NGFF and R are not supported. Load a region of
interest into memory, with coordinates already in micrometres. To run the client's own
tests from a source checkout:

```sh
UV_PROJECT_ENVIRONMENT="$PWD/target/client-venv" \
  uv sync --project clients/python --locked --group test --python 3.12
target/client-venv/bin/python -m unittest discover -s clients/python/tests -p 'test_*.py'
```
