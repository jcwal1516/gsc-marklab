Read-only crosswalk complete. I treated [MASTER\_PLAN.md (line 1)]\(/Users/user/Bench/gsc-marklab/docs/implementation/MASTER\_PLAN.md:1) as authoritative, used the tracker only for current state, and inventoried all 134 numbered pseudocode sections and all 283 declared `FUNCTION` lines, including nested functions.

The tracker’s recorded master-plan SHA-256 matches the current file exactly: `1cdb619edc39d1d3d8c72bdf15930de3a928bf4b90651a05dc481c1238e5f064`.

“Mapped” below means authorized scope, not implemented or ready. Workstream IDs identify the dependency-ordered owner.

## Exhaustive function crosswalk

### Shared substrate and cohort inference

| Pseudocode owner and functionsExact master-plan IDs                                                                                                 |                                                                                                            |
| --------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| §0 purpose; §1 notation                                                                                                                             | Claim tiers and policy only; `FND-07`, `SCALE-01`                                                          |
| §2.1 `ValidateCohortHierarchy`                                                                                                                      | `FND-01`, `DATA-01`, `C-01`, `WS-20`, `WS-C`                                                               |
| §2.2 `ValidateSpatialObjects`                                                                                                                       | `FND-02`, `DATA-01`, `DIM-01`, `C-02`, `WS-21`, `WS-22`                                                    |
| §2.3 `ValidateEmbeddingTable`                                                                                                                       | `FND-04`, `FND-05`, `EMB-CORE`, `EMB-PATCH`, `C-04`, `C-05`, `C-06`, `WS-23`, `WS-24`                      |
| §2.4 `BuildExchangeabilityPlan`                                                                                                                     | `FND-06`, `COH-01`, `CAU-01A`, `WS-31`, `WS-34`, `WS-82`                                                   |
| §2.5 `DeriveSeed`; execution/artifact records                                                                                                       | `FND-07`, `PLAT-01`, `WF-01`, `BACK-01`, `C-03`, `WS-11`, `WS-12`, `WS-13`                                 |
| §2.6 typed availability states                                                                                                                      | `FND-07`, `PLAT-01`, `WS-11`, result-format policy                                                         |
| §3.1 `StableMean`, `StableCovarianceMatrix`                                                                                                         | `FND-07`, `SCALE-01`                                                                                       |
| §3.2 `AdjustPValues`                                                                                                                                | `INF-01C`, `WS-31`                                                                                         |
| §3.3 `RunRestrictedPermutations`                                                                                                                    | `FND-06`, `INF-01A`, `COH-01`, `WS-31`, `WS-34`                                                            |
| §3.4 `ExtremeRankLengthGlobalEnvelope`                                                                                                              | `INF-01B`, `CMP-01B`, `WS-31`, `WS-34`                                                                     |
| §3.5 `BuildNestedCohortSplits`                                                                                                                      | `COH-01`, `EMB-PRED-01`, `WS-34`, `WS-52`                                                                  |
| §4.1 Bayesian model IR                                                                                                                              | `BAY-01`, `WS-40`                                                                                          |
| §4.2 `FitBayesianModel`                                                                                                                             | `BAY-01`, `BAY-02`, `BACK-01`, `WS-40`                                                                     |
| §4.3 `DiagnosePosteriorFit`                                                                                                                         | `BAY-02`, `WS-40`, `WS-44`                                                                                 |
| §4.4 `RunPosteriorPredictiveChecks`                                                                                                                 | `BAY-02`, `WS-40`, `WS-44`                                                                                 |
| §5 `PatientLevelPermutationTest`, `HierarchicalBootstrap`, `PairedSpatialEndpointTest`, `RepeatedMeasuresFreedmanLane`, `MultisiteSpatialInference` | `FND-06`, `COH-01`, `INF-01A`, `INF-01D`, `BAY-03`, `WS-34`, with Bayesian multisite fitting under `WS-41` |
| §6 `FunctionalTwoSamplePermutation`, `MaxTMultipleEndpointPermutation`                                                                              | `CMP-01B`, `INF-01B`, `INF-01C`, `COH-01`, `WS-31`, `WS-34`                                                |
| §7.1 `PatientLevelMMD`                                                                                                                              | `CMP-01C`, `COH-01`, `WS-34`                                                                               |
| §7.2 `PatientLevelEnergyDistance`                                                                                                                   | `CMP-01D`, `COH-01`, `WS-34`                                                                               |
| §7.3 `SpatialFingerprintKernel`, `SpatialMMD`                                                                                                       | `CMP-01A`, `CMP-01C`, `COH-01`, `WS-34`                                                                    |
| §8.1 `TOSTEquivalence`                                                                                                                              | `EQV-01`, `EQV-01B`, `COH-01`, `CMP-01`, `WS-34`                                                           |
| §8.2 `FunctionalEquivalenceBand`                                                                                                                    | `EQV-01`, `CMP-01B`, `COH-01`, `WS-34`                                                                     |
| §8.3 `NoninferiorityTest`                                                                                                                           | `EQV-01`, `EQV-01C`, `COH-01`, `WS-34`                                                                     |
| §8.4 `BootstrapEquivalence`                                                                                                                         | `EQV-01`, `EQV-01B`, `EQV-01C`, `COH-01`, `INF-01D`, `WS-34`                                               |

