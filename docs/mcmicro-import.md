# MCMICRO quantification import

Marklab's Python client imports one MCMICRO single-cell quantification table: the
per-sample CSV that MCQuant writes under `quantification/`. Each row is one segmented cell:

- `CellID`, the label in the segmentation mask, kept inside Marklab's slide-qualified cell
  identity;
- one mean-intensity column per marker;
- `X_centroid` (column) and `Y_centroid` (row) in mask pixels;
- the morphology columns `Area`, `MajorAxisLength`, `MinorAxisLength`, `Eccentricity`,
  `Solidity`, `Extent` and `Orientation`.

```python
from upstream_formats import mcmicro_csv_slide

slide = mcmicro_csv_slide(
    "quantification/sample--unmicst_cell.csv",
    slide_id="sample",
    patient_id="patient-1",
    group="group-a",
    coordinate_frame_id="sample-local-um",
    window=window_in_local_micrometres,
    channels=channels,
    coordinate_scale_um=(x_um_per_pixel, y_um_per_pixel),
    channel_columns={"CD3": "CD3", "Ki67": "Antigen Ki67"},
)
```

The table carries no physical calibration, so the adapter requires both micrometres-per-pixel
values and a window in the same local micrometre frame. Read them from the segmentation
mask's or registered image's OME-XML `PhysicalSizeX` and `PhysicalSizeY`; the adapter never
infers them. Only continuous marker channels are admitted, and the identity, centroid and
morphology columns cannot be declared as channels. One sample per call, at most 100,000 cells
by default and at most 64 MiB per CSV. A missing, empty or duplicated `CellID` fails rather
than dropping a cell; empty marker fields become missing values and zero stays observed.

## Validation on public MCMICRO output

[`validate_mcmicro.py`](../examples/public-cohorts/validate_mcmicro.py) downloads, into a new
directory outside the checkout, the tonsil CyCIF test files of
[nf-core/test-datasets](https://github.com/nf-core/test-datasets) at commit `58806b1` (MIT
licence; tissue from Schapiro et al. 2021): the MCQuant table `cycif_tonsil_cell.csv`
(1,621 cells) and its segmentation mask `cell.ome.tif`, written by s3segmenter v1.3.11. Checks
fixed before the first run:

1. **Mask link:** the mask's label set equals the `CellID` set, every `Area` equals its label's
   pixel count, and every centroid equals the mean row and column of its label's pixels. This
   shows the table was measured on this mask.
2. **Calibration:** the scale is the mask's OME-XML physical size.
3. **Adapter:** it returns the IDs, the scaled centroids and every marker value exactly as an
   independent reading of the CSV.

Result: all checks passed, with centroid error 0.0 px and a scale of 0.65 µm per pixel.

```bash
target/spatial-reference-venv/bin/python -I examples/public-cohorts/validate_mcmicro.py \
  /absolute/new/path/mcmicro-raw /absolute/new/path/results
```

**Limits of this evidence:**

- The files were produced by MCMICRO's component tools (ASHLAR, s3segmenter, MCQuant), not
  by one documented MCMICRO pipeline run, and the MCQuant version is not recorded.
- The 0.65 µm pixel size is written by the test-data build script, not read from instrument
  files. It agrees with the instrument metadata of MCMICRO's exemplar-001 (0.65 µm), which
  has no stated data licence and so was not used.
- The tile is a semi-synthetic test layout, not a cohort, so no biological claim follows.
- Legacy MCMICRO tables with other centroid column names are not admitted.
