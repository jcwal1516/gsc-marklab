use marklab_data::{
    CoordinateFrameId, CoordinateRegistry, CoordinateSpace, CoordinateUnit,
    ImageCoordinateConvention, SpatialAxis, TransformId,
};

use crate::{rational::greatest_common_divisor, EmbeddingError};

mod wire;

/// Reduced positive rational used for exact micrometres-per-pixel declarations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositiveRational {
    numerator: u64,
    denominator: u64,
}

impl PositiveRational {
    /// Require positive, nonzero, already reduced numerator and denominator.
    pub fn new(numerator: u64, denominator: u64) -> Result<Self, EmbeddingError> {
        if numerator == 0
            || denominator == 0
            || greatest_common_divisor(numerator, denominator) != 1
        {
            return Err(EmbeddingError::InvalidPositiveRational);
        }
        Ok(Self {
            numerator,
            denominator,
        })
    }

    /// Positive numerator.
    pub fn numerator(self) -> u64 {
        self.numerator
    }

    /// Positive denominator.
    pub fn denominator(self) -> u64 {
        self.denominator
    }

    fn as_f64(self) -> f64 {
        self.numerator as f64 / self.denominator as f64
    }
}

/// Closed policy for image patches that meet source-image boundaries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PatchBoundaryPolicy {
    /// Admit only patches fully contained in the source image.
    FullyContainedOnly,
    /// Reflect source pixels across the image boundary.
    Reflect,
    /// Pad outside pixels with one explicit RGB value.
    ConstantRgb([u8; 3]),
}

/// By-value image/physical frame, calibration, patch, and token context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmbeddingSpatialContext {
    image_frame_id: CoordinateFrameId,
    physical_frame_id: CoordinateFrameId,
    pixel_convention: ImageCoordinateConvention,
    transform_id: TransformId,
    matrix_bits: [u64; 6],
    mpp_x: PositiveRational,
    mpp_y: PositiveRational,
    patch_px: [u32; 2],
    overlap_px: [u32; 2],
    stride_px: [u32; 2],
    token_patch_px: [u32; 2],
    boundary_policy: PatchBoundaryPolicy,
}

impl EmbeddingSpatialContext {
    /// Validate and freeze exact registry values plus extraction-window semantics.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        registry: &CoordinateRegistry,
        image_frame_id: CoordinateFrameId,
        physical_frame_id: CoordinateFrameId,
        transform_id: TransformId,
        mpp_x: PositiveRational,
        mpp_y: PositiveRational,
        patch_px: [u32; 2],
        overlap_px: [u32; 2],
        token_patch_px: [u32; 2],
        boundary_policy: PatchBoundaryPolicy,
    ) -> Result<Self, EmbeddingError> {
        if patch_px.contains(&0) || token_patch_px.contains(&0) {
            return Err(EmbeddingError::ZeroDimension);
        }
        if overlap_px[0] >= patch_px[0] || overlap_px[1] >= patch_px[1] {
            return Err(EmbeddingError::InvalidSpatialContext);
        }
        let image = registry
            .frame(&image_frame_id)
            .ok_or(EmbeddingError::InvalidSpatialContext)?;
        let pixel_convention = match image.space() {
            CoordinateSpace::Image(convention)
                if image.axes() == [SpatialAxis::X, SpatialAxis::Y]
                    && image.unit() == CoordinateUnit::Pixel =>
            {
                convention
            }
            _ => return Err(EmbeddingError::InvalidSpatialContext),
        };
        let physical = registry
            .frame(&physical_frame_id)
            .ok_or(EmbeddingError::InvalidSpatialContext)?;
        if physical.axes() != [SpatialAxis::X, SpatialAxis::Y]
            || physical.unit() != CoordinateUnit::Micrometer
            || physical.space() != CoordinateSpace::Physical
        {
            return Err(EmbeddingError::InvalidSpatialContext);
        }
        let transform = registry
            .transform(&transform_id)
            .ok_or(EmbeddingError::InvalidSpatialContext)?;
        if transform.source() != &image_frame_id
            || transform.target() != &physical_frame_id
            || transform.uncertainty().is_some()
        {
            return Err(EmbeddingError::InvalidSpatialContext);
        }
        let coefficients = transform.matrix().coefficients();
        let coefficients: [f64; 6] = coefficients
            .try_into()
            .map_err(|_| EmbeddingError::InvalidSpatialContext)?;
        if coefficients[1].to_bits() != 0
            || coefficients[3].to_bits() != 0
            || coefficients[0] <= 0.0
            || coefficients[4] <= 0.0
            || coefficients[0].to_bits() != mpp_x.as_f64().to_bits()
            || coefficients[4].to_bits() != mpp_y.as_f64().to_bits()
        {
            return Err(EmbeddingError::InvalidSpatialContext);
        }
        let matrix_bits = coefficients.map(f64::to_bits);
        let stride_px = [patch_px[0] - overlap_px[0], patch_px[1] - overlap_px[1]];
        Ok(Self {
            image_frame_id,
            physical_frame_id,
            pixel_convention,
            transform_id,
            matrix_bits,
            mpp_x,
            mpp_y,
            patch_px,
            overlap_px,
            stride_px,
            token_patch_px,
            boundary_policy,
        })
    }

    /// Exact image-frame identity.
    pub fn image_frame_id(&self) -> &CoordinateFrameId {
        &self.image_frame_id
    }

    /// Exact physical target-frame identity.
    pub fn physical_frame_id(&self) -> &CoordinateFrameId {
        &self.physical_frame_id
    }

    /// Exact registered pixel-to-physical transform identity.
    pub fn transform_id(&self) -> &TransformId {
        &self.transform_id
    }

    /// Derived X/Y stride in pixels.
    pub fn stride_px(&self) -> [u32; 2] {
        self.stride_px
    }

    /// Closed patch-boundary policy.
    pub fn boundary_policy(&self) -> PatchBoundaryPolicy {
        self.boundary_policy
    }
}