### Bayesian spatial modeling and point processes

| Pseudocode owner and functionsExact master-plan IDs                                                                                                       |                                                                                                                |
| --------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| §9 `BuildHierarchicalMixedModel`, `SummarizePartialPooling`, `BayesianRandomEffectsMetaAnalysis`                                                          | `BAY-03`, `BAY-HIER-A`, `BAY-REG-A`, `WS-41`; spatial components also require `BAY-04`/`BAY-05`, `WS-42`       |
| §10 `ExactGaussianProcessRegression`, nested `Predict`, `MaternKernel`, `MultiOutputGP`                                                                   | `BAY-04`, `BAY-FIELD-A`, `WS-42`                                                                               |
| §11 `VariationalInducingPointGP`, nested `ELBO`, `LowRankPredictiveProcess`, `BuildNNGP`, `NNGPLogDensity`, `BuildSPDEMaternField`                        | `BAY-04`, `BAY-FIELD-A`, `SCALE-01`, `WS-42`                                                                   |
| §12 `ValidateSpatialWeights`, `ProperCAR`, `IntrinsicCAR`, `SpatialAutoregressiveModel`, `BYMModel`, `BYM2Model`, `GMRFLogDensity`                        | `BAY-05`, `BAY-GMRF-A`, `GSP-01` for the graph/weight contract, `WS-42`                                        |
| §13 `SpatiallyVaryingCoefficientModel`                                                                                                                    | `BAY-04`, `BAY-REG-A`, `WS-42`                                                                                 |
| §14 `RunHMC`, `RunNUTS`, `RunAnnealedSMC`, `RunVariationalInference`, `RunLaplaceApproximation`, `RunInlaStyleApproximation`                              | `BAY-02`, `BACK-01`, `WS-40`; authority is backend integration, not an automatic native implementation mandate |
| §15 `PSISLOO`, `CompareBayesianModels`, first `SimulationBasedCalibration`, `RunPriorSensitivity`                                                         | `BAY-02`, `BAY-01`, `WS-40`, `WS-44`                                                                           |
| §16 `InhomogeneousPoissonLogLikelihood`, `FitBayesianInhomogeneousPoisson`, `BuildBermanTurnerData`                                                       | `BAY-PP`, `BAY-PP-A`, `WS-43`                                                                                  |
| §17 `BuildGriddedLGCP`, `BuildSPDE_LGCP`, `SimulateLGCPPosteriorPredictive`                                                                               | `BAY-PP`, `BAY-PP-A`, `BAY-FIELD-A`, `WS-43`, simulator support from `WS-70`                                   |
| §18 `SimulateThomasProcess`, `SimulateMaternClusterProcess`, `FitLatentParentClusterModel`, `FitClusterProcessMinimumContrast`                            | `BAY-PP`, `GEN-01`, `WS-43`, `WS-70`                                                                           |
| §19 `StraussStatistics`, `StraussPapangelou`, `SimulateGibbsBirthDeath`, `FitGibbsPseudolikelihood`, `ExchangeMCMC_Gibbs`, `GeyerSaturationStatistic`     | `BAY-PP`, `BAY-PP-B`, `GEN-01`, `WS-43`, `WS-70`                                                               |
| §20 `MultitypePapangelou`, `FitMultitypeGibbs`, `BuildJointLocationMarkModel`, `BuildJointContinuousMarkModel`, `JointLocationEmbeddingLatentFactorModel` | `BAY-PP`, `BAY-PP-B`, `MRK-02A`, `MRK-02D`, `FND-04`, `EMB-01`, `WS-43`                                        |
| §21 `ReplicatedHierarchicalLGCP`, `ReplicatedClusterModel`                                                                                                | `BAY-PP`, `BAY-03`, `WS-43`                                                                                    |
| §22 `PosteriorPredictivePointProcessDiagnostics`                                                                                                          | `BAY-PP`, `BAY-02`, `PP-01`, `PP-03A`, `WS-44`; K/g are dependencies, not supplied here                        |

### Embeddings, prediction, registration, and transport

