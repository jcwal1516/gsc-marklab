# Task contract — EMB-RETRIEVAL-01 exact analogous-region retrieval

Status: complete

Date: 2026-08-25

Parent requirements: EMB-01, EMB-PRED-01, CMP-01, WS-50, WS-52.

## User outcome

`marklab bayes retrieve-analogous-regions` builds a training-only exact standardized-Euclidean region
index and retrieves filtered analogues with component explanations and an OOD diagnostic.

## Frozen behavior

- consume 4–100,000 unique training regions with exact region/patient/site/domain/provenance identity,
  2–128 ordered `embedding_*` dimensions, plus one finite query with the same declared domain and
  provenance; never admit validation/test rows into fitting;
- fit feature means/population SDs only on training regions, require every dimension vary, freeze
  standardized representations, and retain an immutable exact-index artifact with exact-search
  approximation recall `1`;
- validate query domain/provenance, apply an explicit leakage filter (`exclude_same_patient` or
  `exclude_same_patient_and_site`), require at least `k` eligible regions, and rank by exact squared
  standardized Euclidean distance with region-ID tie break;
- return top-k IDs/patient/site, Euclidean distances, per-component squared-distance contributions,
  and query nearest-distance divided by the median positive training leave-one-out nearest distance as
  a descriptive OOD score;
- enforce candidate × component work bounds and retain all input/query/spec/filter identities. A
  nearest region is an analogue under this frozen metric, never “biologically identical.”

## Validation and claims

A four-region two-dimensional fixture queried near `[1,0]` must rank the expected two regions, retain
component contributions summing to squared distance, report exact-search recall one, and emit the
non-identity claim ceiling.
