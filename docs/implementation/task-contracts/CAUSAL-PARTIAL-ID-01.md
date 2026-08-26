# Task contract — CAUSAL-PARTIAL-ID-01 bounded-outcome Manski ATE bounds

Status: complete

Date: 2026-08-25

Parent requirements: Part XII §111 `PartialIdentificationBounds`, CAU-01B, WS-82.

`marklab causal manski-bounds` consumes unique observations with binary treatment and finite outcome,
known finite outcome support `[L,U]`, and a maximum observation count. Outcomes must lie in support
and both treatment groups must be observed. Without ignorability, monotonicity, exclusion, or other
cross-potential-outcome assumptions, it computes:

`E[Y(1)] in [p*mean(Y|Z=1)+(1-p)*L, p*mean(Y|Z=1)+(1-p)*U]` and
`E[Y(0)] in [(1-p)*mean(Y|Z=0)+p*L, (1-p)*mean(Y|Z=0)+p*U]`.

ATE lower is the lower treated-potential mean minus the upper control-potential mean; ATE upper is
the reverse endpoint. The result retains observed group sizes/means/difference, treatment fraction,
both potential-outcome intervals, exact assumptions, zero inclusion, and the absence of sampling
uncertainty. With support `[0,1]`, treated mean `0.7`, control mean `0.3`, and treatment fraction
`0.5`, the ATE interval is `[-0.3,0.7]`. This is partial identification under bounded outcomes only.