| Pseudocode owner and functionsExact master-plan IDs                                                                                                 |                                                                                                                    |
| --------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| §23 `VectorSemivariogram`, `ProjectedEmbeddingVariograms`, `EmbeddingCrossCovarianceByDistance`, `CrossModalCovarianceByDistance`                   | `EMB-01`, `SIG-01H`, `SIG-01G`, `MOL-01`, `WS-50`; cross-modal work also touches `MM-01`                           |
| §24 `KernelMarkCorrelation`, `TestEmbeddingSpatialDependence`, `BuildEmbeddingKernel`                                                               | `EMB-01`, `SIG-01H`, `FND-06`, `INF-01B`, `WS-50`                                                                  |
| §25 `GraphDirichletEnergy`, `GraphSmoothnessPermutationTest`, `LocalEmbeddingRoughness`                                                             | `EMB-01`, `EMB-CORE`, `GSP-01`, `WS-50`                                                                            |
| §26 `ValidateCellPatchLinks`, `CellPatchContext`, `TestCellPatchComplementarity`, `PatchDependencyWeighting`                                        | `EMB-PATCH`, `FR-02`, `FR-02A`, `EMB-PRED-01`, `WS-51`, `WS-52`                                                    |
| §27 `MultiscaleEmbeddingKernel`, `BuildSpatialFingerprint`, `FingerprintDistance`                                                                   | `FR-02`, `EMB-PATCH`, `CMP-01A`, `WS-51`                                                                           |
| §28 `BuildRegionRetrievalIndex`, `RetrieveAnalogousRegions`, `CompareEmbeddingDistributionsByPatient`, `RegionCompatibility`                        | `EMB-CORE`, `FR-02`, `CMP-01A`, `CMP-01C`, `COH-01`, `WS-50`, `WS-51`                                              |
| §29 `RunM0M5PredictiveWorkflow`, `CalibratePredictions`, `GroupedConformalPredictor`, `OODScore`, `ApplyAbstention`                                 | `EMB-PRED-01`, `COH-01`, `WS-52`; it assumes but does not itself implement `DL-CORE`                               |
| §30 `LateFusion`, `MixtureOfExpertsFusion`, `PredictiveStacking`                                                                                    | `EMB-PRED-01`, `BAY-MM`, `WS-52`, `WS-53`; this does not cover the specific cross-attention scope of `EMB-PRED-02` |
| §31 `MultiResolutionNonrigidRegistration`, `SVFDiffeomorphicRegistration`, nested `ExponentiateVelocity`, `LDDMMRegistration`, nested `Shoot`       | `REG-01`, `REG-01B`, `BACK-01`, `WS-55`                                                                            |
| §32 `ProbabilisticDiffeomorphicRegistration`, `BayesianLandmarkRegistration`, `PropagateTransformUncertainty`, `DeltaPropagateTransformUncertainty` | `BAY-REG`, `REG-01B`, `DIM-01`, `WS-55`                                                                            |
| §33 `ProbabilisticCellCorrespondence`, `EntropicSoftAssignment`                                                                                     | `REG-01C`, `BAY-REG`, `WS-55`                                                                                      |
| §34 `SinkhornOT`, `PartialOT`, `UnbalancedSinkhorn`, `FusedGromovWasserstein`, `PartialUnbalancedFGW`                                               | `FR-03`, `FR-03A`, `FR-03B`; balanced Sinkhorn is only an enabling primitive under this authority                  |
| §35 `BuildSpatialAtlas`, `MapQueryToAtlas`, `ValidateAtlasMapping`                                                                                  | `FR-03`, `FR-03A`, `FR-03B`, `NIC-01F`, `WS-60`                                                                    |

### Graphs and topology

| Pseudocode owner and functionsExact master-plan IDs                                             |                                                          |
| ----------------------------------------------------------------------------------------------- | -------------------------------------------------------- |
| §36 `BuildCanonicalGraph`, `BuildLaplacian`                                                     | `FND-03`, `GSP-01`, `GSP-02`, `WS-61`, `WS-62`           |
| §37 `GraphFourierTransform`, `SummarizeGraphFrequencyBands`, `GraphSpectrumNullTest`            | `FR-01`, `FR-01A`, `GSP-01`, `WS-62`                     |
| §38 `ExactHeatKernel`, `ApplyHeatKernel`, `HeatKernelSignature`, `DiffusionDistance`            | `FR-01`, `FR-01A`, `GSP-01`, `WS-62`                     |
| §39 `SpectralGraphWaveletTransform`, `GraphWaveletEnergy`                                       | `FR-01`, `FR-01B`, `GSP-01`, `WS-62`                     |
| §40 `BuildDiffusionWaveletTree`, `DiffusionWaveletTransform`                                    | `FR-01`, `FR-01B`, `GSP-01`, `WS-62`                     |
| §41 `ChebyshevApply`, `AdaptiveChebyshevOrder`                                                  | `GSP-01`, `FR-01A`, `FR-01B`, `SCALE-01`, `WS-62`        |
| §42 `GraphScattering`, `ValidateGraphScatteringStability`                                       | `GSP-01`, `GSP-03`, `WS-62`                              |
| §43 `BuildHeterogeneousTissueGraph`, `HeterogeneousMessagePassing`                              | `GSP-02`, `HET-01`, `WS-61`                              |
| §44 `BuildHypergraph`, `HypergraphLaplacian`, `HypergraphSignalSmoothness`                      | `GSP-02`, `HET-01`, `WS-61`                              |
| §45 `CountTypedMotifs`, `BuildMotifAdjacency`, `MotifNullTest`                                  | `GSP-02`, `HET-01`, `WS-61`                              |
| §46 `BuildCliqueComplex`, `HodgeLaplacian`, `HodgeDecomposeEdgeFlow`, `FilterKSimplicialSignal` | `GSP-02`, `HET-01`, `WS-61`                              |
| §47 `BuildCellularComplex`                                                                      | `GSP-02`, `HET-01`, `WS-61`                              |
| §48 `ValidateGraphMathematicsSuite`                                                             | `FND-07`, `GSP-01`, `GSP-02`, `GSP-03`, `WS-61`, `WS-62` |
| §49 `ValidateFiltration`, `PersistentHomology`                                                  | `TOP-01`, `TOP-01A`, `WS-63`                             |
| §50 `BuildAlphaFiltration`                                                                      | `TOP-01`, `TOP-01A`, `GEO-01E`, `WS-63`                  |
| §51 `BuildWitnessFiltration`                                                                    | `TOP-01`, `TOP-01A`, `WS-63`                             |
| §52 `PersistenceLandscape`                                                                      | `TOP-01`, `TOP-01A`, `WS-63`                             |
| §53 `PersistenceImage`                                                                          | `TOP-01`, `TOP-01A`, `WS-63`                             |
| §54 `EulerCharacteristicCurve`                                                                  | `TOP-01`, `TOP-01B`, `WS-63`                             |
| §55 `MinkowskiFunctionals2D`, `MorphologicalFunctionalCurve`                                    | `TOP-01`, `TOP-01B`, `GEO-01E`, `WS-63`                  |
| §56 `ConnectivityTransition`                                                                    | `TOP-01`, `TOP-01B`, partially `GEO-01D`, `WS-63`        |
| §57 `ComparePersistenceDistributions`                                                           | `CMP-01F`, `TOP-01`, `COH-01`, `WS-34`, `WS-63`          |
| §58 `TopologyStabilityLaboratory`                                                               | `TOP-01`, `TOP-01A`, `TOP-01B`, `WS-63`                  |
| §59 `ValidateTopologySuite`                                                                     | `FND-07`, `TOP-01`, `WS-63`                              |

