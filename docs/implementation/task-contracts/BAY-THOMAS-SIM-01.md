# Task contract — BAY-THOMAS-SIM-01 bounded Thomas process simulation

Status: complete

Date: 2026-08-24

Parent requirements: BAY-PP, BAY-PP-B, WS-43, WS-71.

## User outcome

`marklab bayes simulate-thomas-process` deterministically simulates a bounded Thomas cluster point
process in an exact half-open rectangle and retains its latent-parent and boundary-truncation artifact.

## Frozen behavior

- require a finite positive-area micrometre rectangle, finite positive parent intensity per square
  micrometre, finite positive mean offspring, finite positive Gaussian displacement SD, one seed,
  positive parent/offspring caps no greater than 100,000, and fresh output;
- expand the rectangle by exactly six Gaussian SDs using the exact Euclidean Minkowski dilation,
  sample a homogeneous Poisson parent process on that rounded rectangle, sample Poisson offspring per
  parent, add independent isotropic Normal-2D displacements, and retain children in the exact window;
- use pinned deterministic ChaCha20 streams and established `rand_distr` Poisson/Normal samplers;
  retain every latent parent, generated/retained child count, every observed child/parent identity,
  exact expanded area, six-SD radius, 2-D radial tail bound, discarded count, and resource caps;
- abort on non-finite arithmetic or exceeded realized caps. Do not silently treat a bounding-box
  expansion as the exact Minkowski window or claim zero boundary-truncation error.

## Validation and claims

A seeded `100x100` micrometre fixture must be byte-repeatable, produce nonempty parent/child artifacts,
place every parent inside the exact six-SD dilation, place every retained child inside the exact
window, and conserve generated children between retained/discarded counts and parent totals. This is
an established-family simulator with explicit finite expansion—not inference, fitted adequacy,
biology, causality, or clinical use.
