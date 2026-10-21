use super::*;

pub(crate) fn run_cli(command: BayesCommand) -> Result<(), CliError> {
    match command {
        BayesCommand::Hierarchy(command) => match command {
            HierarchyCommands::BetaBinomialGroupGenderSlideHierarchyAgreement(arguments) => {
                beta_binomial_group_gender_slide_agreement::run(*arguments)
            }
            HierarchyCommands::BetaBinomialGroupGenderSlideHierarchySensitivity(arguments) => {
                beta_binomial_group_gender_slide_sensitivity::run(*arguments)
            }
            HierarchyCommands::BetaBinomialGroupGenderSlideHierarchySbc(arguments) => {
                beta_binomial_group_gender_slide_sbc::run(*arguments)
            }
            HierarchyCommands::DirichletMultinomialGroup(arguments) => {
                dirichlet_multinomial_group::run(*arguments)
            }
            HierarchyCommands::DirichletMultinomialGroupAgreement(arguments) => {
                dirichlet_multinomial_group_agreement::run(*arguments)
            }
            HierarchyCommands::DirichletMultinomialGroupSensitivity(arguments) => {
                dirichlet_multinomial_group_sensitivity::run(*arguments)
            }
            HierarchyCommands::DirichletMultinomialGroupSbc(arguments) => {
                dirichlet_multinomial_group_sbc::run(*arguments)
            }
            HierarchyCommands::NegativeBinomialHierarchy(args) => {
                negative_binomial_hierarchy::run(args)
            }
            HierarchyCommands::StudentTHierarchy(args) => student_t_hierarchy::run(args),
            HierarchyCommands::StudentTHierarchyAgreement(args) => {
                student_t_hierarchy_agreement::run(args)
            }
            HierarchyCommands::StudentTHierarchySensitivity(args) => {
                student_t_hierarchy_sensitivity::run(args)
            }
            HierarchyCommands::StudentTHierarchySbc(args) => student_t_hierarchy_sbc::run(args),
            HierarchyCommands::BetaBinomialHierarchy(args) => beta_binomial_hierarchy::run(args),
            HierarchyCommands::BetaBinomialHierarchyAgreement(args) => {
                beta_binomial_hierarchy_agreement::run(args)
            }
            HierarchyCommands::BetaBinomialHierarchySensitivity(args) => {
                beta_binomial_hierarchy_sensitivity::run(args)
            }
            HierarchyCommands::BetaBinomialHierarchySbc(args) => {
                beta_binomial_hierarchy_sbc::run(args)
            }
            HierarchyCommands::BetaBinomialGroupRegression(args) => {
                beta_binomial_group_regression::run(args)
            }
            HierarchyCommands::BetaBinomialGroupGenderRegression(args) => {
                beta_binomial_group_gender_regression::run(args)
            }
            HierarchyCommands::BetaBinomialGroupGenderRegressionAgreement(args) => {
                beta_binomial_group_gender_agreement::run(args)
            }
            HierarchyCommands::BetaBinomialGroupGenderRegressionSensitivity(args) => {
                beta_binomial_group_gender_sensitivity::run(args)
            }
            HierarchyCommands::BetaBinomialGroupGenderRegressionSbc(args) => {
                beta_binomial_group_gender_sbc::run(args)
            }
            HierarchyCommands::BetaBinomialGroupGenderSlideHierarchy(args) => {
                beta_binomial_group_gender_slide_hierarchy::run(args)
            }
            HierarchyCommands::BetaBinomialGroupRegressionAgreement(args) => {
                beta_binomial_group_regression_agreement::run(args)
            }
            HierarchyCommands::BetaBinomialGroupRegressionSensitivity(args) => {
                beta_binomial_group_regression_sensitivity::run(args)
            }
            HierarchyCommands::BetaBinomialGroupRegressionSbc(args) => {
                beta_binomial_group_regression_sbc::run(args)
            }
            HierarchyCommands::NormalMean(args) => normal_mean::run(args),
            HierarchyCommands::HierarchicalNormal(args) => hierarchical::run(args),
            HierarchyCommands::HierarchicalNormalAgreement(args) => {
                hierarchical_agreement::run(args)
            }
            HierarchyCommands::HierarchicalNormalPriorSensitivity(args) => {
                hierarchical_sensitivity::run(args)
            }
            HierarchyCommands::HierarchicalNormalSbc(args) => hierarchical_sbc::run(args),
            HierarchyCommands::MetaAnalysis(args) => meta_analysis::run(args),
                    HierarchyCommands::GaussianCrossedNestedHierarchy(args) => gaussian_crossed_nested_hierarchy::run_command(*args),
            HierarchyCommands::HurdleBetaBinomialGroup(args) => hurdle_beta_binomial_group::run(*args),
            HierarchyCommands::NonproportionalOrdinalGroupSite(args) => nonproportional_ordinal_group_site::run(*args),
            HierarchyCommands::OrdinalGroup(args) => ordinal_group::run(*args),
            HierarchyCommands::OrdinalGroupSiteHierarchy(args) => ordinal_group_site_hierarchy::run(*args),
            HierarchyCommands::OrdinalSiteHeldoutComparison(args) => ordinal_site_heldout::run(*args),
        },
        BayesCommand::Fields(command) => match command {
            FieldsCommands::GpRegression(args) => gp::run(args),
            FieldsCommands::AnisotropicGp3d(args) => anisotropic_gp3d::run(args),
            FieldsCommands::MultiOutputGp(args) => multi_output_gp::run(args),
            FieldsCommands::VariationalGp(args) => variational_gp::run(args),
            FieldsCommands::PredictiveProcess(args) => predictive_process::run(args),
            FieldsCommands::NngpDensity(args) => nngp::run(args),
            FieldsCommands::ValidateWeights(args) => weights::run(args),
            FieldsCommands::CarDensity(args) => car::run(args),
            FieldsCommands::SparseCarFit(args) => sparse_car::run(args),
            FieldsCommands::GmrfDensity(args) => gmrf::run(args),
            FieldsCommands::SarLikelihood(args) => sar::run(args),
            FieldsCommands::SarFit(args) => sar_fit::run(args),
            FieldsCommands::BymFit(args) => bym::run(args),
            FieldsCommands::Bym2Fit(args) => bym2::run(args),
            FieldsCommands::SpatialVaryingCoefficient(args) => {
                spatial_varying_coefficient::run(args)
            }
            FieldsCommands::DistanceToResource(args) => distance_to_resource::run(args),
        },
        BayesCommand::SimulationInference(command) => match command {
            SimulationInferenceCommands::RejectionAbcGrowthFront(args) => rejection_abc::run(args),
            SimulationInferenceCommands::SmcAbcGrowthFront(args) => smc_abc::run(args),
            SimulationInferenceCommands::SyntheticLikelihoodGrowthFront(args) => {
                synthetic_likelihood::run(args)
            }
            SimulationInferenceCommands::GrowthFrontRejectionAbcSbc(args) => sbi_sbc::run(args),
            SimulationInferenceCommands::SimulationOod(args) => simulation_ood::run(args),
            SimulationInferenceCommands::GrowthFrontPosteriorPredictiveLab(args) => {
                posterior_predictive_lab::run(args)
            }
        },
        BayesCommand::Inference(command) => match command {
            InferenceCommands::NormalMeanSmc(args) => smc::run(args),
            InferenceCommands::PoissonLogRateLaplace(args) => laplace::run(args),
            InferenceCommands::PoissonLognormalInla(args) => inla::run(args),
            InferenceCommands::PsisLoo(args) => psis_loo::run(args),
            InferenceCommands::CompareModels(args) => model_comparison::run(args),
            InferenceCommands::NormalMeanSbc(args) => sbc::run(args),
            InferenceCommands::NormalMeanPriorSensitivity(args) => prior_sensitivity::run(args),
        },
        BayesCommand::PointProcesses(command) => match command {
            PointProcessesCommands::InhomogeneousPoissonLikelihood(args) => {
                inhomogeneous_poisson::run(args)
            }
            PointProcessesCommands::FitInhomogeneousPoisson(args) => {
                inhomogeneous_poisson_fit::run(args)
            }
            PointProcessesCommands::BermanTurnerRefinement(args) => berman_turner::run(args),
            PointProcessesCommands::BuildGriddedLgcp(args) => gridded_lgcp::run(args),
            PointProcessesCommands::FitGriddedLgcp(args) => gridded_lgcp_fit::run(args),
            PointProcessesCommands::GriddedLgcpAgreement(args) => gridded_lgcp_agreement::run(args),
            PointProcessesCommands::GriddedLgcpSbc(args) => gridded_lgcp_sbc::run(args),
            PointProcessesCommands::GriddedLgcpSensitivity(args) => {
                gridded_lgcp_sensitivity::run(args)
            }
            PointProcessesCommands::GriddedLgcpSpatialPpc(args) => {
                gridded_lgcp_spatial_ppc::run(args)
            }
            PointProcessesCommands::SimulateGriddedLgcpPosteriorPredictive(args) => {
                gridded_lgcp_fit::run_predictive(args)
            }
            PointProcessesCommands::SimulateThomasProcess(args) => thomas::run(args),
            PointProcessesCommands::SimulateMaternClusterProcess(args) => matern_cluster::run(args),
            PointProcessesCommands::FitThomasMinimumContrast(args) => {
                thomas_minimum_contrast::run(args)
            }
            PointProcessesCommands::StraussStatistics(args) => strauss::run(args),
            PointProcessesCommands::SimulateStraussBirthDeath(args) => strauss_gibbs::run(args),
            PointProcessesCommands::FitStraussPseudolikelihood(args) => {
                strauss_pseudolikelihood::run(args)
            }
            PointProcessesCommands::GeyerSaturationStatistic(args) => geyer::run(args),
            PointProcessesCommands::MultitypePapangelou(args) => multitype::run(args),
            PointProcessesCommands::BuildJointLocationMarkModel(args) => joint_mark::run(args),
            PointProcessesCommands::BuildJointContinuousMarkModel(args) => {
                joint_mark::run_continuous(args)
            }
            PointProcessesCommands::BuildJointLocationEmbeddingFactorModel(args) => {
                embedding_factor::run(args)
            }
            PointProcessesCommands::BuildReplicatedHierarchicalLgcp(args) => replicated::run(args),
            PointProcessesCommands::PointProcessPosteriorPredictiveDiagnostics(args) => {
                point_process_ppc::run(args)
            }
        },
        BayesCommand::Embeddings(command) => match command {
            EmbeddingsCommands::VectorSemivariogram(args) => {
                embedding_spatial::run_vector_semivariogram(args)
            }
            EmbeddingsCommands::ProjectedEmbeddingVariograms(args) => {
                embedding_spatial::run_projected_variograms(args)
            }
            EmbeddingsCommands::EmbeddingCrossCovarianceByDistance(args) => {
                embedding_spatial::run_cross_covariance(args)
            }
            EmbeddingsCommands::CrossModalCovarianceByDistance(args) => cross_modal::run(args),
            EmbeddingsCommands::KernelMarkCorrelation(args) => embedding_kernel::run(args),
            EmbeddingsCommands::EmbeddingSpatialDependenceEnvelope(args) => {
                embedding_envelope::run(args)
            }
            EmbeddingsCommands::GraphDirichletEnergy(args) => graph_signal::run_energy(args),
            EmbeddingsCommands::GraphSmoothnessPermutationTest(args) => {
                graph_signal::run_smoothness(args)
            }
            EmbeddingsCommands::LocalEmbeddingRoughness(args) => graph_signal::run_local(args),
            EmbeddingsCommands::TestCellPatchComplementarity(args) => complementarity::run(args),
            EmbeddingsCommands::MultiscaleEmbeddingKernel(args) => multiscale_kernel::run(args),
            EmbeddingsCommands::RetrieveAnalogousRegions(args) => retrieval::run(args),
        },
        BayesCommand::Prediction(command) => match command {
            PredictionCommands::ApplyAbstention(args) => prediction_safety::run_abstention(args),
            PredictionCommands::OodScore(args) => prediction_safety::run_ood(args),
            PredictionCommands::CalibratePredictions(args) => prediction_calibration::run(args),
            PredictionCommands::GroupedConformal(args) => grouped_conformal::run(args),
            PredictionCommands::LateFusion(args) => late_fusion::run(args),
            PredictionCommands::PredictiveStacking(args) => predictive_stacking::run(args),
            PredictionCommands::MixtureOfExpertsFusion(args) => mixture_of_experts::run(args),
        },
        BayesCommand::Transport(command) => match command {
            TransportCommands::SinkhornOt(args) => transport::run_sinkhorn(args),
            TransportCommands::EntropicSoftAssignment(args) => transport::run_soft_assignment(args),
            TransportCommands::FusedGromovWasserstein(args) => fused_gromov::run(args),
            TransportCommands::PartialFusedGromovWasserstein(args) => {
                partial_fused_gromov::run(args)
            }
            TransportCommands::UnbalancedFusedGromovWasserstein(args) => {
                unbalanced_fused_gromov::run(args)
            }
            TransportCommands::UnbalancedSinkhorn(args) => transport::run_unbalanced(args),
            TransportCommands::PartialOt(args) => transport::run_partial(args),
        },
        BayesCommand::ArbitraryWindow(command) => match command {
            ArbitraryWindowCommands::ArbitraryWindowIppAgreement(args) => arbitrary_window_ipp_agreement::run(args),
            ArbitraryWindowCommands::FitArbitraryWindowIpp(args) => arbitrary_window_ipp_fit::run(*args),
            ArbitraryWindowCommands::ArbitraryWindowIppQuadratureSensitivity(args) => arbitrary_window_ipp_quadrature_sensitivity::run(args),
            ArbitraryWindowCommands::ArbitraryWindowIppSbc(args) => arbitrary_window_ipp_sbc::run(*args),
            ArbitraryWindowCommands::ArbitraryWindowIppSensitivity(args) => arbitrary_window_ipp_sensitivity::run(*args),
            ArbitraryWindowCommands::ArbitraryWindowIppSpatialPpc(args) => arbitrary_window_ipp_spatial_ppc::run(args),
            ArbitraryWindowCommands::ArbitraryWindowLgcpAgreement(args) => arbitrary_window_lgcp_agreement::run(args),
            ArbitraryWindowCommands::FitArbitraryWindowLgcp(args) => arbitrary_window_lgcp_fit::run(args),
            ArbitraryWindowCommands::ArbitraryWindowLgcpQuadratureSensitivity(args) => arbitrary_window_lgcp_quadrature_sensitivity::run(args),
            ArbitraryWindowCommands::ArbitraryWindowLgcpSbc(args) => arbitrary_window_lgcp_sbc::run(args),
            ArbitraryWindowCommands::ArbitraryWindowLgcpSensitivity(args) => arbitrary_window_lgcp_sensitivity::run(args),
            ArbitraryWindowCommands::FitCorrelatedReplicatedArbitraryWindowMultitypeLgcp(args) => correlated_replicated_arbitrary_window_multitype_lgcp::run(args),
            ArbitraryWindowCommands::ReplicatedArbitraryWindowLgcpAgreement(args) => replicated_arbitrary_window_lgcp_agreement::run(*args),
            ArbitraryWindowCommands::FitReplicatedArbitraryWindowLgcp(args) => replicated_arbitrary_window_lgcp_fit::run(args),
            ArbitraryWindowCommands::FitReplicatedArbitraryWindowLgcpInferredKernel(args) => replicated_arbitrary_window_lgcp_inferred_kernel::run(args),
            ArbitraryWindowCommands::ReplicatedArbitraryWindowLgcpInferredKernelAgreement(args) => replicated_arbitrary_window_lgcp_inferred_kernel_agreement::run(*args),
            ArbitraryWindowCommands::ReplicatedArbitraryWindowLgcpInferredKernelSbc(args) => replicated_arbitrary_window_lgcp_inferred_kernel_sbc::run(*args),
            ArbitraryWindowCommands::ReplicatedArbitraryWindowLgcpSbc(args) => replicated_arbitrary_window_lgcp_sbc::run(*args),
            ArbitraryWindowCommands::ReplicatedArbitraryWindowLgcpSensitivity(args) => replicated_arbitrary_window_lgcp_sensitivity::run(*args),
            ArbitraryWindowCommands::FitReplicatedArbitraryWindowMultitypeLgcp(args) => replicated_arbitrary_window_multitype_lgcp::run(args),
            ArbitraryWindowCommands::ReplicatedArbitraryWindowMultitypeLgcpAgreement(args) => replicated_arbitrary_window_multitype_lgcp_agreement::run(*args),
            ArbitraryWindowCommands::FitReplicatedArbitraryWindowMultitypeLgcpInferredKernel(args) => replicated_arbitrary_window_multitype_lgcp_inferred_kernel::run(args),
            ArbitraryWindowCommands::ReplicatedArbitraryWindowMultitypeLgcpInferredKernelAgreement(args) => replicated_arbitrary_window_multitype_lgcp_inferred_kernel_agreement::run(*args),
            ArbitraryWindowCommands::ReplicatedArbitraryWindowMultitypeLgcpInferredKernelPriorCalibration(args) => replicated_arbitrary_window_multitype_lgcp_inferred_kernel_prior_calibration::run(*args),
            ArbitraryWindowCommands::ReplicatedArbitraryWindowMultitypeLgcpInferredKernelSbc(args) => replicated_arbitrary_window_multitype_lgcp_inferred_kernel_sbc::run(args),
            ArbitraryWindowCommands::ReplicatedArbitraryWindowMultitypeLgcpPriorCalibration(args) => replicated_arbitrary_window_multitype_lgcp_prior_calibration::run(*args),
            ArbitraryWindowCommands::ReplicatedArbitraryWindowMultitypeLgcpSensitivity(args) => replicated_arbitrary_window_multitype_lgcp_sensitivity::run(*args),
            ArbitraryWindowCommands::ArbitraryWindowIppLikelihood(args) => arbitrary_window_ipp::run(*args),
        },
        BayesCommand::MarkModels(command) => match command {
            MarkModelsCommands::FitConditionalMultitypeMark(args) => conditional_multitype_mark::run(*args),
            MarkModelsCommands::ConditionalMultitypeMarkAgreement(args) => conditional_multitype_mark_agreement::run(*args),
            MarkModelsCommands::ConditionalMultitypeMarkSbc(args) => conditional_multitype_mark_sbc::run(*args),
            MarkModelsCommands::ConditionalMultitypeMarkSensitivity(args) => conditional_multitype_mark_sensitivity::run(*args),
            MarkModelsCommands::FitJointReplicatedLocationEmbedding(args) => joint_replicated_location_embedding::run(args),
            MarkModelsCommands::FitJointReplicatedLocationMark(args) => joint_replicated_location_mark::run(args),
            MarkModelsCommands::FitReplicatedConditionalMultitypeMark(args) => replicated_conditional_multitype_mark::run(args),
            MarkModelsCommands::ReplicatedConditionalMultitypeMarkAgreement(args) => replicated_conditional_multitype_mark_agreement::run(*args),
            MarkModelsCommands::ReplicatedConditionalMultitypeMarkSbc(args) => replicated_conditional_multitype_mark_sbc::run(*args),
            MarkModelsCommands::ReplicatedConditionalMultitypeMarkSensitivity(args) => replicated_conditional_multitype_mark_sensitivity::run(*args),
        },
    }
}
