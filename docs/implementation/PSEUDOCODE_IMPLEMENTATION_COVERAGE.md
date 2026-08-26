# Pseudocode implementation coverage

Date: 2026-08-25

This is the terminal implementation-state companion to the read-only authorization crosswalk. It
does not change pseudocode scope. A `live` declaration has a consumed production specialization or
documented canonical duplicate owner and runnable API/CLI evidence. A `blocked` declaration has no
claimed production implementation; the cited contract states the exact prerequisite. Partial
specializations are live only for their explicit accepted inputs and retain their claim ceiling.

The inventory is the 283 literal `FUNCTION` declarations, including the four indented declarations
`Predict`, `ELBO`, `ExponentiateVelocity`, and `Shoot`.

## Part I — 14 declarations

- live (14): ValidateCohortHierarchy, ValidateSpatialObjects, ValidateEmbeddingTable, BuildExchangeabilityPlan, DeriveSeed, StableMean, StableCovarianceMatrix, AdjustPValues, RunRestrictedPermutations, ExtremeRankLengthGlobalEnvelope, BuildNestedCohortSplits, FitBayesianModel, DiagnosePosteriorFit, RunPosteriorPredictiveChecks
- evidence: C-01/C-02/C-04, COH-PERM-01/COH-MAXT-01/EMB-GLOBAL-ENV-01, NUM-STABLE-01, the prediction workflows, and BAY-NORMAL-01 plus model-family diagnostic/PPC adapters. Stable mean/covariance and validation/PPC concepts use the canonical owners required by the authority errata.

## Part II — 15 declarations

- live (15): PatientLevelPermutationTest, HierarchicalBootstrap, PairedSpatialEndpointTest, RepeatedMeasuresFreedmanLane, MultisiteSpatialInference, FunctionalTwoSamplePermutation, MaxTMultipleEndpointPermutation, PatientLevelMMD, PatientLevelEnergyDistance, SpatialFingerprintKernel, SpatialMMD, TOSTEquivalence, FunctionalEquivalenceBand, NoninferiorityTest, BootstrapEquivalence
- evidence: COH-PERM-01, COH-HBOOT-01, COH-PAIR-01, COH-REPEAT-01, COH-MULTISITE-01, COH-FUNC-01, COH-MAXT-01, COH-MMD-01, COH-ENERGY-01, COH-FINGERPRINT-01, COH-EQV-01, COH-FUNC-EQV-01, COH-NI-01, and COH-BOOT-EQV-01. SpatialMMD reuses the patient-fingerprint MMD owner.

## Part III — 31 declarations

- live (31): BuildHierarchicalMixedModel, SummarizePartialPooling, BayesianRandomEffectsMetaAnalysis, ExactGaussianProcessRegression, Predict, MaternKernel, MultiOutputGP, VariationalInducingPointGP, ELBO, LowRankPredictiveProcess, BuildNNGP, NNGPLogDensity, BuildSPDEMaternField, ValidateSpatialWeights, ProperCAR, IntrinsicCAR, SpatialAutoregressiveModel, BYMModel, BYM2Model, GMRFLogDensity, SpatiallyVaryingCoefficientModel, RunHMC, RunNUTS, RunAnnealedSMC, RunVariationalInference, RunLaplaceApproximation, RunInlaStyleApproximation, PSISLOO, CompareBayesianModels, SimulationBasedCalibration, RunPriorSensitivity
- evidence: prior BAY contracts own the original 29 declarations. BAY-HMC-01 and SPDE-SUITE-01 / IC-0196 and IC-0200 add a fixed-step conjugate-normal HMC chain plus an exact bounded rectangular finite-element mesh/mass/stiffness/precision/projection owner.

## Part IV — 24 declarations

