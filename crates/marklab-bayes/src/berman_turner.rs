use serde::Serialize;
use thiserror::Error;

use crate::{
    inhomogeneous_poisson::validate_and_canonicalize, InhomogeneousPoissonEvent,
    InhomogeneousPoissonSpec, MidpointQuadratureValue, RectangularWindow,
};

#[derive(Clone, Debug)]
pub struct BermanTurnerRefinementSpec {
    pub window: RectangularWindow,
    pub events: Vec<InhomogeneousPoissonEvent>,
    pub coarse_grid_x: u32,
    pub coarse_grid_y: u32,
    pub coarse_quadrature: Vec<MidpointQuadratureValue>,
    pub fine_grid_x: u32,
    pub fine_grid_y: u32,
    pub fine_quadrature: Vec<MidpointQuadratureValue>,
    pub intercept: f64,
    pub coefficient: f64,
    pub convergence_tolerance: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct BermanTurnerNode {
    pub node_id: String,
    pub node_kind: &'static str,
    pub ix: u32,
    pub iy: u32,
    pub x_um: f64,
    pub y_um: f64,
    pub covariate: f64,
    pub offset: f64,
    pub weight_um2: f64,
    pub pseudo_response: f64,
    pub linear_predictor: f64,
    pub weighted_objective_contribution: f64,
}

#[derive(Clone, Debug)]
pub struct BermanTurnerResolution {
    pub grid_x: u32,
    pub grid_y: u32,
    pub cell_area_um2: f64,
    pub node_count: usize,
    pub observed_node_count: usize,
    pub dummy_node_count: usize,
    pub weight_sum_um2: f64,
    pub weighted_objective: f64,
    pub table: Vec<BermanTurnerNode>,
}

#[derive(Clone, Debug)]
pub struct BermanTurnerRefinementResult {
    pub window_area_um2: f64,
    pub coarse: BermanTurnerResolution,
    pub fine: BermanTurnerResolution,
    pub absolute_objective_change: f64,
    pub convergence_tolerance: f64,
    pub converged: bool,
}

#[derive(Debug, Error)]
pub enum BermanTurnerError {
    #[error("invalid Berman-Turner input: {0}")]
    InvalidInput(String),
    #[error("Berman-Turner numerical failure: {0}")]
    Numerical(String),
}

pub fn berman_turner_refinement(
    spec: BermanTurnerRefinementSpec,
) -> Result<BermanTurnerRefinementResult, BermanTurnerError> {
    if !spec.convergence_tolerance.is_finite() || spec.convergence_tolerance <= 0.0 {
        return Err(BermanTurnerError::InvalidInput(
            "convergence tolerance must be finite and positive".into(),
        ));
    }
    if ![
        spec.coarse_grid_x,
        spec.coarse_grid_y,
        spec.fine_grid_x,
        spec.fine_grid_y,
    ]
    .into_iter()
    .all(|value| (1..=1_024).contains(&value))
    {
        return Err(BermanTurnerError::InvalidInput(
            "coarse and fine grid dimensions must be in 1-1024".into(),
        ));
    }
    if !spec.fine_grid_x.is_multiple_of(spec.coarse_grid_x)
        || !spec.fine_grid_y.is_multiple_of(spec.coarse_grid_y)
        || spec.fine_grid_x == spec.coarse_grid_x && spec.fine_grid_y == spec.coarse_grid_y
    {
        return Err(BermanTurnerError::InvalidInput(
            "fine grid dimensions must be nested integer multiples with more cells".into(),
        ));
    }
    let coarse = validate_resolution(
        spec.window,
        spec.events.clone(),
        spec.coarse_grid_x,
        spec.coarse_grid_y,
        spec.coarse_quadrature,
        spec.intercept,
        spec.coefficient,
    )?;
    let fine = validate_resolution(
        spec.window,
        spec.events,
        spec.fine_grid_x,
        spec.fine_grid_y,
        spec.fine_quadrature,
        spec.intercept,
        spec.coefficient,
    )?;
    let absolute_objective_change = (fine.weighted_objective - coarse.weighted_objective).abs();
    if !absolute_objective_change.is_finite() {
        return Err(BermanTurnerError::Numerical(
            "refinement objective change is non-finite".into(),
        ));
    }
    Ok(BermanTurnerRefinementResult {
        window_area_um2: (spec.window.xmax_um - spec.window.xmin_um)
            * (spec.window.ymax_um - spec.window.ymin_um),
        coarse,
        fine,
        absolute_objective_change,
        convergence_tolerance: spec.convergence_tolerance,
        converged: absolute_objective_change <= spec.convergence_tolerance,
    })
}

fn validate_resolution(
    window: RectangularWindow,
    events: Vec<InhomogeneousPoissonEvent>,
    grid_x: u32,
    grid_y: u32,
    quadrature: Vec<MidpointQuadratureValue>,
    intercept: f64,
    coefficient: f64,
) -> Result<BermanTurnerResolution, BermanTurnerError> {
    let mut validated = InhomogeneousPoissonSpec {
        window,
        grid_x,
        grid_y,
        events,
        quadrature,
        intercept,
        coefficient,
    };
    validate_and_canonicalize(&mut validated)
        .map_err(|error| BermanTurnerError::InvalidInput(error.to_string()))?;
    let output_nodes = validated.events.len() + validated.quadrature.len();
    if output_nodes > 100_000 {
        return Err(BermanTurnerError::InvalidInput(
            "Berman-Turner output exceeds 100000 nodes per resolution".into(),
        ));
    }
    let width = window.xmax_um - window.xmin_um;
    let height = window.ymax_um - window.ymin_um;
    let cell_width = width / f64::from(grid_x);
    let cell_height = height / f64::from(grid_y);
    let cell_area = cell_width * cell_height;
    let mut cell_events = vec![Vec::new(); validated.quadrature.len()];
    for event in validated.events {
        let ix = ((event.x_um - window.xmin_um) / cell_width).floor() as usize;
        let iy = ((event.y_um - window.ymin_um) / cell_height).floor() as usize;
        cell_events[iy * grid_x as usize + ix].push(event);
    }
    let mut table = Vec::with_capacity(output_nodes);
    for (index, (node, events)) in validated
        .quadrature
        .into_iter()
        .zip(cell_events)
        .enumerate()
    {
        let weight = cell_area / (events.len() + 1) as f64;
        if !weight.is_finite() || weight <= 0.0 {
            return Err(BermanTurnerError::Numerical(
                "derived Berman-Turner node weight is invalid".into(),
            ));
        }
        for event in events {
            table.push(make_node(
                format!("event:{}", event.event_id),
                "observed",
                node.ix,
                node.iy,
                event.x_um,
                event.y_um,
                event.covariate,
                event.offset,
                weight,
                1.0 / weight,
                intercept,
                coefficient,
            )?);
        }
        let ix = index as u32 % grid_x;
        let iy = index as u32 / grid_x;
        table.push(make_node(
            format!("dummy:{iy}:{ix}"),
            "dummy",
            node.ix,
            node.iy,
            window.xmin_um + (f64::from(ix) + 0.5) * cell_width,
            window.ymin_um + (f64::from(iy) + 0.5) * cell_height,
            node.covariate,
            node.offset,
            weight,
            0.0,
            intercept,
            coefficient,
        )?);
    }
    let weight_sum = compensated_sum(table.iter().map(|node| node.weight_um2))?;
    let weighted_objective = compensated_sum(
        table
            .iter()
            .map(|node| node.weighted_objective_contribution),
    )?;
    let window_area = width * height;
    if (weight_sum - window_area).abs() > 1e-12 * window_area.abs().max(1.0) {
        return Err(BermanTurnerError::Numerical(
            "Berman-Turner weights do not sum to window area".into(),
        ));
    }
    Ok(BermanTurnerResolution {
        grid_x,
        grid_y,
        cell_area_um2: cell_area,
        node_count: table.len(),
        observed_node_count: table
            .iter()
            .filter(|node| node.node_kind == "observed")
            .count(),
        dummy_node_count: table
            .iter()
            .filter(|node| node.node_kind == "dummy")
            .count(),
        weight_sum_um2: weight_sum,
        weighted_objective,
        table,
    })
}

#[allow(clippy::too_many_arguments)]
fn make_node(
    node_id: String,
    node_kind: &'static str,
    ix: u32,
    iy: u32,
    x_um: f64,
    y_um: f64,
    covariate: f64,
    offset: f64,
    weight_um2: f64,
    pseudo_response: f64,
    intercept: f64,
    coefficient: f64,
) -> Result<BermanTurnerNode, BermanTurnerError> {
    let linear_predictor = intercept + coefficient * covariate + offset;
    let contribution = weight_um2 * (pseudo_response * linear_predictor - linear_predictor.exp());
    if !linear_predictor.is_finite() || !contribution.is_finite() {
        return Err(BermanTurnerError::Numerical(
            "Berman-Turner node objective is non-finite".into(),
        ));
    }
    Ok(BermanTurnerNode {
        node_id,
        node_kind,
        ix,
        iy,
        x_um,
        y_um,
        covariate,
        offset,
        weight_um2,
        pseudo_response,
        linear_predictor,
        weighted_objective_contribution: contribution,
    })
}

fn compensated_sum(values: impl Iterator<Item = f64>) -> Result<f64, BermanTurnerError> {
    let mut sum = 0.0;
    let mut correction = 0.0;
    for value in values {
        let next = sum + value;
        if sum.abs() >= value.abs() {
            correction += (sum - next) + value;
        } else {
            correction += (value - next) + sum;
        }
        sum = next;
        if !sum.is_finite() || !correction.is_finite() {
            return Err(BermanTurnerError::Numerical(
                "Berman-Turner accumulation is non-finite".into(),
            ));
        }
    }
    let result = sum + correction;
    if result.is_finite() {
        Ok(result)
    } else {
        Err(BermanTurnerError::Numerical(
            "Berman-Turner accumulation is non-finite".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_intensity_refinement_matches_exact_objective() {
        let window = RectangularWindow {
            xmin_um: 0.0,
            ymin_um: 0.0,
            xmax_um: 2.0,
            ymax_um: 1.0,
        };
        let events = vec![
            InhomogeneousPoissonEvent {
                event_id: "a".into(),
                x_um: 0.25,
                y_um: 0.25,
                covariate: 0.0,
                offset: 0.0,
            },
            InhomogeneousPoissonEvent {
                event_id: "b".into(),
                x_um: 1.75,
                y_um: 0.75,
                covariate: 0.0,
                offset: 0.0,
            },
        ];
        let nodes = |grid_x, grid_y| {
            (0..grid_y)
                .flat_map(|iy| {
                    (0..grid_x).map(move |ix| MidpointQuadratureValue {
                        ix,
                        iy,
                        covariate: 0.0,
                        offset: 0.0,
                    })
                })
                .collect()
        };
        let result = berman_turner_refinement(BermanTurnerRefinementSpec {
            window,
            events,
            coarse_grid_x: 2,
            coarse_grid_y: 1,
            coarse_quadrature: nodes(2, 1),
            fine_grid_x: 4,
            fine_grid_y: 2,
            fine_quadrature: nodes(4, 2),
            intercept: std::f64::consts::LN_2,
            coefficient: 0.0,
            convergence_tolerance: 1e-12,
        })
        .unwrap();
        let exact = 2.0 * std::f64::consts::LN_2 - 4.0;
        assert!((result.coarse.weighted_objective - exact).abs() <= 1e-12);
        assert!((result.fine.weighted_objective - exact).abs() <= 1e-12);
        assert_eq!(result.fine.table.len(), 10);
        assert!(result.converged);
    }
}
