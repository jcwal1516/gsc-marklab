# Task contract — SIM-GROWTH-01 bounded Fisher–KPP growth-front simulation

Status: complete

Date: 2026-08-25

Parent requirements: GEN-01, WS-70, WS-71.

## User outcome

`marklab simulate growth-front` advances an explicitly scaled one-dimensional Fisher–KPP density
profile and returns its complete final state, bounded trajectory summaries, front geometry, solver
diagnostics, and planar-wave control.

## Pseudocode ownership

This workflow owns `SimulateGrowthFront` and the Fisher–KPP immediate-caller specialization of
`ValidateSimulator` and `SimulateReactionDiffusion`. General reaction systems, arbitrary meshes,
stochastic forcing, and `AnalyzeReactionDiffusionPattern` instability bands remain separate work.

## Frozen behavior

- consume 3–100,000 finite equally spaced positions in micrometres with density in `[0,K]`, finite
  nonnegative diffusion/growth, positive carrying capacity/time/step, a strict interior threshold,
  trajectory cadence, and a maximum 250-million cell-step declaration;
- use Strang splitting with the exact logistic reaction flow and centered explicit diffusion on a
  one-dimensional regular grid with reflected-ghost no-flux boundaries;
- reject `D*dt/dx^2 > 0.5`, nonuniform grids, invalid density, excessive work, and every nonfinite or
  invariant-violating numerical state;
- report trapezoidal density mass, threshold-front position and observed speed when a descending
  crossing exists, theoretical planar speed `2*sqrt(D*r)` when defined, all retained snapshots, and
  complete CFL/positivity/range diagnostics;
- classify the result as an experimental mechanistic simulation, never a patient or tumour forecast.

## Validation and claims

Uniform density `0.25`, `r=1`, `K=1`, `D=0`, and `T=ln(3)` reaches exact density `0.5`. A bounded
step profile with `D=0.25`, `r=1` advances more than three micrometres over ten time units and reports
the theoretical planar speed one. A declared CFL of one is rejected before simulation.
