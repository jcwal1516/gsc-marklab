# Task contract — SIM-MECH-01 bounded mechanistic tissue coupling

Status: complete

Date: 2026-08-25

Parent requirements: GEN-01, WS-70, WS-71.

## User outcome

`marklab simulate mechanistic-tissue` advances aligned vascular concentration, density,
level-set interface, and stochastic agent state across bounded coupled intervals and returns the
complete final latent state plus coupling/resource diagnostics.

## Pseudocode ownership

This workflow owns the regular-grid one-way interval specialization of
`SimulateMechanisticTissue`. Reciprocal within-interval coupling, dynamic vessels, arbitrary field
systems, microscopy/segmentation noise, fitted parameters, and biological calibration remain
separate work.

## Frozen behavior

- require one exact regular 2-D micrometre grid shared by vascular, density, interface, and agent
  window state, 1–32 coupled intervals, finite nonnegative coupling parameters, positive numerical
  controls, and aggregate cell/event/pair work no greater than 250 million;
- in each interval, call the canonical solvers in order: vascular transport; logistic scalar
  density with growth multiplied by mean oxygen saturation; level-set evolution with local
  density/oxygen speed; agent competition with oxygen-scaled birth and hypoxia-added death;
- derive interval seeds from one named deterministic base schedule, retain absorbing agent
  extinction across later intervals, and never invent events for an empty latent agent state;
- retain interval coupling values and module work, complete final oxygen/density/interface/agent
  states, vascular mass residuals, and aggregate resource accounting;
- expose only identity latent observation and prohibit digital-twin, patient-forecast, causal, or
  calibrated biological claims.

## Validation and claims

A 3 by 3 source field produces mean oxygen one and saturation one half. That coupling maps uniform
density `0.25` to exact `0.5`, translates the planar interface by one micrometre, and induces one
seeded hypoxia-death event. A second interval continues the fields while retaining zero agents and
zero invented agent events.
