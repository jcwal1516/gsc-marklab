use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};
use marklab::{MarkedPatternResult, MarkedPrePostNode, ResultDocument};
use marklab_bayes::{
    BackendContract, BetaBinomialGroupGenderRegressionWorkerResult,
    BetaBinomialGroupGenderSlideHierarchyWorkerResult, BetaBinomialGroupRegressionWorkerResult,
    BetaBinomialHierarchyWorkerResult, DirichletMultinomialGroupWorkerResult,
    FusedGromovWassersteinWorkerResult, GriddedLgcpFitWorkerResult, HierarchicalWorkerResult,
    NutsSamplingSpec, StudentTHierarchyWorkerResult, WorkerResult,
};
use marklab_workflow::{
    execute_algorithm, ArtifactRef, ArtifactSchema, CacheKeyMaterial, CacheStatus, ContentDigest,
    DurableProject, DurableProjectLimits, DurableRecoveryAction, LocalScheduler, MarklabProject,
    NativeRuntimeProvenance, NodeError, NodeId, NodeSpec, SchedulerLimits, WorkflowGraph,
    WorkflowNode,
};

use super::bayes::{
    self, fused_gromov::PreparedFusedGromovWasserstein, BayesCliError,
    PreparedBetaBinomialGroupGenderRegression, PreparedBetaBinomialGroupGenderSlideHierarchy,
    PreparedBetaBinomialGroupRegression, PreparedBetaBinomialHierarchy,
    PreparedDirichletMultinomialGroup, PreparedGaussianHierarchy, PreparedGriddedLgcpFit,
    PreparedNormalMean, PreparedStudentTHierarchy,
};

#[path = "project/region_retrieval.rs"]
mod region_retrieval;

const MAXIMUM_EXECUTABLE_BYTES: u64 = 1024 * 1024 * 1024;
const MAXIMUM_INPUT_BYTES: u64 = 16 * 1024 * 1024;
const PROJECT_CONTROL_BYTES: usize = 1024 * 1024;
const PROJECT_LEDGER_BYTES: usize = 16 * 1024 * 1024;
const PROJECT_LEDGER_RECORDS: usize = 10_000;
const PROJECT_RECORD_BYTES: usize = 64 * 1024;
const MAXIMUM_RESULT_BYTES: usize = 1024 * 1024;
const MARKED_RESULT_KIND: &str = "application/vnd.marklab.result+json;version=0.3";

#[derive(Clone, Copy)]
enum StaticBackendWorkflow {
    PymcNormalMean,
    PymcHierarchicalNormal,
    PymcBetaBinomialHierarchy,
    PymcBetaBinomialGroupRegression,
    PymcDirichletMultinomialGroup,
    PymcBetaBinomialGroupGenderRegression,
    PymcBetaBinomialGroupGenderSlideHierarchy,
    PymcStudentTHierarchy,
    PymcGriddedLgcp,
    PotFusedGromovWasserstein,
}

#[derive(Clone, Copy)]
struct StaticBackendDescriptor {
    backend_id: &'static str,
    backend_version: &'static str,
    python_version: &'static str,
    license: &'static str,
    input_kinds: &'static [&'static str],
    output_kind: &'static str,
    result_schema_id: &'static str,
    node_id: &'static str,
    node_kind: &'static str,
    implementation_identity: &'static str,
    deterministic_controls: &'static str,
}

impl StaticBackendWorkflow {
    fn descriptor(self) -> StaticBackendDescriptor {
        match self {
            Self::PymcNormalMean => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: "6.3.0",
                python_version: "3.12",
                license: "Apache-2.0",
                input_kinds: &["application/vnd.marklab.source.normal-mean-observations;version=1"],
                output_kind: "application/vnd.marklab.pymc-worker-result+json;version=1",
                result_schema_id: "marklab.pymc_worker_result",
                node_id: "pymc-normal-mean",
                node_kind: "bayesian_fit",
                implementation_identity: "marklab-project-pymc-normal-mean-node-v1",
                deterministic_controls: "seeded-nuts-request",
            },
            Self::PymcHierarchicalNormal => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: "6.3.0",
                python_version: "3.12",
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.hierarchical-normal-observations;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-hierarchical-worker-result+json;version=1",
                result_schema_id: "marklab.pymc_hierarchical_worker_result",
                node_id: "pymc-hierarchical-normal",
                node_kind: "bayesian_hierarchical_fit",
                implementation_identity: "marklab-project-pymc-hierarchical-normal-node-v1",
                deterministic_controls: "seeded-nuts-hierarchical-request",
            },
            Self::PymcBetaBinomialHierarchy => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: "6.3.0",
                python_version: "3.12",
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.beta-binomial-patient-counts;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-beta-binomial-hierarchy-worker-result+json;version=1",
                result_schema_id: "marklab.pymc_beta_binomial_hierarchy_worker_result",
                node_id: "pymc-beta-binomial-hierarchy",
                node_kind: "bayesian_count_hierarchical_fit",
                implementation_identity: "marklab-project-pymc-beta-binomial-hierarchy-node-v1",
                deterministic_controls: "seeded-nuts-beta-binomial-hierarchical-request",
            },
            Self::PymcBetaBinomialGroupRegression => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: "6.3.0",
                python_version: "3.12",
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.beta-binomial-patient-group-counts;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-beta-binomial-group-regression-worker-result+json;version=1",
                result_schema_id: "marklab.pymc_beta_binomial_group_regression_worker_result",
                node_id: "pymc-beta-binomial-group-regression",
                node_kind: "bayesian_count_group_regression_fit",
                implementation_identity:
                    "marklab-project-pymc-beta-binomial-group-regression-node-v1",
                deterministic_controls: "seeded-nuts-beta-binomial-group-regression-request",
            },
            Self::PymcDirichletMultinomialGroup => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: "6.3.0",
                python_version: "3.12",
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.dirichlet-multinomial-patient-group-counts;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-dirichlet-multinomial-group-worker-result+json;version=1",
                result_schema_id: "marklab.pymc_dirichlet_multinomial_group_worker_result",
                node_id: "pymc-dirichlet-multinomial-group",
                node_kind: "bayesian_multiclass_count_group_regression_fit",
                implementation_identity:
                    "marklab-project-pymc-dirichlet-multinomial-group-node-v1",
                deterministic_controls: "seeded-nuts-dirichlet-multinomial-group-request",
            },
            Self::PymcBetaBinomialGroupGenderRegression => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: "6.3.0",
                python_version: "3.12",
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.beta-binomial-patient-group-gender-counts;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-beta-binomial-group-gender-regression-worker-result+json;version=1",
                result_schema_id:
                    "marklab.pymc_beta_binomial_group_gender_regression_worker_result",
                node_id: "pymc-beta-binomial-group-gender-regression",
                node_kind: "bayesian_count_group_gender_regression_fit",
                implementation_identity:
                    "marklab-project-pymc-beta-binomial-group-gender-regression-node-v1",
                deterministic_controls:
                    "seeded-nuts-beta-binomial-group-gender-regression-request",
            },
            Self::PymcBetaBinomialGroupGenderSlideHierarchy => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: "6.3.0",
                python_version: "3.12",
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.beta-binomial-group-gender-slide-counts;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-beta-binomial-group-gender-slide-hierarchy-worker-result+json;version=1",
                result_schema_id:
                    "marklab.pymc_beta_binomial_group_gender_slide_hierarchy_worker_result",
                node_id: "pymc-beta-binomial-group-gender-slide-hierarchy",
                node_kind: "bayesian_repeated_slide_patient_hierarchical_fit",
                implementation_identity:
                    "marklab-project-pymc-beta-binomial-group-gender-slide-hierarchy-node-v1",
                deterministic_controls:
                    "seeded-nuts-beta-binomial-group-gender-slide-hierarchy-request",
            },
            Self::PymcStudentTHierarchy => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: "6.3.0",
                python_version: "3.12",
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.student-t-hierarchy-observations;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-student-t-hierarchy-worker-result+json;version=1",
                result_schema_id: "marklab.pymc_student_t_hierarchy_worker_result",
                node_id: "pymc-student-t-hierarchy",
                node_kind: "bayesian_robust_hierarchical_fit",
                implementation_identity: "marklab-project-pymc-student-t-hierarchy-node-v1",
                deterministic_controls: "seeded-nuts-student-t-hierarchical-request",
            },
            Self::PymcGriddedLgcp => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: "6.3.0",
                python_version: "3.12",
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.point-events;version=1",
                    "application/vnd.marklab.source.gridded-covariates;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-gridded-lgcp-worker-result+json;version=1",
                result_schema_id: "marklab.pymc_gridded_lgcp_worker_result",
                node_id: "pymc-gridded-lgcp",
                node_kind: "bayesian_point_process_field_fit",
                implementation_identity: "marklab-project-pymc-gridded-lgcp-node-v1",
                deterministic_controls: "seeded-nuts-fixed-grid-lgcp-request",
            },
            Self::PotFusedGromovWasserstein => StaticBackendDescriptor {
                backend_id: "pot",
                backend_version: "0.9.7.post1",
                python_version: "3.12",
                license: "MIT",
                input_kinds: &["application/vnd.marklab.source.fgw-input+json;version=1"],
                output_kind:
                    "application/vnd.marklab.pot-fused-gromov-worker-result+json;version=1",
                result_schema_id: "marklab.pot_fused_gromov_wasserstein_worker_result",
                node_id: "pot-fused-gromov-wasserstein",
                node_kind: "descriptive_alignment",
                implementation_identity: "marklab-project-pot-fused-gromov-node-v1",
                deterministic_controls: "fixed-initialization-order-and-solver-controls",
            },
        }
    }
}

