# Capabilities and current evidence

Last updated: 4 September 2026. This is a workflow guide, not a clinical validation certificate.
Method-specific input admission, diagnostic results and resource limits remain authoritative.

Use `marklab --help` for the complete command families, `marklab <family> --help` for methods,
and `marklab <family> <method> --help` for the exact inputs and controls. Run
`marklab backend doctor` before a Python-backed analysis; it checks installation and direct
package pins, not estimator correctness, convergence, GPU support or biological validity.

| Intended workflow | Available foundation | Limits to check before running |
|---|---|---|
| Spatial organization within a physical 2-D ROI | Exact polygon windows, classical K/L and nearest/empty-space workflows, typed nulls and durable execution | The selected estimator's edge correction, mark type, null conditioning, pair/draw limits and calibration evidence. A bounded oracle does not establish universal external agreement. |
| Multiplex protein or cell-state panel | Nullable named quantitative/binary/categorical MarkTable columns; `study run` connects shared Moran/Geary graphs, equal-slide patient reduction, Max-T, durable resume and atomic reports; the Python client admits a bounded AnnData/H5AD profile | Explicit physical coordinates/window, channel units/status/provenance and independent patients are required. Selected channels use observed-cell subgraphs; an unavailable required slide endpoint blocks complete-family inference. This profile remains experimental; nominal categories are retained, not treated as quantitative measurements. See the [complete workflow](multiplex-study.md). |
| Patient/cohort comparison | Patient, paired, blocked and cluster inference; hierarchical bootstrap; Max-T; declared-margin inference | Prespecified endpoints and actual independent patient identities. A nonsignificant difference does not establish equivalence. Protocol margins and their scientific rationale must be supplied. |
| Bayesian spatial fields and hierarchies | Pinned PyMC/NumPyro workers, admitted diagnostics and selected agreement/calibration workflows | Exact required backend, computational domain, convergence, posterior prediction and family-specific validation. The recent real joint location–embedding pilot remains diagnostic-only after nonconvergence and failed embedding posterior prediction. |
| Cell/patch/region/slide embeddings | Verified identity-bound artifacts, explicit row status, spatial/vector summaries and selected real callers | Coordinate/projection correspondence, measurement status and independent patient reduction. Storage scale is not evidence of inference scale. |
| Tissue interfaces and compartments | Exact supported binary geometry, signed interfaces and contact/fragmentation methods | Supported geometry and annotation semantics. Broader multiclass/object uncertainty and pathology validation remain caller-dependent. |
| 3-D, longitudinal and causal designs | Bounded mathematical and model workflows with explicit design admission | Actual section/time/intervention identities and justified design assumptions. A fitted model alone does not establish causal identification or biological replication. |

Existing native and Python workflows retain deterministic seeds, finite or explicitly unavailable
results, bounded work and typed durable replay. These guarantees describe software behavior;
they do not make every method appropriate for every sample size, tissue geometry or clinical claim.

The installed-runtime relocation test executes a bounded real PyMC fit and verifies exact replay
without an interpreter. The local Unix archive smoke verifies asset layout. Clean hosted CI,
Windows execution and broader deployment matrices still need their own executed evidence.

For exact evidence and unresolved prerequisites, see the
[program tracker](implementation/PROGRAM_TRACKER.md),
[validation ledger](implementation/VALIDATION_LEDGER.md), and
[current architecture task](implementation/task-contracts/ARCH-INTEGRATION-01.md).
The [active roadmap](implementation/ACTIVE_ROADMAP.md) identifies the next complete workflows;
older operational records are preserved in the roadmap history.
