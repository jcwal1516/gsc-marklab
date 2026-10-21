# Public point-pattern reference comparisons

Marklab compares its native point-pattern estimators with pinned CRAN `spatstat`
packages on the published `spatstat.data` amacrine and betacells patterns. Both are
animal retinal recordings with documented physical scales. They provide numerical
method checks, not patient-group, clinical, or diagnostic validation.

The executable comparison is
`tests/fixtures/inhomogeneous_spatial/compare_spatstat.R`. It checks the package lock
before running and writes native results, reference curves, command logs, per-case
metrics, and a machine-readable `summary.json` into a caller-supplied new directory.
The homogeneous K/L policy was fixed before the final run: 16 radii at fractions
`1/80` through `16/80` of the window's short side, 1,024 equal reference subdivisions
per requested-radius interval, scaled K error no larger than `1e-10`, and absolute L
error no larger than `1e-10` micrometres. The tolerances were set before any comparison.
The 1,024-subdivision grid was adopted after the sparse-grid directed cross-K comparison
failed ([categorical pairs](categorical-pairs.md)), following spatstat's `compileK`
requirement for a fine radius sequence. Homogeneous K also passes on the sparse grid
(scaled K error at most 8.5e-15), so the change of grid does not affect its agreement.

## Translation-overlap numerical correction

In the first homogeneous K/L comparison, standard-border and isotropic K/L passed their prospectively fixed tolerances, while
translation K/L did not. At 18.75 micrometres on betacells, Marklab reported
`K = 169.17194815987222`; the analytic rectangle estimator and `spatstat` `Kest`
both gave `169.1719483393978`, a scaled difference of `1.0612e-9`. The declared
translation-K threshold was `1e-10`, so the run remains a failure rather than being
accepted under a wider tolerance.

For an axis-aligned rectangle of width `w` and height `h`, translated by `(dx, dy)`,
the exact overlap area is

```text
max(0, w - |dx|) * max(0, h - |dy|).
```

Marklab now uses this analytic form only when the already validated observation window
has one exterior ring containing exactly the four bounding-box corners, no holes, and a
closing fifth position. General polygons, holes, and multipolygons retain the existing
bounded Boolean-intersection implementation. The estimator and result format are
unchanged; this removes avoidable floating-point cancellation from the rectangle case.

