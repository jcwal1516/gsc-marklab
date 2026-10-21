# QuPath PUMA nuclei import

`qupath_nuclei_slide` imports one bounded QuPath GeoJSON `FeatureCollection` of
classified nuclear annotations into a Marklab slide. It preserves each GeoJSON
feature `id`, computes the nucleus point as the polygon's area centroid, and maps
`properties.classification.name` through a caller-declared categorical codebook.

```python
from upstream_formats import qupath_nuclei_slide

slide = qupath_nuclei_slide(
    "training_set_metastatic_roi_008_nuclei.geojson",
    slide_id="training_set_metastatic_roi_008",
    patient_id="caller-known-patient-id",
    group="caller-known-study-group",
    coordinate_frame_id="roi-008-local-um",
    window={
        "type": "MultiPolygon",
        "coordinates": [[[[0, 0], [235.52, 0], [235.52, 235.52],
                          [0, 235.52], [0, 0]]]],
    },
    coordinate_scale_um=(0.23, 0.23),
    classification_channel={
        "id": "nucleus_class",
        "label": "QuPath nucleus classification",
        "kind": "categorical",
        "levels": ["nuclei_apoptosis", "nuclei_lymphocyte", "nuclei_tumor"],
        "measurement_status": "classified",
        "provenance": "PUMA v5 QuPath classification.name",
    },
)
```

The caller must supply the patient, group, coordinate frame, physical observation
window, micrometres per source coordinate unit on each axis, and complete ordered
classification codebook. The importer does not infer patient identity or calibration
from filenames. The ROI name is not a patient identity. Source IDs become Marklab's
lossless slide-qualified cell IDs.

The admitted profile is intentionally narrow and fail-closed:

- `objectType` must be `annotation`, with only QuPath classification properties;
- geometry must be a valid, positive-area `Polygon` with one closed exterior ring;
- holes, `MultiPolygon`, detections, cells, measurements, object graphs, missing
  classifications, non-finite coordinates, and self-intersections are rejected;
- input bytes are bounded before JSON decoding, and nucleus/vertex counts are checked
  before their geometric processing; duplicate JSON fields are rejected; and
- Shapely 2.1.2 supplies validated polygon geometry and area centroids through the
  `upstream-formats` optional dependency.

This is a classification-only PUMA-compatible nuclei annotation profile. It is not a
generic QuPath detection/cell measurement reader. That broader option was deferred
because no small reviewed biological fixture exercised both general object geometry
and measurement columns. A synthetic-only combined reader was also rejected because
it would not establish interoperability with a real exported biological dataset.

## Biological fixture validation

[`validate_qupath_puma.py`](../examples/public-cohorts/validate_qupath_puma.py)
uses one real biological ROI from the CC0 PUMA v5 dataset, “Melanoma Histopathology
Dataset with Tissue and Nuclei Annotations.” PUMA contains diagnostic melanoma
specimens with nuclei polygons adjusted in QuPath. The selected 1024 × 1024 ROI has
168 simple polygon annotations at the published 0.23 µm/pixel calibration: 141 tumor,
22 apoptosis, and 5 lymphocyte nuclei.

The validator downloads only the versioned 15,975,301-byte nuclei GeoJSON archive,
selects `training_set_metastatic_roi_008_nuclei.geojson`, and checks exact feature IDs
and classes. An independent signed-shoelace calculation checks every area centroid
within 1e-9 µm; it does not reuse Shapely or the adapter's centroid calculation.
The validator uses neutral format-validation patient/group values because the selected
ROI proves file interoperability rather than a patient-level scientific analysis.

Install the declared optional geometry dependency into the existing client environment:

```bash
VIRTUAL_ENV="$PWD/target/client-venv" \
  uv sync --project clients/python --locked --group test --extra upstream-formats --active
```

Run a new bounded download and validation outside the checkout:

```bash
target/client-venv/bin/python examples/public-cohorts/validate_qupath_puma.py \
  /absolute/new/path/puma-raw \
  /absolute/new/path/puma-validation
```

Reuse an already preserved archive without modifying it:

```bash
target/client-venv/bin/python examples/public-cohorts/validate_qupath_puma.py \
  /absolute/existing/path/puma-raw \
  /absolute/new/path/puma-validation-reuse \
  --reuse-source
```

Primary source: [PUMA v5 Zenodo record](https://zenodo.org/records/15050523), DOI
[`10.5281/zenodo.15050523`](https://doi.org/10.5281/zenodo.15050523). The record
declares CC0-1.0 and identifies the nuclei archive used here. QuPath documents
GeoJSON object export in its [annotation measurement and export
guide](https://qupath.readthedocs.io/en/stable/docs/advanced/exporting_annotations.html).
