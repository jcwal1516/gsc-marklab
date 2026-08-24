# Marklab Frontier Spatial Pathology — Research-Backed Algorithm Pseudocode Specification

**Status:** implementation blueprint, not executable code  
**Companion charter:** `marklab_frontier_spatial_pathology_master_plan.md`  
**Scope:** the algorithmically complex work after the classical K/L foundation  
**Units:** physical distances are in micrometres unless an input contract explicitly declares another unit

---

## 0. Purpose and interpretation

This document translates the frontier Marklab program into explicit, implementation-oriented pseudocode. It covers every method family requested after the classical point-process foundation:

1. cohort-valid spatial inference;
2. Bayesian spatial modeling;
3. Bayesian point processes;
4. high-dimensional embedding science;
5. registration, atlas mapping, and transport;
6. graph and higher-order tissue mathematics;
7. topology and mathematical morphology;
8. multimodal Bayesian models;
9. generative tissue modeling and simulation-based inference;
10. 3-D, longitudinal, and evolutionary models;
11. causal, interference, perturbational, and active-design research.

The algorithms are not all at the same maturity level. Each specification is labelled:

- **ESTABLISHED:** the statistical object and principal algorithm are well established;
- **ADVANCED:** established methodology with substantial implementation and diagnostic burden;
- **EXPERIMENTAL:** credible research method, but not suitable for an unqualified stable claim;
- **RESEARCH_ONLY:** exploratory method whose identifiability or validation is highly design-dependent.

The pseudocode is deliberately stricter than ordinary library sketches. Every inferential or fitted result must carry:

- the estimand or prediction target;
- the biological replication unit;
- the randomization, likelihood, prior, or generative assumptions;
- exact versus approximate execution mode;
- typed failure and unavailable states;
- seeds, backend versions, artifact digests, and coordinate frames;
- calibration and diagnostics;
- claim maturity.

---

