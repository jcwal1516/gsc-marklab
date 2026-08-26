# Task contract — VALIDATION-LADDER-01 contiguous real-data validation ladder

Status: complete

Date: 2026-08-25

Parent requirements: Part XIII §128 `RealDataValidationLadder`, VAL-01, WS-93.

`marklab policy validation-ladder` consumes a nonempty method ID and exactly one ordered declaration
for stages zero through five: synthetic oracle, public technical benchmark, internal real cohort with
controls, held-out same-site patients, external site/platform, and prospective/perturbational
validation. A completed stage requires at least one unique nonempty evidence reference. Completion
must be contiguous from Stage 0; later completed stages after a gap are invalid rather than promoted.
Every stage retains evidence and unresolved risks.

The result reports the highest contiguous completed stage or unavailable when Stage 0 is incomplete,
its fixed label, complete stage records, and the union of unresolved risks at and above the first
incomplete stage. Completing stages 0–2 and leaving 3–5 incomplete promotes only to Stage 2. This is
evidence bookkeeping; it does not verify evidence contents or establish generalization.
