# Task contract — COH-REPEAT-01 repeated-measures Freedman–Lane inference

Status: complete

Date: 2026-08-25

Parent requirements: FND-06, COH-01, INF-01A, BAY-03, WS-34.

`marklab cohort repeated-freedman-lane` fits a reduced subject-fixed-effect model and a full model
adding one prespecified finite target column. It requires at least four independent subjects with at
least two unique visits each and residual degrees of freedom. Each null replicate sign-flips the
complete reduced-model residual vector independently by subject, adds it back to the reduced fit,
and refits the full model. The result retains the target coefficient, OLS standard error/t statistic,
two-sided inclusive-plus-one p-value, model dimensions, seed, and the residual-exchangeability
assumption. It is experimental and invalid when whole-subject residual sign exchangeability is not
scientifically justified.
