use serde::Serialize;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Serialize)]
pub struct RectangularWindow {
    pub xmin_um: f64,
    pub ymin_um: f64,
    pub xmax_um: f64,
    pub ymax_um: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct InhomogeneousPoissonEvent {
    pub event_id: String,
    pub x_um: f64,
    pub y_um: f64,
    pub covariate: f64,
    pub offset: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct MidpointQuadratureValue {
    pub ix: u32,
    pub iy: u32,
    pub covariate: f64,
    pub offset: f64,
}

#[derive(Clone, Debug)]
pub struct InhomogeneousPoissonSpec {
    pub window: RectangularWindow,
    pub grid_x: u32,
    pub grid_y: u32,
    pub events: Vec<InhomogeneousPoissonEvent>,
    pub quadrature: Vec<MidpointQuadratureValue>,
    pub intercept: f64,
    pub coefficient: f64,
}

#[derive(Clone, Debug)]
pub struct InhomogeneousPoissonLikelihoodResult {
    pub event_count: usize,
    pub quadrature_node_count: usize,
    pub window_area_um2: f64,
    pub cell_width_um: f64,
    pub cell_height_um: f64,
    pub cell_area_um2: f64,
    pub event_term: f64,
    pub integral_term: f64,
    pub log_likelihood: f64,
}

#[derive(Debug, Error)]
pub enum InhomogeneousPoissonError {
    #[error("invalid inhomogeneous Poisson input: {0}")]
    InvalidInput(String),
    #[error("inhomogeneous Poisson numerical failure: {0}")]
    Numerical(String),
}

pub fn inhomogeneous_poisson_log_likelihood(
    mut spec: InhomogeneousPoissonSpec,
) -> Result<InhomogeneousPoissonLikelihoodResult, InhomogeneousPoissonError> {
    validate_and_canonicalize(&mut spec)?;
    let width = spec.window.xmax_um - spec.window.xmin_um;
    let height = spec.window.ymax_um - spec.window.ymin_um;
    let window_area = width * height;
    let cell_width = width / f64::from(spec.grid_x);
    let cell_height = height / f64::from(spec.grid_y);
    let cell_area = cell_width * cell_height;
    if !window_area.is_finite()
        || !cell_width.is_finite()
        || !cell_height.is_finite()
        || !cell_area.is_finite()
        || cell_area <= 0.0
    {
        return Err(InhomogeneousPoissonError::Numerical(
            "rectangle or cell area is non-finite".into(),
        ));
    }
    let event_term = stable_sum(
        spec.events
            .iter()
            .map(|event| spec.intercept + spec.coefficient * event.covariate + event.offset),
    )?;
    let integral_term = stable_sum(spec.quadrature.iter().map(|node| {
        let linear_predictor = spec.intercept + spec.coefficient * node.covariate + node.offset;
        cell_area * linear_predictor.exp()
    }))?;
    let log_likelihood = event_term - integral_term;
    if !log_likelihood.is_finite() {
        return Err(InhomogeneousPoissonError::Numerical(
            "likelihood difference is non-finite".into(),
        ));
    }
    Ok(InhomogeneousPoissonLikelihoodResult {
        event_count: spec.events.len(),
        quadrature_node_count: spec.quadrature.len(),
        window_area_um2: window_area,
        cell_width_um: cell_width,
        cell_height_um: cell_height,
        cell_area_um2: cell_area,
        event_term,
        integral_term,
        log_likelihood,
    })
}

pub(crate) fn validate_and_canonicalize(
    spec: &mut InhomogeneousPoissonSpec,
) -> Result<(), InhomogeneousPoissonError> {
    let window = spec.window;
    if ![
        window.xmin_um,
        window.ymin_um,
        window.xmax_um,
        window.ymax_um,
        spec.intercept,
        spec.coefficient,
    ]
    .into_iter()
    .all(f64::is_finite)
        || window.xmin_um >= window.xmax_um
        || window.ymin_um >= window.ymax_um
        || !(1..=1_024).contains(&spec.grid_x)
        || !(1..=1_024).contains(&spec.grid_y)
    {
        return Err(InhomogeneousPoissonError::InvalidInput(
            "rectangle, grid, intercept, or coefficient is invalid".into(),
        ));
    }
    let node_count = usize::try_from(spec.grid_x.checked_mul(spec.grid_y).ok_or_else(|| {
        InhomogeneousPoissonError::InvalidInput("quadrature grid size overflows".into())
    })?)
    .map_err(|_| {
        InhomogeneousPoissonError::InvalidInput("quadrature grid does not fit memory bounds".into())
    })?;
    if node_count > 1_000_000 || spec.quadrature.len() != node_count {
        return Err(InhomogeneousPoissonError::InvalidInput(
            "quadrature must contain every grid cell exactly once within 1000000 nodes".into(),
        ));
    }
    if !(1..=100_000).contains(&spec.events.len()) {
        return Err(InhomogeneousPoissonError::InvalidInput(
            "inhomogeneous Poisson likelihood requires 1-100000 events".into(),
        ));
    }
    spec.events
        .sort_by(|left, right| left.event_id.cmp(&right.event_id));
    for (index, event) in spec.events.iter().enumerate() {
        if event.event_id.is_empty()
            || event.event_id.trim() != event.event_id
            || (index > 0 && spec.events[index - 1].event_id == event.event_id)
            || ![event.x_um, event.y_um, event.covariate, event.offset]
                .into_iter()
                .all(f64::is_finite)
            || event.x_um < window.xmin_um
            || event.x_um >= window.xmax_um
            || event.y_um < window.ymin_um
            || event.y_um >= window.ymax_um
        {
            return Err(InhomogeneousPoissonError::InvalidInput(
                "events require unique exact IDs, finite values, and half-open window membership"
                    .into(),
            ));
        }
    }
    spec.quadrature.sort_by_key(|node| (node.iy, node.ix));
    for (index, node) in spec.quadrature.iter().enumerate() {
        let expected_ix = index as u32 % spec.grid_x;
        let expected_iy = index as u32 / spec.grid_x;
        if node.ix != expected_ix
            || node.iy != expected_iy
            || !node.covariate.is_finite()
            || !node.offset.is_finite()
        {
            return Err(InhomogeneousPoissonError::InvalidInput(
                "quadrature requires one finite value for every zero-based cell".into(),
            ));
        }
    }
    Ok(())
}

fn stable_sum(values: impl Iterator<Item = f64>) -> Result<f64, InhomogeneousPoissonError> {
    let mut sum = 0.0;
    let mut correction = 0.0;
    for value in values {
        if !value.is_finite() {
            return Err(InhomogeneousPoissonError::Numerical(
                "linear predictor or integrated intensity is non-finite".into(),
            ));
        }
        let next = sum + value;
        if sum.abs() >= value.abs() {
            correction += (sum - next) + value;
        } else {
            correction += (value - next) + sum;
        }
        sum = next;
        if !sum.is_finite() || !correction.is_finite() {
            return Err(InhomogeneousPoissonError::Numerical(
                "likelihood accumulation is non-finite".into(),
            ));
        }
    }
    let result = sum + correction;
    if result.is_finite() {
        Ok(result)
    } else {
        Err(InhomogeneousPoissonError::Numerical(
            "likelihood accumulation is non-finite".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn constant_spec() -> InhomogeneousPoissonSpec {
        InhomogeneousPoissonSpec {
            window: RectangularWindow {
                xmin_um: 0.0,
                ymin_um: 0.0,
                xmax_um: 2.0,
                ymax_um: 1.0,
            },
            grid_x: 2,
            grid_y: 1,
            events: vec![
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
            ],
            quadrature: vec![
                MidpointQuadratureValue {
                    ix: 0,
                    iy: 0,
                    covariate: 0.0,
                    offset: 0.0,
                },
                MidpointQuadratureValue {
                    ix: 1,
                    iy: 0,
                    covariate: 0.0,
                    offset: 0.0,
                },
            ],
            intercept: std::f64::consts::LN_2,
            coefficient: 0.0,
        }
    }

    #[test]
    fn constant_intensity_matches_rectangle_oracle() {
        let result = inhomogeneous_poisson_log_likelihood(constant_spec()).unwrap();
        assert!((result.event_term - 2.0 * std::f64::consts::LN_2).abs() <= 1e-12);
        assert!((result.integral_term - 4.0).abs() <= 1e-12);
        assert!((result.log_likelihood - (2.0 * std::f64::consts::LN_2 - 4.0)).abs() <= 1e-12);
    }

    #[test]
    fn incomplete_quadrature_is_rejected() {
        let mut spec = constant_spec();
        spec.quadrature.pop();
        assert!(matches!(
            inhomogeneous_poisson_log_likelihood(spec),
            Err(InhomogeneousPoissonError::InvalidInput(_))
        ));
    }
}
