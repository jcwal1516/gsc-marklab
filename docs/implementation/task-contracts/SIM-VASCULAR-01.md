# Task contract — SIM-VASCULAR-01 bounded vascular transport

Status: complete

Date: 2026-08-25

Parent requirements: GEN-01, WS-70, WS-71.

## User outcome

`marklab simulate vascular-transport` maps declared vessel sources and cell uptake to a regular
two-dimensional concentration grid, advances bounded conservative transport under a declared
static flow approximation, and returns complete concentration, gradient, hypoxia-region, and
mass-balance evidence.

## Pseudocode ownership

This workflow owns the regular-grid, caller-declared-static-flow specialization of
`SimulateVascularTransport`. Vessel-graph hemodynamics, boundary exchange, dynamic vessels/cells,
nonlinear uptake, implicit diffusion, stabilized higher-order advection, and fitted biological
parameters remain separate work.

## Frozen behavior

- consume a 3–256 by 3–256 finite nonnegative concentration field, equally sized nonnegative
  diffusivity and finite velocity fields, exact declaration
  `caller_declared_static_velocity`, bounded vessel/cell point inputs, positive time/cadence,
  nonnegative hypoxia threshold, and at most 250 million cell-steps;
- deterministically map sorted unique in-grid vessel/cell identities to the nearest grid node,
  retain mapping distance, and add co-located source or linear-uptake rates;
- compose exact nonnegative local constant-source/linear-uptake half-flows around conservative
  arithmetic-face diffusion and face-velocity upwind advection with no-flux outer boundaries;
- reject combined per-cell outgoing diffusion/advection CFL above one and every nonfinite,
  negative, excessive-work, or mass-accounting-invalid state;
- retain complete initial/final fields with gradients and mapped rates, bounded mass/hypoxia
  trajectory, exact source/uptake/transport/balance diagnostics, and final deterministic
  four-neighbor hypoxic regions;
- label caller flow as an approximation and make no causal, patient, or hemodynamic-truth claim.

## Validation and claims

A co-located source rate two and uptake rate one reaches concentration one at `T=ln(2)` with exact
mass balance. One diffusion step maps a centered unit impulse to `0.96` at the center and `0.01` at
each axial neighbor while conserving mass and identifying four isolated hypoxic corner regions.
Uniform rightward flow maps each `[1,0,0]` row to `[0.9,0.1,0]` in one step with advective CFL `0.1`.
