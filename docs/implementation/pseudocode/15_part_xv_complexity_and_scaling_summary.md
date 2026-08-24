# Part XV — Complexity and Scaling Summary

| Family | Representative exact cost | Scalable/approximate path | Dominant validation issue |
|---|---:|---|---|
| Restricted permutation | `O(B × estimator)` | shared plans, sequential stopping only if valid | exchangeability and biological unit |
| Hierarchical bootstrap | `O(B × estimator)` | streaming summaries | correct hierarchy |
| MMD | `O(n²)` | block/linear/random-feature MMD | kernel selection and patient unit |
| Energy distance | `O(n²)` | block/energy sketches with error | metric and unit |
| Exact GP | `O(n³)` time, `O(n²)` memory | inducing, NNGP, SPDE | approximation/calibration |
| HMC/NUTS | gradient-dependent | blocked/sparse/VI/Laplace | convergence and geometry |
| LGCP grid/SPDE | sparse factorization dependent | mesh/low-rank/VI | quadrature and latent field |
| Gibbs exact likelihood | intractable normalizer | exchange MCMC/pseudolikelihood | doubly intractable inference |
| Vector variogram | `O(n²d)` | indexed pairs/blocking/projections | high-dimensional stability |
| OT/FGW | `O(nm)` memory; iterative | sparse/landmark/minibatch | entropy/mass/plan nonidentifiability |
| Eigen graph methods | full `O(n³)` | Lanczos/Chebyshev `O(Km)` | graph dependence/error |
| Persistent homology | output/filtration dependent, worst high | sparse filtrations/witness | combinatorial explosion |
| Multimodal factors | `O(observed_entries × K)` per VI epoch | minibatch/sparse | factor identifiability/missingness |
| Mechanistic PDE | mesh × steps × solver | adaptive/multigrid/GPU | solver/model misspecification |
| SBI | simulations dominate | amortization/sequential design | simulation gap/calibration |
| 3-D registration | image/mesh dependent | multiresolution/GPU | deformation uncertainty |
| Interference causal | assignment/exposure dependent | cluster-level influence methods | positivity/identification |
| Active design | candidate × posterior simulations | surrogate/EIG amortization | utility misspecification |

---

