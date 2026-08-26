# Task contract — CAUSAL-ROSENBAUM-01 matched-pair Rosenbaum sign sensitivity

Status: complete

Date: 2026-08-25

Parent requirements: Part XII §110.1 `RosenbaumSensitivity`, CAU-01B, WS-82.

`marklab causal rosenbaum-sign-sensitivity` consumes unique matched sets containing exactly two
unique units, exactly one treated unit, finite outcomes, a strictly increasing finite Γ grid with
`Γ>=1`, alpha in `(0,1)`, and a maximum set-by-grid work bound. Zero treated-minus-control
differences are reported as ties and excluded from the sign statistic. For `S` positive differences
among `N` non-ties, the one-sided greater-effect hidden-bias model bounds success probability by
`1/(1+Γ)` and `Γ/(1+Γ)` and reports exact binomial upper-tail lower/upper p-value bounds using stable
log recurrences.

The critical Γ is the largest supplied value whose worst-case upper p-value remains at most alpha,
or unavailable when none does. For five positive pairs, Γ values one, two, and four yield upper
p-value bounds `0.5^5`, `(2/3)^5`, and `0.8^5`; at alpha `0.05`, critical Γ is one. This is a
matched-pair sign-test sensitivity model, not confounding correction or identification.
