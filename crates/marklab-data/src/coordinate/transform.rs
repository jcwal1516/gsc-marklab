use marklab_core::{CoordinateFrameId, TransformId, UncertaintyId};

use super::{CoordinateError, SpatialDimension};

#[derive(Clone, Debug, PartialEq)]
enum AffineCoefficients {
    Two([f64; 6]),
    Three([f64; 12]),
}

/// A finite same-dimensional affine transform matrix.
///
/// Coefficients are row-major in a 2-by-3 or 3-by-4 matrix. Matrix rows follow
/// the target frame's axis order, non-translation columns follow the source
/// frame's axis order, and translation values use the target frame's unit.
#[derive(Clone, Debug, PartialEq)]
pub struct TransformMatrix(AffineCoefficients);

impl TransformMatrix {
    /// Validate a row-major two-dimensional affine matrix.
    pub fn affine_2d(coefficients: [f64; 6]) -> Result<Self, CoordinateError> {
        validate_coefficients(&coefficients)?;
        Ok(Self(AffineCoefficients::Two(coefficients)))
    }

    /// Validate a row-major three-dimensional affine matrix.
    pub fn affine_3d(coefficients: [f64; 12]) -> Result<Self, CoordinateError> {
        validate_coefficients(&coefficients)?;
        Ok(Self(AffineCoefficients::Three(coefficients)))
    }

    /// Construct a two-dimensional identity matrix.
    pub fn identity_2d() -> Self {
        Self(AffineCoefficients::Two([1.0, 0.0, 0.0, 0.0, 1.0, 0.0]))
    }

    /// Construct a three-dimensional identity matrix.
    pub fn identity_3d() -> Self {
        Self(AffineCoefficients::Three([
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0,
        ]))
    }

    /// Matrix input and output dimension.
    pub fn dimension(&self) -> SpatialDimension {
        match &self.0 {
            AffineCoefficients::Two(_) => SpatialDimension::Two,
            AffineCoefficients::Three(_) => SpatialDimension::Three,
        }
    }

    /// Row-major affine coefficients.
    pub fn coefficients(&self) -> &[f64] {
        match &self.0 {
            AffineCoefficients::Two(coefficients) => coefficients,
            AffineCoefficients::Three(coefficients) => coefficients,
        }
    }

    pub(crate) fn apply_2d(&self, values: [f64; 2]) -> Option<[f64; 2]> {
        let AffineCoefficients::Two(matrix) = &self.0 else {
            return None;
        };
        Some([
            matrix[0] * values[0] + matrix[1] * values[1] + matrix[2],
            matrix[3] * values[0] + matrix[4] * values[1] + matrix[5],
        ])
    }

    pub(crate) fn apply_3d(&self, values: [f64; 3]) -> Option<[f64; 3]> {
        let AffineCoefficients::Three(matrix) = &self.0 else {
            return None;
        };
        Some([
            matrix[0] * values[0] + matrix[1] * values[1] + matrix[2] * values[2] + matrix[3],
            matrix[4] * values[0] + matrix[5] * values[1] + matrix[6] * values[2] + matrix[7],
            matrix[8] * values[0] + matrix[9] * values[1] + matrix[10] * values[2] + matrix[11],
        ])
    }
}

/// One directed, explicitly named transform between registered frames.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameTransform {
    id: TransformId,
    source: CoordinateFrameId,
    target: CoordinateFrameId,
    matrix: TransformMatrix,
    uncertainty: Option<UncertaintyId>,
}

impl FrameTransform {
    /// Declare a directed transform for later registry validation.
    pub fn new(
        id: TransformId,
        source: CoordinateFrameId,
        target: CoordinateFrameId,
        matrix: TransformMatrix,
        uncertainty: Option<UncertaintyId>,
    ) -> Self {
        Self {
            id,
            source,
            target,
            matrix,
            uncertainty,
        }
    }

    /// Transform identity.
    pub fn id(&self) -> &TransformId {
        &self.id
    }

    /// Source-frame identity.
    pub fn source(&self) -> &CoordinateFrameId {
        &self.source
    }

    /// Target-frame identity.
    pub fn target(&self) -> &CoordinateFrameId {
        &self.target
    }

    /// Same-dimensional affine matrix.
    pub fn matrix(&self) -> &TransformMatrix {
        &self.matrix
    }

    /// Optional registration uncertainty expressed in the target frame.
    pub fn uncertainty(&self) -> Option<&UncertaintyId> {
        self.uncertainty.as_ref()
    }
}

/// A named uncertainty artifact and optional conservative scalar radius.
#[derive(Clone, Debug, PartialEq)]
pub struct UncertaintyReference {
    id: UncertaintyId,
    frame: CoordinateFrameId,
    conservative_radius: Option<f64>,
}

impl UncertaintyReference {
    /// Validate an uncertainty reference without treating absence as zero.
    pub fn new(
        id: UncertaintyId,
        frame: CoordinateFrameId,
        conservative_radius: Option<f64>,
    ) -> Result<Self, CoordinateError> {
        if conservative_radius.is_some_and(|radius| !radius.is_finite() || radius < 0.0) {
            return Err(CoordinateError::InvalidUncertaintyRadius { uncertainty: id });
        }
        Ok(Self {
            id,
            frame,
            conservative_radius,
        })
    }

    /// Uncertainty identity.
    pub fn id(&self) -> &UncertaintyId {
        &self.id
    }

    /// Frame in which the uncertainty is expressed.
    pub fn frame(&self) -> &CoordinateFrameId {
        &self.frame
    }

    /// Conservative scalar radius, when the artifact supplies one.
    pub fn conservative_radius(&self) -> Option<f64> {
        self.conservative_radius
    }
}

fn validate_coefficients(coefficients: &[f64]) -> Result<(), CoordinateError> {
    if let Some(index) = coefficients
        .iter()
        .position(|coefficient| !coefficient.is_finite())
    {
        return Err(CoordinateError::NonFiniteTransformCoefficient { index });
    }
    Ok(())
}