- live (24): InhomogeneousPoissonLogLikelihood, FitBayesianInhomogeneousPoisson, BuildBermanTurnerData, BuildGriddedLGCP, BuildSPDE_LGCP, SimulateLGCPPosteriorPredictive, SimulateThomasProcess, SimulateMaternClusterProcess, FitLatentParentClusterModel, FitClusterProcessMinimumContrast, StraussStatistics, StraussPapangelou, SimulateGibbsBirthDeath, FitGibbsPseudolikelihood, ExchangeMCMC_Gibbs, GeyerSaturationStatistic, MultitypePapangelou, FitMultitypeGibbs, BuildJointLocationMarkModel, BuildJointContinuousMarkModel, JointLocationEmbeddingLatentFactorModel, ReplicatedHierarchicalLGCP, ReplicatedClusterModel, PosteriorPredictivePointProcessDiagnostics
- evidence: prior BAY point-process contracts own the original 19 declarations. BAY-ADV-CLUSTER-01 / IC-0197 owns label-invariant birth/death parent-count inference, exact finite-state exchange MCMC, its consumed symmetric binary multitype fit, and partially pooled replicated cluster parameters. SPDE-SUITE-01 / IC-0200 owns rectangular SPDE LGCP quadrature/MAP and mesh sensitivity.

## Part V — 29 declarations

- live (29): VectorSemivariogram, ProjectedEmbeddingVariograms, EmbeddingCrossCovarianceByDistance, CrossModalCovarianceByDistance, KernelMarkCorrelation, TestEmbeddingSpatialDependence, BuildEmbeddingKernel, GraphDirichletEnergy, GraphSmoothnessPermutationTest, LocalEmbeddingRoughness, ValidateCellPatchLinks, CellPatchContext, TestCellPatchComplementarity, PatchDependencyWeighting, MultiscaleEmbeddingKernel, BuildSpatialFingerprint, FingerprintDistance, BuildRegionRetrievalIndex, RetrieveAnalogousRegions, CompareEmbeddingDistributionsByPatient, RegionCompatibility, RunM0M5PredictiveWorkflow, CalibratePredictions, GroupedConformalPredictor, OODScore, ApplyAbstention, LateFusion, MixtureOfExpertsFusion, PredictiveStacking
- evidence: EMB-VARIO-01 through EMB-LOCAL-ROUGH-01, C-05/EMB-CELL-PATCH-CONTEXT-01/EMB-COMPLEMENT-01/EMB-PATCH-DEPENDENCY-01, EMB-MULTISCALE-KERNEL-01/COH-FINGERPRINT-01/EMB-RETRIEVAL-01/COH-REGION-COMPAT-01, and the M0–M5/calibration/conformal/OOD/abstention/fusion/stacking task contracts. Every public specialization has a CLI; cell-patch mechanics are immediate internal consumers.

## Part VI — 19 declarations

- live (19): MultiResolutionNonrigidRegistration, SVFDiffeomorphicRegistration, ExponentiateVelocity, LDDMMRegistration, Shoot, ProbabilisticDiffeomorphicRegistration, BayesianLandmarkRegistration, PropagateTransformUncertainty, DeltaPropagateTransformUncertainty, ProbabilisticCellCorrespondence, EntropicSoftAssignment, SinkhornOT, PartialOT, UnbalancedSinkhorn, FusedGromovWasserstein, PartialUnbalancedFGW, BuildSpatialAtlas, MapQueryToAtlas, ValidateAtlasMapping
- evidence: REG-NONRIGID-01 through ATLAS-01 / IC-0178–IC-0183 own pinned SimpleITK B-spline, stationary-velocity scaling/squaring, landmark Hamiltonian shooting, a variational translation-SVF posterior, GP landmark deformation/MC/delta/correspondence, and biological-similarity atlas build/map/LOPO validation. REG-SOFT-ASSIGN-01 through REG-PARTIAL-FGW-01 own transport. Every new registration/atlas claim remains synthetic/experimental and correspondence is compatibility, never cell identity.

## Part VII — 31 declarations

