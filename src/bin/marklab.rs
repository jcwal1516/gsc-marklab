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
#[path = "marklab/graph.rs"]
mod graph;
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
                                "marked-prepost"
                                    | "normal-mean"
                                    | "hierarchical-normal"
                                    | "beta-binomial-hierarchy"
                                    | "beta-binomial-group-regression"
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
                        matches!(
                            subcommand.to_str(),
                            Some("hmc-normal" | "advanced-cluster" | "spde-suite")
                        )
                    }) =>
        {
            bayes_advanced::run_cli().map_err(bayes_advanced::into_marklab_error)
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
