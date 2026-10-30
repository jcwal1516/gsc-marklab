# Input and output formats

Physical units, cell identity and patient membership are part of the analysis input.
Marklab does not infer them from a filename, pixel coordinates, or an array named `spatial`.
The following are the supported paths; a compatible extension alone is not a schema contract.

The segmentation or embedding model is chosen upstream. Marklab's cell-table workflows
and spatial-vector analyses do not require CellViT. For example, the vector-semivariogram
input declares object IDs, coordinates in micrometres, feature names and equally sized
finite vectors; their width is supplied by the input. The Rust `CellEmbeddingTable` also
accepts a caller-declared positive dimension. Native model exports still need mapping to
the selected input contract.

The named CellViT readers and complete version-1 provenance profile are optional adapters
for a particular pipeline. Their fixed dimension, calibration and source fields apply
only to that profile. A matching CellViT bundle is not required to use another model's
outputs through the shared analysis inputs.
The generic NPY readers and version-2 cell-embedding provenance also support other
models through the verified storage path; its tensor declarations come from the producer.

| Path | Read/write behavior | Executed file evidence |
|---|---|---|
| Standalone cell CSV and Parquet | Read the declared binary-pattern columns; export retained cells to Parquet. An export cannot recover source rows excluded at ingestion. | The [Schürch field check](validation-schurch.md) reads 1,107 observed cells, writes Parquet and reconstructs coordinates, marks and metadata exactly. Synthetic tests cover optional fields, nulls and malformed input. |
| Multiplex cell tables | Python `cell_table_slide` maps explicit columns and per-axis physical scales to a native study slide. | The Schürch workflow imports 138,567 observed cells and two quantitative markers across 70 fields. |
| AnnData H5AD | AnnData reads/writes the file; `anndata_slide` imports a bounded, materialized object with explicitly calibrated coordinates. Arbitrary backed/lazy objects are not supported. | Sparse H5AD round trips cover missingness and categories; a derived H5AD containing 1,107 observed Schürch cells reconstructs the full declared slide exactly. This is not an upstream-provided H5AD. |
| Steinbock object CSVs | Join one intensity table to one regionprops table by exact `Object` identity; require explicit X/Y scales and window. | The [Steinbock adapter](steinbock-import.md) reads genuine upstream export files from the official eight-object IMC mock fixture, with exact values and zero coordinate error. This fixture is not a biological cohort. |
| MCMICRO quantification CSV | Read one MCQuant table by `CellID` with `X_centroid`/`Y_centroid` in mask pixels; require explicit X/Y scales and window; morphology columns are not channels. | The [MCMICRO adapter](mcmicro-import.md) reads nf-core's public tonsil CyCIF test output exactly (1,621 cells); its centroids and areas match the segmentation mask and its OME-XML gives 0.65 µm/px. Not a biological cohort. |
| QuPath classified nucleus polygons | Import the bounded PUMA-compatible single-ring annotation profile with explicit IDs, class codebook, physical scale and window. | [PUMA melanoma ROI](qupath-import.md): 168 preserved IDs/classes and independent centroid agreement within 1e-9 µm. Cells/detections, general measurements, holes and multipolygons are outside this profile. |
| GeoJSON windows | Read valid Polygon/MultiPolygon geometry, optionally in Feature/FeatureCollection form, in the declared micrometre frame. | Synthetic shape/hole/boundary tests and acquired-field GeoJSON in the Schürch CSV/Parquet check. A field rectangle is not a tissue segmentation. |
| WSI pixel input | The `wsi` feature reads bounded regions through `wsi-rs`; available codecs have separate fixtures. No WSI writer is provided. | A public Aperio JPEG-2000 SVS region passes the existing independent OpenSlide pixel comparison. Other codec fixtures do not establish universal scanner or OME metadata compatibility. |
| Model-independent NPY vectors | Read NPY v1/v2, finite little-endian float32, row-major, 1–65,536 components. `bayes vector-semivariogram --vectors` pairs the matrix with coordinate rows in the same order; that statistic requires at least two components. | [Common ViT output widths](variograms.md#model-output-in-npy-format), nonstandard and boundary widths have exact-value/streaming/finite/byte-limit checks. Known-answer CLI checks cover 3- and 2,560-component matrices, source identity and row-count rejection. |
| Optional CellViT CSV/NPY import | The 1,280-column NPY payload reader is distinct from the fixed-calibration normalized `cellvit_he_bundle` v1 CSV/manifest profile. These are specific adapters, not the shared spatial-vector input contract. | [Actual STHELAR features](cellvit-import.md) give exact native NPY agreement for 128 cells × 1,280 components. The strict v1 full bundle has synthetic fixtures only; unavailable checkpoint, probability and morphology metadata are not invented. |
| Embedding Arrow IPC/Parquet | Read/write Marklab's strict internal physical profile and exact artifact bindings, using either legacy CellViT or model-independent version-2 provenance. | Non-CellViT 3- and 2,560-component vectors pass verified round trips. All 163,840 [STHELAR feature values and 128 IDs](cellvit-import.md#native-file-codec-check) survive actual-file round trips exactly, using explicitly synthetic codec provenance. Existing hostile-file/resource checks remain; generic pyarrow schemas are not accepted. |

For native binary-pattern columns see the [quick start](../README.md#quick-start). For
quantitative marker panels see [multiplex studies](multiplex-study.md); for internal
columnar contracts see [the public API](public-api.md#embedding-artifact-files).

## Public image check

[`download_wsi.py`](../examples/public-cohorts/download_wsi.py) downloads the existing
`JP2K-33003-1.svs` validation fixture into a new external directory. The
[publisher's Aperio index](https://openslide.cs.cmu.edu/download/openslide-testdata/Aperio/)
permits use and redistribution with or without modification; this is **not CC0**.
The old fixture manifest's CC0 label was corrected. The download is bounded at 80 MiB and
checks the already-required fixture digest once while receiving the source bytes.

From the 63,847,265-byte source, OpenSlide independently decoded the fixed
level-zero region `(7000,8000,256,256)`. The ignored test
`public_aperio_jp2k_region_matches_openslide_oracle` passed the unchanged limits: every
channel's maximum absolute difference at most 24, and mean absolute RGBA difference at most
2.5. With `wsi-rs` 0.9.0 (`j2k` 0.13.1) the comparison observed no difference: a maximum of 0 in
every channel and a mean absolute difference of 0. With `wsi-rs` 0.5.0 it observed maxima of 18,
17, 24 and 0 for R, G, B and A and a mean absolute difference of 2.414661407470703, which the
lossy-codec fixture contract permits; lossless fixtures still require exact equality.

```bash
python3 examples/public-cohorts/download_wsi.py --out /new/external/aperio-source
openslide-write-png /new/external/aperio-source/JP2K-33003-1.svs \
  7000 8000 0 256 256 /new/external/aperio-source/openslide-region.png
MARKLAB_PUBLIC_APERIO_SVS=/new/external/aperio-source/JP2K-33003-1.svs \
MARKLAB_PUBLIC_APERIO_ORACLE_PNG=/new/external/aperio-source/openslide-region.png \
  cargo +1.99.0 test --locked --package marklab --features wsi,cli \
  --test wsi_integration public_aperio_jp2k_region_matches_openslide_oracle \
  -- --ignored --exact --nocapture
```

Raw inputs remain outside the checkout. Use fresh output directories for repeated checks;
reuse unchanged raw downloads rather than fetching or hashing them again.

## Upstream boundaries

Marklab remains downstream of segmentation, image models and viewers. A user can map a
calibrated upstream cell table into the declared profile, but this is not automatic support
for QuPath projects, native CellViT graphs, HoVer-Net JSON, or every MCMICRO export.
SpatialData/OME-Zarr also needs explicit transforms between its coordinate systems before
a spatial array can be treated as micrometres. These native formats require separate,
tested adapters; the [coverage table](capabilities.md#method-coverage) states their scope.
