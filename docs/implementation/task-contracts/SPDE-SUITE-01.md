# SPDE-SUITE-01 — Shared rectangular finite-element SPDE workflow

Own `BuildSPDEMaternField`, `BuildSPDE_LGCP`, and the SPDE factor caller through IC-0200. The
missing-command red preceded implementation. A 7x7 mesh yields 49 vertices/72 triangles, positive
precision, exact barycentric row sums, conserved LGCP count with right-heavy intensity, and a
low-error one-factor reconstruction across two mesh resolutions.
