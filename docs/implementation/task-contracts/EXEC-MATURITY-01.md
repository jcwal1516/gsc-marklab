# Task contract — EXEC-MATURITY-01 result-maturity precedence

Status: complete

Date: 2026-08-25

Parent requirements: Part XIII §131 `DetermineResultMaturity`, REL-01, WS-94.

`marklab policy determine-maturity` consumes one declared method maturity (`validated`,
`established`, `experimental`, `research_only`, `unsupported_for_claim`), execution mode
(`exact`,`approximate`), and explicit diagnostic/provenance/claim booleans. It never upgrades.
Incomplete provenance, nonconvergence, severe diagnostics, or unsupported causal identification are
terminal `unsupported_for_claim` reasons. Unvalidated approximation caps at `research_only`.
Predictive clinical claims without external validation and Bayesian results without both SBC and
posterior-predictive requirements cap at `experimental`. Reasons are emitted once in fixed policy
order, including softer failures even when a terminal failure controls the final maturity.

The primary oracle starts `validated`, uses unvalidated approximation, lacks predictive external
validation, and lacks Bayesian SBC; it ends `research_only` with all three reasons. Adding an
unsupported causal-identification claim ends `unsupported_for_claim`. This policy result does not
promote any existing free-form result schema or certify a method.
