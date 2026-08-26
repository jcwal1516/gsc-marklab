# Task contract — TOPOLOGY-01 topology and mathematical morphology laboratory

Status: complete for bounded experimental specializations

Date: 2026-08-25

Parent requirements: TOP-01, TOP-01A, TOP-01B, GEO-01E, CMP-01F, WS-63.

## Pseudocode ownership

This contract originally owned the disposition of `ValidateFiltration`, `PersistentHomology`,
`BuildAlphaFiltration`, `BuildWitnessFiltration`, `PersistenceLandscape`, `PersistenceImage`,
`EulerCharacteristicCurve`, `MinkowskiFunctionals2D`, `MorphologicalFunctionalCurve`,
`ConnectivityTransition`, `ComparePersistenceDistributions`, `TopologyStabilityLaboratory`, and
`ValidateTopologySuite`. TOP-ALPHA-01, TOP-WITNESS-01, TOP-MORPH-01, and TOP-CONNECT-01 now own the
first ten declarations through bounded experimental workflows. TOP-COMPARE-01, TOP-STABILITY-01,
and TOP-VALIDATE-01 now own the final three declarations through IC-0162–IC-0164. Real
pathology-linked filtration/segmentation adequacy, sparse-memory scaling, and external validation
remain limitations rather than missing function owners.

## Named missing data and backend

- TOP-01A has no prespecified pathology-linked filtration, independently replicated patient cohort,
  or outcome that makes persistence/landscape/image variation interpretable; alpha-complex geometry
  also lacks the promoted GEO-01E exact/robust predicate owner;
- TOP-01B has no admitted binary mask/interface representation with physical pixel/polygon scale,
  connectivity/perimeter convention, segmentation uncertainty, pathology endpoint, and replicated
  cohort for Euler/Minkowski/morphological/percolation curves;
- GUDHI 3.13.0 is now lock-pinned with its effective CGAL/GPLv3 alpha-complex license recorded;
  scikit-image 0.26.0 is BSD-3-Clause pinned for raster morphology.

Generic matrix reduction, landscape/image, Euler, raster morphology, or union-find helpers have no
immediate production caller until those scientific inputs are admitted. Implementing them now would
create a synthetic topology API while the master plan classifies the family as watch/experimental and
requires endpoint-linked replication. Existing polygon-window topology validation is an observation
window integrity owner, not persistent homology or tissue morphology.

## Resume condition

Resume as one topology/morphology laboratory when a fixed filtration or mask/interface basis,
physical scales, perturbation model, patient/site design, and pathology endpoint are admitted. Pin a
proven persistence backend for TOP-01A; implement the more interpretable Euler/Minkowski/percolation
branch first when TOP-01B data arrive; validate hand Betti/Euler/Minkowski/percolation oracles and
patient-level comparison before any biomarker claim.