- live (31): BuildCanonicalGraph, BuildLaplacian, GraphFourierTransform, SummarizeGraphFrequencyBands, GraphSpectrumNullTest, ExactHeatKernel, ApplyHeatKernel, HeatKernelSignature, DiffusionDistance, SpectralGraphWaveletTransform, GraphWaveletEnergy, ChebyshevApply, AdaptiveChebyshevOrder, BuildDiffusionWaveletTree, DiffusionWaveletTransform, GraphScattering, ValidateGraphScatteringStability, BuildHeterogeneousTissueGraph, HeterogeneousMessagePassing, BuildHypergraph, HypergraphLaplacian, HypergraphSignalSmoothness, CountTypedMotifs, BuildMotifAdjacency, MotifNullTest, BuildCliqueComplex, HodgeLaplacian, HodgeDecomposeEdgeFlow, FilterKSimplicialSignal, BuildCellularComplex, ValidateGraphMathematicsSuite
- evidence: IC-0145–IC-0157 and the graph task contracts own all 31 declarations through bounded consumed specializations. Diffusion wavelets, scattering, typed heterogeneous messages, normalized hypergraphs, typed triangle motifs, clique/Hodge operators, perturbation-checked cellular complexes, and the exact-fixture validation ledger remain synthetic/research-only; unsupported graph rules, registration, sparse scale, GPU parity, and pathology performance are reported rather than implied.

## Part VIII — 13 declarations

- live (13): ValidateFiltration, PersistentHomology, BuildAlphaFiltration, BuildWitnessFiltration, PersistenceLandscape, PersistenceImage, EulerCharacteristicCurve, MinkowskiFunctionals2D, MorphologicalFunctionalCurve, ConnectivityTransition, ComparePersistenceDistributions, TopologyStabilityLaboratory, ValidateTopologySuite
- evidence: IC-0158–IC-0164 and TOP-ALPHA-01/TOP-WITNESS-01/TOP-MORPH-01/TOP-CONNECT-01/TOP-COMPARE-01/TOP-STABILITY-01/TOP-VALIDATE-01 own all 13 declarations through bounded experimental workflows. Real pathology-linked filtration/segmentation adequacy, representative sparse-memory scaling, and external patient validation remain claim limitations rather than unimplemented declarations.

## Part IX — 18 declarations

- live (18): ValidateMultimodalDesign, FitProbabilisticCCA, FitBayesianPCCA, FitMultiviewFactorModel, AddHierarchicalFactorStructure, BayesianMatrixFactorization, SpatialBayesianMatrixFactorization, BayesianCPFactorization, BayesianTuckerFactorization, FitSpatialLatentFactorModel, FitSPDESpatialFactorModel, FitMultiresolutionSpatialFactors, InferMissingModalities, TrainModalityRobustInference, CompileJointPathologyModel, FitJointPathologyModel, CompareMultimodalModels, ValidateMultimodalBayesianSuite
- evidence: prior MM contracts own the original 17 declarations. SPDE-SUITE-01 / IC-0200 consumes the shared rectangular finite-element projection/precision in a one-factor fixed-hyperparameter Laplace-MAP model with synthetic reconstruction and mesh sensitivity.

## Part X — 32 declarations

- live (32): ValidateSimulator, SimulateReactionDiffusion, AnalyzeReactionDiffusionPattern, SimulateSpatialCompetition, SimulateAgentCompetition, SimulateVascularTransport, FitDistanceToResourceModel, SimulateGrowthFront, EvolveInterfaceLevelSet, SimulateMechanisticTissue, NeuralCoxIntensity, TrainNeuralCoxProcess, NeuralMarkedPointLikelihood, TrainStaticNeuralMarkedProcess, FlowPointPatternModel, TrainPointSetFlow, TrainPointSetDiffusion, SamplePointSetDiffusion, SoftPairHistogram, SummaryMatchingLoss, RejectionABC, SMC_ABC, EstimateSyntheticLogLikelihood, SyntheticLikelihoodMCMC, TrainNPE, TrainNLE, TrainNRE, SequentialSBI, SimulationBasedCalibration, DetectSimulationOOD, PosteriorPredictiveLaboratory, ValidateGenerativeTissueModel
- evidence: prior simulator/SBI contracts own the original 19 declarations. NEURAL-PP-01, NEURAL-SET-01, NEURAL-SBI-01, and NEURAL-VALIDATE-01 / IC-0184–IC-0187 own a pinned JAX marked Cox likelihood, equivariant logistic-normal flow, score diffusion/sample path, pinned sbi 0.26.1 NPE/NLE/NRE/two-round NPE, and a consumed repeated-artifact model card. All learned evidence remains deterministic-CPU synthetic/research-only.

## Part XI — 21 declarations

