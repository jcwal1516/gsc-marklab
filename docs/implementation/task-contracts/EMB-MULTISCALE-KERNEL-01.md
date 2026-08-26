# Task contract — EMB-MULTISCALE-KERNEL-01 prespecified multiscale embedding kernel

Status: complete

Date: 2026-08-25

Parent requirements: EMB-01, EMB-PATCH, CMP-01, WS-50, WS-51.

## User outcome

`marklab bayes multiscale-embedding-kernel` computes a prespecified weighted kernel between two
samples' exact scale-aligned embedding summaries and reports scale-grid sensitivity.

## Frozen behavior

- consume exactly two unique sample IDs, 1–64 exact increasing positive physical scales, 1–128
  ordered `embedding_*` dimensions at every sample/scale, and one exact positive finite prespecified
  weight per scale summing to one within tolerance;
- require identical sample scale grids and feature order. Evaluate the same declared PSD base kernel
  at every scale: linear dot, nonzero-vector cosine, RBF with positive frozen bandwidth, or Laplacian
  with positive frozen scale;
- compute `sum_l scale_weight_l * base_kernel(summary_a_l, summary_b_l)` in compensated `f64`, retain
  each raw kernel/weight/contribution, and report drop-one-scale sensitivity with remaining weights
  renormalized to one;
- retain exact sample/input/weight, feature, physical-scale, base-kernel, parameter, and work identities.
  Scale weights are caller-prespecified; no test-outcome selection, inference, biological identity, or
  real-asset claim is made.

## Validation and claims

For linear scale kernels `3` and `2` with weights `1/4` and `3/4`, the total must be `2.25`, per-scale
contributions `[0.75,1.5]`, and renormalized drop-scale totals `[2,3]`.
