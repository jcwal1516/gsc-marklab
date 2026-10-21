# Steinbock object-table import

Marklab's Python client can import one bounded pair of Steinbock object-data CSV files:

- the per-image mean-intensity table from `intensities/`; and
- the matching per-image region-properties table from `regionprops/`, when centroid was
  selected during measurement.

Both files must contain the same unique, nonempty `Object` identities. The adapter preserves
that source identity inside Marklab's slide-qualified cell identity. It follows Steinbock's
scikit-image coordinate order: `centroid-0` is row/Y and `centroid-1` is column/X. The join is
exact; a missing, extra, or duplicated object fails instead of deleting a cell.

```python
from upstream_formats import steinbock_csv_slide

slide = steinbock_csv_slide(
    "steinbock/intensities/roi-1.csv",
    "steinbock/regionprops/roi-1.csv",
    slide_id="roi-1",
    patient_id="patient-1",
    group="group-a",
    coordinate_frame_id="roi-1-local-um",
    window=window_in_local_micrometres,
    channels=channels,
    coordinate_scale_um=(x_um_per_pixel, y_um_per_pixel),
    channel_columns={"CD3": "CD3", "CD8": "CD8a"},
)
```

Steinbock's general object CSV contract contains pixel coordinates, not a universal physical
calibration. The adapter therefore requires both micrometres-per-pixel values and a window in
the same local micrometre frame. It never guesses calibration from a filename, an image type,
or a nominal modality. For IMC data, Steinbock's `images.csv` may contain documented
`acquisition_width_um`, `acquisition_height_um`, `width_px`, and `height_px` metadata; callers
may validate and explicitly pass the two ratios. Anisotropic scales are retained.

The admitted profile is intentionally narrow. It accepts continuous selected intensity
channels, one ROI per call, at most 100,000 objects by default, and at most 64 MiB per CSV.
Empty marker fields become missing values and numeric zero stays observed. It does not import
Steinbock masks, precomputed neighbor graphs, combined multi-image indices, FCS, phenotype
labels, or morphology columns as assay channels. The existing AnnData adapter remains the path
for a caller-controlled, bounded AnnData object.

## Reproducible upstream-file validation

[`validate_steinbock.py`](../examples/public-cohorts/validate_steinbock.py) downloads the
official
[BodenmillerGroup TestData v1.0.7 release](https://github.com/BodenmillerGroup/TestData/releases/tag/v1.0.7)
into a new directory outside the checkout. The repository declares
[CC BY 4.0](https://github.com/BodenmillerGroup/TestData/blob/main/LICENSE). The validator
admits a maximum 24 MiB archive and extracts only `images.csv` plus one eight-object ROI's
intensity and regionprops tables. It preserves the archive and raw CSVs.

The TestData project's
[dataset README](https://github.com/BodenmillerGroup/TestData/blob/main/datasets/210308_ImcTestData/README.md)
identifies this source as an IMC mock dataset generated with CyTOF Software and processed with
the documented segmentation pipeline. These are genuine upstream Steinbock fixture files, but
they are not measurements from a biological cohort.

The validator uses the source image dimensions and IMC acquisition dimensions to calculate
both physical scales, independently calculates the expected IDs, X/Y coordinates, and marker
values, and compares them with the CSV adapter. It then writes a bounded H5AD derived from
those preserved mock-fixture rows, reads it with the project's locked AnnData 0.12.4
environment, and compares `anndata_slide` with the same oracle. The generated H5AD is
mock-fixture-derived validation output; it is not represented as biological data or an
upstream-provided H5AD.

```bash
target/client-venv/bin/python examples/public-cohorts/validate_steinbock.py \
  /absolute/new/path/steinbock-raw \
  /absolute/new/path/gsc-results
```

Later runs can reuse the preserved download while still writing each validation result to a
new directory:

```bash
target/client-venv/bin/python examples/public-cohorts/validate_steinbock.py \
  /absolute/existing/path/steinbock-raw \
  /absolute/new/path/gsc-results \
  --reuse-source
```

The upstream format definitions are documented in Steinbock's
[file-type specification](https://bodenmillergroup.github.io/steinbock/latest/file-types/)
and [export documentation](https://bodenmillergroup.github.io/steinbock/latest/cli/export/).

## Formats not covered by this adapter

- QuPath GeoJSON preserves geometry and classifications, but the reviewed official fixture is
  a bare polygon without cell measurements or physical calibration. Tissue-window GeoJSON is
  already supported separately.
- HoVer-Net documents nucleus IDs, centroids, contours, types, and probabilities under MIT,
  but no small official output fixture with physical calibration was identified. Its JSON
  coordinates remain pixels unless paired with independently reviewed image metadata.
- STHELAR now provides CC-BY-4.0 real CellViT-SAM-H embeddings in SpatialData. The bounded
  validation and the distinction from the fixed normalized bundle are documented in
  [CellViT embedding import](cellvit-import.md).
- MCMICRO's smallest official exemplar is 240 MiB and its maintained pipeline has moved to
  nf-core. A current, small, licensed quantification output is needed before fixing an adapter
  contract.
- Squidpy's small upstream H5AD test file has stable observation IDs, sparse values, and
  `obsm['spatial']`, but those coordinates are full-resolution image pixels and the file does
  not provide micrometres-per-pixel. Marklab does not relabel them as physical coordinates.
- SpatialData/OME-Zarr requires transform-aware coordinate-system handling. OME-XML and
  OME-TIFF can supply physical image calibration, but they do not themselves provide the
  cell-table caller needed by this adapter.
