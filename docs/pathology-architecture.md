# Pathology architecture

`marklab pathology architecture` describes tumor epithelium as regions rather than cell
positions. For each slide it answers:

- Where is the advancing tumor front, after the tissue edge and any regions you exclude?
- How much glandular, solid or fragmented epithelium lies in each distance band from that front?
- How fragmented, lumen-forming and irregular are the epithelial pieces in each band?

It works from polygons you provide. It does not segment epithelium, classify architecture
or predict anything from H&E; produce those upstream (for example, a segmentation model
plus a region classifier, or annotations exported from QuPath) and declare their source.
This workflow is at the Research tier: it has analytic geometry checks but no public
real-data demonstration (see [capabilities](capabilities.md#command-maturity)).

```sh
cargo +1.99.0 run --locked --package marklab --features cli -- \
  pathology architecture --recipe examples/pathology-architecture/recipe.json \
  --out target/pathology-architecture-example
```

The output directory must be new or empty. The example is synthetic: one square gland with
a lumen and two small fragments, chosen so every area and distance can be checked by hand.
Its design values are not recommendations.

## Recipe

Start from the example recipe (version 1). Unknown fields are errors. Coordinates are
micrometres in one declared frame per slide; convert pixel annotations with the slide's
calibration first.

- **`architecture`**: the class names in order (up to 16, for example `glandular`,
  `solid`, `moderately_fragmented`, `highly_fragmented`), how they were assigned and their
  source. `unclassified` is reserved for epithelium outside every region.
- **`epithelium`**: how the epithelium polygons were produced and their source.
- **`declared_front`** (optional): source of hand-drawn fronts, required when any slide has one.
- **`design`**:
  - `closing_radius_um`: radius of the morphological closing that joins nearby epithelium
    into a tumor bulk.
  - `minimum_bulk_area_um2`: smaller bulk pieces (isolated fragments or buds) do not form a front.
  - `tissue_edge_tolerance_um`: bulk boundary closer than this to the tissue edge is not front.
  - `band_edges_um`: increasing signed distances, 2 to 33 edges.
  - `contour_sample_spacing_um` and `curvature_smoothing_um`: contour resampling step and
    Gaussian smoothing scale for curvature (0 disables smoothing).
- **Slides**: each has its real patient ID, unique slide and coordinate-frame IDs and:
  - `window`: the tissue outline as a GeoJSON MultiPolygon, with artifacts removed.
  - `epithelium`: one MultiPolygon whose pieces are the epithelial components. Pieces must
    not touch or cross; clean segmentation output before export.
  - `boundary_uncertainty_um`: distance bands narrower than twice this are rejected.
  - `architecture_regions`: `id`, `class` and MultiPolygon `geometry`. Regions must not
    overlap. Draw them coarsely; each epithelial component takes the class of the region
    containing its interior point, and band areas use the exact overlap.
  - `lumens`, `mucin` (optional MultiPolygons): must not overlap epithelium.
  - `front_exclusions` (optional MultiPolygon): tumor boundary inside it is not front, for
    example a luminal surface or a lateral margin with adjacent mucosa.
  - `declared_front` (optional MultiLineString): a hand-drawn front, used only for agreement.

Every layer must lie inside the window. Slides, regions and components are put in a
canonical order, so input order does not change results.

## Tumor bulk and invasive front

The tumor bulk is the union of epithelium, lumens and mucin, closed (dilated then eroded)
by `closing_radius_um`, with holes filled, clipped to the window and kept only for pieces
of at least `minimum_bulk_area_um2`. Filling holes keeps enclosed intratumoral stroma on
the tumor side. The front is the bulk boundary farther than `tissue_edge_tolerance_um`
from the tissue edge and outside `front_exclusions`. A slide with no qualifying bulk or no
remaining front reports why and has no bands.

When a slide has a declared front, the result samples both fronts every
`contour_sample_spacing_um` and reports the mean, 95th-percentile and maximum distance in
each direction. Use it to check the derived front against a pathologist's line before
relying on it.

## Distance bands

Signed distance is the distance to the nearest front segment, negative inside the bulk and
positive outside. Bands include their lower edge and exclude their upper edge, except the
last band, which includes both. Band areas come from round offsets of the front clipped to
the bulk or to the host tissue. Arcs are polygonal; the result reports
`maximum_buffer_chord_error_um`, the largest radial gap between a true arc and its polygon
(`max(|distance|)*(1-cos(0.05))` over band edges, closing radius and edge tolerance).

Each band reports tissue area, epithelial area and, for each class, the epithelial area
inside that class's regions. Whole components are assigned to the band of their interior
point; for those components each class reports the count, total and median area,
normalized area entropy (0 for one component), pooled mean absolute curvature,
area-weighted isoperimetric quotient and, when lumens are declared, the fraction
enclosing at least one lumen.

## Components

Every epithelial component reports its class, interior point, area, perimeter (exterior
and holes), hole count, enclosed lumen count (each lumen is assigned to the smallest hole
containing its interior point), isoperimetric quotient `4*pi*area/perimeter^2`, signed
front distance and band.

Curvature follows contour roughness: each ring is resampled at the declared spacing,
smoothed with a circular Gaussian of the declared scale, and its absolute turning angles
are summed. Mean absolute curvature is that sum divided by the smoothed length. A simple
convex ring turns by exactly `2*pi`; indentations add turning. Rings with fewer than 16
samples, or too short for the complete smoothing kernel, are left out of both sums.

These are descriptions of the polygons you supplied. They do not establish that a region
is invasive, that a fragment is a tumor bud or that architecture causes a cell state.

## Outputs

`result.json` holds the design, layer provenance and all results. `components.csv` and
`bands.csv` hold the same rows as tables, one row per component and per band and class;
empty fields are unavailable values. `slide-NNN-architecture.geojson` holds the tumor
bulk, the front and component interior points in the declared micrometre frame, not
geographic coordinates. Apply the inverse calibration to overlay them on slide pixels.

## Limits

A recipe can be at most 64 MiB with up to 64 slides. The optional `limits` section sets the
maximum vertices per layer or derived geometry (default 2 million, at most 20 million),
polygon components per layer (default 200,000), total geometry work (default 2 billion
units, at most 200 billion) and estimated memory (default 2 GiB, at most 8 GiB). Hitting a
limit stops the run before anything is written.
