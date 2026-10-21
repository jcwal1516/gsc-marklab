# CellViT embedding import

Marklab has two optional CellViT-related input surfaces with different scopes.
CellViT is one upstream model choice. These adapters do not define the inputs for
other models: shared cell-table and spatial-vector workflows accept their own declared
schemas, coordinates and feature dimensions. See [input and output formats](formats.md).
For a different encoder's NPY output, use `EmbeddingNpyMatrix` or the
[generic spatial-vector CLI input](variograms.md#model-output-in-npy-format). For verified
storage, use the model-independent version-2 tensor and provenance declarations described
in [the public API](public-api.md#embedding-artifact-files).

`CellVitNpySummary` and `CellVitNpyMatrix` accept a finite, little-endian,
row-major `float32` NPY matrix with exactly 1,280 columns. These readers describe
the normalized vector payload supplied to Marklab. Callers bind its rows to reviewed
cell identities when constructing a `CellEmbeddingTable`, which can then use the
existing Arrow IPC and Parquet artifact paths.

`import_cellvit_he_bundle_*` accepts the normalized `cellvit_he_bundle` version 1
profile. That profile is a specific pipeline contract: its 23-column CSV, source
manifest fields, calibration, graph-position scale, row alignment, and source
artifact identities are all fixed. It is not a universal adapter for CellViT output,
and the repository's synthetic fixtures establish only that normalized contract.

## Real public embedding validation

[`validate_sthelar_cellvit.py`](../examples/public-cohorts/validate_sthelar_cellvit.py)
uses the openly published STHELAR dataset from
[BioStudies S-BIAD2146](https://www.ebi.ac.uk/biostudies/studies/S-BIAD2146), licensed
CC BY 4.0. The associated
[Scientific Data article](https://doi.org/10.1038/s41597-026-06937-6) describes the
published `features_cellvit` tables as nucleus embeddings produced by the pretrained
CellViT-SAM-H 40x encoder. Its ViT-Huge encoder has a 1,280-component embedding. For
each cell, token features belonging to its nucleus are averaged within a patch and
then averaged across overlapping patches.

The bounded validation uses the biological `heart_s0` slide. Its published SpatialData
archive is 1,670,520,884 bytes, but the validator does not download the complete
archive. It uses HTTP byte ranges to preserve 20 reviewed Zarr metadata and data
members outside the checkout. These members contain:

- the first complete `features_cellvit/X` row chunk;
- the full `features_cellvit.obs.cell_id` vector;
- the full `table_cells.obs.cell_id` vector; and
- the two `table_cells.obsm['spatial']` coordinate chunks.

The published embedding array is a Zarr v2 `float32` array with shape
26,363 by 1,280 and chunks of 1,648 by 160. The validator fixes its representative
selection before comparison as the first 128 rows in published Zarr order. It
reconstructs those rows from all eight column chunks, writes a standard NPY, reloads
it, and requires exact `float32` equality with the source values. It also requires
128 unique published cell IDs, exact identity matches in `table_cells`, and finite
coordinates. The reviewed run transferred 15,867,956 bytes of the remote archive;
all selected vectors were finite and no selected row was all zero.

The `table_cells.obsm['spatial']` values originate from the Xenium cell table loaded
by STHELAR through `spatialdata_io.xenium`. The
[10x Xenium output
contract](https://www.10xgenomics.com/support/software/xenium-onboard-analysis/latest/analysis/xoa-output-understanding-outputs)
defines cell-centroid X and Y in micrometres, so the derived identity CSV records them
as `x_um` and `y_um`. The published Zarr metadata itself uses the generic axis unit
label `unit`; the validator therefore records the Xenium contract as the basis for
the physical-unit interpretation instead of claiming that the Zarr array is
self-describing.

Run the validator with the existing Python environment, which supplies NumPy and
Numcodecs:

```bash
target/client-venv/bin/python examples/public-cohorts/validate_sthelar_cellvit.py \
  /absolute/new/path/sthelar-cellvit-raw \
  /absolute/new/path/sthelar-cellvit-validation
```

Later runs can revalidate the preserved selected members without another remote
archive transfer while still writing a new result directory:

```bash
target/client-venv/bin/python examples/public-cohorts/validate_sthelar_cellvit.py \
  /absolute/existing/path/sthelar-cellvit-raw \
  /absolute/new/path/sthelar-cellvit-validation \
  --reuse-source
```

The result directory contains a derived 128 by 1,280 NPY, a row-aligned CSV containing
only `cell_id`, source row, and Xenium centroid, and a validation summary. It also
contains an independent expected-value JSON written directly from the reconstructed
Zarr values before the NPY is saved. That oracle records every selected ID and all
1,280 `float32` values as JSON numbers so native-reader tests do not define their
expected result by reading the NPY under test. The source directory preserves the
exact selected Zarr member payloads plus the BioStudies metadata and a short source
description. It does not create a checksum manifest.

Before allocating decoded chunks, the validator inspects each Blosc header. Numeric
chunks must have exactly the byte length implied by their reviewed Zarr shape and are
decoded into preallocated arrays. Variable-length cell-ID chunks have a decoded-byte
cap derived from the reviewed row count and a 64-byte maximum UTF-8 ID, require the
exact encoded string count, and decode into a preallocated object array.

## Provenance boundary

STHELAR identifies the model family and CellViT-SAM-H 40x variant, architecture,
embedding width, and extraction procedure. The published `features_cellvit` table
does not encode an exact checkpoint artifact identity. The bounded Python validation
therefore establishes exact equality between the selected published Zarr values and
the derived NPY, plus the cell identity binding and coordinates. The separate
[native file-codec check](#native-file-codec-check) establishes exact agreement through
`CellVitNpyMatrix`, `CellEmbeddingTable`, Arrow IPC, and Parquet. The published inputs
cannot by themselves construct Marklab's strict complete `CellEmbeddingProvenance`
artifact, which requires
an exact checkpoint artifact and content identity.

The validator does not invent type probabilities, a QC pass flag, nucleus morphology,
H&E pixel coordinates, or missing checkpoint identity. Although some separate STHELAR
tables contain RNA labels, PanNuke predictions, and morphology measurements, substituting
those fields into the normalized version 1 bundle would change that bundle's semantics.
Native SpatialData ingestion and a broader versioned CellViT profile remain separate
work from validating the published vector matrix.

The optional full normalized version 1 reader still lacks an authentic public-file check.
That limits claims about this specific adapter; it is not a requirement for other
models or a release criterion for the shared model-independent analysis workflows.
Its fixed `mpp = 0.37744`, `graph_position_scale = 0.662356930902925`, tumor-mask
identity, and native `cells.json`/`cells.pt` identities cannot be recovered from the
selected STHELAR tables. The [Xenium output contract](https://www.10xgenomics.com/support/software/xenium-onboard-analysis/latest/analysis/xoa-output-understanding-outputs)
supports centroids in micrometres; it does not establish that H&E pixel frame.
Converting those centroids by the profile's required scale or filling QC/source
fields with placeholders would create a synthetic normalization fixture, not
real-file evidence for that admission profile.

## Native file-codec check

The ignored test `public_sthelar_vectors_round_trip_through_native_npy_arrow_and_parquet`
in `cellvit_embedding_artifact_graph` reads the derived NPY with Marklab and compares
all 163,840 numeric components against the independent JSON oracle reconstructed from
published Zarr chunks. It then writes actual Arrow IPC and Parquet files, reopens both
with Marklab, and checks all 128 cell IDs and every component exactly.

The test reuses the existing **synthetic provenance fixture only as a file-codec
context**. Its checkpoint and extraction records are test data, not assertions about
the STHELAR model. This demonstrates numerical file transport and identity retention;
it does not admit these vectors as a fully verified checkpoint/provenance artifact or
exercise the fixed-calibration `cellvit_he_bundle` version 1 profile.

In the executed run, all numeric and identity checks passed exactly. The first attempt stopped before
writing output because Parquet needed 8,399,288 retained bytes, exceeding the reused
small-fixture allowance of 8,388,608 bytes. The public-file test now explicitly permits
16 MiB of retained memory; production limits and the exact-value criterion are unchanged.

```bash
MARKLAB_PUBLIC_STHELAR_INPUT=/absolute/existing/path/sthelar-validation \
MARKLAB_PUBLIC_EMBEDDING_OUTPUT=/absolute/new/path/native-codec-check \
  cargo +1.99.0 test --locked --package marklab \
  --test cellvit_embedding_artifact_graph \
  public_sthelar_vectors_round_trip_through_native_npy_arrow_and_parquet \
  -- --ignored --nocapture
```
