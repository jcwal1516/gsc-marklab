use marklab_core::backend::is_worker_timeout;
use std::path::PathBuf;

use clap::Subcommand;
use marklab_spatial3d::protocol::{
    validate_alpha_complex, validate_clone_models, validate_deformation_biology,
    validate_serial_stack, AdvancedValidationRequest, AdvancedValidationResult, AlphaBackend,
    AlphaComplex3dSpec, AlphaComplexRequest, AlphaComplexResult, CloneModelsRequest,
    CloneModelsResult, CloneModelsSpec, DeformationBiologyRequest, DeformationBiologyResult,
    DeformationBiologySpec, ScipyBackend, SerialStackRequest, SerialStackResult, SerialStackSpec,
    ValidationBackend,
};

use marklab_core::backend::sha256_hex;

use super::topology::{publish_json, read_input, read_required, run_worker};
use crate::cli_error::{into_marklab_error, CliError};
use marklab_core::backend::versions::GUDHI_BACKEND;
use marklab_core::backend::versions::GUDHI_VERSION;
use marklab_core::backend::versions::NUMPY_VERSION;
use marklab_core::backend::versions::PYTHON_VERSION;
use marklab_core::backend::versions::SCIPY_VERSION;

#[derive(Debug, Subcommand)]
pub(crate) enum Spatial3dModelCommand {
    /// Reconstruct a serial-section stack with landmark registration uncertainty.
    SerialStack {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Compute an exact 3-D alpha complex and its persistence diagrams.
    AlphaComplex {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Separate registration deformation from declared biological change.
    DeformationBiology {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Fit uncertain clone-location diffusion and clone-niche contrasts.
    CloneModels {
        /// JSON specification containing observations and settings for this analysis.
        #[arg(long)]
        input: PathBuf,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
    /// Run synthetic 3-D, serial-section and clone-model checks.
    ValidateAdvanced {
        /// Random seed for reproducible simulation or inference.
        #[arg(long)]
        seed: u64,
        /// Maximum worker execution time in seconds.
        #[arg(long)]
        timeout_seconds: u64,
        /// Destination for the analysis output.
        #[arg(long)]
        out: PathBuf,
    },
}

pub(crate) fn run_cli(command: Spatial3dModelCommand) -> Result<(), CliError> {
    match command {
        Spatial3dModelCommand::SerialStack { input, out } => run_serial_stack(input, out),
        Spatial3dModelCommand::AlphaComplex { input, out } => run_alpha_complex(input, out),
        Spatial3dModelCommand::DeformationBiology { input, out } => {
            run_deformation_biology(input, out)
        }
        Spatial3dModelCommand::CloneModels { input, out } => run_clone_models(input, out),
        Spatial3dModelCommand::ValidateAdvanced {
            seed,
            timeout_seconds,
            out,
        } => run_validate_advanced(seed, timeout_seconds, out),
    }
}

fn run_validate_advanced(seed: u64, timeout_seconds: u64, out: PathBuf) -> Result<(), CliError> {
    if !is_worker_timeout(timeout_seconds) {
        return Err(CliError::Input(
            "advanced 3-D validation timeout must be between 1 and 3600 seconds".into(),
        ));
    }
    let repository = marklab::backend::python_backend_assets_root()?;
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python/marklab_advanced3d_validation_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let request = AdvancedValidationRequest {
        format: "marklab.advanced3d_validation_request",
        version: 1,
        backend: ValidationBackend {
            name: "gudhi_numpy_scipy_advanced3d_validation".into(),
            gudhi_version: GUDHI_VERSION.into(),
            scipy_version: SCIPY_VERSION.into(),
            numpy_version: NUMPY_VERSION.into(),
            python_version: PYTHON_VERSION.into(),
            license: "GPLv3_effective_via_CGAL_plus_BSD-3-Clause".into(),
            environment_lock_sha256: sha256_hex(&lock),
            worker_sha256: sha256_hex(&worker),
        },
        seed,
    };
    let request_bytes = serde_json::to_vec(&serde_json::to_value(&request)?)?;
    let response = run_worker(&repository, &worker_path, &request_bytes, timeout_seconds)?;
    let result: AdvancedValidationResult = serde_json::from_slice(&response)
        .map_err(|error| schema_error("advanced 3-D validation result", error))?;
    result
        .validate_identity(&request.backend, &sha256_hex(&request_bytes))
        .map_err(|message| CliError::Backend(message.into()))?;
    publish_json(&out, &serde_json::to_value(&result)?)
}

fn run_clone_models(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(&input)?;
    let mut spec: CloneModelsSpec =
        serde_json::from_slice(&bytes).map_err(|error| schema_error("clone model input", error))?;
    spec.cells
        .sort_by(|left, right| left.cell_id.cmp(&right.cell_id));
    spec.clone_locations
        .sort_by(|left, right| left.clone_id.cmp(&right.clone_id));
    validate_clone_models(&spec).map_err(CliError::Input)?;
    let repository = marklab::backend::python_backend_assets_root()?;
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python/marklab_scipy_clone_models_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let request = CloneModelsRequest {
        format: "marklab.scipy_clone_models_request",
        version: 1,
        backend: ScipyBackend {
            name: "scipy_uncertain_clone_models".into(),
            scipy_version: SCIPY_VERSION.into(),
            numpy_version: NUMPY_VERSION.into(),
            python_version: PYTHON_VERSION.into(),
            license: "BSD-3-Clause".into(),
            environment_lock_sha256: sha256_hex(&lock),
            worker_sha256: sha256_hex(&worker),
        },
        tree: &spec.tree,
        clone_labels: &spec.clone_labels,
        clone_locations: &spec.clone_locations,
        cells: &spec.cells,
        diffusion_prior: &spec.diffusion_prior,
        posterior_draws: spec.posterior_draws,
        seed: spec.seed,
    };
    let request_bytes = serde_json::to_vec(&serde_json::to_value(&request)?)?;
    let response = run_worker(
        &repository,
        &worker_path,
        &request_bytes,
        spec.timeout_seconds,
    )?;
    let result: CloneModelsResult = serde_json::from_slice(&response)
        .map_err(|error| schema_error("clone model result", error))?;
    result
        .validate_identity(&request.backend, &sha256_hex(&request_bytes))
        .map_err(|message| CliError::Backend(message.into()))?;
    publish_json(&out, &serde_json::to_value(&result)?)
}

fn run_deformation_biology(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(&input)?;
    let mut spec: DeformationBiologySpec = serde_json::from_slice(&bytes)
        .map_err(|error| schema_error("deformation-biology input", error))?;
    spec.points
        .sort_by(|left, right| left.point_id.cmp(&right.point_id));
    validate_deformation_biology(&spec).map_err(CliError::Input)?;
    let repository = marklab::backend::python_backend_assets_root()?;
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python/marklab_scipy_deformation_biology_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let request = DeformationBiologyRequest {
        format: "marklab.scipy_deformation_biology_request",
        version: 1,
        backend: ScipyBackend {
            name: "scipy_rbf_deformation_biology".into(),
            scipy_version: SCIPY_VERSION.into(),
            numpy_version: NUMPY_VERSION.into(),
            python_version: PYTHON_VERSION.into(),
            license: "BSD-3-Clause".into(),
            environment_lock_sha256: sha256_hex(&lock),
            worker_sha256: sha256_hex(&worker),
        },
        coordinate_frame: &spec.coordinate_frame,
        points: &spec.points,
        deformation_translation_draws: &spec.deformation_translation_draws,
        change_model: &spec.change_model,
        interpolation: &spec.interpolation,
    };
    let request_bytes = serde_json::to_vec(&serde_json::to_value(&request)?)?;
    let response = run_worker(
        &repository,
        &worker_path,
        &request_bytes,
        spec.timeout_seconds,
    )?;
    let result: DeformationBiologyResult = serde_json::from_slice(&response)
        .map_err(|error| schema_error("deformation-biology result", error))?;
    result
        .validate_identity(&request.backend, &sha256_hex(&request_bytes))
        .map_err(|message| CliError::Backend(message.into()))?;
    publish_json(&out, &serde_json::to_value(&result)?)
}

fn run_alpha_complex(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(&input)?;
    let mut spec: AlphaComplex3dSpec =
        serde_json::from_slice(&bytes).map_err(|error| schema_error("3-D alpha input", error))?;
    spec.points
        .sort_by(|left, right| left.point_id.cmp(&right.point_id));
    validate_alpha_complex(&spec).map_err(CliError::Input)?;
    let repository = marklab::backend::python_backend_assets_root()?;
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python/marklab_gudhi_alpha3d_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let request = AlphaComplexRequest {
        format: "marklab.gudhi_alpha3d_request",
        version: 1,
        backend: AlphaBackend {
            name: "gudhi_exact_alpha_complex_3d".into(),
            version: GUDHI_BACKEND.into(),
            numpy_version: NUMPY_VERSION.into(),
            python_version: PYTHON_VERSION.into(),
            license: "GPLv3_effective_via_CGAL".into(),
            environment_lock_sha256: sha256_hex(&lock),
            worker_sha256: sha256_hex(&worker),
        },
        coordinate_frame: &spec.coordinate_frame,
        points: &spec.points,
        maximum_squared_alpha_um2: spec.maximum_squared_alpha_um2,
        maximum_homology_dimension: spec.maximum_homology_dimension,
    };
    let request_bytes = serde_json::to_vec(&serde_json::to_value(&request)?)?;
    let response = run_worker(
        &repository,
        &worker_path,
        &request_bytes,
        spec.timeout_seconds,
    )?;
    let result: AlphaComplexResult = serde_json::from_slice(&response)
        .map_err(|error| schema_error("3-D alpha result", error))?;
    result
        .validate_identity(&request.backend, &sha256_hex(&request_bytes))
        .map_err(|message| CliError::Backend(message.into()))?;
    publish_json(&out, &serde_json::to_value(&result)?)
}

fn run_serial_stack(input: PathBuf, out: PathBuf) -> Result<(), CliError> {
    let bytes = read_input(&input)?;
    let mut spec: SerialStackSpec = serde_json::from_slice(&bytes)
        .map_err(|error| schema_error("serial stack input", error))?;
    spec.sections
        .sort_by(|left, right| left.z_um.total_cmp(&right.z_um));
    validate_serial_stack(&spec).map_err(CliError::Input)?;
    let repository = marklab::backend::python_backend_assets_root()?;
    let lock_path = repository.join("workers/python/uv.lock");
    let worker_path = repository.join("workers/python/marklab_scipy_serial_stack_worker.py");
    let lock = read_required(&lock_path)?;
    let worker = read_required(&worker_path)?;
    let request = SerialStackRequest {
        format: "marklab.scipy_serial_stack_request",
        version: 1,
        backend: ScipyBackend {
            name: "scipy_gaussian_landmark_stack".into(),
            scipy_version: SCIPY_VERSION.into(),
            numpy_version: NUMPY_VERSION.into(),
            python_version: PYTHON_VERSION.into(),
            license: "BSD-3-Clause".into(),
            environment_lock_sha256: sha256_hex(&lock),
            worker_sha256: sha256_hex(&worker),
        },
        stack_id: &spec.stack_id,
        coordinate_unit: &spec.coordinate_unit,
        sections: &spec.sections,
        reference_section_id: &spec.reference_section_id,
        landmark_noise_standard_deviation_um: spec.landmark_noise_standard_deviation_um,
        posterior_draws: spec.posterior_draws,
        seed: spec.seed,
    };
    let request_bytes = serde_json::to_vec(&serde_json::to_value(&request)?)?;
    let response = run_worker(
        &repository,
        &worker_path,
        &request_bytes,
        spec.timeout_seconds,
    )?;
    let result: SerialStackResult = serde_json::from_slice(&response)
        .map_err(|error| schema_error("serial stack result", error))?;
    result
        .validate_identity(&request.backend, &sha256_hex(&request_bytes))
        .map_err(|message| CliError::Backend(message.into()))?;
    publish_json(&out, &serde_json::to_value(&result)?)
}

fn schema_error(context: &str, error: serde_json::Error) -> CliError {
    // Keep serde's field and type detail; it names what to fix in the input file.
    CliError::Json(<serde_json::Error as serde::de::Error>::custom(format!(
        "{context} schema mismatch: {error}"
    )))
}

pub(crate) fn execute(command: Spatial3dModelCommand) -> marklab::Result<()> {
    run_cli(command).map_err(into_marklab_error)
}
