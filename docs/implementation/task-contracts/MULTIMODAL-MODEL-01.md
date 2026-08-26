# Task contract — MULTIMODAL-MODEL-01 Bayesian multimodal model family

Status: all declarations have bounded consumed owners; real-data promotion remains blocked

Date: 2026-08-25

Parent requirements: BAY-MM, MM-01, BAY-FIELD-A, EMB-PRED-02, WS-53.

## Pseudocode ownership

This contract originally owned the disposition of `ValidateMultimodalDesign`, `FitProbabilisticCCA`,
`FitBayesianPCCA`, `FitMultiviewFactorModel`, `AddHierarchicalFactorStructure`,
`BayesianMatrixFactorization`, `SpatialBayesianMatrixFactorization`, `BayesianCPFactorization`,
`BayesianTuckerFactorization`, `FitSpatialLatentFactorModel`, `FitSPDESpatialFactorModel`,
`FitMultiresolutionSpatialFactors`, `InferMissingModalities`, `TrainModalityRobustInference`,
`CompileJointPathologyModel`, `FitJointPathologyModel`, `CompareMultimodalModels`, and
`ValidateMultimodalBayesianSuite`. MM-PCCA-01 through MM-VALIDATE-01 own the original bounded
declarations through IC-0165–IC-0177. SPDE-SUITE-01 / IC-0200 now owns the formerly blocked
`FitSPDESpatialFactorModel` declaration through the shared rectangular finite-element projection and
precision service.

## Available but non-equivalent owners

- joint categorical/continuous mark and location–embedding factor constructors preserve exact model
  factorization and identifiability constraints, but do not fit the general Part IX models;
- calibrated patient-OOF late fusion, stacking, context/availability mixture-of-experts, M0–M5
  complementarity, and explicit-pair cross-modal covariance cover predictive/statistical baselines,
  not latent multiview likelihoods, missing-modality posteriors, or joint pathology inference;
- exact/sparse GP, GMRF, hierarchy, PyMC, VI, Laplace, and SMC components exist separately, but no
  admitted multimodal design composes them into these functions.

## Named missing data and prerequisites

No matched measured multimodal cohort binds stable entity-level joins, patient/site hierarchy,
measurement versus imported-prediction status, modality likelihoods, offsets, missingness mechanism,
coordinate frames, technical covariates, training/test boundaries, and held-out outcomes. General
spatial-latent and joint models
still lack admitted production geometry/data, and synthetic fitting alone cannot satisfy mandated
missingness, confounding, posterior-calibration, and patient-held-out promotion requirements.

## Resume condition

Resume with `ValidateMultimodalDesign` and pCCA/M0–M5 baselines when a matched measured cohort is
admitted. Add Bayesian/multiview/matrix/tensor/spatial factors only through pinned backends and compare
them to those baselines inside patient/site-held-out folds. Missing-modality and joint-pathology claims
require declared MAR/structural/MNAR sensitivity, per-pattern calibration, and measured targets.
