# Task contract — COH-BOOT-EQV-01 hierarchical bootstrap equivalence

Status: complete

Date: 2026-08-25

Parent requirements: EQV-01, EQV-01B, EQV-01C, COH-01, INF-01D, WS-34.

`marklab cohort bootstrap-equivalence` immediately consumes the patient-first/specimen-within-patient
bootstrap owned by COH-HBOOT-01. It applies exact prespecified finite lower/upper margins to that
workflow's two-sided nearest-rank percentile interval and reports equivalence only when both interval
bounds are strictly internal. The result retains hierarchy, interval method/level, margins/rationale,
replicate evidence, and an experimental percentile-interval claim ceiling. BCa/studentized methods
and estimator-specific smoothness calibration remain separate.