### Multimodal and generative modeling

| Pseudocode owner and functionsExact master-plan IDs                                                                                                 |                                                                                                               |
| --------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| §60 `ValidateMultimodalDesign`                                                                                                                      | `MM-01`, `BAY-MM`, `FND-04`, `FND-05`, `WS-53`                                                                |
| §61 `FitProbabilisticCCA`, `FitBayesianPCCA`                                                                                                        | `BAY-MM`, `MM-01`, `WS-53`                                                                                    |
| §62 `FitMultiviewFactorModel`, `AddHierarchicalFactorStructure`                                                                                     | `BAY-MM`, `BAY-03`, `MM-01`, `WS-53`                                                                          |
| §63 `BayesianMatrixFactorization`, `SpatialBayesianMatrixFactorization`                                                                             | `BAY-MM`, `BAY-04`, `BAY-05`, `WS-53`                                                                         |
| §64 `BayesianCPFactorization`, `BayesianTuckerFactorization`                                                                                        | `BAY-MM`, `WS-53`                                                                                             |
| §65 `FitSpatialLatentFactorModel`, `FitSPDESpatialFactorModel`, `FitMultiresolutionSpatialFactors`                                                  | `BAY-MM`, `BAY-04`, `BAY-05`, `WS-42`, `WS-53`                                                                |
| §66 `InferMissingModalities`, `TrainModalityRobustInference`                                                                                        | `BAY-MM`, `MM-01`, `WS-53`                                                                                    |
| §67 `CompileJointPathologyModel`, `FitJointPathologyModel`                                                                                          | `BAY-MM`, `MM-01`, `WS-53`                                                                                    |
| §68 `CompareMultimodalModels`                                                                                                                       | `BAY-MM`, `EMB-PRED-01`, `WS-44`, `WS-52`, `WS-53`                                                            |
| §69 `ValidateMultimodalBayesianSuite`                                                                                                               | `FND-07`, `BAY-MM`, `WS-53`, `WS-93`                                                                          |
| §70 `ValidateSimulator`                                                                                                                             | `GEN-01`, `WS-70`                                                                                             |
| §71 `SimulateReactionDiffusion`, `AnalyzeReactionDiffusionPattern`                                                                                  | `GEN-01`, `WS-71`                                                                                             |
| §72 `SimulateSpatialCompetition`, `SimulateAgentCompetition`                                                                                        | `GEN-01`, `BAY-EVO`, `WS-71`                                                                                  |
| §73 `SimulateVascularTransport`, `FitDistanceToResourceModel`                                                                                       | `GEN-01`, `GEO-01G`, `PATH-01`, `BAY-REG-A`, `WS-33`, `WS-71`                                                 |
| §74 `SimulateGrowthFront`, `EvolveInterfaceLevelSet`                                                                                                | `GEN-01`, `BAY-EVO`, `GEO-01H`, `WS-71`                                                                       |
| §75 `SimulateMechanisticTissue`                                                                                                                     | `GEN-01`, `WS-70`, `WS-71`                                                                                    |
| §76 `NeuralCoxIntensity`, `TrainNeuralCoxProcess`                                                                                                   | `GEN-01A`, `BAY-PP-A`, `WS-72`                                                                                |
| §77 `NeuralMarkedPointLikelihood`, `TrainStaticNeuralMarkedProcess`                                                                                 | `GEN-01A`, `BAY-PP`, `WS-72`                                                                                  |
| §78 `FlowPointPatternModel`, `TrainPointSetFlow`                                                                                                    | `GEN-01B`, `WS-72`                                                                                            |
| §79 `TrainPointSetDiffusion`, `SamplePointSetDiffusion`                                                                                             | `GEN-01B`, `WS-72`                                                                                            |
| §80 `SoftPairHistogram`, `SummaryMatchingLoss`                                                                                                      | Supporting implementation technique under `GEN-01A`, `GEN-01B`, `WS-72`; no standalone master-plan capability |
| §§81–86 `RejectionABC`, `SMC_ABC`, `EstimateSyntheticLogLikelihood`, `SyntheticLikelihoodMCMC`, `TrainNPE`, `TrainNLE`, `TrainNRE`, `SequentialSBI` | `BAY-SBI`, `WS-73`                                                                                            |
| §87 second `SimulationBasedCalibration`                                                                                                             | `BAY-SBI`, `BAY-02`, `WS-40`, `WS-73`                                                                         |
| §88 `DetectSimulationOOD`                                                                                                                           | `BAY-SBI`, `GEN-01`, `WS-73`, `WS-74`                                                                         |
| §89 `PosteriorPredictiveLaboratory`                                                                                                                 | `GEN-01`, `BAY-SBI`, `WS-74`                                                                                  |
| §90 `ValidateGenerativeTissueModel`                                                                                                                 | `GEN-01`, `GEN-01A`, `GEN-01B`, `WS-74`                                                                       |

