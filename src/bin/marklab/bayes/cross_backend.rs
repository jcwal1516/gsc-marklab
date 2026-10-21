//! Shared execution for the typed cross-backend model commands.

use std::path::Path;

use marklab_bayes::*;
use serde::{de::DeserializeOwned, Serialize};

use super::{publish_json, run_worker, worker_identity, worker_process::WorkerIdentity, CliError};

/// A concrete worker request paired with its model-specific result validator.
pub(super) trait ValidatedWorkerRequest: Serialize + Sized {
    const WORKER: &'static str;
    type Response: DeserializeOwned;
    type Value;

    fn identity(repository: &Path) -> Result<WorkerIdentity, CliError> {
        worker_identity(repository, Self::WORKER)
    }

    fn validate_result(
        self,
        response: Self::Response,
        request_sha256: &str,
    ) -> Result<Self::Value, CliError>;
}

/// The prepared model retains its existing input and PyMC result validation.
pub(super) trait CrossBackendModel: Sized {
    type Request;
    type InputIdentity: Clone;
    type Fit;
    type NumpyroRequest: ValidatedWorkerRequest;

    fn input_identity(&self) -> &Self::InputIdentity;
    fn same_input(left: &Self::InputIdentity, right: &Self::InputIdentity) -> bool;
    fn timeout_seconds(&self) -> u64;
    fn execute(&self) -> Result<Self::Fit, CliError>;
    fn numpyro_request(&self, identity: WorkerIdentity) -> Result<Self::NumpyroRequest, CliError>;
    fn into_parts(self) -> (Self::Request, Self::InputIdentity);
}