- live (21): ValidateDimensionality, ReconstructSerialSectionStack, FitBayesianSectionStack, PropagateStackUncertainty, ValidateWindow3D, KFunction3D, InhomogeneousK3D, CrossK3D, Build3DAlphaComplex, FitAnisotropic3DGP, Build3DSpatialGraph, KalmanFilter, RauchTungStriebelSmoother, NonlinearGaussianFilter, ParticleFilter, ParticleSmoother, FitDeformationBiologyModel, PhylogeneticSpatialAssociation, FitClonePhylogeography, FitCloneNicheModel, Validate3DLongitudinalSuite
- evidence: prior DIM/LONG/EVO contracts own the original 13 declarations. DIM-STACK-01, DIM-ALPHA3D-01, LONG-DEFORM-BIO-01, EVO-CLONE-MODELS-01, and DIM-ADV-VALIDATE-01 / IC-0188–IC-0191 own paired serial translation/posterior propagation, exact GUDHI 3-D alpha, deformation/biology separation, uncertain clone diffusion/niche fitting, and an executable umbrella ledger. Real serial/longitudinal/clone evidence remains a claim limit.

## Part XII — 24 declarations

- live (24): ValidateCausalDesign, ComputeExposureMapping, EstimateExposureMean, DirectAndSpilloverEffects, InterferenceRandomizationTest, FitSpatialDoseResponse, EstimateSpatialPropensity, CrossFittedAIPW, ExposureAIPW, SpatialDML, NegativeControlAnalysis, RosenbaumSensitivity, BiasFunctionSensitivity, PartialIdentificationBounds, AnalyzeSpatialPerturbationExperiment, SpatialMediationAnalysis, EstimateExpectedInformationGain, SequentialBayesianDesign, SelectActiveROIs, SelectAdditionalStains, SelectRegistrationLandmarks, AllocateReplicates, SpatialPowerSimulation, ValidateCausalActiveSuite
- evidence: prior causal/design contracts own the original nine declarations. CAUSAL-OBS-01, CAUSAL-PERTURB-01, ACTIVE-DESIGN-01, and CAUSAL-ACTIVE-VALIDATE-01 / IC-0192–IC-0195 own bounded synthetic observational nuisance/cross-fit estimators, randomized perturbation with research-only mediation, sequential/budgeted acquisition/allocation/power, and an executable validation ledger. No synthetic result identifies a real treatment effect or validates an operational acquisition policy.

## Part XIII — 12 declarations

- live (12): ExecuteAlgorithm, SelectExecutionMode, StableLogSumExp, StableLogMeanExp, StableWeightedMean, StableCovariance, DeterministicParallelReduce, DiagnoseFittedModel, RunCalibrationSuite, RealDataValidationLadder, BenchmarkAlgorithmScaling, DetermineResultMaturity
- evidence: prior execution/numerics/validation/maturity contracts own seven declarations. EXEC-ALGORITHM-01 / IC-0198 extracts the verified durable typed-node transaction with `project classical` as caller. RUNTIME-VALIDATION-01 / IC-0199 uses fixed contiguous threaded partitions in Gaussian calibration, consumes a fixed-step HMC artifact in typed diagnostics, and benchmarks equivalent integer-sum work with checksums/timed phases.

## Registered pseudocode gaps

The material gap families in `PSEUDOCODE_GAP_REGISTER.md` remain authority-blocked: Bayesian
nonparametric niche/domain models, cross-attention fusion, foundation-model admission, equivariant
learned tissue models, a general method-specific interval/coverage contract, broader pathology
geometry/domain workstreams, and per-family backend/oracle selections require dedicated reviewed
pseudocode before implementation. The current mandate does not authorize inventing those contracts;
adjacent implementations are not counted as closure.

## Totals and current state

- live declarations: 283
- blocked declarations: 0
- declarations inventoried: 283

This is the terminal declaration accounting, not a blanket validation or maturity claim. Every
literal pseudocode function has a consumed production specialization or canonical duplicate owner;
scope and evidence ceilings remain binding. The user explicitly unpaused the narrow PLAT-DUR-01
work needed for unified execution on 2026-08-25; no worktree, stage, commit, push, or deployment was
performed.