The package reference is the documented translation-corrected estimator
`A/[n(n-1)] sum I(d_ij <= r) e_ij` in
[`Kest`](https://rdrr.io/cran/spatstat.explore/man/Kest.html). A direct R calculation
using rectangle overlap agreed with `Kest` within `4.2e-15` scaled error across both
public patterns, independently locating the discrepancy in Marklab's former rectangle
overlap calculation.

The first rerun after the analytic correction stopped while reopening the amacrine translation result because an ordinary
`serde_json` value decode changed a finite JSON number by one ULP, invalidating the
stored K/L identity. Marklab now enables `serde_json`'s maintained `float_roundtrip`
parser mode, which preserves the exact finite `f64` bits written by its serializer while
retaining Serde's strict object, integer, type, duplicate-field, and depth handling. The
public result schema and field meanings remain unchanged. Workflow identities were
advanced wherever the corrected parser can change serialized numerical output, so
earlier cached results cannot be reused under a new numerical identity.

The small translation cross-g fixture also retains the corrected round-trip value.
Its analytic value `49 / (8 pi)` rounds to `1.9496480528757179`; the former parsed
fixture value was `1.949648052875718`. Both exact replay and expected-file comparisons
remain required.

## Final homogeneous K/L agreement

In the passing integrated run, all six homogeneous comparisons passed the prospectively fixed tolerances:

| Pattern | Correction | Maximum scaled K error | Maximum absolute L error (µm) |
| --- | --- | ---: | ---: |
| amacrine | border | `2.139e-16` | `3.553e-15` |
| amacrine | translation | `1.492e-15` | `4.263e-14` |
| amacrine | isotropic | `8.507e-15` | `7.105e-14` |
| betacells | border | `4.045e-16` | `2.842e-14` |
| betacells | translation | `8.225e-16` | `5.684e-14` |
| betacells | isotropic | `1.262e-15` | `8.527e-14` |

The runner also records `Kest` evaluated directly on the sparse requested-radius grid
and writes its untouched 513-point package-default curves. Those are descriptive
convention checks. Equality is assessed against the declared 16,385-point grid that
contains every requested radius exactly. The same integrated run passed the six
inhomogeneous border-K/L cases, all four directed categorical cross-K/L cases, and the
independent GET tied-curve fixture.

## Pair-correlation agreement

The extended passing run uses 16 radii at fractions `3/80` through `18/80` of the window's short side, an
Epanechnikov half-width of `1/40` of the short side, the `r` divisor, no zero-distance
correction, and a maximum scaled error of `1e-10`. Every agreement metric is
fail-closed: unequal or empty vectors and any non-finite value at a declared radius
fail the run.

For homogeneous translation and isotropic pair correlation, Marklab agrees directly
with [`pcf.ppp`](https://rdrr.io/cran/spatstat.explore/man/pcf.ppp.html) using its
analytic `fast = FALSE` path:

| Pattern | Maximum scaled translation-g error | Maximum scaled isotropic-g error |
| --- | ---: | ---: |
| amacrine | `3.390e-15` | `2.220e-15` |
| betacells | `1.741e-15` | `1.287e-15` |

The directed homogeneous and inhomogeneous cross-g checks use independent pair sums
with `spatstat`'s public [`edge.Trans`](https://rdrr.io/cran/spatstat.explore/man/edge.Trans.html)
and [`edge.Ripley`](https://rdrr.io/cran/spatstat.explore/man/edge.Ripley.html) weights. Homogeneous cross-g
agreed within `1.668e-15` scaled error in both directions on both patterns;
inhomogeneous cross-g, supplied with Marklab's native type-specific point intensities,
agreed within `1.903e-15`. The R runner decodes Marklab's exact tagged-float documents
from their decimal `uint64` bit integers, so point intensities enter the reference sum
with the same finite `f64` values used by the native analysis.

[`pcfcross`](https://rdrr.io/cran/spatstat.explore/man/pcfcross.html) and
`pcfcross.inhom` delegate to a fast discretised smoother. Their public package curves
are therefore retained as descriptive implementation checks rather than equality
oracles. On the requested sparse grid, their largest scaled differences were
`7.762e-4` for homogeneous cross-g and `1.156e-3` for inhomogeneous cross-g. With
1,024 subdivisions per requested-radius interval, those maxima fell to `1.005e-6`
and `1.073e-6`, respectively. Neither the sparse nor the 1,024-subdivision package
curves meet the original fixed `1e-10` exact-agreement criterion. That package check
remains a failure; the passing analytic pair-sum oracle is reported separately and
does not convert the package comparison into a pass.
The integrated machine summary records `analytic_reference_passed: true` and
`package_exact_agreement_passed: false`, with all eight failed comparison CSV paths.

Marklab's standard-border g and cross-g estimators use eligibility at radius plus
bandwidth. `spatstat` does not expose the same estimator, so those native methods have
no package-equality claim in this comparison.

## Empty-space, nearest-neighbour, and J agreement

Marklab's nearest-space workflow uses reduced-sample F and G estimators. F evaluates
empty-space distances at a declared `64 x 64` cell-centred grid; G evaluates each
event's nearest distinct neighbour. On the rectangular public windows, a `spatstat`
mask with the same dimensions has exactly the same pixel centres as Marklab's probe
grid. [`Fest`](https://rdrr.io/cran/spatstat.explore/man/Fest.html) on that mask and
[`Gest`](https://rdrr.io/cran/spatstat.explore/man/Gest.html) on the original polygon
therefore provide named reduced-sample references. J is checked as the documented
survival ratio `(1 - G) / (1 - F)` at every radius where the denominator exceeds the
fixed `1e-12` threshold.

Both amacrine and betacells matched F and G exactly at all 16 declared radii, and J
exactly at every radius admitted by the denominator policy: the maximum scaled F, G,
and finite-J errors were all zero. A single `Jest` call does not express Marklab's
combination of cell-centred mask probes for F and exact polygon boundary distances for
G, so the comparison composes the two public package estimators and records that
convention explicitly.

These comparisons establish numerical agreement of the implemented estimators on two
published animal retinal point patterns. They do not establish patient-level power,
clinical validity, diagnostic performance, or fitted-null calibration.
