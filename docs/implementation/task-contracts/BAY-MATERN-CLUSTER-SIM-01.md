# Task contract — BAY-MATERN-CLUSTER-SIM-01 bounded Matérn cluster simulation

Status: complete

Date: 2026-08-24

Parent requirements: BAY-PP, BAY-PP-B, WS-43, WS-71, BAY-THOMAS-SIM-01.

## User outcome

`marklab bayes simulate-matern-cluster-process` deterministically simulates a bounded Matérn
Neyman–Scott cluster process in an exact half-open rectangle with latent-parent provenance.

## Frozen behavior

- require the IC-0068 window/intensity/mean-offspring/seed/resource contract with a finite positive
  offspring radius in micrometres;
- expand the rectangle by exactly that radius using the exact Euclidean Minkowski dilation, sample a
  homogeneous Poisson parent process on the rounded rectangle, sample Poisson offspring per parent,
  and sample each displacement uniformly on the exact disc using `r=R*sqrt(U)` and angle `2*pi*V`;
- retain children in the exact observed window and preserve every parent, per-parent generated and
  retained count, every retained child/parent identity, proposal/count accounting, RNG/sampler
  identity, exact expanded area, and resource caps;
- because displacement support is bounded by the radius, declare the expanded-parent treatment exact
  for this rectangle. Do not call uniform-radius sampling a uniform disc or hide bounding-box parents.

## Validation and claims

A seeded `100x100` fixture must be byte-repeatable, place every parent inside the exact radius
dilation, every retained child inside the window, every child no farther than the radius from its
parent, and conserve all generated/retained/discarded counts. This is an exact-window simulator for
the declared bounded process—not inference, fitted adequacy, biology, causality, or clinical use.