impl StaticBackendDescriptor {
    fn validate_request_backend(self, backend: &BackendContract) -> Result<(), BayesCliError> {
        if backend.name != self.backend_id
            || backend.version != self.backend_version
            || backend.python_version != self.python_version
            || !is_sha256(&backend.environment_lock_sha256)
            || !is_sha256(&backend.worker_sha256)
        {
            return Err(BayesCliError::Input(format!(
                "{} durable backend identity is incomplete or differs from its static descriptor",
                self.backend_id
            )));
        }
        Ok(())
    }

    fn configuration_digest(
        self,
        backend: &BackendContract,
        request_bytes: &[u8],
    ) -> ContentDigest {
        let mut frames = vec![
            b"marklab-static-durable-backend-v1".as_slice(),
            self.backend_id.as_bytes(),
            self.backend_version.as_bytes(),
            self.python_version.as_bytes(),
            self.license.as_bytes(),
            backend.environment_lock_sha256.as_bytes(),
            backend.worker_sha256.as_bytes(),
        ];
        frames.extend(self.input_kinds.iter().map(|kind| kind.as_bytes()));
        frames.extend([
            self.result_schema_id.as_bytes(),
            self.deterministic_controls.as_bytes(),
            request_bytes,
        ]);
        ContentDigest::from_framed(frames)
    }

    fn execution_policy(self) -> Vec<u8> {
        format!(
            "marklab-static-process-policy-v1\0backend={}\0version={}\0python={}\0license={}\0controls={}\0environment=cleared\0network=cleared\0bounded-process=true",
            self.backend_id,
            self.backend_version,
            self.python_version,
            self.license,
            self.deterministic_controls
        )
        .into_bytes()
    }

