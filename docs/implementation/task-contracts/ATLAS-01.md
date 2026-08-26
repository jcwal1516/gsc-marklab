# Task contract — ATLAS-01 reference/query spatial atlas workflow

Status: synthetic workflow complete; real promotion data-dependent

Date: 2026-08-25

Parent requirements: NIC-01F, FR-03, FR-03A, FR-03B, WS-60.

## Pseudocode ownership

This contract owns the current blocked disposition of `BuildSpatialAtlas`, `MapQueryToAtlas`, and
`ValidateAtlasMapping`.

## Available prerequisites

- exact training-only analogous-region retrieval with domain/provenance checks and OOD score;
- balanced, KL-unbalanced, fixed-mass partial, dustbin, balanced FGW, and fixed-mass partial-FGW
  descriptive transport primitives with sensitivity and non-correspondence semantics;
- synthetic cell/patch/region embedding contracts and patient-level inference primitives.

## Named missing data and boundaries

- no admitted independent reference/query atlas cohort binds modality, model/checkpoint, stain,
  support objects, patient/site hierarchy, feature uncertainty, geometry, and training population;
- no held-out reference patients with domain/region labels and missing-type, subsampling, deformation,
  stain/scanner, or site perturbation evidence exist to calibrate probabilities or unmatched mass;
- registration/deformation uncertainty and homologous-anatomy evidence remain blocked on REG-01B,
  GEO-01, BACK-01, and WS-55.

`BuildRegionRetrievalIndex` is not silently renamed into an atlas: its serialized view omits the
private training rows needed for replay and it estimates neither prototype distributions nor
cross-patient variability. `RetrieveAnalogousRegions` is not `MapQueryToAtlas`: it returns exact
nearest neighbors under a frozen metric, not calibrated domain probabilities or propagated
embedding/segmentation/registration uncertainty. A synthetic-only atlas schema would violate the
immediate-caller rule because no admitted production reference/query artifact can consume it.

## Resume condition

Resume all three functions together when a provenance-complete reference/query cohort and held-out
validation split are admitted. The first workflow must build one versioned replayable atlas, map
held-out queries through prespecified nearest/soft/transport methods, and validate calibration,
recovery, stability, OOD/unmatched mass, and claim limits without asserting cell identity.

## Synthetic completion — 2026-08-25

IC-0183 now owns a consumed biological-similarity specialization using frozen measured region
features, patient-replicated domain prototypes, shrinkage Mahalanobis probability/OOD mapping, LOPO
validation, and declared perturbations. The original missing-real-cohort limits remain the promotion
ceiling rather than an implementation blocker. No physical registration or cell identity is claimed.
