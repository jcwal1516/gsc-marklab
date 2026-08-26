use serde::Serialize;
use thiserror::Error;

use crate::{
    inhomogeneous_poisson::validate_and_canonicalize, InhomogeneousPoissonEvent,
    InhomogeneousPoissonSpec, MidpointQuadratureValue, RectangularWindow,
};

#[derive(Clone, Debug)]
pub struct GriddedLgcpSpec {
    pub window: RectangularWindow,
    pub grid_x: u32,
    pub grid_y: u32,
    pub events: Vec<InhomogeneousPoissonEvent>,
    pub grid: Vec<MidpointQuadratureValue>,
    pub intercept_prior_mean: f64,
    pub intercept_prior_sd: f64,
    pub coefficient_prior_mean: f64,
    pub coefficient_prior_sd: f64,
    pub field_amplitude: f64,
    pub field_length_scale_um: f64,
    pub jitter: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct GriddedLgcpModelIr {
    pub family: &'static str,
    pub coordinate_unit: &'static str,
    pub window: &'static str,
    pub cell_rule: &'static str,
    pub intercept_prior: &'static str,
    pub intercept_prior_mean: f64,
    pub intercept_prior_sd: f64,
    pub coefficient_prior: &'static str,
    pub coefficient_prior_mean: f64,
    pub coefficient_prior_sd: f64,
    pub latent_field: &'static str,
    pub kernel: &'static str,
    pub field_amplitude: f64,
    pub field_length_scale_um: f64,
    pub jitter: f64,
    pub likelihood: &'static str,
    pub maturity: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct GriddedLgcpCell {
    pub ix: u32,
    pub iy: u32,
    pub midpoint_x_um: f64,
    pub midpoint_y_um: f64,
    pub area_um2: f64,
    pub covariate: f64,
    pub offset: f64,
    pub count: u64,
}

#[derive(Clone, Debug)]
pub struct GriddedLgcpModel {
    pub model: GriddedLgcpModelIr,
    pub window: RectangularWindow,
    pub grid_x: u32,
    pub grid_y: u32,
    pub event_count: usize,
    pub cell_area_um2: f64,
    pub cells: Vec<GriddedLgcpCell>,
    pub field_covariance: Vec<f64>,
    pub field_cholesky: Vec<f64>,
    pub covariance_positive_definite: bool,
    pub dense_covariance_elements: usize,
    pub dense_factorization_work_units: u64,
}

#[derive(Debug, Error)]
pub enum GriddedLgcpError {
    #[error("invalid gridded LGCP input: {0}")]
    InvalidInput(String),
    #[error("gridded LGCP numerical failure: {0}")]
    Numerical(String),
}

pub fn build_gridded_lgcp(spec: GriddedLgcpSpec) -> Result<GriddedLgcpModel, GriddedLgcpError> {
    if ![
        spec.intercept_prior_mean,
        spec.intercept_prior_sd,
        spec.coefficient_prior_mean,
        spec.coefficient_prior_sd,
        spec.field_amplitude,
        spec.field_length_scale_um,
        spec.jitter,
    ]
    .into_iter()
    .all(f64::is_finite)
        || spec.intercept_prior_sd <= 0.0
        || spec.coefficient_prior_sd <= 0.0
        || spec.field_amplitude <= 0.0
        || spec.field_length_scale_um <= 0.0
        || spec.jitter <= 0.0
    {
        return Err(GriddedLgcpError::InvalidInput(
            "LGCP priors and kernel controls require finite means and positive scales".into(),
        ));
    }
    let mut validated = InhomogeneousPoissonSpec {
        window: spec.window,
        grid_x: spec.grid_x,
        grid_y: spec.grid_y,
        events: spec.events,
        quadrature: spec.grid,
        intercept: 0.0,
        coefficient: 0.0,
    };
    validate_and_canonicalize(&mut validated)
        .map_err(|error| GriddedLgcpError::InvalidInput(error.to_string()))?;
    let dimension = validated.quadrature.len();
    if !(2..=64).contains(&dimension) {
        return Err(GriddedLgcpError::InvalidInput(
            "gridded LGCP construction requires 2-64 complete cells".into(),
        ));
    }
    let width = validated.window.xmax_um - validated.window.xmin_um;
    let height = validated.window.ymax_um - validated.window.ymin_um;
    let cell_width = width / f64::from(validated.grid_x);
    let cell_height = height / f64::from(validated.grid_y);
    let cell_area = cell_width * cell_height;
    let mut counts = vec![0_u64; dimension];
    for event in &validated.events {
        let ix = ((event.x_um - validated.window.xmin_um) / cell_width).floor() as usize;
        let iy = ((event.y_um - validated.window.ymin_um) / cell_height).floor() as usize;
        counts[iy * validated.grid_x as usize + ix] += 1;
    }
    let cells = validated
        .quadrature
        .iter()
        .enumerate()
        .map(|(index, node)| GriddedLgcpCell {
            ix: node.ix,
            iy: node.iy,
            midpoint_x_um: validated.window.xmin_um + (f64::from(node.ix) + 0.5) * cell_width,
            midpoint_y_um: validated.window.ymin_um + (f64::from(node.iy) + 0.5) * cell_height,
            area_um2: cell_area,
            covariate: node.covariate,
            offset: node.offset,
            count: counts[index],
        })
        .collect::<Vec<_>>();
    let dense_covariance_elements = dimension * dimension;
    let dense_factorization_work_units = dimension as u64 * dimension as u64 * dimension as u64;
    let mut covariance = vec![0.0; dense_covariance_elements];
    for row in 0..dimension {
        for column in 0..dimension {
            let dx = cells[row].midpoint_x_um - cells[column].midpoint_x_um;
            let dy = cells[row].midpoint_y_um - cells[column].midpoint_y_um;
            let distance = dx.hypot(dy);
            let scaled = 3.0_f64.sqrt() * distance / spec.field_length_scale_um;
            let mut value = spec.field_amplitude.powi(2) * (1.0 + scaled) * (-scaled).exp();
            if row == column {
                value += spec.jitter;
            }
            if !value.is_finite() {
                return Err(GriddedLgcpError::Numerical(
                    "LGCP field covariance is non-finite".into(),
                ));
            }
            covariance[row * dimension + column] = value;
        }
    }
    let field_cholesky = cholesky(&covariance, dimension)?;
    Ok(GriddedLgcpModel {
        model: GriddedLgcpModelIr {
            family: "gridded_log_gaussian_cox_process",
            coordinate_unit: "micrometer",
            window: "half_open_rectangle_complete_cells",
            cell_rule: "center_evaluated_covariate_and_offset",
            intercept_prior: "normal",
            intercept_prior_mean: spec.intercept_prior_mean,
            intercept_prior_sd: spec.intercept_prior_sd,
            coefficient_prior: "normal",
            coefficient_prior_mean: spec.coefficient_prior_mean,
            coefficient_prior_sd: spec.coefficient_prior_sd,
            latent_field: "zero_mean_dense_gaussian",
            kernel: "matern_3_2_euclidean_2d_cell_midpoints",
            field_amplitude: spec.field_amplitude,
            field_length_scale_um: spec.field_length_scale_um,
            jitter: spec.jitter,
            likelihood: "poisson_cell_area_exp_fixed_plus_latent_log_intensity",
            maturity: "experimental_model_construction",
        },
        window: validated.window,
        grid_x: validated.grid_x,
        grid_y: validated.grid_y,
        event_count: validated.events.len(),
        cell_area_um2: cell_area,
        cells,
        field_covariance: covariance,
        field_cholesky,
        covariance_positive_definite: true,
        dense_covariance_elements,
        dense_factorization_work_units,
    })
}

fn cholesky(matrix: &[f64], dimension: usize) -> Result<Vec<f64>, GriddedLgcpError> {
    let mut lower = vec![0.0; matrix.len()];
    for row in 0..dimension {
        for column in 0..=row {
            let mut value = matrix[row * dimension + column];
            for inner in 0..column {
                value -= lower[row * dimension + inner] * lower[column * dimension + inner];
            }
            if row == column {
                if !value.is_finite() || value <= 0.0 {
                    return Err(GriddedLgcpError::Numerical(
                        "LGCP field covariance is not positive definite".into(),
                    ));
                }
                lower[row * dimension + column] = value.sqrt();
            } else {
                lower[row * dimension + column] = value / lower[column * dimension + column];
            }
        }
    }
    Ok(lower)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_by_two_grid_has_exact_counts_and_kernel_diagonal() {
        let model = build_gridded_lgcp(GriddedLgcpSpec {
            window: RectangularWindow {
                xmin_um: 0.0,
                ymin_um: 0.0,
                xmax_um: 2.0,
                ymax_um: 2.0,
            },
            grid_x: 2,
            grid_y: 2,
            events: vec![
                InhomogeneousPoissonEvent {
                    event_id: "a".into(),
                    x_um: 0.25,
                    y_um: 0.25,
                    covariate: -1.0,
                    offset: 0.0,
                },
                InhomogeneousPoissonEvent {
                    event_id: "b".into(),
                    x_um: 1.75,
                    y_um: 1.75,
                    covariate: 1.0,
                    offset: 0.0,
                },
            ],
            grid: (0..2)
                .flat_map(|iy| {
                    (0..2).map(move |ix| MidpointQuadratureValue {
                        ix,
                        iy,
                        covariate: f64::from(iy * 2 + ix) - 1.5,
                        offset: 0.0,
                    })
                })
                .collect(),
            intercept_prior_mean: 0.0,
            intercept_prior_sd: 2.0,
            coefficient_prior_mean: 0.0,
            coefficient_prior_sd: 2.0,
            field_amplitude: 1.5,
            field_length_scale_um: 2.0,
            jitter: 1e-6,
        })
        .unwrap();
        assert_eq!(
            model
                .cells
                .iter()
                .map(|cell| cell.count)
                .collect::<Vec<_>>(),
            [1, 0, 0, 1]
        );
        for index in 0..4 {
            assert!((model.field_covariance[index * 4 + index] - 2.250001).abs() <= 1e-12);
        }
    }
}
