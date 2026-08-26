# Task contract — GSP-WAVELET-01 exact spectral graph wavelets

Status: complete

Date: 2026-08-25

Parent requirements: GSP-01, FR-01, FR-01B, WS-62, GSP-SPECTRAL-01.

`marklab graph wavelet` reuses the canonical graph Fourier owner and applies the fixed band-pass
`g(x)=x exp(-x)` at 1–64 increasing positive scales plus low-pass `h(x)=exp(-x)` at one positive
scale. It retains every node coefficient and scale energy. For the three-node path signal, which is
the lambda-one eigenmode with energy two, scale-one band-pass and low-pass energies both equal
`2 exp(-2)` and scale-two energy is smaller. The result is experimental exact small-graph signal
analysis, without a pathology endpoint or Chebyshev approximation claim.