### 3-D, causal, execution, and release contracts

| Pseudocode owner and functionsExact master-plan IDs                                                                                                                                                 |                                                                                                                                            |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| §91 `ValidateDimensionality`                                                                                                                                                                        | `DIM-01`, `WS-21`, `WS-80`                                                                                                                 |
| §92 `ReconstructSerialSectionStack`, `FitBayesianSectionStack`                                                                                                                                      | `DIM-01`, `BAY-REG`, `REG-01B`, `WS-55`, `WS-80`                                                                                           |
| §93 `PropagateStackUncertainty`                                                                                                                                                                     | `DIM-01`, `BAY-REG`, `WS-80`                                                                                                               |
| §94 `ValidateWindow3D`                                                                                                                                                                              | `DIM-01`, `WS-80`                                                                                                                          |
| §95 `KFunction3D`                                                                                                                                                                                   | `DIM-01`, extension of `PP-01`, `WS-80`                                                                                                    |
| §96 `InhomogeneousK3D`, `CrossK3D`                                                                                                                                                                  | `DIM-01`, extensions of `PP-02` and `PP-03B`, `WS-80`                                                                                      |
| §97 `FitAnisotropic3DGP`                                                                                                                                                                            | `DIM-01`, `BAY-04`, `BAY-FIELD-A`, `WS-80`                                                                                                 |
| §98 `Build3DSpatialGraph`, `Build3DAlphaComplex`                                                                                                                                                    | `DIM-01`, `GSP-02`, `TOP-01A`, `WS-80`                                                                                                     |
| §99 `KalmanFilter`, `RauchTungStriebelSmoother`, `NonlinearGaussianFilter`, `ParticleFilter`, `ParticleSmoother`                                                                                    | `TIME-01`, `DIM-01`, `WS-81`                                                                                                               |
| §100 `FitDeformationBiologyModel`                                                                                                                                                                   | `TIME-01`, `DIM-01`, `BAY-REG`, `WS-81`                                                                                                    |
| §101 `FitClonePhylogeography`, `PhylogeneticSpatialAssociation`, `FitCloneNicheModel`                                                                                                               | `BAY-EVO`, `CLN-01C`, `CLN-01D`, `CLN-02`, `WS-54`, `WS-81`                                                                                |
| §102 `Validate3DLongitudinalSuite`                                                                                                                                                                  | `FND-07`, `DIM-01`, `TIME-01`, `WS-80`, `WS-81`                                                                                            |
| §103 `ValidateCausalDesign`                                                                                                                                                                         | `CAU-01`, `CAU-01A`, `CAU-01B`, `WS-82`                                                                                                    |
| §104 `ComputeExposureMapping`, `EstimateExposureMean`, `DirectAndSpilloverEffects`, `InterferenceRandomizationTest`                                                                                 | `CAU-01`, `CAU-01A`, `WS-82`                                                                                                               |
| §105 `FitSpatialDoseResponse`                                                                                                                                                                       | `CAU-01`, `PERT-01`, `WS-83`                                                                                                               |
| §§106–111 `EstimateSpatialPropensity`, `CrossFittedAIPW`, `ExposureAIPW`, `SpatialDML`, `NegativeControlAnalysis`, `RosenbaumSensitivity`, `BiasFunctionSensitivity`, `PartialIdentificationBounds` | `CAU-01`, `CAU-01B`, `WS-82`                                                                                                               |
| §112 `AnalyzeSpatialPerturbationExperiment`                                                                                                                                                         | `PERT-01`, `CAU-01A`, `WS-83`                                                                                                              |
| §113 `SpatialMediationAnalysis`                                                                                                                                                                     | No exact method-level master-plan authority; nearest umbrellas are `CAU-01` and `PERT-01`, but neither explicitly admits spatial mediation |
| §114 `EstimateExpectedInformationGain`, `SequentialBayesianDesign`                                                                                                                                  | `ACT-01`, `WS-84`                                                                                                                          |
| §§115–119 `SelectActiveROIs`, `SelectAdditionalStains`, `SelectRegistrationLandmarks`, `AllocateReplicates`, `SpatialPowerSimulation`                                                               | `ACT-01`, `WS-84`; landmark selection also depends on `BAY-REG`                                                                            |
| §120 `ValidateCausalActiveSuite`                                                                                                                                                                    | `CAU-01`, `PERT-01`, `ACT-01`, `WS-82`, `WS-83`, `WS-84`                                                                                   |
| §121 algorithm descriptor                                                                                                                                                                           | `PLAT-01`, `WF-01`, `BACK-01`, `FND-07`, `WS-11`, `WS-12`, `WS-13`, `WS-94`                                                                |
| §122 `ExecuteAlgorithm`                                                                                                                                                                             | `PLAT-01`, `WF-01`, `BACK-01`, `FND-07`, `WS-11`, `WS-12`, `WS-13`                                                                         |
| §123 `SelectExecutionMode`                                                                                                                                                                          | `BACK-01`, `SCALE-01`, `WS-13`                                                                                                             |
| §124 `StableLogSumExp`, `StableLogMeanExp`, `StableWeightedMean`, `StableCovariance`, `DeterministicParallelReduce`                                                                                 | `FND-07`, `SCALE-01`                                                                                                                       |
| §125 `DiagnoseFittedModel`                                                                                                                                                                          | `BAY-02`, `FND-07`, `WS-40`, `WS-44`                                                                                                       |
| §126 `RunCalibrationSuite`                                                                                                                                                                          | `FND-07`, `WS-93`                                                                                                                          |
| §127 generative validation catalogue                                                                                                                                                                | `FND-07`, `GEN-01`, `WS-70`, `WS-93`                                                                                                       |
| §128 `RealDataValidationLadder`                                                                                                                                                                     | `FND-07`, `COH-01`, `WS-93`                                                                                                                |
| §129 `BenchmarkAlgorithmScaling`                                                                                                                                                                    | `SCALE-01`, `FND-07`, `WS-93`                                                                                                              |
| §130 availability mapping                                                                                                                                                                           | `FND-07`, `PLAT-01`, result-format policy                                                                                                  |
| §131 `DetermineResultMaturity`                                                                                                                                                                      | `FND-07`, `WS-94`, claim-tier policy                                                                                                       |
| §§132–133 promotion and kill procedures                                                                                                                                                             | `FND-07`, `WS-93`, `WS-94`, plus the family-specific promotion/kill criteria in each applicable card                                       |

