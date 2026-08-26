# Task contract — BAY-MULTITYPE-PAP-01 typed multitype Papangelou intensity

Status: complete

Date: 2026-08-25

Parent requirements: BAY-PP, BAY-PP-B, WS-43, BAY-STRAUSS-STAT-01.

## User outcome

`marklab bayes multitype-papangelou` evaluates one typed multitype Gibbs insertion intensity with an
explicit baseline and complete symmetric finite-range pair-potential matrix.

## Frozen behavior

- consume exact `point_id,x_um,y_um,type_id`, exact `type_id,log_baseline_per_um2`, and complete
  `type_a,type_b,log_pair_potential,radius_um` CSVs; require 1–64 types, 0–100,000 unique points,
  finite coordinates/baselines/potentials, positive radii, full unique KxK rows, exact symmetry, a
  declared proposal type/coordinate distinct from existing points, bounded visits, and fresh output;
- start at the proposed type's constant log baseline; for every existing point with type l and
  Euclidean distance `<=radius[k,l]`, add the exact pair log potential; exponentiate once and reject
  overflow. Retain every included/excluded point contribution and matrix/input identity;
- this milestone admits constant baseline fields and a symmetric scientific model only. Directional,
  spatially varying baselines require a separate contract rather than silent defaults;
- do not call one conditional-intensity evaluation a fitted/normalizable multitype Gibbs process or
  an attraction/repulsion discovery.

## Validation and claims

With proposal type A baseline `log(2)`, one A neighbor contributing `log(0.5)`, and one B neighbor
contributing `log(2)`, the pair terms cancel and Papangelou intensity is exactly `2`. This validates
typed matrix/radius mechanics only—not inference, multiplicity control, biology, causality, or
clinical use.
