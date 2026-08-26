# Task contract — SIM-INTERFACE-01 bounded level-set interface evolution

Status: complete

Date: 2026-08-25

Parent requirements: GEN-01, WS-70, WS-71.

## User outcome

`marklab simulate evolve-interface` evolves a declared signed level-set field under a spatial normal
speed and curvature weight, returning complete states, interface summaries, bounded trajectory, and
solver/reinitialization evidence.

## Pseudocode ownership

This workflow owns the regular 2-D, static-speed-field specialization of
`EvolveInterfaceLevelSet`. Time-dependent/local coupled speed functions, adaptive meshes,
higher-order Hamilton–Jacobi schemes, and coupling to resource, agent, or treatment fields remain
separate work.

## Frozen behavior

- consume a 3–256 by 3–256 finite row-major level set and equally sized finite normal-speed field,
  positive micrometre spacings/time controls, nonnegative curvature weight, optional positive
  reinitialization cadence, and explicit cell-step and reinitialization-distance-visit limits;
- evolve `phi_t = -(F + gamma*kappa)|grad(phi)|` with first-order Godunov upwinding, centered
  curvature, and linear-extrapolation ghosts under an explicit combined advection/curvature CFL
  limit of `0.5`;
- derive step boundaries from the validated plan, preserve complete initial/final level sets, and
  report exact-zero-node plus horizontal/vertical crossing counts at every retained time;
- reinitialize, when requested, to signed Euclidean distance from sampled zero-contour nodes and
  edge crossings without changing the sign partition or exact zero nodes, under a declared exact
  distance-visit bound;
- expose an identity zero-contour observation and an experimental non-forecast claim ceiling.

## Validation and claims

A planar signed-distance field with unit normal speed advances exactly one micrometre in one time
unit, even with positive curvature weight because planar curvature is zero, and executes exactly ten
steps. A field scaled to twice planar signed distance returns to unit signed distance after one
reinitialization while retaining identical interface diagnostics and reporting positive bounded
distance work.
