# Nematic orientation and half-defect screening

`marklab numerics nematic-orientation` summarizes declared axial cell or nuclear orientations
and screens a prespecified regular grid for robust `+1/2` and `-1/2` nematic defects. It is a
static shape analysis. It does not infer forces, material stress, dynamics or causality.

```bash
marklab numerics nematic-orientation \
  --input examples/nematic-orientation.json \
  --out out/nematic-orientation.json
```

The request names the measured axis in `orientation_source` (for example,
`nuclear_major_axis`) and the weighting design in `weight_name`. The input orientation is in
radians. It is axial, so `theta` and `theta + pi` are identical. Every row also supplies a
positive explicit weight. MarkLab computes

```text
Q = weighted mean(2 n n^T - I)
Q_xx = weighted mean(cos(2 theta))
Q_xy = weighted mean(sin(2 theta))
Q_yy = -Q_xx
S = sqrt(Q_xx^2 + Q_xy^2)
director = 0.5 atan2(Q_xy, Q_xx) modulo pi
```

`S` ranges from zero for balanced directions to one for perfect alignment. The global estimate
uses all supplied cells. A local estimate includes the focal cell and all cells within
`local.radius_um`. Grid samples use cells within `field.support_radius_um`. A local or grid
director is unavailable when its circular support crosses the tissue exterior or a hole, has
fewer than the declared minimum cells, or has `S < minimum_resolved_order`. Low-order rows keep
their measured `Q` and `S` but omit the unstable director.

For each grid plaquette, MarkLab traverses lower-left, lower-right, upper-right and upper-left
and sums wrapped differences of the doubled director phase. Doubled-phase winding `+1` or `-1`
is nematic charge `+1/2` or `-1/2`. A plaquette is resolved only when:

- the whole square plus the field-support radius lies inside the observation window;
- all four corners have adequate support and at least `minimum_corner_order`;
- every phase step is below `maximum_edge_phase_step_rad`, avoiding branch-cut ambiguity; and
- the winding is within `winding_tolerance` of an integer.

Every plaquette is retained with a status. Low order, incomplete support, a boundary or hole,
ambiguous phase steps, non-quantized winding and unsupported higher winding are never counted as
half defects. Grid origin, spacing, support radius and thresholds are analysis choices and should
be fixed before looking at the result. Coarse grids can miss paired or closely spaced defects;
fine grids do not restore orientations that were not measured.

The strict JSON request is limited to 16 MiB. Caller ceilings bound cells, grid points, total
neighbor visits and plaquettes, and built-in ceilings also bound grid and query work. The output
records those settings, the coordinate frame, global/local/grid estimates, every plaquette
status, resolved half-defect counts and work used.
