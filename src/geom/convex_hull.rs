use crate::errors::{MarklabError, Result};

const ORIENTATION_TOLERANCE: f64 = 1.0e-12;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Point2 {
    pub x: f64,
    pub y: f64,
}

impl Point2 {
    pub(crate) const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ConvexHull2D {
    Polygon {
        vertices: Vec<Point2>,
        unique_points: usize,
    },
    InsufficientUniquePoints {
        unique_points: usize,
    },
    Collinear {
        unique_points: usize,
    },
}

impl ConvexHull2D {
    pub(crate) fn from_points(points: &[Point2]) -> Result<Self> {
        if points
            .iter()
            .any(|point| !point.x.is_finite() || !point.y.is_finite())
        {
            return Err(MarklabError::Validation(
                "convex-hull coordinates must be finite".into(),
            ));
        }

        let mut points = points.to_vec();
        points.sort_by(|left, right| {
            left.x
                .total_cmp(&right.x)
                .then_with(|| left.y.total_cmp(&right.y))
        });
        points.dedup_by(|left, right| left.x == right.x && left.y == right.y);
        let unique_points = points.len();
        if unique_points < 3 {
            return Ok(Self::InsufficientUniquePoints { unique_points });
        }

        let mut lower = Vec::new();
        for point in &points {
            while lower.len() >= 2
                && orientation(lower[lower.len() - 2], lower[lower.len() - 1], *point)
                    <= ORIENTATION_TOLERANCE
            {
                lower.pop();
            }
            lower.push(*point);
        }
        let mut upper = Vec::new();
        for point in points.iter().rev() {
            while upper.len() >= 2
                && orientation(upper[upper.len() - 2], upper[upper.len() - 1], *point)
                    <= ORIENTATION_TOLERANCE
            {
                upper.pop();
            }
            upper.push(*point);
        }
        lower.pop();
        upper.pop();
        lower.extend(upper);
        if lower.len() < 3 {
            Ok(Self::Collinear { unique_points })
        } else {
            Ok(Self::Polygon {
                vertices: lower,
                unique_points,
            })
        }
    }

    pub(crate) fn unique_points(&self) -> usize {
        match self {
            Self::Polygon { unique_points, .. } => *unique_points,
            Self::InsufficientUniquePoints { unique_points }
            | Self::Collinear { unique_points } => *unique_points,
        }
    }

    pub(crate) fn contains(&self, point: Point2) -> Option<bool> {
        let Self::Polygon { vertices, .. } = self else {
            return None;
        };
        Some((0..vertices.len()).all(|index| {
            orientation(
                vertices[index],
                vertices[(index + 1) % vertices.len()],
                point,
            ) >= -ORIENTATION_TOLERANCE
        }))
    }
}

fn orientation(origin: Point2, left: Point2, right: Point2) -> f64 {
    let left_dx = left.x - origin.x;
    let left_dy = left.y - origin.y;
    let right_dx = right.x - origin.x;
    let right_dy = right.y - origin.y;
    let scale = left_dx
        .abs()
        .max(left_dy.abs())
        .max(right_dx.abs())
        .max(right_dy.abs());
    if scale == 0.0 {
        0.0
    } else {
        (left_dx / scale) * (right_dy / scale) - (left_dy / scale) * (right_dx / scale)
    }
}
