use std::{fs, path::PathBuf};

use marklab_bayes::{
    build_nngp, full_gp_log_density, nngp_log_density, NngpError, NngpObservation, NngpSpec,
};
use serde::{Deserialize, Serialize};

use super::{publish_json, BayesCliError, MAXIMUM_INPUT_BYTES};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FieldRow {
    coordinate_id: String,
    x_um: f64,
    field_value: f64,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    input_path: PathBuf,
    mean: f64,
    amplitude: f64,
    length_scale_um: f64,
    neighbors: usize,
    jitter: f64,
    variance_tolerance: f64,
    include_full_reference: bool,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let observations = read_field(&input_path)?;
    let plan = build_nngp(NngpSpec {
        mean,
        amplitude,
        length_scale_um,
        neighbors,
        jitter,
        variance_tolerance,
        observations,
    })
    .map_err(map_error)?;
    let nngp_density = nngp_log_density(&plan).map_err(map_error)?;
    let full_reference = if include_full_reference {
        let full = full_gp_log_density(&plan).map_err(map_error)?;
        Some(FullReference {
            log_density: full,
            absolute_log_density_difference: (nngp_density - full).abs(),
        })
    } else {
        None
    };
    let result = NngpOutput {
        format: "marklab.nngp_density",
        version: 1,
        coordinate_unit: "micrometre",
        kernel: "matern_3_2",
        ordering: "ascending_physical_coordinate",
        input_path: input_path.display().to_string(),
        mean,
        amplitude,
        length_scale_um,
        neighbors,
        jitter,
        variance_tolerance,
        ordered_coordinate_ids: plan
            .observations
            .iter()
            .map(|row| row.coordinate_id.clone())
            .collect(),
        neighbor_counts: plan.neighbor_indices.iter().map(Vec::len).collect(),
        conditional_variances: plan.conditional_variances.clone(),
        nngp_log_density: nngp_density,
        full_reference,
        claim_status: "experimental_approximation_diagnostic",
    };
    publish_json(&output_path, &result)
}

fn read_field(path: &std::path::Path) -> Result<Vec<NngpObservation>, BayesCliError> {
    let metadata = fs::metadata(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(BayesCliError::Input(
            "NNGP input must be a regular file within the 16 MiB limit".into(),
        ));
    }
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader
        .headers()?
        .iter()
        .eq(["coordinate_id", "x_um", "field_value"])
    {
        return Err(BayesCliError::Input(
            "NNGP headers must be exactly: coordinate_id,x_um,field_value".into(),
        ));
    }
    let mut rows = Vec::new();
    for row in reader.deserialize::<FieldRow>() {
        let row = row?;
        rows.push(NngpObservation {
            coordinate_id: row.coordinate_id,
            x_um: row.x_um,
            field_value: row.field_value,
        });
        if rows.len() > 10_000 {
            return Err(BayesCliError::Input("NNGP row count exceeds 10000".into()));
        }
    }
    Ok(rows)
}

fn map_error(error: NngpError) -> BayesCliError {
    match error {
        NngpError::InvalidInput(message) => BayesCliError::Input(message),
        NngpError::Numerical(message) => BayesCliError::Backend(message),
    }
}

#[derive(Debug, Serialize)]
struct NngpOutput {
    format: &'static str,
    version: u32,
    coordinate_unit: &'static str,
    kernel: &'static str,
    ordering: &'static str,
    input_path: String,
    mean: f64,
    amplitude: f64,
    length_scale_um: f64,
    neighbors: usize,
    jitter: f64,
    variance_tolerance: f64,
    ordered_coordinate_ids: Vec<String>,
    neighbor_counts: Vec<usize>,
    conditional_variances: Vec<f64>,
    nngp_log_density: f64,
    full_reference: Option<FullReference>,
    claim_status: &'static str,
}

#[derive(Debug, Serialize)]
struct FullReference {
    log_density: f64,
    absolute_log_density_difference: f64,
}
