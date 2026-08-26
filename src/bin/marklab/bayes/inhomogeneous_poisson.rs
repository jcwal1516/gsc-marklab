use std::{fs, path::PathBuf};

use marklab_bayes::{
    inhomogeneous_poisson_log_likelihood, sha256_hex, InhomogeneousPoissonError,
    InhomogeneousPoissonEvent, InhomogeneousPoissonSpec, MidpointQuadratureValue,
    RectangularWindow,
};
use serde::{Deserialize, Serialize};

use super::{publish_json, BayesCliError, MAXIMUM_INPUT_BYTES};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EventRow {
    event_id: String,
    x_um: f64,
    y_um: f64,
    covariate: f64,
    offset: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct QuadratureRow {
    ix: u32,
    iy: u32,
    covariate: f64,
    offset: f64,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    events_path: PathBuf,
    quadrature_path: PathBuf,
    xmin_um: f64,
    ymin_um: f64,
    xmax_um: f64,
    ymax_um: f64,
    grid_x: u32,
    grid_y: u32,
    intercept: f64,
    coefficient: f64,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let events = read_events(&events_path)?;
    let quadrature = read_quadrature(&quadrature_path)?;
    let events_sha256 = sha256_hex(&serde_json::to_vec(&events)?);
    let quadrature_sha256 = sha256_hex(&serde_json::to_vec(&quadrature)?);
    let window = RectangularWindow {
        xmin_um,
        ymin_um,
        xmax_um,
        ymax_um,
    };
    let result = inhomogeneous_poisson_log_likelihood(InhomogeneousPoissonSpec {
        window,
        grid_x,
        grid_y,
        events,
        quadrature,
        intercept,
        coefficient,
    })
    .map_err(map_error)?;
    publish_json(
        &output_path,
        &InhomogeneousPoissonOutput {
            format: "marklab.inhomogeneous_poisson_likelihood",
            version: 1,
            events_path,
            quadrature_path,
            events_sha256,
            quadrature_sha256,
            coordinate_unit: "micrometer",
            area_unit: "square_micrometer",
            window,
            grid_x,
            grid_y,
            event_count: result.event_count,
            quadrature_node_count: result.quadrature_node_count,
            window_area_um2: result.window_area_um2,
            cell_width_um: result.cell_width_um,
            cell_height_um: result.cell_height_um,
            cell_area_um2: result.cell_area_um2,
            intercept,
            coefficient,
            event_term: result.event_term,
            integral_term: result.integral_term,
            log_likelihood: result.log_likelihood,
            interpretation: "fixed_parameter_log_linear_point_process_likelihood",
            claim_status: "experimental_fixed_likelihood",
        },
    )
}

fn validate_file(path: &std::path::Path) -> Result<(), BayesCliError> {
    let metadata = fs::metadata(path).map_err(|source| BayesCliError::Io {
        path: path.to_owned(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(BayesCliError::Input(
            "inhomogeneous Poisson inputs must be regular files within the 16 MiB limit".into(),
        ));
    }
    Ok(())
}

pub(super) fn read_events(
    path: &std::path::Path,
) -> Result<Vec<InhomogeneousPoissonEvent>, BayesCliError> {
    validate_file(path)?;
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader
        .headers()?
        .iter()
        .eq(["event_id", "x_um", "y_um", "covariate", "offset"])
    {
        return Err(BayesCliError::Input(
            "event headers must be exactly: event_id,x_um,y_um,covariate,offset".into(),
        ));
    }
    let mut events = Vec::new();
    for row in reader.deserialize::<EventRow>() {
        let row = row?;
        events.push(InhomogeneousPoissonEvent {
            event_id: row.event_id,
            x_um: row.x_um,
            y_um: row.y_um,
            covariate: row.covariate,
            offset: row.offset,
        });
        if events.len() > 100_000 {
            return Err(BayesCliError::Input(
                "inhomogeneous Poisson event count exceeds 100000".into(),
            ));
        }
    }
    Ok(events)
}

pub(super) fn read_quadrature(
    path: &std::path::Path,
) -> Result<Vec<MidpointQuadratureValue>, BayesCliError> {
    validate_file(path)?;
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader
        .headers()?
        .iter()
        .eq(["ix", "iy", "covariate", "offset"])
    {
        return Err(BayesCliError::Input(
            "quadrature headers must be exactly: ix,iy,covariate,offset".into(),
        ));
    }
    let mut quadrature = Vec::new();
    for row in reader.deserialize::<QuadratureRow>() {
        let row = row?;
        quadrature.push(MidpointQuadratureValue {
            ix: row.ix,
            iy: row.iy,
            covariate: row.covariate,
            offset: row.offset,
        });
        if quadrature.len() > 1_000_000 {
            return Err(BayesCliError::Input(
                "inhomogeneous Poisson quadrature exceeds 1000000 nodes".into(),
            ));
        }
    }
    Ok(quadrature)
}

fn map_error(error: InhomogeneousPoissonError) -> BayesCliError {
    match error {
        InhomogeneousPoissonError::InvalidInput(message) => BayesCliError::Input(message),
        InhomogeneousPoissonError::Numerical(message) => BayesCliError::Backend(message),
    }
}

#[derive(Serialize)]
struct InhomogeneousPoissonOutput {
    format: &'static str,
    version: u32,
    events_path: PathBuf,
    quadrature_path: PathBuf,
    events_sha256: String,
    quadrature_sha256: String,
    coordinate_unit: &'static str,
    area_unit: &'static str,
    window: RectangularWindow,
    grid_x: u32,
    grid_y: u32,
    event_count: usize,
    quadrature_node_count: usize,
    window_area_um2: f64,
    cell_width_um: f64,
    cell_height_um: f64,
    cell_area_um2: f64,
    intercept: f64,
    coefficient: f64,
    event_term: f64,
    integral_term: f64,
    log_likelihood: f64,
    interpretation: &'static str,
    claim_status: &'static str,
}
