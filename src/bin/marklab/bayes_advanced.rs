use std::{collections::BTreeSet, path::PathBuf};

use clap::{Parser, Subcommand};
use marklab::{ObservationWindow2D, ObservationWindowLimits};
use marklab_topology::sha256_hex;
use serde::Deserialize;
use serde_json::Value;

use super::topology::{publish_json, read_input, read_required, run_worker, TopologyCliError};

#[derive(Clone)]
pub(crate) struct PreparedAdvancedBayes {
    pub input_bytes: Vec<u8>,
    pub lock_bytes: Vec<u8>,
    pub worker_bytes: Vec<u8>,
    pub request_bytes: Vec<u8>,
    repository: PathBuf,
    worker_path: PathBuf,
    request: Value,
    timeout_seconds: u64,
    expected_format: &'static str,
}

#[allow(
    dead_code,
    reason = "all fields participate in strict input deserialization"
)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AdaptiveWindowSpdeSpec {
    window_frame: String,
    window: Value,
    base_resolution: u32,
    boundary_refinement_levels: u32,
    maximum_relative_area_error: f64,
    alpha: u32,
    kappa: f64,
    tau: f64,
    factor_observations: Vec<AdaptiveWindowSpdeObservation>,
    factor_count: u32,
    factor_noise_sd: f64,
    maximum_iterations: u32,
    maximum_vertices: usize,
    maximum_triangles: usize,
    maximum_boundary_segment_triangle_checks: u64,
    maximum_projection_triangle_visits: u64,
    memory_budget_mib: usize,
    maximum_result_bytes: usize,
    timeout_seconds: u64,
}

#[allow(
    dead_code,
    reason = "all fields participate in strict input deserialization"
)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AdaptiveWindowSpdeObservation {
    region_id: String,
    coordinates: [f64; 2],
    values: Vec<f64>,
}

#[allow(
    dead_code,
    reason = "all fields participate in strict input deserialization"
)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NonstationaryAdaptiveWindowSpdeSpec {
    window_frame: String,
    window: Value,
    base_resolution: u32,
    boundary_refinement_levels: u32,
    maximum_relative_area_error: f64,
    background: NonstationarySpdeParameters,
    parameter_regions: Vec<NonstationarySpdeRegion>,
    alpha: u32,
    factor_observations: Vec<AdaptiveWindowSpdeObservation>,
    factor_count: u32,
    factor_noise_sd: f64,
    maximum_iterations: u32,
    maximum_vertices: usize,
    maximum_triangles: usize,
    maximum_candidate_points: usize,
    maximum_boundary_segment_triangle_checks: u64,
    maximum_projection_triangle_visits: u64,
    memory_budget_mib: usize,
    maximum_result_bytes: usize,
    timeout_seconds: u64,
}

#[allow(
    dead_code,
    reason = "all fields participate in strict input deserialization"
)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NonstationarySpdeParameters {
    kappa: f64,
    tau: f64,
    anisotropy_ratio: f64,
    major_axis_degrees: f64,
}

#[allow(
    dead_code,
    reason = "all fields participate in strict input deserialization"
)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NonstationarySpdeRegion {
    region_id: String,
    center: [f64; 2],
    radius: f64,
    kappa: f64,
    tau: f64,
    anisotropy_ratio: f64,
    major_axis_degrees: f64,
    interior_refinement_levels: u32,
}

#[derive(Debug, Parser)]
#[command(name = "marklab")]
struct AdvancedBayesCli {
    #[command(subcommand)]
    command: BayesTopLevel,
}

#[derive(Debug, Subcommand)]
enum BayesTopLevel {
    Bayes {
        #[command(subcommand)]
        command: AdvancedBayesCommand,
    },
}

