# Task contract — NEURAL-GEN-01 frontier neural generator admission audit

Status: synthetic workflows complete; real promotion data-dependent

Date: 2026-08-25

Parent requirements: GEN-01A, GEN-01B, WS-72.

## Pseudocode disposition

This contract jointly dispositions `NeuralCoxIntensity`, `TrainNeuralCoxProcess`,
`NeuralMarkedPointLikelihood`, `TrainStaticNeuralMarkedProcess`, `FlowPointPatternModel`,
`TrainPointSetFlow`, `TrainPointSetDiffusion`, `SamplePointSetDiffusion`, and
`ValidateGenerativeTissueModel`.

None has an admissible production implementation in the current repository. The pinned Python 3.12
environment contains PyMC/PyTensor/SciPy/POT but no reviewed pinned PyTorch/JAX neural training
backend, model serialization contract, architecture/version/license decision, or deterministic
accelerator policy. The repository also has no admitted provenance-complete independent-patient
pattern/window/context train and held-out corpus with prespecified classical/LGCP comparisons,
count/K/g calibration targets, privacy/memorization audit basis, and mode-collapse criteria.
Consequently, `ValidateGenerativeTissueModel` has neither an admitted learned model nor independent
training/held-out inputs to validate. Implementing its metric shell alone would not establish the
required model calibration or privacy boundary.

## Why adjacent completed work is not substituted

- Exact IPP/LGCP and mechanistic simulators are valid simpler baselines, not trained neural Cox,
  marked, flow, or diffusion models.
- A hand-written affine softplus helper would not own training, uncertainty, held-out calibration,
  quadrature bias/variance, or the required comparison and therefore would be orphan infrastructure.
- Existing fusion workers operate on declared patient-OOF probabilities and are not set/point-process
  generator backends.
- An arbitrary coordinate ordering cannot be introduced for static unordered tissue patterns.

## Resume conditions

Resume only after one decision pins a reviewed backend/version/license/runtime/serialization and one
admitted dataset contract supplies independent patient splits, exact windows/contexts/marks, simpler
baselines, held-out count/K/g targets, privacy/memorization policy, and mode-collapse gates. The first
implementation must begin with neural Cox likelihood and fixed-quadrature bias/variance evidence;
marked, flow, and diffusion families remain downstream.

## Synthetic completion — 2026-08-25

DEC-0199/0200/0202 and IC-0184/0185/0187 now satisfy the backend, serialization, independent
synthetic-patient, quadrature, likelihood, permutation, support, diversity, memorization,
membership-audit, and simpler-baseline conditions under deterministic CPU execution. Missing real
cohorts and broader endpoints remain promotion limits rather than implementation blockers.
