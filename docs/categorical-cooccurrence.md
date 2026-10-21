# Categorical co-occurrence

**Experimental** for the observed radius-dependent score described here.

Marklab's categorical co-occurrence analysis measures distance-dependent enrichment
between every ordered pair of declared cell classes. For a radius `r`, let
`N[source,target,r]` be the number of directed pairs of distinct cells whose distance
is at most `r`. Let `D[source,r]` and `D[target,r]` be the corresponding class edge
incidences and let `T[r]` be the total number of directed pairs. The reported value is

```text
N[source,target,r] * T[r] / (D[source,r] * D[target,r]).
```

A value of one means that the target class's share of the source class's neighbors
equals the target class's share of all directed edge endpoints at that radius. Values
above one indicate enrichment and values below one indicate depletion. This is the
statistic implemented by Squidpy `gr.co_occurrence`; it is not a pair-correlation,
cross-G, or global class-prevalence statistic.

## Version 1 input

The CLI accepts one strict JSON document with `format` equal to
`marklab.categorical_cooccurrence` and `version` equal to `1`. Unknown fields fail.
The request declares:

- a nonempty coordinate-frame ID and the exact unit `micrometer`;
- an inline GeoJSON `MultiPolygon` observation window in that physical frame;
- one categorical channel with an ID, label, categorical kind/unit, measurement
  status, provenance text, and an ordered list of unique class IDs;
- cells with unique IDs, finite `x_um` and `y_um`, and one declared `class_id`;
- strictly increasing, finite, positive radii in micrometres; and
- explicit limits for input bytes, points, classes, radii, output matrix bytes,
  directed pair visits, and retained bytes.

Every point must be inside or on the declared observation-window boundary. Every
declared class must be observed. Distinct cells may have identical coordinates and
are then neighbors at every positive radius. A cell is never paired with its own row.
Distance comparison is inclusive (`distance <= radius`) and uses the submitted
`float64` coordinates without an implicit unit conversion.

The command is:

```bash
target/debug/marklab numerics categorical-co-occurrence \
  --input /absolute/path/request.json \
  --out /absolute/new/path/result.json
```

The destination must not already exist. The CLI checks its hard input-file cap before
decoding and then enforces the request's equal or narrower declared limits.

## Result contract

The result has `format` equal to `marklab.categorical_cooccurrence_result`, version
`1`, the coordinate-frame and channel IDs, the ordered classes, point/class counts,
and the number of directed neighbor visits at the largest radius. `matrices` appear in
increasing radius order. Within each matrix, `entries` use source-major then
target-major class order and report:

- the directed pair count;
- source and target edge incidences;
- status `available` with a finite numeric `score`; or
- status `unavailable_zero_edge_incidence` with a JSON `null` score.

Each matrix also reports its total directed-pair count.

Zero observed pairs with nonzero source and target incidences are available and have
value zero. A zero source or target incidence makes the conditional ratio undefined;
Marklab keeps that distinction instead of writing a numeric zero. Counts and statuses
are included so consumers can audit each ratio without reconstructing the neighbor
graph.

The implementation builds one bounded spatial index, queries each source cell once at
the maximum radius, and assigns a visited pair to its first eligible radius. Prefix
sums produce the requested cumulative matrices. It does not retain a neighbor-edge
list. The directed-pair-visit limit applies to visits at the largest requested radius;
matrix and retained-byte limits are checked before allocating their bounded storage.

## Squidpy agreement on public tissue data

[`validate_squidpy_cooccurrence.py`](../examples/public-cohorts/validate_squidpy_cooccurrence.py)
uses the CC-BY-4.0 Schürch colorectal CODEX table from
[Mendeley Data](https://doi.org/10.17632/mpjzbtfgfr.1). It selects the biological
`reg001_A` ROI, excludes the published `dirt` class, and fixes 1,107 cells, 19 cell
types, the published 0.377442 micrometres-per-pixel calibration, and radii 10, 25, 50,
and 100 micrometres before comparison.

Squidpy 1.8.3 casts both coordinates and explicit thresholds to `float32`. Its output
is `float64` with axes `(target, source, radius)`. The validator therefore computes
pair membership independently in both `float64` and Squidpy's `float32` arithmetic
and requires exact membership agreement on this fixture. It then compares Squidpy to
the independent integer-count formula with
`16 * eps64 * (1 + abs(expected))` tolerance, and compares Marklab's native `float64`
result to the independent formula with absolute tolerance `1e-12`.

The fixed directed-pair totals are 1,806, 11,666, 42,836, and 141,924. The validator
also records the nearest squared-distance threshold margins and coordinate/distance
quantization errors so the exact membership agreement is distinguishable from a
rounded score comparison.

The native run passed all four radii and all 19-by-19 class matrices. Native formula error was exactly
zero; Squidpy's largest absolute formula error was `3.552713678800501e-15`, below its
prespecified bound. Pair membership agreed exactly in both coordinate precisions.
This is one observed tissue field; it does not establish a null distribution, patient
inference, or robustness to different tissue shapes and cell prevalences.

First preserve the source with [`schurch.py`](../examples/public-cohorts/schurch.py).
Then run the reference and native comparison in the existing Squidpy environment:

```bash
target/spatial-reference-venv/bin/python \
  examples/public-cohorts/validate_squidpy_cooccurrence.py \
  /absolute/path/schurch-preserved-source \
  /absolute/new/path/cooccurrence-validation \
  --binary /absolute/path/target/debug/marklab
```

`--reference-only` is available to inspect the independent and Squidpy references
before a native binary exists. Its validation summary records the native path as not
run; only a run without that option establishes native agreement.
