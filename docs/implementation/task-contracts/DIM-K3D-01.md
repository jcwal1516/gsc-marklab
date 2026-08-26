# Task contract — DIM-K3D-01 dimensionality-aware cuboid K/L workflow

Status: complete

Date: 2026-08-25

Parent requirements: Part XI §§91, 94–95, DIM-01, WS-80.

`marklab spatial3d k-function` consumes finite points with exactly three coordinate columns, one
positive-volume axis-aligned cuboid in the same declared nanometre/micrometre/millimetre unit,
positive voxel spacing, optional symmetric positive-definite 3×3 anisotropy matrix, strictly
increasing nonnegative physical radii, one correction (`none`, `border`, or `translation`), and a
maximum unordered-pair count. All coordinates, window bounds, spacing, distances, radii, volume,
and surface area are normalized to micrometres before estimation. The anisotropy matrix acts on
normalized physical displacement and must not be used as a 2-D correction surrogate.

The cuboid validator proves finite ordered bounds, positive finite volume/surface area, one
component, no cavity, exact containment, and Euclidean boundary distance. Homogeneous 3-D K uses
`V/(n(n-1))` times the ordered eligible contribution. `none` has unit weight; `translation` uses
`V / ((width-|dx|)(height-|dy|)(depth-|dz|))`; `border` uses directed reference points whose
Euclidean distance from every cuboid face is at least the radius and normalizes by their count.
`L3=(3K/(4π))^(1/3)`. Fewer than two points, outside points, zero border references, invalid overlap,
or exceeded pair work are explicit errors/statuses, never silent 2-D fallback.

The initial hand oracle uses two points one micrometre apart in a 10×10×10 micrometre cuboid. At
radius one, uncorrected and border K are 1000 and translation K is `10000/9`; normalized millimetre
input must produce identical micrometre results.
