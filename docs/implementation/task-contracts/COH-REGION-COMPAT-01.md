# Task contract — COH-REGION-COMPAT-01 descriptive region compatibility

Status: complete

Date: 2026-08-25

Parent requirements: COH-01, EMB-01, WS-50.

## User outcome

`marklab cohort region-compatibility` compares two exact versioned spatial fingerprints and reports
weighted distance plus propagated within-patient endpoint uncertainty without implying population
inference.

## Frozen behavior

- reuse the complete COH-FINGERPRINT-01 specification, exact-axis validation, weighted curve-L2
  distance, digest, missing-component, and resource contracts;
- interpret stored nonnegative endpoint uncertainties as standard uncertainties, assume endpoints and
  components are independent, and apply the first-order L2 delta method at the observed curves;
- combine weighted component standard uncertainties by root-sum-square; return unavailable rather
  than inventing a value at a zero-distance nondifferentiable component with positive uncertainty;
- label every result `within_patient_descriptive_only`. Population inference requires a separate
  replicated patient-level design.

## Validation and claims

For values `[0,0]` versus `[3,4]` on axis `[0,1]`, weight two, and standard uncertainty `0.1` at all
four endpoints, distance is `2*sqrt(12.5)`, component standard uncertainty is `0.1`, and weighted
total standard uncertainty is `0.2`.