## Unmapped or incompletely mapped master-plan IDs

These are master-plan obligations with no direct function-level owner in the pseudocode.

### Intentionally outside the pseudocode’s stated scope

The specification says it starts “after the classical K/L foundation” [at line 5 (line 5)]\(/Users/user/Downloads/marklab\_frontier\_algorithm\_pseudocode\_spec.md:5). Consequently, it does not directly specify:

- Control/replatform/current behavior: `A-01–A-03`, `B-01–B-04`, `CUR-01–CUR-18`, `WS-00`, `WS-01`, `WS-10`, `WS-A`, `WS-B`.
- Existing descriptive behavior: `EQV-01A`, `REG-01A`.
- Stable 2-D classical algorithms: `PP-01`, `PP-02`, `PP-03`, `PP-03A`, `PP-03B`, `PP-04`, `PP-04A`, `PP-04B`, `PP-04C`, `PP-05`, `PP-06A`, `PP-06B`, `PP-06C`, `WS-30`.
- Classical nulls lacking a dedicated function: `NUL-01A`, `NUL-01C`, `NUL-01E`.
- General scalar/categorical mark statistics: `MRK-01A`, `MRK-01B`, `MRK-01C`.
- Scalar geostatistics and local maps: `SIG-01A–SIG-01F`.
- Core pathology geometry: `GEO-01A`, `GEO-01B`, `GEO-01C`, `GEO-01F`.
- General niches/domains: `NIC-01A–NIC-01E`.
- Import and basic clone geometry: `CLN-01A`, `CLN-01B`.
- `IHC-01`, `BULK-01`, `CCC-01`, `CMP-01E`.
- Product surfaces: `UX-01`, `WS-25`, `WS-90`, `WS-91`, `WS-92`.

The 3-D K/cross-K and posterior-predictive references do not substitute for the missing 2-D stable estimators.

### Material frontier omissions

- `BAY-NP`: no finite mixture, DP/Pitman–Yor, HMRF/Potts, topic, or overlapping-niche algorithm.
- `EMB-PRED-02`: no cross-attention or transformer-fusion algorithm; mixture-of-experts is not equivalent.
- `DL-CORE`: no foundation-model execution adapter, checkpoint/license admission, or patient/site validation workflow.
- `EQ-01`: no E(2)/SE(2)-equivariant learned tissue model. This ID is also absent from the tracker.
- `INF-01D`: interval/coverage behavior appears in selected methods, but there is no general method-specific interval/coverage contract.
- `GEO-01D`, `PATH-01`, `WS-32`, `WS-33`, and `WS-60` have only partial coverage through topology, resource/front, embedding, and atlas functions.

