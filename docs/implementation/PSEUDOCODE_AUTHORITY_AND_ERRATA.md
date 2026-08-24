# Pseudocode Authority, Installation, and Normative Corrections

## Authority order

When these documents disagree, use this order:

1. `docs/implementation/MASTER_PLAN.md` — authoritative scope, requirement IDs, workstream order, promotion policy, and product decisions.
2. This file — normative corrections to the current pseudocode document.
3. `docs/implementation/PSEUDOCODE_FULL.md` and the exact split files under `docs/implementation/pseudocode/` — algorithm detail subordinate to the master plan.
4. `docs/implementation/PSEUDOCODE_CROSSWALK_REVIEW.md` — read-only audit evidence, not implementation authority.
5. `docs/implementation/STATUS.md` and active task handoff — current implementation state only.

The full pseudocode source has SHA-256:

```text
c0508109be2a954502bb1b1989ee98936f1c1838bd25fd92fcb33f09054fda51
```

The split files concatenate byte-for-byte to the full source. Verify this with:

```bash
python3 docs/implementation/verify_pseudocode_pack.py
```

## Scope correction

The pseudocode specification begins after the classical two-dimensional K/L foundation. It does not replace or authorize missing foundational K/L, point-process-window, edge-correction, null, or stable mark-statistic work. Those remain governed by the master plan.

## Normative corrections

### 1. Stale requested-function index

Part XIV is informative only. Its section-number cross-references are stale. Use the actual headings in the split files and the master-plan requirement/workstream IDs. Do not derive execution order or ownership from the old index.

### 2. Canonical duplicate owners

The following repeated pseudocode concepts must have one internal implementation owner and domain-specific wrappers only:

| Concept | Canonical owner contract |
|---|---|
| Simulation-based calibration | one shared Bayesian/SBI calibration service; model-family wrappers supply simulators and posterior summaries |
| Posterior-predictive checking | one shared PPC engine; domain wrappers define statistics and discrepancy functions |
| Stable covariance | one common finite checked covariance primitive |
| MMD | one cohort-valid kernel two-sample implementation; embedding/fingerprint callers supply kernels |
| Entropic Sinkhorn | one transport primitive; correspondence and FGW callers reuse it |
| Transform-uncertainty propagation | one registration uncertainty service used by serial-section and 3-D workflows |
| Validation orchestration | one validation runner with family-specific suites |

Do not create parallel `new`, `v2`, `final`, or local copies of these algorithms.

### 3. Algorithm execution modes

`AlgorithmDescriptor` defines exact and approximate mode sets. `SelectExecutionMode` must derive its supported mode set from those fields; it must not read an undefined `descriptor.modes` field.

### 4. Result maturity and claim ceilings

The only maturity tiers are those authorized by the master plan. `association_only` is a claim ceiling or limitation reason, not a new maturity tier.

### 5. Spatial mediation

`SpatialMediationAnalysis` is not authorized as a stable or scheduled public method by the current master plan. It requires an explicit decision record, a causal design contract, and identified estimands before roadmap placement or implementation.

### 6. Internal-only enabling algorithms

Balanced `SinkhornOT`, `SoftPairHistogram`, and `SummaryMatchingLoss` are internal enabling primitives unless the master plan is amended to authorize separate public result families.

### 7. Backend algorithms

Pseudocode for HMC/NUTS, INLA-style inference, nonrigid registration, optimal transport, persistent homology, multimodal factor models, neural generators, and SBI specifies capability and contracts. It does not mandate a native Rust port. Backend admission, version, license, environment digest, semantic oracle, and fallback policy must be frozen first.

### 8. Missing master-plan method families

The current pseudocode does not directly specify every master-plan capability. The gap register below is binding: missing functions must be added through a dedicated, reviewed pseudocode task before implementation. They must not be improvised from adjacent methods.
