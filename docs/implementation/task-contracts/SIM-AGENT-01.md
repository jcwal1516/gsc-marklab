# Task contract — SIM-AGENT-01 bounded stochastic agent competition

Status: complete

Date: 2026-08-25

Parent requirements: GEN-01, WS-70, WS-71.

## User outcome

`marklab simulate agent-competition` runs a deterministic-seed continuous-time two-species spatial
birth/death/movement/phenotype-switch process and returns its final observed pattern, retained latent
event history, complete event counts, and termination/resource diagnostics.

## Frozen behavior

- consume one bounded rectangular micrometre window, 1–1,000,000 unique in-window agents, two sets
  of finite nonnegative event rates, a positive competition radius, nonnegative opposite-neighbor
  death increment and movement/birth jitter scales, positive final time, seed, and explicit event,
  agent, pair-visit, and retained-log limits;
- recompute exact opposite-species neighbor counts with a bounded pair scan before every event;
  suppress birth at the agent cap and fail before exceeding 250 million declared pair visits;
- sample Gillespie waiting times and events from deterministic ChaCha20 namespace
  `agent_competition_v1_chacha20`; reflect birth/movement coordinates at the rectangle and issue
  deterministic noncolliding generated IDs;
- preserve complete birth/death/move/switch counts even when the event log is truncated, distinguish
  final-time, maximum-event, zero-rate, and all-agents-removed termination, and sort final output IDs;
- make no evolutionary truth, causal treatment, calibrated population, or scalable dynamic-index
  claim. The exact pair scan is intentionally bounded.

## Validation and claims

A one-agent death-only process removes the agent in one event and produces byte-identical results
under repeated seed 42. A zero-rate process stops at time zero with the input agent unchanged and no
invented event.
