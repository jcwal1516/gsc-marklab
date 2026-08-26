# Task contract — PART-XII-BLOCKERS-01 remaining causal/active-design prerequisite audit

Status: bounded synthetic implementations complete; real identification, data, and prospective
action remain promotion blockers

Date: 2026-08-25

Parent requirements: Part XII §§105–109, 112–113, 114.2, 115–120, CAU-01, PERT-01, ACT-01,
WS-82–84.

## Causal and perturbational fitting functions

`FitSpatialDoseResponse`, `EstimateSpatialPropensity`, `CrossFittedAIPW`, `ExposureAIPW`, and
`SpatialDML` require an admitted cohort with genuine treatment/exposure assignment and timing,
eligible independent clusters, baseline confounders measured before treatment, outcomes after
treatment, prespecified spatial context, overlap/positivity targets, cluster-held-out folds, and
learner/backend choices. IC-0130 validates a randomized synthetic specialization; it is not an
observational nuisance-learning corpus or identification argument.

`NegativeControlAnalysis` requires plausibly valid negative-control exposure/outcome variables and
the admitted primary estimator/adjustment set. `AnalyzeSpatialPerturbationExperiment` additionally
requires a real perturbation project, prespecified scales/outcomes/multiplicity, randomized or
defensible assignment, and independent patient/animal/well replication. `SpatialMediationAnalysis`
requires treatment-before-mediator-before-outcome timing plus declared treatment-induced
interference and mediator–outcome identification/sensitivity assumptions. None of those inputs is
present. No cross-sectional proximity or synthetic graph is relabelled causal.

## Sequential and operational active-design functions

`SequentialBayesianDesign`, `SelectActiveROIs`, `SelectAdditionalStains`,
`SelectRegistrationLandmarks`, `AllocateReplicates`, and `SpatialPowerSimulation` require admitted
candidate actions, acquisition costs/budgets, compatibility/overlap/separation/compartment/patient
constraints, calibrated posterior or pilot/generative models, technical failure/quality models, an
acquisition/update interface, and prospective outcome evidence. IC-0132 owns only scalar Gaussian
nested-MC EIG estimator validation; it does not rank or execute real actions.

`ValidateCausalActiveSuite` remains partial: IC-0130–0135 cover exact randomized exposure mechanics,
mapping branches, analytic EIG, bias arithmetic, bounded-outcome partial identification, and
matched-pair Gamma curves. The suite cannot claim DR/DML robustness/coverage, positivity behavior in
admitted cohorts, negative-control calibration, active-selection benefit over baselines, or external
prospective validation until the functions above exist.

## Resume conditions

Resume causal fitting only after a versioned provenance-complete treatment cohort and identification
contract supply the timing, clusters, covariates, exposure maps, outcomes, overlap, negative controls,
folds, and estimands above. Resume active design only after a prospective action/cost/constraint API,
calibrated posterior/simulator, safe acquisition sandbox, and validation outcomes are admitted. The
first resumed implementation must remain research-only until prospective evidence passes.

## 2026-08-25 synthetic-oracle resumption

The full-program mandate explicitly admitted synthetic/public oracles. CAUSAL-OBS-01,
CAUSAL-PERTURB-01, ACTIVE-DESIGN-01, and CAUSAL-ACTIVE-VALIDATE-01 now consume bounded synthetic
specializations for all declarations listed above. The original real-identification and prospective
resume conditions remain promotion blockers, not implementation blockers; no claim ceiling changed.
