# Task contract — DIM-WEIGHTED-K3D-01 inhomogeneous and directed cross K/g

Status: complete

Date: 2026-08-25

Parent requirements: Part XI §96, DIM-01, WS-80, IC-0126.

`marklab spatial3d inhomogeneous-k` and `marklab spatial3d cross-k` consume the exact physical
cuboid/unit/spacing/anisotropy/correction contract of IC-0126 plus a positive finite intensity in
points per cubic micrometre for every point. Inhomogeneous K accumulates ordered same-process pairs;
cross-K accumulates only directed A-reference to B-target pairs. None/translation results divide
weighted contributions by cuboid volume; border results admit references within the radius-eroded
cuboid and divide by its exact positive volume. Cross-g is the K increment divided by its 3-D
spherical shell volume.

All group IDs are unique and nonempty, every point lies in the cuboid, radii are strictly increasing,
and pair/radius work has caller and hard bounds. Two points with intensity `0.002` in a 1000 µm³
cuboid give uncorrected inhomogeneous K 500 at their distance; one A and one B point each with
intensity `0.001` give directed cross-K 1000. These are supplied-intensity descriptive estimators,
not fitted intensity or Poisson/model calibration.
