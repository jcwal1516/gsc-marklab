# Task contract — SIM-RD-01 bounded scalar reaction–diffusion diagnostics

Status: complete

Date: 2026-08-25

Parent requirements: GEN-01, WS-70, WS-71.

## User outcome

`marklab simulate reaction-diffusion` advances a nonnegative scalar field on a regular periodic
two-dimensional grid and returns complete states, bounded trajectory summaries, solver evidence,
and discrete-Fourier pattern diagnostics.

## Pseudocode ownership

This workflow owns the consumed regular-grid scalar specialization of `SimulateReactionDiffusion`
and `AnalyzeReactionDiffusionPattern`. Vector reaction systems, arbitrary meshes and boundaries,
adaptive/stiff or implicit solvers, stochastic forcing, and boundary/discretization sensitivity
ensembles remain separate work rather than implied capabilities of this specialization.

## Frozen behavior

- consume a 2–256 by 2–256 row-major finite nonnegative field with positive micrometre spacings,
  finite nonnegative scalar diffusion, a typed linear or logistic reaction, positive final
  time/step, trajectory cadence, and at most 250 million declared cell-steps;
- apply exact reaction half-flows around a periodic explicit five-point diffusion step, rejecting
  `D*dt*(1/dx^2 + 1/dy^2) > 0.5` and every nonfinite, negative, or excessive-work state;
- derive executed step boundaries from the validated ceiling plan so floating-point accumulation
  cannot create work beyond the declared cell-step budget;
- retain complete initial/final fields, mean/variance/range/integral trajectory, CFL/range/work
  diagnostics, and an identity observation model;
- report the centered periodic 2-D discrete-Fourier dominant mode and power fraction. Compute a
  discrete linearized nonzero-mode wavelength band only when the declared reference state is a
  homogeneous reaction equilibrium; otherwise return an explicit unavailable status;
- classify the result as an experimental mechanistic simulation, never biological mechanism proof.

## Validation and claims

With zero diffusion and linear rate `ln(2)`, four values double exactly over one time unit in ten
planned steps and report exactly 40 cell-steps. A 4 by 4 alternating field has a two-micrometre
dominant wavelength with unit nonzero spectral power fraction. One periodic diffusion step with
`D=0.1`, `dt=0.1`, and unit spacing maps the alternating `1.1/0.9` mode to `1.096/0.904` and reports
CFL `0.02`. The positive-growth non-equilibrium control does not receive an instability band.