    fn result_schema(self) -> Result<ArtifactSchema, BayesCliError> {
        ArtifactSchema::new(self.result_schema_id, 1)
            .map_err(|error| BayesCliError::Input(error.to_string()))
    }
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct ProjectCli {
    #[command(subcommand)]
    command: ProjectTopLevel,
}

#[derive(Debug, Subcommand)]
enum ProjectTopLevel {
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
}

#[derive(Debug, Subcommand)]
enum ProjectCommand {
    RegionRetrieval {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        training: PathBuf,
        #[arg(long)]
        query: PathBuf,
        #[arg(long)]
        k: u32,
        #[arg(long)]
        leakage_policy: String,
        #[arg(long)]
        maximum_component_candidate_visits: u64,
        #[arg(long)]
        out: PathBuf,
    },
    MarkedPrepost {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        pre: PathBuf,
        #[arg(long)]
        post: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    NormalMean {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long, allow_hyphen_values = true)]
        prior_mean: f64,
        #[arg(long)]
        prior_sd: f64,
        #[arg(long)]
        known_sigma: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    HierarchicalNormal {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long, allow_hyphen_values = true)]
        global_prior_mean: f64,
        #[arg(long)]
        global_prior_sd: f64,
        #[arg(long)]
        between_patient_sd_prior: f64,
        #[arg(long)]
        known_sigma: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    StudentTHierarchy {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long, allow_hyphen_values = true)]
        global_prior_mean: f64,
        #[arg(long)]
        global_prior_sd: f64,
        #[arg(long)]
        between_patient_sd_prior: f64,
        #[arg(long)]
        observation_sd_prior: f64,
        #[arg(long)]
        degrees_of_freedom_excess_rate: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    BetaBinomialHierarchy {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        population_alpha: f64,
        #[arg(long)]
        population_beta: f64,
        #[arg(long)]
        concentration_prior_sd: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    BetaBinomialGroupRegression {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        reference_group: String,
        #[arg(long)]
        comparison_group: String,
        #[arg(long, allow_hyphen_values = true)]
        intercept_prior_mean: f64,
        #[arg(long)]
        intercept_prior_sd: f64,
        #[arg(long)]
        group_effect_prior_sd: f64,
        #[arg(long)]
        concentration_prior_sd: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    DirichletMultinomialGroup {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        reference_group: String,
        #[arg(long)]
        comparison_group: String,
        #[arg(long)]
        logit_prior_sd: f64,
        #[arg(long)]
        group_effect_prior_sd: f64,
        #[arg(long)]
        concentration_prior_sd: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    BetaBinomialGroupGenderRegression {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        reference_group: String,
        #[arg(long)]
        comparison_group: String,
        #[arg(long)]
        reference_gender: String,
        #[arg(long)]
        comparison_gender: String,
        #[arg(long, allow_hyphen_values = true)]
        intercept_prior_mean: f64,
        #[arg(long)]
        intercept_prior_sd: f64,
        #[arg(long)]
        group_effect_prior_sd: f64,
        #[arg(long)]
        gender_effect_prior_sd: f64,
        #[arg(long)]
        concentration_prior_sd: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    BetaBinomialGroupGenderSlideHierarchy {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        reference_group: String,
        #[arg(long)]
        comparison_group: String,
        #[arg(long)]
        reference_gender: String,
        #[arg(long)]
        comparison_gender: String,
        #[arg(long, allow_hyphen_values = true)]
        intercept_prior_mean: f64,
        #[arg(long)]
        intercept_prior_sd: f64,
        #[arg(long)]
        group_effect_prior_sd: f64,
        #[arg(long)]
        gender_effect_prior_sd: f64,
        #[arg(long)]
        patient_log_odds_sd_prior_sd: f64,
        #[arg(long)]
        slide_concentration_prior_sd: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    GriddedLgcp {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        events: PathBuf,
        #[arg(long)]
        grid: PathBuf,
        #[arg(long, allow_hyphen_values = true)]
        xmin_um: f64,
        #[arg(long, allow_hyphen_values = true)]
        ymin_um: f64,
        #[arg(long, allow_hyphen_values = true)]
        xmax_um: f64,
        #[arg(long, allow_hyphen_values = true)]
        ymax_um: f64,
        #[arg(long)]
        grid_x: u32,
        #[arg(long)]
        grid_y: u32,
        #[arg(long, allow_hyphen_values = true)]
        intercept_prior_mean: f64,
        #[arg(long)]
        intercept_prior_sd: f64,
        #[arg(long, allow_hyphen_values = true)]
        coefficient_prior_mean: f64,
        #[arg(long)]
        coefficient_prior_sd: f64,
        #[arg(long)]
        field_amplitude: f64,
        #[arg(long)]
        field_length_scale_um: f64,
        #[arg(long)]
        jitter: f64,
        #[arg(long)]
        chains: u32,
        #[arg(long)]
        tune: u32,
        #[arg(long)]
        draws: u32,
        #[arg(long)]
        target_accept: f64,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
    FusedGromovWasserstein {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        alpha: f64,
        #[arg(long)]
        epsilon: f64,
        #[arg(long)]
        feature_scale: f64,
        #[arg(long)]
        structure_scale: f64,
        #[arg(long)]
        tolerance: f64,
        #[arg(long)]
        maximum_iterations: u32,
        #[arg(long)]
        timeout_seconds: u64,
        #[arg(long)]
        out: PathBuf,
    },
}

pub(super) fn run_cli() -> Result<(), BayesCliError> {
    match ProjectCli::parse_from(std::env::args_os()).command {
        ProjectTopLevel::Project {
            command:
                ProjectCommand::RegionRetrieval {
                    project,
                    training,
                    query,
                    k,
                    leakage_policy,
                    maximum_component_candidate_visits,
                    out,
                },
        } => region_retrieval::run(
            project,
            training,
            query,
            k,
            leakage_policy,
            maximum_component_candidate_visits,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::MarkedPrepost {
                    project,
                    pre,
                    post,
                    out,
                },
        } => run_marked_prepost(project, pre, post, out),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::NormalMean {
                    project,
                    input,
                    prior_mean,
                    prior_sd,
                    known_sigma,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => run_normal_mean(
            project,
            input,
            prior_mean,
            prior_sd,
            known_sigma,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::HierarchicalNormal {
                    project,
                    input,
                    global_prior_mean,
                    global_prior_sd,
                    between_patient_sd_prior,
                    known_sigma,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => run_hierarchical_normal(
            project,
            input,
            global_prior_mean,
            global_prior_sd,
            between_patient_sd_prior,
            known_sigma,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::BetaBinomialHierarchy {
                    project,
                    input,
                    population_alpha,
                    population_beta,
                    concentration_prior_sd,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => run_beta_binomial_hierarchy(
            project,
            input,
            population_alpha,
            population_beta,
            concentration_prior_sd,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::BetaBinomialGroupRegression {
                    project,
                    input,
                    reference_group,
                    comparison_group,
                    intercept_prior_mean,
                    intercept_prior_sd,
                    group_effect_prior_sd,
                    concentration_prior_sd,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => run_beta_binomial_group_regression(
            project,
            input,
            reference_group,
            comparison_group,
            intercept_prior_mean,
            intercept_prior_sd,
            group_effect_prior_sd,
            concentration_prior_sd,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::DirichletMultinomialGroup {
                    project,
                    input,
                    reference_group,
                    comparison_group,
                    logit_prior_sd,
                    group_effect_prior_sd,
                    concentration_prior_sd,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => run_dirichlet_multinomial_group(
            project,
            input,
            reference_group,
            comparison_group,
            logit_prior_sd,
            group_effect_prior_sd,
            concentration_prior_sd,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::BetaBinomialGroupGenderRegression {
                    project,
                    input,
                    reference_group,
                    comparison_group,
                    reference_gender,
                    comparison_gender,
                    intercept_prior_mean,
                    intercept_prior_sd,
                    group_effect_prior_sd,
                    gender_effect_prior_sd,
                    concentration_prior_sd,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => run_beta_binomial_group_gender_regression(
            project,
            input,
            reference_group,
            comparison_group,
            reference_gender,
            comparison_gender,
            intercept_prior_mean,
            intercept_prior_sd,
            group_effect_prior_sd,
            gender_effect_prior_sd,
            concentration_prior_sd,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::BetaBinomialGroupGenderSlideHierarchy {
                    project,
                    input,
                    reference_group,
                    comparison_group,
                    reference_gender,
                    comparison_gender,
                    intercept_prior_mean,
                    intercept_prior_sd,
                    group_effect_prior_sd,
                    gender_effect_prior_sd,
                    patient_log_odds_sd_prior_sd,
                    slide_concentration_prior_sd,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => run_beta_binomial_group_gender_slide_hierarchy(
            project,
            input,
            reference_group,
            comparison_group,
            reference_gender,
            comparison_gender,
            intercept_prior_mean,
            intercept_prior_sd,
            group_effect_prior_sd,
            gender_effect_prior_sd,
            patient_log_odds_sd_prior_sd,
            slide_concentration_prior_sd,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::StudentTHierarchy {
                    project,
                    input,
                    global_prior_mean,
                    global_prior_sd,
                    between_patient_sd_prior,
                    observation_sd_prior,
                    degrees_of_freedom_excess_rate,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => run_student_t_hierarchy(
            project,
            input,
            global_prior_mean,
            global_prior_sd,
            between_patient_sd_prior,
            observation_sd_prior,
            degrees_of_freedom_excess_rate,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::GriddedLgcp {
                    project,
                    events,
                    grid,
                    xmin_um,
                    ymin_um,
                    xmax_um,
                    ymax_um,
                    grid_x,
                    grid_y,
                    intercept_prior_mean,
                    intercept_prior_sd,
                    coefficient_prior_mean,
                    coefficient_prior_sd,
                    field_amplitude,
                    field_length_scale_um,
                    jitter,
                    chains,
                    tune,
                    draws,
                    target_accept,
                    seed,
                    timeout_seconds,
                    out,
                },
        } => run_gridded_lgcp(
            project,
            events,
            grid,
            xmin_um,
            ymin_um,
            xmax_um,
            ymax_um,
            grid_x,
            grid_y,
            intercept_prior_mean,
            intercept_prior_sd,
            coefficient_prior_mean,
            coefficient_prior_sd,
            field_amplitude,
            field_length_scale_um,
            jitter,
            NutsSamplingSpec {
                chains,
                tune_per_chain: tune,
                draws_per_chain: draws,
                target_accept,
                seed,
            },
            timeout_seconds,
            out,
        ),
        ProjectTopLevel::Project {
            command:
                ProjectCommand::FusedGromovWasserstein {
                    project,
                    input,
                    alpha,
                    epsilon,
                    feature_scale,
                    structure_scale,
                    tolerance,
                    maximum_iterations,
                    timeout_seconds,
                    out,
                },
        } => run_fused_gromov_wasserstein(
            project,
            input,
            alpha,
            epsilon,
            feature_scale,
            structure_scale,
            tolerance,
            maximum_iterations,
            timeout_seconds,
            out,
        ),
    }
}

fn run_marked_prepost(
    project_path: PathBuf,
    pre_path: PathBuf,
    post_path: PathBuf,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let pre_path = result_document_path(pre_path);
    let post_path = result_document_path(post_path);
    let pre_source = source_artifact(&pre_path, MARKED_RESULT_KIND)?;
    let post_source = source_artifact(&post_path, MARKED_RESULT_KIND)?;
    let pre = MarkedResultImportNode::new("marked-pre-import", pre_path, pre_source.clone())?;
    let post = MarkedResultImportNode::new("marked-post-import", post_path, post_source.clone())?;

    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(pre_source)
        .and_then(|()| project.register_reference(post_source))
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let import_graph = WorkflowGraph::new([pre.spec().clone(), post.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let import_schema = ArtifactSchema::new("marklab.marked_pattern_result", 1)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let pre_run = execute_algorithm(
        &mut durable,
        &mut project,
        &import_graph,
        &pre,
        &scheduler,
        import_schema.clone(),
        runtime.clone(),
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let post_run = execute_algorithm(
        &mut durable,
        &mut project,
        &import_graph,
        &post,
        &scheduler,
        import_schema,
        runtime.clone(),
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let comparison = MarkedPrePostNode::new(
        NodeId::new("marked-prepost").map_err(|error| BayesCliError::Input(error.to_string()))?,
        pre.spec().id().clone(),
        &pre_run,
        post.spec().id().clone(),
        &post_run,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let graph = WorkflowGraph::new([
        pre.spec().clone(),
        post.spec().clone(),
        comparison.spec().clone(),
    ])
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let comparison_run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &comparison,
        &scheduler,
        ArtifactSchema::new("marklab.marked_prepost_result", 1)
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;

    bayes::publish_json(
        &output_path,
        &ResultDocument::marked_prepost(comparison_run.output),
    )?;
    eprintln!(
        "project marked-prepost cache_status: pre={} post={} comparison={}",
        cache_status_name(pre_run.cache_status),
        cache_status_name(post_run.cache_status),
        cache_status_name(comparison_run.cache_status)
    );
    Ok(())
}

fn cache_status_name(status: CacheStatus) -> &'static str {
    match status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    }
}

fn result_document_path(path: PathBuf) -> PathBuf {
    if path.is_dir() {
        path.join("result.json")
    } else {
        path
    }
}

struct MarkedResultImportNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    output: MarkedPatternResult,
}

impl MarkedResultImportNode {
    fn new(node_id: &str, input_path: PathBuf, input: ArtifactRef) -> Result<Self, BayesCliError> {
        let output = read_marked_result(&input_path)?;
        let observed = source_artifact(&input_path, MARKED_RESULT_KIND)?;
        if observed != input {
            return Err(BayesCliError::Input(format!(
                "marked result changed while its durable import was prepared: {}",
                input_path.display()
            )));
        }
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(node_id).map_err(|error| BayesCliError::Input(error.to_string()))?,
                "marked_result_import",
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            output,
        })
    }
}

impl WorkflowNode for MarkedResultImportNode {
    type Output = MarkedPatternResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed =
            source_artifact(&self.input_path, MARKED_RESULT_KIND).map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(format!(
                "marked result no longer matches its durable identity: {}",
                self.input_path.display()
            ))));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: ContentDigest::from_bytes(
                b"marklab-marked-result-import-configuration-v1",
            ),
            execution_policy: b"bounded-typed-result-import-v1",
            implementation_identity: "marklab-project-marked-result-import-v1",
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        Ok(self.output.clone())
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        ResultDocument::marked(output.clone())
            .to_json_pretty()
            .map(String::into_bytes)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let json = std::str::from_utf8(bytes).map_err(NodeError::decode)?;
        ResultDocument::from_json(json)
            .and_then(ResultDocument::into_marked_pattern)
            .map_err(NodeError::decode)
    }

    fn output_kind(&self) -> &'static str {
        MARKED_RESULT_KIND
    }
}

fn read_marked_result(path: &Path) -> Result<MarkedPatternResult, BayesCliError> {
    let mut file = File::open(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })?;
    let mut bytes = Vec::new();
    copy_bounded(&mut file, &mut bytes, MAXIMUM_INPUT_BYTES).map_err(|source| {
        BayesCliError::Io {
            path: path.to_owned(),
            source,
        }
    })?;
    let json = std::str::from_utf8(&bytes)
        .map_err(|error| BayesCliError::Input(format!("invalid result UTF-8: {error}")))?;
    ResultDocument::from_json(json)
        .and_then(ResultDocument::into_marked_pattern)
        .map_err(|error| BayesCliError::Input(error.to_string()))
}

#[allow(clippy::too_many_arguments)]
fn run_normal_mean(
    project_path: PathBuf,
    input_path: PathBuf,
    prior_mean: f64,
    prior_sd: f64,
    known_sigma: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let backend = StaticBackendWorkflow::PymcNormalMean.descriptor();
    let before = source_artifact(&input_path, backend.input_kinds[0])?;
    let prepared = bayes::prepare_normal_mean(
        input_path.clone(),
        prior_mean,
        prior_sd,
        known_sigma,
        sampling,
        timeout_seconds,
    )?;
    let after = source_artifact(&input_path, backend.input_kinds[0])?;
    if before != after {
        return Err(BayesCliError::Input(
            "normal-mean input changed while the durable request was prepared".into(),
        ));
    }

    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);

    let request_for_output = prepared.request.clone();
    let input_identity = prepared.input_identity.clone();
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before.clone())
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node = NormalMeanProjectNode::new(input_path, before, prepared, backend)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        backend.result_schema()?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    let fit = run.output.into_fit(request_for_output, input_identity);
    bayes::publish_json(&output_path, &fit)?;
    eprintln!("project normal-mean cache_status={cache_status}");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_hierarchical_normal(
    project_path: PathBuf,
    input_path: PathBuf,
    global_prior_mean: f64,
    global_prior_sd: f64,
    between_patient_sd_prior: f64,
    known_sigma: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let backend = StaticBackendWorkflow::PymcHierarchicalNormal.descriptor();
    let before = source_artifact(&input_path, backend.input_kinds[0])?;
    let prepared = bayes::prepare_hierarchical(
        input_path.clone(),
        global_prior_mean,
        global_prior_sd,
        between_patient_sd_prior,
        known_sigma,
        sampling,
        timeout_seconds,
    )?;
    let after = source_artifact(&input_path, backend.input_kinds[0])?;
    if before != after {
        return Err(BayesCliError::Input(
            "hierarchical-normal input changed while the durable request was prepared".into(),
        ));
    }

    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);

    let request_for_output = prepared.request.clone();
    let input_identity = prepared.input_identity.clone();
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before.clone())
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node = HierarchicalNormalProjectNode::new(input_path, before, prepared, backend)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        backend.result_schema()?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    let fit = run.output.into_fit(request_for_output, input_identity);
    bayes::publish_json(&output_path, &fit)?;
    eprintln!("project hierarchical-normal cache_status={cache_status}");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_beta_binomial_hierarchy(
    project_path: PathBuf,
    input_path: PathBuf,
    population_alpha: f64,
    population_beta: f64,
    concentration_prior_sd: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let backend = StaticBackendWorkflow::PymcBetaBinomialHierarchy.descriptor();
    let before = source_artifact(&input_path, backend.input_kinds[0])?;
    let prepared = bayes::prepare_beta_binomial_hierarchy(
        input_path.clone(),
        population_alpha,
        population_beta,
        concentration_prior_sd,
        sampling,
        timeout_seconds,
    )?;
    let after = source_artifact(&input_path, backend.input_kinds[0])?;
    if before != after {
        return Err(BayesCliError::Input(
            "beta-binomial-hierarchy input changed while the durable request was prepared".into(),
        ));
    }
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let request_for_output = prepared.request.clone();
    let input_identity = prepared.input_identity.clone();
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before.clone())
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node = BetaBinomialHierarchyProjectNode::new(input_path, before, prepared, backend)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        backend.result_schema()?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    let fit = run.output.into_result(request_for_output, input_identity);
    bayes::publish_json(&output_path, &fit)?;
    eprintln!("project beta-binomial-hierarchy cache_status={cache_status}");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_beta_binomial_group_regression(
    project_path: PathBuf,
    input_path: PathBuf,
    reference_group: String,
    comparison_group: String,
    intercept_prior_mean: f64,
    intercept_prior_sd: f64,
    group_effect_prior_sd: f64,
    concentration_prior_sd: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let backend = StaticBackendWorkflow::PymcBetaBinomialGroupRegression.descriptor();
    let before = source_artifact(&input_path, backend.input_kinds[0])?;
    let prepared = bayes::prepare_beta_binomial_group_regression(
        input_path.clone(),
        reference_group,
        comparison_group,
        intercept_prior_mean,
        intercept_prior_sd,
        group_effect_prior_sd,
        concentration_prior_sd,
        sampling,
        timeout_seconds,
    )?;
    let after = source_artifact(&input_path, backend.input_kinds[0])?;
    if before != after {
        return Err(BayesCliError::Input(
            "beta-binomial-group-regression input changed while the durable request was prepared"
                .into(),
        ));
    }
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let request_for_output = prepared.request.clone();
    let input_identity = prepared.input_identity.clone();
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before.clone())
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node = BetaBinomialGroupRegressionProjectNode::new(input_path, before, prepared, backend)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        backend.result_schema()?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    let fit = run.output.into_result(request_for_output, input_identity);
    bayes::publish_json(&output_path, &fit)?;
    eprintln!("project beta-binomial-group-regression cache_status={cache_status}");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_dirichlet_multinomial_group(
    project_path: PathBuf,
    input_path: PathBuf,
    reference_group: String,
    comparison_group: String,
    logit_prior_sd: f64,
    group_effect_prior_sd: f64,
    concentration_prior_sd: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let backend = StaticBackendWorkflow::PymcDirichletMultinomialGroup.descriptor();
    let before = source_artifact(&input_path, backend.input_kinds[0])?;
    let prepared = bayes::prepare_dirichlet_multinomial_group(
        input_path.clone(),
        reference_group,
        comparison_group,
        logit_prior_sd,
        group_effect_prior_sd,
        concentration_prior_sd,
        sampling,
        timeout_seconds,
    )?;
    let after = source_artifact(&input_path, backend.input_kinds[0])?;
    if before != after {
        return Err(BayesCliError::Input(
            "dirichlet-multinomial-group input changed while the durable request was prepared"
                .into(),
        ));
    }
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let request_for_output = prepared.request.clone();
    let input_identity = prepared.input_identity.clone();
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before.clone())
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node = DirichletMultinomialGroupProjectNode::new(input_path, before, prepared, backend)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        backend.result_schema()?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    let fit = run.output.into_result(request_for_output, input_identity);
    bayes::publish_json(&output_path, &fit)?;
    eprintln!("project dirichlet-multinomial-group cache_status={cache_status}");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_beta_binomial_group_gender_regression(
    project_path: PathBuf,
    input_path: PathBuf,
    reference_group: String,
    comparison_group: String,
    reference_gender: String,
    comparison_gender: String,
    intercept_prior_mean: f64,
    intercept_prior_sd: f64,
    group_effect_prior_sd: f64,
    gender_effect_prior_sd: f64,
    concentration_prior_sd: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let backend = StaticBackendWorkflow::PymcBetaBinomialGroupGenderRegression.descriptor();
    let before = source_artifact(&input_path, backend.input_kinds[0])?;
    let prepared = bayes::prepare_beta_binomial_group_gender_regression(
        input_path.clone(),
        reference_group,
        comparison_group,
        reference_gender,
        comparison_gender,
        intercept_prior_mean,
        intercept_prior_sd,
        group_effect_prior_sd,
        gender_effect_prior_sd,
        concentration_prior_sd,
        sampling,
        timeout_seconds,
    )?;
    let after = source_artifact(&input_path, backend.input_kinds[0])?;
    if before != after {
        return Err(BayesCliError::Input(
            "beta-binomial-group-gender-regression input changed while the durable request was prepared"
                .into(),
        ));
    }
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let request_for_output = prepared.request.clone();
    let input_identity = prepared.input_identity.clone();
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before.clone())
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node =
        BetaBinomialGroupGenderRegressionProjectNode::new(input_path, before, prepared, backend)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        backend.result_schema()?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    let fit = run.output.into_result(request_for_output, input_identity);
    bayes::publish_json(&output_path, &fit)?;
    eprintln!("project beta-binomial-group-gender-regression cache_status={cache_status}");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_beta_binomial_group_gender_slide_hierarchy(
    project_path: PathBuf,
    input_path: PathBuf,
    reference_group: String,
    comparison_group: String,
    reference_gender: String,
    comparison_gender: String,
    intercept_prior_mean: f64,
    intercept_prior_sd: f64,
    group_effect_prior_sd: f64,
    gender_effect_prior_sd: f64,
    patient_log_odds_sd_prior_sd: f64,
    slide_concentration_prior_sd: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let backend = StaticBackendWorkflow::PymcBetaBinomialGroupGenderSlideHierarchy.descriptor();
    let before = source_artifact(&input_path, backend.input_kinds[0])?;
    let prepared = bayes::prepare_beta_binomial_group_gender_slide_hierarchy(
        input_path.clone(),
        reference_group,
        comparison_group,
        reference_gender,
        comparison_gender,
        intercept_prior_mean,
        intercept_prior_sd,
        group_effect_prior_sd,
        gender_effect_prior_sd,
        patient_log_odds_sd_prior_sd,
        slide_concentration_prior_sd,
        sampling,
        timeout_seconds,
    )?;
    let after = source_artifact(&input_path, backend.input_kinds[0])?;
    if before != after {
        return Err(BayesCliError::Input(
            "beta-binomial-group-gender-slide-hierarchy input changed while the durable request was prepared"
                .into(),
        ));
    }
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let request_for_output = prepared.request.clone();
    let input_identity = prepared.input_identity.clone();
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before.clone())
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node = BetaBinomialGroupGenderSlideHierarchyProjectNode::new(
        input_path, before, prepared, backend,
    )?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        backend.result_schema()?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    let fit = run.output.into_result(request_for_output, input_identity);
    bayes::publish_json(&output_path, &fit)?;
    eprintln!("project beta-binomial-group-gender-slide-hierarchy cache_status={cache_status}");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_student_t_hierarchy(
    project_path: PathBuf,
    input_path: PathBuf,
    global_prior_mean: f64,
    global_prior_sd: f64,
    between_patient_sd_prior: f64,
    observation_sd_prior: f64,
    degrees_of_freedom_excess_rate: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let backend = StaticBackendWorkflow::PymcStudentTHierarchy.descriptor();
    let before = source_artifact(&input_path, backend.input_kinds[0])?;
    let prepared = bayes::prepare_student_t_hierarchy(
        input_path.clone(),
        global_prior_mean,
        global_prior_sd,
        between_patient_sd_prior,
        observation_sd_prior,
        degrees_of_freedom_excess_rate,
        sampling,
        timeout_seconds,
    )?;
    let after = source_artifact(&input_path, backend.input_kinds[0])?;
    if before != after {
        return Err(BayesCliError::Input(
            "student-t-hierarchy input changed while the durable request was prepared".into(),
        ));
    }
    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);
    let request_for_output = prepared.request.clone();
    let input_identity = prepared.input_identity.clone();
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before.clone())
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node = StudentTHierarchyProjectNode::new(input_path, before, prepared, backend)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        backend.result_schema()?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    let fit = run.output.into_result(request_for_output, input_identity);
    bayes::publish_json(&output_path, &fit)?;
    eprintln!("project student-t-hierarchy cache_status={cache_status}");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_gridded_lgcp(
    project_path: PathBuf,
    events_path: PathBuf,
    grid_path: PathBuf,
    xmin_um: f64,
    ymin_um: f64,
    xmax_um: f64,
    ymax_um: f64,
    grid_x: u32,
    grid_y: u32,
    intercept_prior_mean: f64,
    intercept_prior_sd: f64,
    coefficient_prior_mean: f64,
    coefficient_prior_sd: f64,
    field_amplitude: f64,
    field_length_scale_um: f64,
    jitter: f64,
    sampling: NutsSamplingSpec,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let backend = StaticBackendWorkflow::PymcGriddedLgcp.descriptor();
    let before = [
        source_artifact(&events_path, backend.input_kinds[0])?,
        source_artifact(&grid_path, backend.input_kinds[1])?,
    ];
    let prepared = bayes::prepare_gridded_lgcp(
        events_path.clone(),
        grid_path.clone(),
        xmin_um,
        ymin_um,
        xmax_um,
        ymax_um,
        grid_x,
        grid_y,
        intercept_prior_mean,
        intercept_prior_sd,
        coefficient_prior_mean,
        coefficient_prior_sd,
        field_amplitude,
        field_length_scale_um,
        jitter,
        sampling,
        timeout_seconds,
        None,
    )?;
    let after = [
        source_artifact(&events_path, backend.input_kinds[0])?,
        source_artifact(&grid_path, backend.input_kinds[1])?,
    ];
    if before != after {
        return Err(BayesCliError::Input(
            "gridded-LGCP input changed while the durable request was prepared".into(),
        ));
    }

    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);

    let request_for_output = prepared.request.clone();
    let input_identity = prepared.input_identity.clone();
    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    for input in &before {
        project
            .register_reference(input.clone())
            .map_err(|error| BayesCliError::Input(error.to_string()))?;
    }
    let node = GriddedLgcpProjectNode::new(events_path, grid_path, before, prepared, backend)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        backend.result_schema()?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    let fit = run.output.into_fit(request_for_output, input_identity);
    bayes::publish_json(&output_path, &fit)?;
    eprintln!("project gridded-lgcp cache_status={cache_status}");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_fused_gromov_wasserstein(
    project_path: PathBuf,
    input_path: PathBuf,
    alpha: f64,
    epsilon: f64,
    feature_scale: f64,
    structure_scale: f64,
    tolerance: f64,
    maximum_iterations: u32,
    timeout_seconds: u64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let backend = StaticBackendWorkflow::PotFusedGromovWasserstein.descriptor();
    let before = source_artifact(&input_path, backend.input_kinds[0])?;
    let prepared = bayes::fused_gromov::prepare(
        input_path.clone(),
        alpha,
        epsilon,
        feature_scale,
        structure_scale,
        tolerance,
        maximum_iterations,
        timeout_seconds,
    )?;
    let after = source_artifact(&input_path, backend.input_kinds[0])?;
    if before != after {
        return Err(BayesCliError::Input(
            "FGW input changed while the durable request was prepared".into(),
        ));
    }

    let runtime = native_runtime_provenance()?;
    let limits = DurableProjectLimits::new(
        PROJECT_CONTROL_BYTES,
        PROJECT_LEDGER_BYTES,
        PROJECT_LEDGER_RECORDS,
        PROJECT_RECORD_BYTES,
        MAXIMUM_RESULT_BYTES,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let mut durable = DurableProject::open_or_create(&project_path, limits)
        .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    report_recovery(&durable);

    let mut project = MarklabProject::with_inline_artifact_limit(MAXIMUM_RESULT_BYTES)
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    project
        .register_reference(before.clone())
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let node = FusedGromovProjectNode::new(input_path, before, prepared, backend)?;
    let graph = WorkflowGraph::new([node.spec().clone()])
        .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: MAXIMUM_RESULT_BYTES,
    })
    .map_err(|error| BayesCliError::Input(error.to_string()))?;
    let run = execute_algorithm(
        &mut durable,
        &mut project,
        &graph,
        &node,
        &scheduler,
        backend.result_schema()?,
        runtime,
    )
    .map_err(|error| BayesCliError::Backend(error.to_string()))?;
    let cache_status = match run.cache_status {
        CacheStatus::Hit => "hit",
        CacheStatus::Miss => "miss",
    };
    bayes::fused_gromov::publish_result(&output_path, &node.prepared, run.output)?;
    eprintln!("project fused-gromov-wasserstein cache_status={cache_status}");
    Ok(())
}

struct NormalMeanProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    prepared: PreparedNormalMean,
    backend: StaticBackendDescriptor,
    execution_policy: Vec<u8>,
}

impl NormalMeanProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        prepared: PreparedNormalMean,
        backend: StaticBackendDescriptor,
    ) -> Result<Self, BayesCliError> {
        backend.validate_request_backend(&prepared.request.backend)?;
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(backend.node_id)
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                backend.node_kind,
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            prepared,
            backend,
            execution_policy: backend.execution_policy(),
        })
    }
}

impl WorkflowNode for NormalMeanProjectNode {
    type Output = WorkerResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(&self.input_path, self.backend.input_kinds[0])
            .map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "normal-mean input no longer matches its durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self
                .backend
                .configuration_digest(&self.prepared.request.backend, &self.prepared.request_bytes),
            execution_policy: &self.execution_policy,
            implementation_identity: self.backend.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::execute_normal_mean(&self.prepared).map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        serde_json::to_vec(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: WorkerResult = serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        output
            .validate(&self.prepared.request, &self.prepared.request_sha256)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        self.backend.output_kind
    }
}

struct HierarchicalNormalProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    prepared: PreparedGaussianHierarchy,
    backend: StaticBackendDescriptor,
    execution_policy: Vec<u8>,
}

impl HierarchicalNormalProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        prepared: PreparedGaussianHierarchy,
        backend: StaticBackendDescriptor,
    ) -> Result<Self, BayesCliError> {
        backend.validate_request_backend(&prepared.request.backend)?;
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(backend.node_id)
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                backend.node_kind,
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            prepared,
            backend,
            execution_policy: backend.execution_policy(),
        })
    }
}

impl WorkflowNode for HierarchicalNormalProjectNode {
    type Output = HierarchicalWorkerResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(&self.input_path, self.backend.input_kinds[0])
            .map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "hierarchical-normal input no longer matches its durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self
                .backend
                .configuration_digest(&self.prepared.request.backend, &self.prepared.request_bytes),
            execution_policy: &self.execution_policy,
            implementation_identity: self.backend.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::execute_hierarchical(&self.prepared).map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        serde_json::to_vec(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: HierarchicalWorkerResult =
            serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        output
            .validate(&self.prepared.request, &self.prepared.request_sha256)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        self.backend.output_kind
    }
}

struct BetaBinomialHierarchyProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    prepared: PreparedBetaBinomialHierarchy,
    backend: StaticBackendDescriptor,
    execution_policy: Vec<u8>,
}

impl BetaBinomialHierarchyProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        prepared: PreparedBetaBinomialHierarchy,
        backend: StaticBackendDescriptor,
    ) -> Result<Self, BayesCliError> {
        backend.validate_request_backend(&prepared.request.backend)?;
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(backend.node_id)
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                backend.node_kind,
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            prepared,
            backend,
            execution_policy: backend.execution_policy(),
        })
    }
}

impl WorkflowNode for BetaBinomialHierarchyProjectNode {
    type Output = BetaBinomialHierarchyWorkerResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(&self.input_path, self.backend.input_kinds[0])
            .map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "beta-binomial-hierarchy input no longer matches its durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self
                .backend
                .configuration_digest(&self.prepared.request.backend, &self.prepared.request_bytes),
            execution_policy: &self.execution_policy,
            implementation_identity: self.backend.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::execute_beta_binomial_hierarchy(&self.prepared).map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        serde_json::to_vec(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: BetaBinomialHierarchyWorkerResult =
            serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        output
            .validate(&self.prepared.request, &self.prepared.request_sha256)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        self.backend.output_kind
    }
}

struct BetaBinomialGroupRegressionProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    prepared: PreparedBetaBinomialGroupRegression,
    backend: StaticBackendDescriptor,
    execution_policy: Vec<u8>,
}

impl BetaBinomialGroupRegressionProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        prepared: PreparedBetaBinomialGroupRegression,
        backend: StaticBackendDescriptor,
    ) -> Result<Self, BayesCliError> {
        backend.validate_request_backend(&prepared.request.backend)?;
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(backend.node_id)
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                backend.node_kind,
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            prepared,
            backend,
            execution_policy: backend.execution_policy(),
        })
    }
}

impl WorkflowNode for BetaBinomialGroupRegressionProjectNode {
    type Output = BetaBinomialGroupRegressionWorkerResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(&self.input_path, self.backend.input_kinds[0])
            .map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "beta-binomial-group-regression input no longer matches its durable identity"
                    .into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self
                .backend
                .configuration_digest(&self.prepared.request.backend, &self.prepared.request_bytes),
            execution_policy: &self.execution_policy,
            implementation_identity: self.backend.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::execute_beta_binomial_group_regression(&self.prepared).map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        serde_json::to_vec(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: BetaBinomialGroupRegressionWorkerResult =
            serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        output
            .validate(&self.prepared.request, &self.prepared.request_sha256)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        self.backend.output_kind
    }
}

struct DirichletMultinomialGroupProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    prepared: PreparedDirichletMultinomialGroup,
    backend: StaticBackendDescriptor,
    execution_policy: Vec<u8>,
}

impl DirichletMultinomialGroupProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        prepared: PreparedDirichletMultinomialGroup,
        backend: StaticBackendDescriptor,
    ) -> Result<Self, BayesCliError> {
        backend.validate_request_backend(&prepared.request.backend)?;
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(backend.node_id)
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                backend.node_kind,
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            prepared,
            backend,
            execution_policy: backend.execution_policy(),
        })
    }
}

impl WorkflowNode for DirichletMultinomialGroupProjectNode {
    type Output = DirichletMultinomialGroupWorkerResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(&self.input_path, self.backend.input_kinds[0])
            .map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "dirichlet-multinomial-group input no longer matches its durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self
                .backend
                .configuration_digest(&self.prepared.request.backend, &self.prepared.request_bytes),
            execution_policy: &self.execution_policy,
            implementation_identity: self.backend.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::execute_dirichlet_multinomial_group(&self.prepared).map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        serde_json::to_vec(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: DirichletMultinomialGroupWorkerResult =
            serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        output
            .validate(&self.prepared.request, &self.prepared.request_sha256)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        self.backend.output_kind
    }
}

struct BetaBinomialGroupGenderRegressionProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    prepared: PreparedBetaBinomialGroupGenderRegression,
    backend: StaticBackendDescriptor,
    execution_policy: Vec<u8>,
}

impl BetaBinomialGroupGenderRegressionProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        prepared: PreparedBetaBinomialGroupGenderRegression,
        backend: StaticBackendDescriptor,
    ) -> Result<Self, BayesCliError> {
        backend.validate_request_backend(&prepared.request.backend)?;
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(backend.node_id)
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                backend.node_kind,
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            prepared,
            backend,
            execution_policy: backend.execution_policy(),
        })
    }
}

impl WorkflowNode for BetaBinomialGroupGenderRegressionProjectNode {
    type Output = BetaBinomialGroupGenderRegressionWorkerResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(&self.input_path, self.backend.input_kinds[0])
            .map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "beta-binomial-group-gender-regression input no longer matches its durable identity"
                    .into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self
                .backend
                .configuration_digest(&self.prepared.request.backend, &self.prepared.request_bytes),
            execution_policy: &self.execution_policy,
            implementation_identity: self.backend.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::execute_beta_binomial_group_gender_regression(&self.prepared)
            .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        serde_json::to_vec(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: BetaBinomialGroupGenderRegressionWorkerResult =
            serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        output
            .validate(&self.prepared.request, &self.prepared.request_sha256)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        self.backend.output_kind
    }
}

struct BetaBinomialGroupGenderSlideHierarchyProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    prepared: PreparedBetaBinomialGroupGenderSlideHierarchy,
    backend: StaticBackendDescriptor,
    execution_policy: Vec<u8>,
}

impl BetaBinomialGroupGenderSlideHierarchyProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        prepared: PreparedBetaBinomialGroupGenderSlideHierarchy,
        backend: StaticBackendDescriptor,
    ) -> Result<Self, BayesCliError> {
        backend.validate_request_backend(&prepared.request.backend)?;
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(backend.node_id)
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                backend.node_kind,
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            prepared,
            backend,
            execution_policy: backend.execution_policy(),
        })
    }
}

impl WorkflowNode for BetaBinomialGroupGenderSlideHierarchyProjectNode {
    type Output = BetaBinomialGroupGenderSlideHierarchyWorkerResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(&self.input_path, self.backend.input_kinds[0])
            .map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "beta-binomial-group-gender-slide-hierarchy input no longer matches its durable identity"
                    .into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self
                .backend
                .configuration_digest(&self.prepared.request.backend, &self.prepared.request_bytes),
            execution_policy: &self.execution_policy,
            implementation_identity: self.backend.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::execute_beta_binomial_group_gender_slide_hierarchy(&self.prepared)
            .map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        serde_json::to_vec(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: BetaBinomialGroupGenderSlideHierarchyWorkerResult =
            serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        output
            .validate(&self.prepared.request, &self.prepared.request_sha256)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        self.backend.output_kind
    }
}

struct StudentTHierarchyProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    prepared: PreparedStudentTHierarchy,
    backend: StaticBackendDescriptor,
    execution_policy: Vec<u8>,
}

impl StudentTHierarchyProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        prepared: PreparedStudentTHierarchy,
        backend: StaticBackendDescriptor,
    ) -> Result<Self, BayesCliError> {
        backend.validate_request_backend(&prepared.request.backend)?;
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(backend.node_id)
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                backend.node_kind,
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            prepared,
            backend,
            execution_policy: backend.execution_policy(),
        })
    }
}

impl WorkflowNode for StudentTHierarchyProjectNode {
    type Output = StudentTHierarchyWorkerResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(&self.input_path, self.backend.input_kinds[0])
            .map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "student-t-hierarchy input no longer matches its durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self
                .backend
                .configuration_digest(&self.prepared.request.backend, &self.prepared.request_bytes),
            execution_policy: &self.execution_policy,
            implementation_identity: self.backend.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::execute_student_t_hierarchy(&self.prepared).map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        serde_json::to_vec(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: StudentTHierarchyWorkerResult =
            serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        output
            .validate(&self.prepared.request, &self.prepared.request_sha256)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        self.backend.output_kind
    }
}

struct GriddedLgcpProjectNode {
    spec: NodeSpec,
    input_paths: [PathBuf; 2],
    input_artifacts: [ArtifactRef; 2],
    prepared: PreparedGriddedLgcpFit,
    backend: StaticBackendDescriptor,
    execution_policy: Vec<u8>,
}

impl GriddedLgcpProjectNode {
    fn new(
        events_path: PathBuf,
        grid_path: PathBuf,
        inputs: [ArtifactRef; 2],
        prepared: PreparedGriddedLgcpFit,
        backend: StaticBackendDescriptor,
    ) -> Result<Self, BayesCliError> {
        backend.validate_request_backend(&prepared.request.backend)?;
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(backend.node_id)
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                backend.node_kind,
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_paths: [events_path, grid_path],
            input_artifacts: inputs,
            prepared,
            backend,
            execution_policy: backend.execution_policy(),
        })
    }
}

impl WorkflowNode for GriddedLgcpProjectNode {
    type Output = GriddedLgcpFitWorkerResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        for index in 0..self.input_paths.len() {
            let observed =
                source_artifact(&self.input_paths[index], self.backend.input_kinds[index])
                    .map_err(NodeError::input)?;
            if observed != self.input_artifacts[index] {
                return Err(NodeError::input(BayesCliError::Input(
                    "gridded-LGCP input no longer matches its durable identity".into(),
                )));
            }
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self
                .backend
                .configuration_digest(&self.prepared.request.backend, &self.prepared.request_bytes),
            execution_policy: &self.execution_policy,
            implementation_identity: self.backend.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::execute_gridded_lgcp(&self.prepared).map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        serde_json::to_vec(output)
            .map(Vec::into_boxed_slice)
            .map_err(NodeError::encoding)
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: GriddedLgcpFitWorkerResult =
            serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        output
            .validate(&self.prepared.request, &self.prepared.request_sha256)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        self.backend.output_kind
    }
}

struct FusedGromovProjectNode {
    spec: NodeSpec,
    input_path: PathBuf,
    input_artifacts: [ArtifactRef; 1],
    prepared: PreparedFusedGromovWasserstein,
    backend: StaticBackendDescriptor,
    execution_policy: Vec<u8>,
}

