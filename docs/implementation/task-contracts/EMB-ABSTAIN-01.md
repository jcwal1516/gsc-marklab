# Task contract — EMB-ABSTAIN-01 prespecified prediction abstention

Status: complete

Date: 2026-08-25

Parent requirements: EMB-PRED-01, WS-52.

## User outcome

`marklab bayes apply-abstention` applies frozen validation-derived uncertainty and OOD thresholds to
finite predictions and emits an auditable retain/abstain decision for each prediction.

## Frozen behavior

- require exact non-empty prediction identity, finite prediction, finite nonnegative uncertainty and
  OOD score, and finite nonnegative prespecified thresholds;
- retain equality at either threshold and abstain only on strict exceedance;
- report uncertainty and OOD reasons independently and in canonical order; suppress the prediction
  value on abstention rather than returning it as an actionable prediction;
- label the threshold source `prespecified_validation_policy`. Threshold selection, clinical utility,
  and downstream action remain outside this owner.

## Validation and claims

The CLI oracle retains a low-uncertainty/in-domain prediction, abstains for uncertainty alone, and
reports both reasons when uncertainty and OOD exceed their thresholds.
