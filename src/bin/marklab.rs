#[cfg(all(feature = "dhat-heap", not(feature = "allocator-mimalloc")))]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[cfg(all(feature = "allocator-mimalloc", not(feature = "dhat-heap")))]
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[path = "marklab/bayes.rs"]
mod bayes;
#[path = "marklab/bayes_advanced.rs"]
mod bayes_advanced;
#[path = "marklab/causal.rs"]
mod causal;
#[path = "marklab/causal_model.rs"]
mod causal_model;
#[path = "marklab/cohort.rs"]
mod cohort;
#[path = "marklab/exclusive_json_output.rs"]
mod exclusive_json_output;
#[path = "marklab/graph.rs"]
mod graph;
#[path = "marklab/local_multivariate.rs"]
mod local_multivariate;
#[path = "marklab/longitudinal.rs"]
mod longitudinal;
#[path = "marklab/multimodal_model.rs"]
mod multimodal_model;
#[path = "marklab/neural.rs"]
mod neural;
#[path = "marklab/numerics.rs"]
mod numerics;
#[path = "marklab/policy.rs"]
mod policy;
#[path = "marklab/project.rs"]
mod project;
#[path = "marklab/registration.rs"]
mod registration;
#[path = "marklab/spatial3d.rs"]
mod spatial3d;
#[path = "marklab/spatial3d_model.rs"]
mod spatial3d_model;
#[path = "marklab/spatial3d_registered.rs"]
mod spatial3d_registered;
#[path = "marklab/topology.rs"]
mod topology;