### Correctly absent because the master plan gates or rejects them

- `PP-06D` toroidal default.
- `CAU-01C` causal discovery as causal evidence.
- `GEN-01C` “digital twin.”
- `SPC-01A`, `SPC-01B`, `WAV-01A`, `WAV-01B`.

Their absence is not an implementation gap to fill automatically.

## Pseudocode without sufficient master-plan authority

1. `SpatialMediationAnalysis` (§113) has no exact master ID. `CAU-01` is the nearest umbrella, but mediation is not in its method list or `WS-82/83`. It requires an explicit authoritative decision before roadmap placement.
2. `SinkhornOT` (§34.1) is defensible only as an internal primitive for `FR-03A`, `REG-01C`, partial/unbalanced OT, or FGW. The master plan does not authorize balanced OT as a separate public deliverable.
3. `SoftPairHistogram` and `SummaryMatchingLoss` (§80) are internal learned-model techniques under `GEN-01A/B`, not standalone research outcomes or result families.
4. The inference-engine algorithms in §14 are authorized as backend capabilities, but the master plan’s integrated-backend policy does not authorize native Rust implementations merely because pseudocode exists.
5. §§121–133 can instantiate the already-authorized project/workflow/validation contracts. They cannot independently create a new public API, result schema, maturity tier, or availability vocabulary without the repository’s required recorded decision.

## Numbering and authority inconsistencies

- The requested-function index is stale. It maps patient-level permutation to §10, but the body places it in §5; current §10 is Gaussian processes. Similar offsets affect hierarchical bootstrap, MMD, energy distance, GP, CAR/SAR, Bayesian engines, and point processes. The index then claims Parts IV/V retain repeated local numbering, but the body is globally and continuously numbered 0–133. See [the stale index (line 5755)]\(/Users/user/Downloads/marklab\_frontier\_algorithm\_pseudocode\_spec.md:5755).
- `SimulationBasedCalibration` is declared twice with incompatible signatures: [§15.3 (line 1363)]\(/Users/user/Downloads/marklab\_frontier\_algorithm\_pseudocode\_spec.md:1363) and [§87 (line 4377)]\(/Users/user/Downloads/marklab\_frontier\_algorithm\_pseudocode\_spec.md:4377).
- The pseudocode’s “companion charter” names `marklab_frontier_spatial_pathology_master_plan.md`, not the actual authoritative repository path, and carries no master-plan digest.
- The tracker says all 233 master-plan IDs appear once, but `EQ-01` exists in the authoritative master plan [at line 818 (line 818)]\(/Users/user/Bench/gsc-marklab/docs/implementation/MASTER\_PLAN.md:818) and has no tracker row. Under the tracker’s own inclusion policy, this is one missing canonical row.
- `BAY-REG` is used for two different meanings inside the master plan: spatial regression [at line 643 (line 643)]\(/Users/user/Bench/gsc-marklab/docs/implementation/MASTER\_PLAN.md:643) and probabilistic registration [at line 677 (line 677)]\(/Users/user/Bench/gsc-marklab/docs/implementation/MASTER\_PLAN.md:677). The detailed card and tracker use `BAY-REG` for registration and `BAY-REG-A` for spatial regression; this crosswalk follows that more specific interpretation.
- `EMB-01` is also overloaded: the rank table calls it primary CellViT support, while the detailed card defines rotation-invariant spatial dependence. `EMB-CORE` is the unambiguous embedding-artifact ID.
- `PP-01` is broad in the high-value list but narrow—homogeneous K/L—in the detailed matrix and tracker. Subordinate `PP-02–06` IDs must not be inferred complete from the broad occurrence.
- `SelectExecutionMode` reads `descriptor.modes`, while `AlgorithmDescriptor` defines `exact_modes[]` and `approximate_modes[]`.
- `DetermineResultMaturity` introduces `association_only`, which is not one of the master plan’s five maturity tiers. Association limits should be represented as a claim ceiling/reason, not a new tier unless separately authorized.

## Duplicate coverage requiring one internal owner

These overlaps should share primitives without merging or weakening their distinct master-plan scopes:

| Duplicate clusterLocations        |                                               |
| --------------------------------- | --------------------------------------------- |
| Simulation-based calibration      | §§15.3, 87, and universal harness §126        |
| Posterior-predictive checks       | §§4.4, 22, 89, and diagnostic dispatcher §125 |
| Stable covariance                 | §§3.1 and 124                                 |
| MMD                               | §§7.1, 7.3, and 28.2                          |
| Entropic Sinkhorn iteration       | §§33.2 and 34.1                               |
| Validation orchestration          | §§48, 59, 69, 90, 102, 120, and 126           |
| Transform-uncertainty propagation | §§32.3–32.4 and 93                            |

The domain-specific outputs remain separate; only the shared numerical or orchestration primitive should have one canonical owner.

## Missing backend and oracle requirements

The reference list is directionally useful but does not satisfy the master plan’s backend-admission contract because it generally says “or equivalent” and supplies no exact version, license, environment digest, schema, or selected semantic oracle.