#[derive(Debug, Subcommand)]
enum AdvancedBayesCommand {
    HmcNormal {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    AdvancedCluster {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    SpdeSuite {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    AdaptiveWindowSpde {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    NonstationaryAdaptiveWindowSpde {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
}

pub(crate) fn run_cli() -> Result<(), TopologyCliError> {
    match AdvancedBayesCli::parse().command {
        BayesTopLevel::Bayes {
            command: AdvancedBayesCommand::HmcNormal { input, out },
        } => run("hmc_normal", input, out),
        BayesTopLevel::Bayes {
            command: AdvancedBayesCommand::AdvancedCluster { input, out },
        } => run("advanced_cluster", input, out),
        BayesTopLevel::Bayes {
            command: AdvancedBayesCommand::SpdeSuite { input, out },
        } => run("spde_suite", input, out),
        BayesTopLevel::Bayes {
            command: AdvancedBayesCommand::AdaptiveWindowSpde { input, out },
        } => run("adaptive_window_spde", input, out),
        BayesTopLevel::Bayes {
            command: AdvancedBayesCommand::NonstationaryAdaptiveWindowSpde { input, out },
        } => run("nonstationary_adaptive_window_spde", input, out),
    }
}

fn run(mode: &str, input: PathBuf, out: PathBuf) -> Result<(), TopologyCliError> {
    let prepared = prepare(mode, &input)?;
    let result = execute(&prepared)?;
    publish_json(&out, &result)
}

pub(crate) fn prepare(
    mode: &str,
    input: &std::path::Path,
) -> Result<PreparedAdvancedBayes, TopologyCliError> {
    let input_bytes = read_input(input)?;
    let spec: Value = serde_json::from_slice(&input_bytes)?;
    let timeout = spec
        .get("timeout_seconds")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            TopologyCliError::Input(
                "advanced Bayesian input requires integer timeout_seconds".into(),
            )
        })?;
    if !(1..=3_600).contains(&timeout) || !spec.is_object() {
        return Err(TopologyCliError::Input(
            "advanced Bayesian timeout must be between 1 and 3600 seconds".into(),
        ));
    }
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let worker_path = repository.join("workers/python/marklab_scipy_advanced_bayes_worker.py");
    let lock = read_required(&repository.join("workers/python/uv.lock"))?;
    let worker = read_required(&worker_path)?;
    let adaptive_window = if matches!(
        mode,
        "adaptive_window_spde" | "nonstationary_adaptive_window_spde"
    ) {
        let geometry = if mode == "adaptive_window_spde" {
            serde_json::from_value::<AdaptiveWindowSpdeSpec>(spec.clone())?.window
        } else {
            serde_json::from_value::<NonstationaryAdaptiveWindowSpdeSpec>(spec.clone())?.window
        };
        let geometry_bytes = serde_json::to_vec(&geometry)?;
        let geometry_text = std::str::from_utf8(&geometry_bytes).map_err(|_| {
            TopologyCliError::Input("adaptive SPDE window serialization is not UTF-8".into())
        })?;
        Some(
            ObservationWindow2D::from_geojson_str(
                geometry_text,
                ObservationWindowLimits::default(),
            )
            .map_err(|error| TopologyCliError::Input(error.to_string()))?,
        )
    } else {
        None
    };
    let mut request = serde_json::json!({
        "format":"marklab.scipy_advanced_bayes_request", "version":1, "mode":mode,
        "backend":{"name":"numpy_scipy_advanced_bayes","scipy_version":"1.18.1","numpy_version":"2.4.6","python_version":"3.12","license":"BSD-3-Clause","environment_lock_sha256":sha256_hex(&lock),"worker_sha256":sha256_hex(&worker)},
        "spec":spec, "input_sha256":sha256_hex(&input_bytes)
    });
    if let Some(window) = adaptive_window {
        request["canonical_window_sha256"] =
            Value::String(window.descriptor().logical_digest.to_string());
        request["canonical_window_area"] = Value::from(window.area_um2());
    }
    let request_bytes = serde_json::to_vec(&request)?;
    let expected = match mode {
        "hmc_normal" => "marklab.fixed_step_hmc_normal_mean",
        "advanced_cluster" => "marklab.advanced_cluster_and_gibbs_models",
        "spde_suite" => "marklab.rectangular_spde_suite",
        "adaptive_window_spde" => "marklab.adaptive_window_spde",
        "nonstationary_adaptive_window_spde" => "marklab.nonstationary_adaptive_window_spde",
        _ => {
            return Err(TopologyCliError::Input(format!(
                "unknown advanced Bayesian mode {mode}"
            )))
        }
    };
    Ok(PreparedAdvancedBayes {
        input_bytes,
        lock_bytes: lock,
        worker_bytes: worker,
        request_bytes,
        repository,
        worker_path,
        request,
        timeout_seconds: timeout,
        expected_format: expected,
    })
}

pub(crate) fn execute(prepared: &PreparedAdvancedBayes) -> Result<Value, TopologyCliError> {
    let response = run_worker(
        &prepared.repository,
        &prepared.worker_path,
        &prepared.request_bytes,
        prepared.timeout_seconds,
    )?;
    let result: Value = serde_json::from_slice(&response)?;
    validate_result(prepared, &result)?;
    Ok(result)
}

pub(crate) fn validate_result(
    prepared: &PreparedAdvancedBayes,
    result: &Value,
) -> Result<(), TopologyCliError> {
    if result["format"] != prepared.expected_format
        || result["backend"] != prepared.request["backend"]
        || result["request_sha256"] != sha256_hex(&prepared.request_bytes)
        || (matches!(
            prepared.expected_format,
            "marklab.adaptive_window_spde" | "marklab.nonstationary_adaptive_window_spde"
        ) && (result["window"]["canonical_sha256"]
            != prepared.request["canonical_window_sha256"]
            || result["window"]["exact_area"] != prepared.request["canonical_window_area"]))
    {
        return Err(TopologyCliError::Backend(
            "advanced Bayesian result identity mismatch".into(),
        ));
    }
    if prepared.expected_format == "marklab.nonstationary_adaptive_window_spde" {
        validate_nonstationary_adaptive_result(&prepared.request["spec"], result)?;
    }
    Ok(())
}

fn validate_nonstationary_adaptive_result(
    spec: &Value,
    result: &Value,
) -> Result<(), TopologyCliError> {
    let invalid = || {
        TopologyCliError::Backend(
            "nonstationary adaptive SPDE result structure or mathematics differ".into(),
        )
    };
    let top = result.as_object().ok_or_else(invalid)?;
    let expected_top = BTreeSet::from([
        "assumptions",
        "backend",
        "background",
        "claim_status",
        "format",
        "limitations",
        "limits",
        "mesh",
        "parameter_regions",
        "request_sha256",
        "spatial_factor",
        "statistical_unit",
        "version",
        "window",
    ]);
    if top.keys().map(String::as_str).collect::<BTreeSet<_>>() != expected_top
        || result["version"] != 1
        || result["statistical_unit"] != "within_specimen_field_diagnostic"
        || result["claim_status"] != "fitted_nonstationary_anisotropic_adaptive_spde_diagnostic"
    {
        return Err(invalid());
    }
    let mesh = result["mesh"].as_object().ok_or_else(invalid)?;
    let expected_mesh = BTreeSet::from([
        "alpha",
        "anisotropic_stiffness_matrix",
        "basis",
        "boundary_condition",
        "mass_matrix",
        "maximum_precision_eigenvalue",
        "maximum_precision_symmetry_error",
        "maximum_projection_row_sum_error",
        "mesh_area",
        "minimum_precision_eigenvalue",
        "operator",
        "precision_matrix",
        "relative_area_error",
        "triangle_count",
        "triangles",
        "vertex_count",
        "vertices",
    ]);
    let vertex_count = mesh
        .get("vertex_count")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(invalid)?;
    let triangle_count = mesh
        .get("triangle_count")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(invalid)?;
    let finite = |key: &str| {
        mesh.get(key)
            .and_then(Value::as_f64)
            .is_some_and(f64::is_finite)
    };
    if mesh.keys().map(String::as_str).collect::<BTreeSet<_>>() != expected_mesh
        || mesh["operator"] != "mass_lumped_nonstationary_alpha_two_spde"
        || mesh["basis"] != "piecewise_linear_triangular"
        || mesh["boundary_condition"] != "natural_neumann_with_positive_local_kappa"
        || mesh["alpha"] != 2
        || vertex_count < 3
        || triangle_count == 0
        || mesh["vertices"]
            .as_array()
            .is_none_or(|rows| rows.len() != vertex_count)
        || mesh["triangles"]
            .as_array()
            .is_none_or(|rows| rows.len() != triangle_count)
        || ![
            "mesh_area",
            "relative_area_error",
            "minimum_precision_eigenvalue",
            "maximum_precision_eigenvalue",
            "maximum_precision_symmetry_error",
            "maximum_projection_row_sum_error",
        ]
        .iter()
        .all(|key| finite(key))
        || mesh["minimum_precision_eigenvalue"]
            .as_f64()
            .is_none_or(|value| value <= 0.0)
        || mesh["maximum_precision_eigenvalue"]
            .as_f64()
            .zip(mesh["minimum_precision_eigenvalue"].as_f64())
            .is_none_or(|(maximum, minimum)| maximum < minimum)
        || mesh["maximum_precision_symmetry_error"]
            .as_f64()
            .is_none_or(|value| value > 1e-10)
        || mesh["maximum_projection_row_sum_error"]
            .as_f64()
            .is_none_or(|value| value > 1e-10)
    {
        return Err(invalid());
    }
    for name in [
        "mass_matrix",
        "anisotropic_stiffness_matrix",
        "precision_matrix",
    ] {
        let matrix = mesh
            .get(name)
            .and_then(Value::as_object)
            .ok_or_else(invalid)?;
        if matrix.keys().map(String::as_str).collect::<BTreeSet<_>>()
            != BTreeSet::from(["columns", "rows", "shape", "values"])
            || matrix["shape"] != serde_json::json!([vertex_count, vertex_count])
        {
            return Err(invalid());
        }
        let rows = matrix["rows"].as_array().ok_or_else(invalid)?;
        let columns = matrix["columns"].as_array().ok_or_else(invalid)?;
        let values = matrix["values"].as_array().ok_or_else(invalid)?;
        if rows.len() != columns.len()
            || rows.len() != values.len()
            || rows.iter().chain(columns).any(|value| {
                value
                    .as_u64()
                    .is_none_or(|index| index >= vertex_count as u64)
            })
            || values
                .iter()
                .any(|value| value.as_f64().is_none_or(|number| !number.is_finite()))
        {
            return Err(invalid());
        }
    }
    let expected_regions = spec["parameter_regions"].as_array().ok_or_else(invalid)?;
    let actual_regions = result["parameter_regions"].as_array().ok_or_else(invalid)?;
    if expected_regions.len() != actual_regions.len() {
        return Err(invalid());
    }
    validate_local_parameter_summary(&spec["background"], &result["background"], "background")?;
    for (expected, actual) in expected_regions.iter().zip(actual_regions) {
        let identity = expected["region_id"].as_str().ok_or_else(invalid)?;
        validate_local_parameter_summary(expected, actual, identity)?;
        if actual["interior_refinement_levels"] != expected["interior_refinement_levels"] {
            return Err(invalid());
        }
    }
    if result["spatial_factor"]["gradient_maximum"]
        .as_f64()
        .is_none_or(|value| !value.is_finite() || value > 2e-4)
        || result["limits"]
            != serde_json::json!({
                "maximum_vertices": spec["maximum_vertices"],
                "maximum_triangles": spec["maximum_triangles"],
                "maximum_candidate_points": spec["maximum_candidate_points"],
                "maximum_boundary_segment_triangle_checks": spec["maximum_boundary_segment_triangle_checks"],
                "maximum_projection_triangle_visits": spec["maximum_projection_triangle_visits"],
                "memory_budget_mib": spec["memory_budget_mib"],
                "maximum_result_bytes": spec["maximum_result_bytes"],
                "maximum_iterations": spec["maximum_iterations"],
            })
    {
        return Err(invalid());
    }
    Ok(())
}

fn validate_local_parameter_summary(
    expected: &Value,
    actual: &Value,
    identity: &str,
) -> Result<(), TopologyCliError> {
    let invalid = || {
        TopologyCliError::Backend(
            "nonstationary adaptive SPDE local parameter summary differs".into(),
        )
    };
    let summary = actual.as_object().ok_or_else(invalid)?;
    if summary.keys().map(String::as_str).collect::<BTreeSet<_>>()
        != BTreeSet::from([
            "anisotropy_ratio",
            "anisotropy_tensor",
            "interior_refinement_levels",
            "kappa",
            "major_axis_degrees",
            "median_triangle_area",
            "region_id",
            "tau",
            "triangle_count",
        ])
        || actual["region_id"] != identity
        || actual["kappa"] != expected["kappa"]
        || actual["tau"] != expected["tau"]
        || actual["anisotropy_ratio"] != expected["anisotropy_ratio"]
        || actual["major_axis_degrees"] != expected["major_axis_degrees"]
        || actual["triangle_count"]
            .as_u64()
            .is_none_or(|count| count == 0)
        || actual["median_triangle_area"]
            .as_f64()
            .is_none_or(|area| !area.is_finite() || area <= 0.0)
    {
        return Err(invalid());
    }
    let ratio = expected["anisotropy_ratio"].as_f64().ok_or_else(invalid)?;
    let radians = expected["major_axis_degrees"]
        .as_f64()
        .ok_or_else(invalid)?
        .to_radians();
    let cosine = radians.cos();
    let sine = radians.sin();
    let tensor = [
        [
            ratio * cosine * cosine + sine * sine / ratio,
            (ratio - 1.0 / ratio) * sine * cosine,
        ],
        [
            (ratio - 1.0 / ratio) * sine * cosine,
            ratio * sine * sine + cosine * cosine / ratio,
        ],
    ];
    let reported: [[f64; 2]; 2] =
        serde_json::from_value(actual["anisotropy_tensor"].clone()).map_err(|_| invalid())?;
    if reported
        .iter()
        .flatten()
        .zip(tensor.iter().flatten())
        .any(|(left, right)| (left - right).abs() > 1e-12 * right.abs().max(1.0))
    {
        return Err(invalid());
    }
    Ok(())
}

pub(crate) fn into_marklab_error(error: TopologyCliError) -> marklab::MarklabError {
    marklab::MarklabError::Validation(error.to_string())
}