fn main() -> marklab::Result<()> {
    match std::env::args_os().nth(1).as_deref() {
        Some(command)
            if command == std::ffi::OsStr::new("project")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        matches!(
                            subcommand.to_str(),
                            Some(
                                "vector-semivariogram"
                                    | "embedding-cross-covariance-by-distance"
                                    | "kernel-mark-correlation"
                                    | "embedding-spatial-dependence-envelope"
                                    | "graph-dirichlet-energy"
                                    | "graph-smoothness-permutation-test"
                                    | "local-embedding-roughness"
                                    | "local-multivariate-moran"
                                    | "max-t-calibration"
                                    | "patient-nested-fields"
                                    | "longitudinal-kalman-smooth"
                                    | "multiscale-embedding-kernel"
                                    | "cohort-cluster-covariate-permutation"
                                    | "cohort-energy"
                                    | "causal-randomized-interference"
                                    | "gaussian-eig"
                                    | "cohort-hierarchical-max-t"
                                    | "cohort-hierarchical-bootstrap"
                                    | "cohort-max-t"
                                    | "cohort-mmd"
                                    | "cohort-multisite-covariate-contrast"
                                    | "projected-embedding-variograms"
                                    | "marked-prepost"
                                    | "sparse-radius-heat"
                                    | "sparse-radius-basis"
                                    | "graph-motif-triangle-summary"
                                    | "smc-abc-growth-front"
                                    | "spatial-varying-coefficient"
                                    | "spatial3d-k-function"
                                    | "spatial3d-voxel-k-function"
                                    | "spatial3d-registered-serial-voxel-k"
                                    | "spatial3d-registered-longitudinal-voxel-k-change"
                                    | "sparse-radius-fourier-energy"
                                    | "sparse-radius-heat-stability"
                                    | "sparse-radius-diffusion-wavelet"
                                    | "sparse-radius-scattering"
                                    | "test-cell-patch-complementarity"
                                    | "witness-persistence"
                                    | "witness-persistence-stability"
                                    | "witness-persistence-bottleneck-stability"
                                    | "arbitrary-window-ipp-likelihood"
                                    | "adaptive-window-spde"
                                    | "fit-arbitrary-window-ipp"
                                    | "arbitrary-window-ipp-spatial-ppc"
                                    | "arbitrary-window-lgcp"
                                    | "replicated-arbitrary-window-lgcp"
                                    | "replicated-arbitrary-window-lgcp-inferred-kernel"
                                    | "replicated-arbitrary-window-multitype-lgcp"
                                    | "replicated-arbitrary-window-multitype-lgcp-inferred-kernel"
                                    | "replicated-arbitrary-window-multitype-lgcp-inferred-kernel-sbc"
                                    | "correlated-replicated-arbitrary-window-multitype-lgcp"
                                    | "joint-replicated-location-embedding"
                                    | "joint-replicated-location-mark"
                                    | "conditional-multitype-mark"
                                    | "replicated-conditional-multitype-mark"
                                    | "region-retrieval"
                                    | "normal-mean"
                                    | "hierarchical-normal"
                                    | "beta-binomial-hierarchy"
                                    | "beta-binomial-group-regression"
                                    | "dirichlet-multinomial-group"
                                    | "beta-binomial-group-gender-regression"
                                    | "beta-binomial-group-gender-slide-hierarchy"
                                    | "student-t-hierarchy"
                                    | "gridded-lgcp"
                                    | "fused-gromov-wasserstein"
                            )
                        )
                    }) =>
        {
            project::run_cli().map_err(bayes::into_marklab_error)
        }
        Some(command) if command == std::ffi::OsStr::new("cohort") => {
            cohort::run_cli().map_err(cohort::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand
                            == std::ffi::OsStr::new(
                                "beta-binomial-group-gender-slide-hierarchy-sbc",
                            )
                    }) =>
        {
            bayes::run_beta_binomial_group_gender_slide_hierarchy_sbc_cli()
                .map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand
                            == std::ffi::OsStr::new(
                                "beta-binomial-group-gender-slide-hierarchy-sensitivity",
                            )
                    }) =>
        {
            bayes::run_beta_binomial_group_gender_slide_hierarchy_sensitivity_cli()
                .map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand
                            == std::ffi::OsStr::new(
                                "beta-binomial-group-gender-slide-hierarchy-agreement",
                            )
                    }) =>
        {
            bayes::run_beta_binomial_group_gender_slide_hierarchy_agreement_cli()
                .map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand == std::ffi::OsStr::new("dirichlet-multinomial-group")
                    }) =>
        {
            bayes::run_dirichlet_multinomial_group_cli().map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand == std::ffi::OsStr::new("dirichlet-multinomial-group-agreement")
                    }) =>
        {
            bayes::run_dirichlet_multinomial_group_agreement_cli()
                .map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand
                            == std::ffi::OsStr::new("dirichlet-multinomial-group-sensitivity")
                    }) =>
        {
            bayes::run_dirichlet_multinomial_group_sensitivity_cli()
                .map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand == std::ffi::OsStr::new("dirichlet-multinomial-group-sbc")
                    }) =>
        {
            bayes::run_dirichlet_multinomial_group_sbc_cli().map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        matches!(
                            subcommand.to_str(),
                            Some(
                                "hmc-normal"
                                    | "advanced-cluster"
                                    | "spde-suite"
                                    | "adaptive-window-spde"
                            )
                        )
                    }) =>
        {
            bayes_advanced::run_cli().map_err(bayes_advanced::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand == std::ffi::OsStr::new("arbitrary-window-ipp-likelihood")
                    }) =>
        {
            bayes::run_arbitrary_window_ipp_cli().map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand == std::ffi::OsStr::new("fit-arbitrary-window-lgcp")
                    }) =>
        {
            bayes::run_arbitrary_window_lgcp_fit_cli().map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand == std::ffi::OsStr::new("arbitrary-window-lgcp-agreement")
                    }) =>
        {
            bayes::run_arbitrary_window_lgcp_agreement_cli().map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand
                            == std::ffi::OsStr::new("arbitrary-window-lgcp-quadrature-sensitivity")
                    }) =>
        {
            bayes::run_arbitrary_window_lgcp_quadrature_sensitivity_cli()
                .map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand == std::ffi::OsStr::new("arbitrary-window-lgcp-sbc")
                    }) =>
        {
            bayes::run_arbitrary_window_lgcp_sbc_cli().map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand == std::ffi::OsStr::new("fit-replicated-arbitrary-window-lgcp")
                    }) =>
        {
            bayes::run_replicated_arbitrary_window_lgcp_fit_cli().map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand
                            == std::ffi::OsStr::new(
                                "fit-replicated-arbitrary-window-lgcp-inferred-kernel",
                            )
                    }) =>
        {
            bayes::run_replicated_arbitrary_window_lgcp_inferred_kernel_cli()
                .map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand
                            == std::ffi::OsStr::new(
                                "replicated-arbitrary-window-lgcp-inferred-kernel-agreement",
                            )
                    }) =>
        {
            bayes::run_replicated_arbitrary_window_lgcp_inferred_kernel_agreement_cli()
                .map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand
                            == std::ffi::OsStr::new(
                                "replicated-arbitrary-window-lgcp-inferred-kernel-sbc",
                            )
                    }) =>
        {
            bayes::run_replicated_arbitrary_window_lgcp_inferred_kernel_sbc_cli()
                .map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand == std::ffi::OsStr::new("fit-conditional-multitype-mark")
                    }) =>
        {
            bayes::run_conditional_multitype_mark_cli().map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand == std::ffi::OsStr::new("conditional-multitype-mark-agreement")
                    }) =>
        {
            bayes::run_conditional_multitype_mark_agreement_cli().map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand == std::ffi::OsStr::new("conditional-multitype-mark-sensitivity")
                    }) =>
        {
            bayes::run_conditional_multitype_mark_sensitivity_cli()
                .map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand == std::ffi::OsStr::new("conditional-multitype-mark-sbc")
                    }) =>
        {
            bayes::run_conditional_multitype_mark_sbc_cli().map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand
                            == std::ffi::OsStr::new("fit-replicated-conditional-multitype-mark")
                    }) =>
        {
            bayes::run_replicated_conditional_multitype_mark_cli()
                .map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand
                            == std::ffi::OsStr::new(
                                "replicated-conditional-multitype-mark-agreement",
                            )
                    }) =>
        {
            bayes::run_replicated_conditional_multitype_mark_agreement_cli()
                .map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand
                            == std::ffi::OsStr::new(
                                "replicated-conditional-multitype-mark-sensitivity",
                            )
                    }) =>
        {
            bayes::run_replicated_conditional_multitype_mark_sensitivity_cli()
                .map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand
                            == std::ffi::OsStr::new("replicated-conditional-multitype-mark-sbc")
                    }) =>
        {
            bayes::run_replicated_conditional_multitype_mark_sbc_cli()
                .map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand
                            == std::ffi::OsStr::new(
                                "fit-replicated-arbitrary-window-multitype-lgcp",
                            )
                    }) =>
        {
            bayes::run_replicated_arbitrary_window_multitype_lgcp_cli()
                .map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand
                            == std::ffi::OsStr::new(
                                "replicated-arbitrary-window-multitype-lgcp-agreement",
                            )
                    }) =>
        {
            bayes::run_replicated_arbitrary_window_multitype_lgcp_agreement_cli()
                .map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand == std::ffi::OsStr::new(
                            "replicated-arbitrary-window-multitype-lgcp-inferred-kernel-prior-calibration",
                        )
                    }) =>
        {
            bayes::run_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_prior_calibration_cli()
                .map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand == std::ffi::OsStr::new(
                            "replicated-arbitrary-window-multitype-lgcp-inferred-kernel-sbc",
                        )
                    }) =>
        {
            bayes::run_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_sbc_cli()
                .map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand == std::ffi::OsStr::new("fit-joint-replicated-location-mark")
                    }) =>
        {
            bayes::run_joint_replicated_location_mark_cli().map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand
                            == std::ffi::OsStr::new(
                                "fit-correlated-replicated-arbitrary-window-multitype-lgcp",
                            )
                    }) =>
        {
            bayes::run_correlated_replicated_arbitrary_window_multitype_lgcp_cli()
                .map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand
                            == std::ffi::OsStr::new(
                                "fit-joint-replicated-location-embedding",
                            )
                    }) =>
        {
            bayes::run_joint_replicated_location_embedding_cli()
                .map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand == std::ffi::OsStr::new(
                            "replicated-arbitrary-window-multitype-lgcp-inferred-kernel-agreement",
                        )
                    }) =>
        {
            bayes::run_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_agreement_cli()
                .map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand
                            == std::ffi::OsStr::new(
                                "fit-replicated-arbitrary-window-multitype-lgcp-inferred-kernel",
                            )
                    }) =>
        {
            bayes::run_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_cli()
                .map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand
                            == std::ffi::OsStr::new(
                                "replicated-arbitrary-window-multitype-lgcp-prior-calibration",
                            )
                    }) =>
        {
            bayes::run_replicated_arbitrary_window_multitype_lgcp_prior_calibration_cli()
                .map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand
                            == std::ffi::OsStr::new(
                                "replicated-arbitrary-window-multitype-lgcp-sensitivity",
                            )
                    }) =>
        {
            bayes::run_replicated_arbitrary_window_multitype_lgcp_sensitivity_cli()
                .map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand
                            == std::ffi::OsStr::new("replicated-arbitrary-window-lgcp-agreement")
                    }) =>
        {
            bayes::run_replicated_arbitrary_window_lgcp_agreement_cli()
                .map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand
                            == std::ffi::OsStr::new("replicated-arbitrary-window-lgcp-sensitivity")
                    }) =>
        {
            bayes::run_replicated_arbitrary_window_lgcp_sensitivity_cli()
                .map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand == std::ffi::OsStr::new("replicated-arbitrary-window-lgcp-sbc")
                    }) =>
        {
            bayes::run_replicated_arbitrary_window_lgcp_sbc_cli().map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand == std::ffi::OsStr::new("arbitrary-window-lgcp-sensitivity")
                    }) =>
        {
            bayes::run_arbitrary_window_lgcp_sensitivity_cli().map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand == std::ffi::OsStr::new("fit-arbitrary-window-ipp")
                    }) =>
        {
            bayes::run_arbitrary_window_ipp_fit_cli().map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand == std::ffi::OsStr::new("arbitrary-window-ipp-agreement")
                    }) =>
        {
            bayes::run_arbitrary_window_ipp_agreement_cli().map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand == std::ffi::OsStr::new("arbitrary-window-ipp-sensitivity")
                    }) =>
        {
            bayes::run_arbitrary_window_ipp_sensitivity_cli().map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand
                            == std::ffi::OsStr::new("arbitrary-window-ipp-quadrature-sensitivity")
                    }) =>
        {
            bayes::run_arbitrary_window_ipp_quadrature_sensitivity_cli()
                .map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand == std::ffi::OsStr::new("arbitrary-window-ipp-sbc")
                    }) =>
        {
            bayes::run_arbitrary_window_ipp_sbc_cli().map_err(bayes::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("bayes")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        subcommand == std::ffi::OsStr::new("arbitrary-window-ipp-spatial-ppc")
                    }) =>
        {
            bayes::run_arbitrary_window_ipp_spatial_ppc_cli().map_err(bayes::into_marklab_error)
        }
        Some(command) if command == std::ffi::OsStr::new("bayes") => {
            bayes::run_cli().map_err(bayes::into_marklab_error)
        }
        Some(command) if command == std::ffi::OsStr::new("longitudinal") => {
            longitudinal::run_cli().map_err(longitudinal::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("spatial3d")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        matches!(
                            subcommand.to_str(),
                            Some(
                                "serial-stack"
                                    | "alpha-complex"
                                    | "deformation-biology"
                                    | "clone-models"
                                    | "validate-advanced"
                            )
                        )
                    }) =>
        {
            spatial3d_model::run_cli().map_err(spatial3d_model::into_marklab_error)
        }
        Some(command) if command == std::ffi::OsStr::new("spatial3d") => {
            spatial3d::run_cli().map_err(spatial3d::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("causal")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        matches!(
                            subcommand.to_str(),
                            Some(
                                "observational"
                                    | "perturbation"
                                    | "active-design"
                                    | "validate-active"
                            )
                        )
                    }) =>
        {
            causal_model::run_cli().map_err(causal_model::into_marklab_error)
        }
        Some(command) if command == std::ffi::OsStr::new("causal") => {
            causal::run_cli().map_err(causal::into_marklab_error)
        }
        Some(command) if command == std::ffi::OsStr::new("numerics") => {
            numerics::run_cli().map_err(numerics::into_marklab_error)
        }
        Some(command) if command == std::ffi::OsStr::new("policy") => {
            policy::run_cli().map_err(policy::into_marklab_error)
        }
        Some(command) if command == std::ffi::OsStr::new("graph") => {
            graph::run_cli().map_err(graph::into_marklab_error)
        }
        Some(command) if command == std::ffi::OsStr::new("topology") => {
            topology::run_cli().map_err(topology::into_marklab_error)
        }
        Some(command) if command == std::ffi::OsStr::new("registration") => {
            registration::run_cli().map_err(registration::into_marklab_error)
        }
        Some(command) if command == std::ffi::OsStr::new("neural") => {
            neural::run_cli().map_err(neural::into_marklab_error)
        }
        Some(command)
            if command == std::ffi::OsStr::new("multimodal")
                && std::env::args_os()
                    .nth(2)
                    .as_deref()
                    .is_some_and(|subcommand| {
                        matches!(
                            subcommand.to_str(),
                            Some(
                                "pcca"
                                    | "bayesian-pcca"
                                    | "mofa"
                                    | "matrix-factor"
                                    | "hierarchical-factor"
                                    | "spatial-matrix-factor"
                                    | "tensor-factor"
                                    | "spatial-latent-factor"
                                    | "multiresolution-factor"
                                    | "dropout-robust"
                                    | "joint-pathology"
                                    | "compare-models"
                                    | "validate"
                            )
                        )
                    }) =>
        {
            multimodal_model::run_cli().map_err(multimodal_model::into_marklab_error)
        }
        _ => marklab::run_cli(),
    }
}
