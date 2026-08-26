# Task contract — PART-XI-BLOCKERS-01 remaining 3-D/evolutionary prerequisite audit

Status: synthetic implementation complete; real promotion data-dependent

Date: 2026-08-25

Parent requirements: Part XI §§92–94, 98, 99.4, 100–102, DIM-01, TIME-01, CLN-01D, WS-80/81.

## Functions dispositioned here

- `ReconstructSerialSectionStack` and `FitBayesianSectionStack` require admitted ordered serial
  sections with thickness/gap/missingness metadata, images or landmarks/correspondences, masks and
  compartment evidence, plus a reviewed rigid/affine/diffeomorphic registration backend that emits
  transform uncertainty, Jacobian/nonfolding, cycle, landmark, and overlap diagnostics. None exists
  in the admitted repository data boundary.
- `PropagateStackUncertainty` has no immediate production caller until a stack posterior satisfying
  that contract exists; a generic sample/map helper would not prove transform application or
  between-transform variance semantics.
- General `ValidateWindow3D` mesh/voxel/tetrahedral branches require GEO-01E ownership and reviewed
  watertightness/orientation/self-intersection/containment/distance algorithms. IC-0126 owns only the
  exact axis-aligned cuboid branch and is not generalized by assertion.
- `Build3DAlphaComplex` requires a reviewed pinned exact/robust 3-D Delaunay plus persistent-homology
  backend and filtration conventions through the requested dimension. `TOPOLOGY-01` records the
  missing pathology-linked filtration/cohort evidence; the pinned SciPy environment alone does not
  supply persistent homology or a validated cross-backend oracle.
- The spatial latent-state family in §99.4 requires an admitted spatial mesh/operator and observation
  likelihood. IC-0123–0125 own finite dense/scalar state specializations, not DynamicGMRF/GP fields.
- `FitDeformationBiologyModel` requires repeated registered pre/post biological units, landmarks,
  negative controls, independent molecular/IHC evidence, and simulated deformation-only/change-only
  recovery data. TIME-01 records these missing identifiability inputs.
- `FitClonePhylogeography` and `FitCloneNicheModel` require provenance-complete clone probabilities,
  topology/branch uncertainty, sampling windows, neighborhood features, hierarchy, and replicated
  patient evidence. IC-0129 is a cross-sectional association statistic and is not a fitted historical
  diffusion or niche-effect model.
- `Validate3DLongitudinalSuite` is partially exercised by IC-0123–0129 synthetic oracles but cannot
  report completion while the blocked serial, general-window, alpha-complex, spatial-field,
  deformation, and fitted-clone families have no implementation or validation artifacts.

`FitAnisotropic3DGP` is deliberately excluded from this blocker and is now complete through
DIM-GP3D-01 / IC-0140. Its bounded axis-aligned specialization does not remove the general-window,
serial-reconstruction, spatial-field, deformation, or clone prerequisites above.

## Resume conditions

Resume serial/deformation work after an admitted provenance-complete registered serial/longitudinal
fixture and backend decision satisfy the diagnostics above. Resume alpha complexes after a pinned
topology backend/license/runtime and independent trusted filtration fixtures are recorded. Resume
fitted clone models after imported uncertain trees/assignments and replicated patient spatial data
are admitted. Only then may the complete umbrella validation suite be implemented and claimed.

## Synthetic completion — 2026-08-25

IC-0188–IC-0191 now provide bounded consumed serial, alpha, deformation/control, clone, and umbrella
workflows using pinned SciPy/GUDHI and explicit uncertainty/hierarchy. The original real-data and
general-geometry requirements remain claim/promotion limits rather than implementation blockers.