impl FusedGromovProjectNode {
    fn new(
        input_path: PathBuf,
        input: ArtifactRef,
        prepared: PreparedFusedGromovWasserstein,
        backend: StaticBackendDescriptor,
    ) -> Result<Self, BayesCliError> {
        backend.validate_request_backend(&prepared.request.backend)?;
        Ok(Self {
            spec: NodeSpec::new(
                NodeId::new(backend.node_id)
                    .map_err(|error| BayesCliError::Input(error.to_string()))?,
                backend.node_kind,
                1,
                Vec::new(),
            )
            .map_err(|error| BayesCliError::Input(error.to_string()))?,
            input_path,
            input_artifacts: [input],
            prepared,
            backend,
            execution_policy: backend.execution_policy(),
        })
    }
}

impl WorkflowNode for FusedGromovProjectNode {
    type Output = FusedGromovWassersteinWorkerResult;

    fn spec(&self) -> &NodeSpec {
        &self.spec
    }

    fn input_artifacts(&self) -> &[ArtifactRef] {
        &self.input_artifacts
    }

    fn verify_input_content(&self) -> Result<(), NodeError> {
        let observed = source_artifact(&self.input_path, self.backend.input_kinds[0])
            .map_err(NodeError::input)?;
        if observed != self.input_artifacts[0] {
            return Err(NodeError::input(BayesCliError::Input(
                "FGW input no longer matches its durable identity".into(),
            )));
        }
        Ok(())
    }

    fn cache_key_material(&self) -> CacheKeyMaterial<'_> {
        CacheKeyMaterial {
            configuration_digest: self
                .backend
                .configuration_digest(&self.prepared.request.backend, &self.prepared.request_bytes),
            execution_policy: &self.execution_policy,
            implementation_identity: self.backend.implementation_identity,
        }
    }

    fn execute(&self) -> Result<Self::Output, NodeError> {
        bayes::fused_gromov::execute(&self.prepared).map_err(NodeError::execution)
    }

    fn encode_output(&self, output: &Self::Output) -> Result<Box<[u8]>, NodeError> {
        let mut encoded = serde_json::to_vec(output).map_err(NodeError::encoding)?;
        for _ in 0..8 {
            let normalized: FusedGromovWassersteinWorkerResult =
                serde_json::from_slice(&encoded).map_err(NodeError::encoding)?;
            let next = serde_json::to_vec(&normalized).map_err(NodeError::encoding)?;
            if next == encoded {
                return Ok(next.into_boxed_slice());
            }
            encoded = next;
        }
        Err(NodeError::encoding(BayesCliError::Input(
            "typed POT result JSON did not reach a stable canonical representation".into(),
        )))
    }

    fn decode_output(&self, bytes: &[u8]) -> Result<Self::Output, NodeError> {
        let output: FusedGromovWassersteinWorkerResult =
            serde_json::from_slice(bytes).map_err(NodeError::decode)?;
        output
            .validate(&self.prepared.request, &self.prepared.request_sha256)
            .map_err(NodeError::decode)?;
        Ok(output)
    }

    fn output_kind(&self) -> &'static str {
        self.backend.output_kind
    }
}

fn source_artifact(path: &Path, kind: &str) -> Result<ArtifactRef, BayesCliError> {
    let mut file = File::open(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })?;
    let mut digest = ContentDigest::builder();
    let copied = copy_bounded(&mut file, &mut digest, MAXIMUM_INPUT_BYTES).map_err(|source| {
        BayesCliError::Io {
            path: path.to_owned(),
            source,
        }
    })?;
    let (digest, byte_len) = digest.finish();
    if copied != byte_len {
        return Err(BayesCliError::Input(
            "source hashing byte count mismatch".into(),
        ));
    }
    ArtifactRef::new(kind, digest, byte_len)
        .map_err(|error| BayesCliError::Input(error.to_string()))
}

fn native_runtime_provenance() -> Result<NativeRuntimeProvenance, BayesCliError> {
    let executable = executable_artifact()?;
    let git_sha = match env!("MARKLAB_BUILD_GIT_SHA") {
        "" => None,
        value => Some(value.to_owned()),
    };
    let git_dirty = match env!("MARKLAB_BUILD_GIT_DIRTY") {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    };
    NativeRuntimeProvenance::new(
        env!("CARGO_PKG_VERSION"),
        git_sha,
        git_dirty,
        env!("MARKLAB_BUILD_RUSTC"),
        compiled_features(),
        executable,
    )
    .map_err(|error| BayesCliError::Input(error.to_string()))
}

fn executable_artifact() -> Result<ArtifactRef, BayesCliError> {
    let path = std::env::current_exe().map_err(|source| BayesCliError::Io {
        path: PathBuf::from("current executable"),
        source,
    })?;
    let mut file = File::open(&path).map_err(|source| BayesCliError::Io {
        path: path.clone(),
        source,
    })?;
    let mut digest = ContentDigest::builder();
    let copied =
        copy_bounded(&mut file, &mut digest, MAXIMUM_EXECUTABLE_BYTES).map_err(|source| {
            BayesCliError::Io {
                path: path.clone(),
                source,
            }
        })?;
    let (digest, byte_len) = digest.finish();
    if copied != byte_len {
        return Err(BayesCliError::Input(
            "executable hashing byte count mismatch".into(),
        ));
    }
    ArtifactRef::new("application/vnd.marklab.executable", digest, byte_len)
        .map_err(|error| BayesCliError::Input(error.to_string()))
}

fn copy_bounded(
    reader: &mut dyn Read,
    writer: &mut dyn Write,
    maximum: u64,
) -> std::io::Result<u64> {
    let mut limited = reader.take(maximum.saturating_add(1));
    let copied = std::io::copy(&mut limited, writer)?;
    if copied > maximum {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "input exceeds its provenance hashing limit",
        ));
    }
    Ok(copied)
}

fn compiled_features() -> Vec<String> {
    let candidates = [
        (cfg!(feature = "allocator-mimalloc"), "allocator-mimalloc"),
        (cfg!(feature = "cli"), "cli"),
        (cfg!(feature = "csv"), "csv"),
        (cfg!(feature = "dhat-heap"), "dhat-heap"),
        (cfg!(feature = "parallel"), "parallel"),
        (cfg!(feature = "parquet"), "parquet"),
        (cfg!(feature = "wsi"), "wsi"),
    ];
    candidates
        .into_iter()
        .filter_map(|(enabled, name)| enabled.then_some(name.to_owned()))
        .collect()
}

fn report_recovery(project: &DurableProject) {
    let report = project.open_report();
    if report.action() != DurableRecoveryAction::None
        || !report.artifact_store().quarantined().is_empty()
        || !report.artifact_store().issues().is_empty()
    {
        eprintln!(
            "project recovery: action={:?}, quarantined_objects={}, retained_issues={}",
            report.action(),
            report.artifact_store().quarantined().len(),
            report.artifact_store().issues().len()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn backend(
        name: &'static str,
        version: &'static str,
        lock_byte: char,
        worker_byte: char,
    ) -> BackendContract {
        BackendContract {
            name,
            version,
            python_version: "3.12",
            environment_lock_sha256: lock_byte.to_string().repeat(64),
            worker_sha256: worker_byte.to_string().repeat(64),
        }
    }

    #[test]
    fn static_backend_identity_binds_environment_worker_and_request_bytes() {
        let descriptor = StaticBackendWorkflow::PymcNormalMean.descriptor();
        let exact = backend("pymc", "6.3.0", 'a', 'b');
        descriptor
            .validate_request_backend(&exact)
            .expect("exact descriptor");
        let baseline = descriptor.configuration_digest(&exact, b"request-a");

        let changed_lock = backend("pymc", "6.3.0", 'c', 'b');
        let changed_worker = backend("pymc", "6.3.0", 'a', 'd');
        assert_ne!(
            baseline,
            descriptor.configuration_digest(&changed_lock, b"request-a")
        );
        assert_ne!(
            baseline,
            descriptor.configuration_digest(&changed_worker, b"request-a")
        );
        assert_ne!(
            baseline,
            descriptor.configuration_digest(&exact, b"request-b")
        );

        let wrong_backend = backend("pot", "0.9.7.post1", 'a', 'b');
        assert!(descriptor.validate_request_backend(&wrong_backend).is_err());
    }
}
