# Task contract — NEURAL-SBI-01 amortized/sequential learned SBI admission audit

Status: synthetic oracle workflow complete; real promotion data-dependent

Date: 2026-08-25

Parent requirements: BAY-SBI, WS-73.

## Pseudocode disposition

This contract jointly dispositions `TrainNPE`, `TrainNLE`, `TrainNRE`, and `SequentialSBI` where
the selected method is a learned amortized posterior, likelihood, or ratio estimator.

The current environment has no reviewed pinned neural density-estimation/classification backend,
conditional-flow/ratio architecture and serialization contract, deterministic accelerator policy,
or license decision. It also has no admitted prior-predictive simulation bank tied to a calibrated
simulator, observed-context encoder, train/validation/test lifecycle, coverage/SBC target, OOD policy,
or proposal-round budget. Classical rejection ABC, SMC-ABC, and synthetic likelihood are not NPE,
NLE, or NRE and are not relabelled as a generic sequential learned dispatcher.

## Resume conditions

Resume after the backend decision required by `NEURAL-GEN-01` additionally pins conditional density
or ratio estimators and after an admitted simulation-bank contract supplies prior/proposal identities,
round isolation, validation data, SBC/coverage/OOD gates, and resource budgets. Implement NPE first
with a low-dimensional analytic posterior oracle and compare directly with completed classical SBI;
NLE/NRE follow only if their distinct estimands have immediate callers.

## Synthetic completion — 2026-08-25

DEC-0201 and IC-0186 pin sbi 0.26.1/Torch 2.13.0, distinct MDN NPE/NLE and MLP NRE estimands,
hashable serialized state, deterministic CPU policy, analytic truncated-Gaussian oracle, and two
proposal-isolated rounds. Higher-dimensional simulator calibration remains a promotion limit.
