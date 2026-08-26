# Task contract — SBI-SBC-01 rejection-ABC simulation-based calibration

Status: complete

Date: 2026-08-25

Parent requirements: BAY-SBI, WS-73.

## User outcome

`marklab bayes growth-front-rejection-abc-sbc` repeatedly draws a true Fisher–KPP growth rate,
simulates its observed mass, runs the canonical rejection-ABC specialization, and returns complete
rank, interval-coverage, failure, seed, and work diagnostics.

## Pseudocode ownership

This workflow owns the uniform-prior deterministic-growth-front/rejection-ABC specialization of
`SimulationBasedCalibration`. Other inference algorithms, stochastic simulators, multidimensional
randomized ranks, conditional coverage, and real calibration remain separate work.

## Frozen behavior

- run 20–10,000 prior-predictive replicates under a named ChaCha20 namespace;
- for each replicate draw truth from the exact uniform growth prior, call the canonical simulator,
  then call IC-0117 with a domain-separated seed and the simulated final mass;
- rank truth against every accepted posterior draw, retain one of `N+1` possible ranks, and report
  histogram, mean normalized rank, and maximum empirical-CDF deviation;
- compute a caller-prespecified equal-tailed interval and overall coverage, retaining each replicate's
  truth, observation, rank, interval, coverage, proposals, and status;
- record acceptance-target failures without fabricating ranks, propagate specification/simulator
  failures, and fail if no replicate completes;
- cap observed plus maximum inference simulation work across all replicates at 250 million declared
  cell-steps and make synthetic diagnostic claims only.

## Validation and claims

One hundred replicates with thirty posterior draws have zero inference failures, mean normalized rank
within `0.1` of one half, empirical 90% interval coverage between `0.8` and one, and byte-identical
replay from seed `20260825`.