fn execute_worker<R: ValidatedWorkerRequest>(
    repository: &Path,
    request: R,
    timeout_seconds: u64,
) -> Result<R::Value, CliError> {
    let request_bytes = serde_json::to_vec(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let result_bytes = run_worker(repository, R::WORKER, &request_bytes, timeout_seconds)?;
    let response = serde_json::from_slice(&result_bytes)?;
    request.validate_result(response, &request_sha256)
}

pub(super) fn agreement<M: CrossBackendModel, Output: Serialize>(
    prepared: M,
    output_path: &Path,
    finish: impl FnOnce(
        M::Request,
        M::InputIdentity,
        M::Fit,
        <M::NumpyroRequest as ValidatedWorkerRequest>::Value,
    ) -> Result<Output, CliError>,
) -> Result<(), CliError> {
    let pymc = prepared.execute()?;
    let repository = marklab::backend::python_backend_assets_root()?;
    let identity = M::NumpyroRequest::identity(&repository)?;
    let request = prepared.numpyro_request(identity)?;
    let numpyro = execute_worker(&repository, request, prepared.timeout_seconds())?;
    let (request, input_identity) = prepared.into_parts();
    let result = finish(request, input_identity, pymc, numpyro)?;
    publish_json(output_path, &result)
}

pub(super) fn sbc<R: ValidatedWorkerRequest>(
    timeout_seconds: u64,
    output_path: &Path,
    request: impl FnOnce(WorkerIdentity) -> Result<R, CliError>,
) -> Result<(), CliError>
where
    R::Value: Serialize,
{
    let repository = marklab::backend::python_backend_assets_root()?;
    let request = request(R::identity(&repository)?)?;
    let result = execute_worker(&repository, request, timeout_seconds)?;
    publish_json(output_path, &result)
}

pub(super) fn validate_sensitivity_controls(
    lower: f64,
    upper: f64,
    shift: f64,
    message: &str,
) -> Result<(), CliError> {
    if !lower.is_finite()
        || !(0.0..1.0).contains(&lower)
        || !upper.is_finite()
        || !(1.0..=10.0).contains(&upper)
        || !shift.is_finite()
        || shift <= 0.0
    {
        return Err(CliError::Input(message.into()));
    }
    Ok(())
}

pub(super) fn ensure_same_input<M: CrossBackendModel>(
    left: &M::InputIdentity,
    right: &M::InputIdentity,
    message: &str,
) -> Result<(), CliError> {
    if !M::same_input(left, right) {
        return Err(CliError::Input(message.into()));
    }
    Ok(())
}

pub(super) fn sensitivity<M: CrossBackendModel, Scenario: Copy, Run, const N: usize>(
    scenarios: [Scenario; N],
    input_changed_message: &str,
    mut prepare: impl FnMut(Scenario) -> Result<M, CliError>,
    mut finish: impl FnMut(Scenario, &M, M::Fit) -> Run,
) -> Result<(M::InputIdentity, Vec<Run>), CliError> {
    let mut input_identity = None;
    let mut runs = Vec::with_capacity(N);
    for scenario in scenarios {
        let prepared = prepare(scenario)?;
        if let Some(expected) = &input_identity {
            ensure_same_input::<M>(expected, prepared.input_identity(), input_changed_message)?;
        } else {
            input_identity = Some(prepared.input_identity().clone());
        }
        let result = prepared.execute()?;
        runs.push(finish(scenario, &prepared, result));
    }
    Ok((input_identity.expect("baseline scenario exists"), runs))
}

macro_rules! model {
    ($module:ident, $prepared:ident, $request:ty, $identity:ty, $fit:ty, $numpyro:ty, $($input_field:ident),+) => {
        impl CrossBackendModel for super::$module::$prepared {
            type Request = $request;
            type InputIdentity = $identity;
            type Fit = $fit;
            type NumpyroRequest = $numpyro;
            fn input_identity(&self) -> &Self::InputIdentity { &self.input_identity }
            fn same_input(left: &Self::InputIdentity, right: &Self::InputIdentity) -> bool { $(left.$input_field == right.$input_field)&&+ }
            fn timeout_seconds(&self) -> u64 { self.timeout_seconds }
            fn execute(&self) -> Result<Self::Fit, CliError> { super::$module::execute(self) }
            fn numpyro_request(&self, identity: WorkerIdentity) -> Result<Self::NumpyroRequest, CliError> {
                Ok(<$numpyro>::new(self.request.clone(), identity.environment_lock_sha256, identity.worker_sha256)?)
            }
            fn into_parts(self) -> (Self::Request, Self::InputIdentity) { (self.request, self.input_identity) }
        }
    };
}

model!(
    hierarchical,
    PreparedGaussianHierarchy,
    GaussianHierarchyWorkerRequest,
    GaussianHierarchyInputIdentity,
    HierarchicalWorkerResult,
    NumpyroHierarchyWorkerRequest,
    patient_data_sha256
);
model!(
    gridded_lgcp_fit,
    PreparedGriddedLgcpFit,
    GriddedLgcpFitWorkerRequest,
    GriddedLgcpFitInputIdentity,
    GriddedLgcpFitWorkerResult,
    NumpyroGriddedLgcpWorkerRequest,
    events_sha256,
    grid_sha256
);
model!(
    beta_binomial_hierarchy,
    PreparedBetaBinomialHierarchy,
    BetaBinomialHierarchyWorkerRequest,
    BetaBinomialHierarchyInputIdentity,
    BetaBinomialHierarchyWorkerResult,
    NumpyroBetaBinomialHierarchyWorkerRequest,
    patient_data_sha256
);
model!(
    beta_binomial_group_regression,
    PreparedBetaBinomialGroupRegression,
    BetaBinomialGroupRegressionWorkerRequest,
    BetaBinomialGroupRegressionInputIdentity,
    BetaBinomialGroupRegressionWorkerResult,
    NumpyroBetaBinomialGroupRegressionWorkerRequest,
    patient_data_sha256
);
model!(
    beta_binomial_group_gender_regression,
    PreparedBetaBinomialGroupGenderRegression,
    BetaBinomialGroupGenderRegressionWorkerRequest,
    BetaBinomialGroupGenderRegressionInputIdentity,
    BetaBinomialGroupGenderRegressionWorkerResult,
    NumpyroBetaBinomialGroupGenderWorkerRequest,
    patient_data_sha256
);
model!(
    beta_binomial_group_gender_slide_hierarchy,
    PreparedBetaBinomialGroupGenderSlideHierarchy,
    BetaBinomialGroupGenderSlideHierarchyWorkerRequest,
    BetaBinomialGroupGenderSlideHierarchyInputIdentity,
    BetaBinomialGroupGenderSlideHierarchyWorkerResult,
    NumpyroBetaBinomialGroupGenderSlideHierarchyWorkerRequest,
    slide_data_sha256
);
model!(
    dirichlet_multinomial_group,
    PreparedDirichletMultinomialGroup,
    DirichletMultinomialGroupWorkerRequest,
    DirichletMultinomialGroupInputIdentity,
    DirichletMultinomialGroupWorkerResult,
    NumpyroDirichletMultinomialGroupWorkerRequest,
    patient_data_sha256
);
model!(
    student_t_hierarchy,
    PreparedStudentTHierarchy,
    StudentTHierarchyWorkerRequest,
    GaussianHierarchyInputIdentity,
    StudentTHierarchyWorkerResult,
    NumpyroStudentTHierarchyWorkerRequest,
    patient_data_sha256
);

macro_rules! agreement_worker {
    ($request:ty, $response:ty, $value:ty, $worker:literal) => {
        impl ValidatedWorkerRequest for $request {
            const WORKER: &'static str = $worker;
            type Response = $response;
            type Value = $value;
            fn validate_result(
                self,
                response: Self::Response,
                request_sha256: &str,
            ) -> Result<Self::Value, CliError> {
                Ok(response.into_validated_payload(&self, request_sha256)?)
            }
        }
    };
}

impl ValidatedWorkerRequest for NumpyroHierarchyWorkerRequest {
    const WORKER: &'static str = "marklab_numpyro_hierarchical_worker.py";
    type Response = NumpyroHierarchyWorkerResult;
    type Value = NumpyroHierarchyWorkerResult;
    fn validate_result(
        self,
        response: Self::Response,
        request_sha256: &str,
    ) -> Result<Self::Value, CliError> {
        response.validate(&self, request_sha256)?;
        Ok(response)
    }
}

agreement_worker!(
    NumpyroGriddedLgcpWorkerRequest,
    NumpyroGriddedLgcpWorkerResult,
    GriddedLgcpFitWorkerResult,
    "marklab_numpyro_gridded_lgcp_worker.py"
);
agreement_worker!(
    NumpyroBetaBinomialHierarchyWorkerRequest,
    NumpyroBetaBinomialHierarchyWorkerResult,
    BetaBinomialHierarchyWorkerResult,
    "marklab_numpyro_beta_binomial_hierarchy_worker.py"
);
agreement_worker!(
    NumpyroBetaBinomialGroupRegressionWorkerRequest,
    NumpyroBetaBinomialGroupRegressionWorkerResult,
    BetaBinomialGroupRegressionWorkerResult,
    "marklab_numpyro_beta_binomial_group_regression_worker.py"
);
agreement_worker!(
    NumpyroBetaBinomialGroupGenderWorkerRequest,
    NumpyroBetaBinomialGroupGenderWorkerResult,
    BetaBinomialGroupGenderRegressionWorkerResult,
    "marklab_numpyro_beta_binomial_group_gender_worker.py"
);
agreement_worker!(
    NumpyroBetaBinomialGroupGenderSlideHierarchyWorkerRequest,
    NumpyroBetaBinomialGroupGenderSlideHierarchyWorkerResult,
    BetaBinomialGroupGenderSlideHierarchyWorkerResult,
    "marklab_numpyro_beta_binomial_group_gender_slide_hierarchy_worker.py"
);
agreement_worker!(
    NumpyroDirichletMultinomialGroupWorkerRequest,
    NumpyroDirichletMultinomialGroupWorkerResult,
    DirichletMultinomialGroupWorkerResult,
    "marklab_numpyro_dirichlet_multinomial_group_worker.py"
);
agreement_worker!(
    NumpyroStudentTHierarchyWorkerRequest,
    NumpyroStudentTHierarchyWorkerResult,
    StudentTHierarchyWorkerResult,
    "marklab_numpyro_student_t_hierarchy_worker.py"
);

macro_rules! sbc_worker {
    ($request:ty, $response:ty, $value:ty, $worker:literal $(, $identity:path)?) => {
        impl ValidatedWorkerRequest for $request {
            const WORKER: &'static str = $worker;
            type Response = $response;
            type Value = $value;
            $(fn identity(repository: &Path) -> Result<WorkerIdentity, CliError> { $identity(repository) })?
            fn validate_result(self, response: Self::Response, request_sha256: &str) -> Result<Self::Value, CliError> {
                response.validate(&self, request_sha256)?;
                Ok(response.into_result(self))
            }
        }
    };
}

sbc_worker!(
    NumpyroHierarchySbcWorkerRequest,
    NumpyroHierarchySbcWorkerResult,
    HierarchicalSbcResult,
    "marklab_numpyro_hierarchical_sbc_worker.py"
);
sbc_worker!(
    NumpyroGriddedLgcpSbcWorkerRequest,
    NumpyroGriddedLgcpSbcWorkerResult,
    GriddedLgcpSbcResult,
    "marklab_numpyro_gridded_lgcp_sbc_worker.py"
);
sbc_worker!(
    NumpyroBetaBinomialSbcWorkerRequest,
    NumpyroBetaBinomialSbcWorkerResult,
    BetaBinomialSbcResult,
    "marklab_numpyro_beta_binomial_hierarchy_sbc_worker.py"
);
sbc_worker!(
    NumpyroBetaBinomialGroupSbcWorkerRequest,
    NumpyroBetaBinomialGroupSbcWorkerResult,
    BetaBinomialGroupSbcResult,
    "marklab_numpyro_beta_binomial_group_regression_sbc_worker.py"
);
sbc_worker!(
    NumpyroBetaBinomialGroupGenderSbcWorkerRequest,
    NumpyroBetaBinomialGroupGenderSbcWorkerResult,
    BetaBinomialGroupGenderSbcResult,
    "marklab_numpyro_beta_binomial_group_gender_sbc_worker.py"
);
sbc_worker!(
    NumpyroBetaBinomialGroupGenderSlideSbcWorkerRequest,
    NumpyroBetaBinomialGroupGenderSlideSbcWorkerResult,
    BetaBinomialGroupGenderSlideSbcResult,
    "marklab_numpyro_beta_binomial_group_gender_slide_hierarchy_sbc_worker.py",
    super::beta_binomial_group_gender_slide_sbc::identity
);
sbc_worker!(
    NumpyroDirichletMultinomialGroupSbcWorkerRequest,
    NumpyroDirichletMultinomialGroupSbcWorkerResult,
    DirichletMultinomialGroupSbcResult,
    "marklab_numpyro_dirichlet_multinomial_group_sbc_worker.py"
);
sbc_worker!(
    NumpyroStudentTSbcWorkerRequest,
    NumpyroStudentTSbcWorkerResult,
    StudentTSbcResult,
    "marklab_numpyro_student_t_hierarchy_sbc_worker.py"
);
