# Task contract — GSP-SPECTRAL-01 canonical graph Fourier workflow

Status: complete

Date: 2026-08-25

Parent requirements: FND-03, GSP-01, FR-01, FR-01A, WS-61, WS-62.

`marklab graph spectral` consumes 2–128 exact-ID finite 2-D micrometre nodes with scalar signal,
one positive physical radius, binary weights, a combinatorial Laplacian, ordered nonoverlapping
frequency bands, and an exact pair limit. It canonicalizes nodes/edges by ID, rejects isolates,
digests the complete graph contract, and uses a bounded deterministic Jacobi decomposition with
canonical eigenvector signs. It retains the dense Laplacian, ordered modes, Fourier coefficients,
reconstruction error, band energy/fractions, pair work, and rotations. The three-node path oracle
has eigenvalues `0,1,3`; signal `[1,0,-1]` has middle-band energy `2` and zero other-band energy.
This is an experimental bounded graph-signal specialization, not a biological adjacency claim.