| FamilyMissing evidence before implementation or promotion |                                                                                                                                                               |
| --------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Cohort tests/equivalence                                  | Selected independent implementation for restricted permutation, hierarchical bootstrap, MMD, energy, TOST/noninferiority, including finite-sample conventions |
| Bayesian lifecycle                                        | Exact CmdStan version plus one selected GPU-capable Python backend; model compiler contract; two-backend agreement fixtures; diagnostic normalization         |
| GP/GMRF/INLA                                              | Selected GPyTorch/GPflow and INLA/inlabru versions/licenses; matching Matérn/range, graph scaling, constraint, and approximation conventions                  |
| Bayesian point processes                                  | Exact `spatstat`/INLA/Stan functions and settings for each estimator; quadrature, likelihood, prior, and posterior-predictive agreement                       |
| Embedding prediction                                      | Admitted model backend, patient-held-out dataset, checkpoint/license provenance, calibration/conformal/OOD oracle                                             |
| Registration                                              | One selected nonrigid/diffeomorphic backend with uncertainty output, pinned environment/license, deformation and landmark truth fixtures                      |
| OT/atlas                                                  | Selected POT or other solver version/license; objective, entropy, transported-mass, stopping-residual, and non-identifiability fixtures                       |
| Graph mathematics                                         | Exact sparse-linear-algebra reference and frozen Laplacian/normalization/filter conventions; PyG/DGL alone is not a numerical oracle                          |
| Topology                                                  | One selected pinned persistence backend and exact filtration/tie/reduction conventions                                                                        |
| Multimodal factors                                        | Selected pCCA/MOFA-like and Bayesian factor backends; no concrete interoperability oracle is currently named                                                  |
| PDE/mechanistic models                                    | Selected numerical solver, discretization and convergence oracle, conservation/stability tests                                                                |
| Neural generators/SBI                                     | Selected PyTorch/JAX and `sbi` versions/licenses/checkpoints; support, calibration, memorization, and simpler-model comparisons                               |
| 3-D/longitudinal                                          | New 3-D point-process oracle, stack-reconstruction oracle, and state-space filtering/smoothing differential reference                                         |
| Causal/active design                                      | Selected estimator/design oracle plus eligible randomized or observational designs; literature citations alone do not establish operational parity            |

## Required prerequisite order

The dependency order should remain:

1. Complete the sole active classical workflow: `FND-02`, `FND-03`, `FND-06`, `FND-07`, `PP-01`, `PP-06A`, `NUL-01A`, `INF-01B`, `WS-22`, `WS-30`, `WS-31`.
2. Finish durable `PLAT-01`/`WF-01` contracts before `BACK-01`.
3. Complete `FND-04`/`WS-23` before general marks, signal, niches, or marked point processes.
4. Complete `FND-06` and then `COH-01` before `CMP-01`, `EQV-01`, predictive claims, or patient-level Bayesian claims.
5. Establish `BAY-01` then `BAY-02`; only afterward proceed to `BAY-03/04/05`, and only then `BAY-PP`, `BAY-MM`, or `BAY-REG`.
6. Establish exact graph/weight contracts before `FR-01A`, then wavelets/scattering; topology follows stable filtration and pathology endpoints.
7. Complete `WS-70` simulators before mechanistic fitting or `BAY-SBI`; `WS-74` follows validated `WS-71–73`.
8. Registration uncertainty precedes 3-D/longitudinal biological modeling.
9. `CAU-01` and `ACT-01` remain data-design gated; no causal or prospective-design algorithm should be promoted from pseudocode alone.

## Bounded future roadmap recommendations

After the current active classical outcome reaches its checkpoint and is formally promoted:

1. **Patient-level scalar randomization vertical slice.** Advance only `FND-06`, `COH-01`, `CMP-01`, `CMP-01A`, `INF-01A`, `INF-01C`, `NUL-01B`, `NUL-01D`, `WF-01`, `WS-12`, `WS-31`, and `WS-34`. Keep bootstrap, functional curves, MMD, energy, and equivalence out of this first slice.
2. **One functional cohort-comparison slice.** Add `CMP-01B` using one curve per patient and the existing ERL machinery. Do not bundle MMD, energy, graph kernels, topology, or equivalence.
3. **One multi-backend Bayesian vertical slice, only after durable backend admission.** Use `BACK-01`, `BAY-01`, `BAY-02`, `BAY-03`, `WS-13`, `WS-40`, and `WS-41` to express one small hierarchical model in CmdStan and one pinned GPU-capable Python backend, with prior predictive checks, diagnostics, SBC, and cross-backend agreement.

No Rust port should be recommended until the selected external implementation is pinned and validated, a representative workload shows a material bottleneck or integration deficit, semantic equivalence is demonstrated, and the proposed Rust path retains a simpler portable fallback. This especially applies to HMC/NUTS, INLA/SPDE, nonrigid registration, OT/FGW, persistence, multimodal factor models, neural generators, and SBI.

No files were edited and no tests were run; this was a documentation-only, read-only analysis. The worktree remained populated by pre-existing concurrent user changes.