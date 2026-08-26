# Task contract — COH-FINGERPRINT-01 versioned spatial fingerprint and distance

Status: complete

Date: 2026-08-24

Parent requirements: CMP-01A, FR-02, EMB-PATCH, WS-34, WS-51.

## User outcome

`marklab cohort fingerprint-distance` builds two versioned structured sample fingerprints from
prespecified named common-axis components and reports a decomposed weighted-L2 distance.

## Frozen behavior

- Strict CSV columns are `sample_id,component,axis,value,uncertainty,weight`; exactly the two
  CLI-declared sample IDs are present. Every component has the identical finite strictly increasing
  axis in both samples, at least two points, finite values, finite nonnegative uncertainties, and
  one finite positive prespecified component weight repeated exactly on all component rows.
- Version one declares normalization `none`, uncertainty weighting `none`, missing-component policy
  `reject`, and no training-fitted transform. Component names and identities remain explicit.
- Each fingerprint has a canonical SHA-256 content digest and shares a canonical specification
  digest. Per-component distance is the square root of the trapezoidal integral of squared value
  difference. Total distance is the stable sum of prespecified weight times component distance and
  retains every contribution.
- A one-component fixture with curves `[0,0]` and `[3,4]` on axis `[0,1]`, weight 2, has component
  distance `sqrt(12.5)` and total distance `2*sqrt(12.5)`. Axis/component/weight mismatch and
  undeclared/missing samples fail before output publication.

## Non-goals

No component computation from raw spatial data, interpolation, normalization, uncertainty
weighting, missing-component imputation, learned weights, retrieval index, cohort p-value, durable
project, or biological claim in version one.

## Completion evidence

- `cargo +1.96.0 test --locked --package marklab-cohort` passed independently computed SHA-256
  goldens, exact weighted-L2 hand oracle, and incompatible-axis rejection.
- `cargo +1.96.0 test --locked --all-features --test cohort_fingerprint_cli` passed the structured
  two-fingerprint workflow; warning-denied cohort Clippy passed.
