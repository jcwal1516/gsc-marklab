# SPDE-FACTOR-01 — Rectangular SPDE spatial factors

Status: complete for the IC-0200 specialization

Own `FitSPDESpatialFactorModel` through the shared SPDE-SUITE-01 mesh/precision/projection owner.

The pseudocode requires a mesh, SPDE precision `Q_spde`, and observation projection matrix. The
repository has no promoted exact 2-D mesh/window/projection owner; BAY-04/BAY-05 and prior SPDE
contracts retain the same prerequisite. Graph-Laplacian IC-0170 and exact-GP IC-0172 are not
equivalent and are not relabelled. Resume only when the geometry workstream admits that owner and a
reviewed INLA/Laplace/VI backend composition. This historical prerequisite was resolved on
2026-08-25 by DEC-0215 for a hole-free rectangular specialization only. The earlier graph/GP owners
remain distinct and broader constrained geometry remains unavailable.
