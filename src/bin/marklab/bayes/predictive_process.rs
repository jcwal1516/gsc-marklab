use std::path::PathBuf;

use marklab_bayes::{
    low_rank_predictive_process, FieldCoordinate1D, PredictiveProcessError, PredictiveProcessSpec,
};
use serde::{Deserialize, Serialize};

use super::{publish_json, BayesCliError, MAXIMUM_INPUT_BYTES};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CoordinateRow {
    coordinate_id: String,
    x_um: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct KnotRow {
    knot_id: String,
    x_um: f64,
}

pub(super) fn run(
    input_path: PathBuf,
    knot_path: PathBuf,
    amplitude: f64,
    length_scale_um: f64,
    jitter: f64,
    diagonal_correction: bool,
    output_path: PathBuf,
) -> Result<(), BayesCliError> {
    let coordinates = read_coordinates(&input_path)?;
    let knots = read_knots(&knot_path)?;
    let plan = low_rank_predictive_process(PredictiveProcessSpec {
        amplitude,
        length_scale_um,
        jitter,
        diagonal_correction,
        coordinates,
        knots,
    })
    .map_err(map_error)?;
    let exact_variance = amplitude * amplitude;
    let residuals = plan
        .coordinates
        .iter()
        .zip(&plan.low_rank_diagonal)
        .zip(&plan.residual_variances)
        .map(|((coordinate, low_rank), residual)| ResidualOutput {
            coordinate_id: coordinate.id.clone(),
            x_um: coordinate.x_um,
            low_rank_variance: *low_rank,
            residual_variance: *residual,
            corrected_variance: if diagonal_correction {
                *low_rank + *residual
            } else {
                *low_rank
            },
        })
        .collect::<Vec<_>>();
    let residual_sum = plan.residual_variances.iter().sum::<f64>();
    let residual_max = plan
        .residual_variances
        .iter()
        .copied()
        .fold(0.0_f64, f64::max);
    let coordinate_count = plan.coordinates.len();
    let result = PredictiveProcessOutput {
        format: "marklab.predictive_process",
        version: 1,
        kernel: "matern_3_2",
        coordinate_unit: "micrometre",
        amplitude,
        length_scale_um,
        jitter,
        diagonal_correction,
        input: InputIdentity {
            coordinate_path: input_path.display().to_string(),
            knot_path: knot_path.display().to_string(),
            coordinates: coordinate_count,
            knots: plan.knots.len(),
        },
        summary: Summary {
            mean_residual_variance: residual_sum / coordinate_count as f64,
            maximum_residual_variance: residual_max,
            trace_fraction_retained: 1.0
                - residual_sum / (coordinate_count as f64 * exact_variance),
        },
        residuals,
        claim_status: "experimental_approximation_diagnostic",
    };
    publish_json(&output_path, &result)
}

fn read_coordinates(path: &std::path::Path) -> Result<Vec<FieldCoordinate1D>, BayesCliError> {
    validate_file(path)?;
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader.headers()?.iter().eq(["coordinate_id", "x_um"]) {
        return Err(BayesCliError::Input(
            "predictive-process headers must be exactly: coordinate_id,x_um".into(),
        ));
    }
    let mut rows = Vec::new();
    for row in reader.deserialize::<CoordinateRow>() {
        let row = row?;
        rows.push(FieldCoordinate1D {
            id: row.coordinate_id,
            x_um: row.x_um,
        });
        if rows.len() > 2_000 {
            return Err(BayesCliError::Input(
                "predictive-process coordinate count exceeds 2000".into(),
            ));
        }
    }
    Ok(rows)
}

fn read_knots(path: &std::path::Path) -> Result<Vec<FieldCoordinate1D>, BayesCliError> {
    validate_file(path)?;
    let mut reader = csv::ReaderBuilder::new().from_path(path)?;
    if !reader.headers()?.iter().eq(["knot_id", "x_um"]) {
        return Err(BayesCliError::Input(
            "predictive-process knot headers must be exactly: knot_id,x_um".into(),
        ));
    }
    let mut rows = Vec::new();
    for row in reader.deserialize::<KnotRow>() {
        let row = row?;
        rows.push(FieldCoordinate1D {
            id: row.knot_id,
            x_um: row.x_um,
        });
        if rows.len() > 128 {
            return Err(BayesCliError::Input(
                "predictive-process knot count exceeds 128".into(),
            ));
        }
    }
    Ok(rows)
}

fn validate_file(path: &std::path::Path) -> Result<(), BayesCliError> {
    super::input_file::validate_regular_file(
        path,
        MAXIMUM_INPUT_BYTES,
        "predictive-process input must be a regular file within the 16 MiB limit",
    )
}

fn map_error(error: PredictiveProcessError) -> BayesCliError {
    match error {
        PredictiveProcessError::InvalidInput(message) => BayesCliError::Input(message),
        PredictiveProcessError::Numerical(message) => BayesCliError::Backend(message),
    }
}

#[derive(Debug, Serialize)]
struct PredictiveProcessOutput {
    format: &'static str,
    version: u32,
    kernel: &'static str,
    coordinate_unit: &'static str,
    amplitude: f64,
    length_scale_um: f64,
    jitter: f64,
    diagonal_correction: bool,
    input: InputIdentity,
    summary: Summary,
    residuals: Vec<ResidualOutput>,
    claim_status: &'static str,
}

#[derive(Debug, Serialize)]
struct InputIdentity {
    coordinate_path: String,
    knot_path: String,
    coordinates: usize,
    knots: usize,
}

#[derive(Debug, Serialize)]
struct Summary {
    mean_residual_variance: f64,
    maximum_residual_variance: f64,
    trace_fraction_retained: f64,
}

#[derive(Debug, Serialize)]
struct ResidualOutput {
    coordinate_id: String,
    x_um: f64,
    low_rank_variance: f64,
    residual_variance: f64,
    corrected_variance: f64,
}
