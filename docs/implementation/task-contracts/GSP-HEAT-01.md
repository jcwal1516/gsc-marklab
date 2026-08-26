# Task contract — GSP-HEAT-01 exact small-graph heat methods

Status: complete

Date: 2026-08-25

Parent requirements: GSP-01, FR-01, FR-01A, WS-62, GSP-SPECTRAL-01.

`marklab graph heat` reuses the canonical graph/Laplacian/eigensystem and evaluates
`H_t=U diag(exp(-t lambda)) U'` for 1–64 increasing nonnegative times. It retains every dense
kernel, its application to the declared node signal, diagonal heat signatures, and declared-pair
uniform-node-L2 diffusion distances. At zero it must reproduce identity, the input signal, unit
signatures, and `sqrt(2)` between distinct delta rows; positive-time kernels must remain finite.
This is bounded exact small-graph diffusion, not a matrix-free approximation or calibrated physical
diffusion scale.
