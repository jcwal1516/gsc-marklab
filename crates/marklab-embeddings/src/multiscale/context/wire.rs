use marklab_data::{CohortHierarchy, CoordinateFrameId, CoordinateRegistry, SlideId, TransformId};
use serde::{Deserialize, Serialize};

use crate::{
    context::image_coordinate_convention_name as convention_name, PatchBoundaryPolicy,
    PositiveRational,
};

use super::{EffectiveReceptiveField, PatchEmbeddingContext, CONTEXT_FORMAT, CONTEXT_VERSION};
use crate::multiscale::error::MultiscaleEmbeddingError;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WireContext {
    format: String,
    version: u32,
    owning_slide_id: String,
    image_frame_id: String,
    image_axes: [String; 2],
    image_space: String,
    image_unit: String,
    pixel_convention: String,
    physical_frame_id: String,
    physical_axes: [String; 2],
    physical_space: String,
    physical_unit: String,
    transform_id: String,
    transform_source_frame_id: String,
    transform_target_frame_id: String,
    transform_uncertainty: Option<()>,
    transform_matrix_bits: [String; 6],
    mpp_x: WireRational,
    mpp_y: WireRational,
    source_image_px: [u64; 2],
    patch_px: [u32; 2],
    stride_px: [u32; 2],
    overlap_px: [u32; 2],
    effective_receptive_field: WireReceptiveField,
    boundary_policy: WireBoundaryPolicy,
}

impl From<&PatchEmbeddingContext> for WireContext {
    fn from(context: &PatchEmbeddingContext) -> Self {
        Self {
            format: CONTEXT_FORMAT.to_owned(),
            version: CONTEXT_VERSION,
            owning_slide_id: context.owning_slide_id.as_str().to_owned(),
            image_frame_id: context.image_frame_id.as_str().to_owned(),
            image_axes: ["x".to_owned(), "y".to_owned()],
            image_space: "image".to_owned(),
            image_unit: "pixel".to_owned(),
            pixel_convention: convention_name(context.pixel_convention).to_owned(),
            physical_frame_id: context.physical_frame_id.as_str().to_owned(),
            physical_axes: ["x".to_owned(), "y".to_owned()],
            physical_space: "physical".to_owned(),
            physical_unit: "micrometer".to_owned(),
            transform_id: context.transform_id.as_str().to_owned(),
            transform_source_frame_id: context.image_frame_id.as_str().to_owned(),
            transform_target_frame_id: context.physical_frame_id.as_str().to_owned(),
            transform_uncertainty: None,
            transform_matrix_bits: context.matrix_bits.map(|bits| format!("{bits:016x}")),
            mpp_x: WireRational::from(context.mpp_x),
            mpp_y: WireRational::from(context.mpp_y),
            source_image_px: context.source_image_px,
            patch_px: context.patch_px,
            stride_px: context.stride_px,
            overlap_px: context.overlap_px,
            effective_receptive_field: WireReceptiveField::from(context.effective_receptive_field),
            boundary_policy: WireBoundaryPolicy::from(context.boundary_policy),
        }
    }
}

impl WireContext {
    pub(super) fn into_context(
        self,
        hierarchy: &CohortHierarchy,
        registry: &CoordinateRegistry,
        maximum_retained_bytes: usize,
    ) -> Result<PatchEmbeddingContext, MultiscaleEmbeddingError> {
        if self.format != CONTEXT_FORMAT
            || self.version != CONTEXT_VERSION
            || self.image_axes != ["x", "y"]
            || self.image_space != "image"
            || self.image_unit != "pixel"
            || self.physical_axes != ["x", "y"]
            || self.physical_space != "physical"
            || self.physical_unit != "micrometer"
            || self.transform_uncertainty.is_some()
        {
            return Err(MultiscaleEmbeddingError::InvalidCanonicalJson);
        }
        let parsed_bits = self
            .transform_matrix_bits
            .iter()
            .map(|value| parse_bits(value))
            .collect::<Result<Vec<_>, _>>()?;
        let image_frame_id = CoordinateFrameId::new(self.image_frame_id)
            .map_err(|_| MultiscaleEmbeddingError::InvalidCanonicalJson)?;
        let physical_frame_id = CoordinateFrameId::new(self.physical_frame_id)
            .map_err(|_| MultiscaleEmbeddingError::InvalidCanonicalJson)?;
        if self.transform_source_frame_id != image_frame_id.as_str()
            || self.transform_target_frame_id != physical_frame_id.as_str()
        {
            return Err(MultiscaleEmbeddingError::InvalidCanonicalJson);
        }
        let context = PatchEmbeddingContext::new(
            hierarchy,
            registry,
            SlideId::new(self.owning_slide_id)
                .map_err(|_| MultiscaleEmbeddingError::InvalidCanonicalJson)?,
            image_frame_id,
            physical_frame_id,
            TransformId::new(self.transform_id)
                .map_err(|_| MultiscaleEmbeddingError::InvalidCanonicalJson)?,
            self.mpp_x.into_rational()?,
            self.mpp_y.into_rational()?,
            self.source_image_px,
            self.patch_px,
            self.stride_px,
            self.overlap_px,
            self.effective_receptive_field.into_domain()?,
            self.boundary_policy.into_domain(),
            maximum_retained_bytes,
        )?;
        if self.pixel_convention != convention_name(context.pixel_convention)
            || parsed_bits.as_slice() != context.matrix_bits
        {
            return Err(MultiscaleEmbeddingError::InvalidCanonicalJson);
        }
        Ok(context)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireRational {
    numerator: u64,
    denominator: u64,
}

impl From<PositiveRational> for WireRational {
    fn from(value: PositiveRational) -> Self {
        Self {
            numerator: value.numerator(),
            denominator: value.denominator(),
        }
    }
}

impl WireRational {
    fn into_rational(self) -> Result<PositiveRational, MultiscaleEmbeddingError> {
        PositiveRational::new(self.numerator, self.denominator)
            .map_err(|_| MultiscaleEmbeddingError::InvalidCanonicalJson)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum WireReceptiveField {
    FullInput,
    CenteredRectangle {
        width_px: WireRational,
        height_px: WireRational,
    },
}

impl From<EffectiveReceptiveField> for WireReceptiveField {
    fn from(value: EffectiveReceptiveField) -> Self {
        match value {
            EffectiveReceptiveField::FullInput => Self::FullInput,
            EffectiveReceptiveField::CenteredRectangle {
                width_px,
                height_px,
            } => Self::CenteredRectangle {
                width_px: WireRational::from(width_px),
                height_px: WireRational::from(height_px),
            },
        }
    }
}

impl WireReceptiveField {
    fn into_domain(self) -> Result<EffectiveReceptiveField, MultiscaleEmbeddingError> {
        match self {
            Self::FullInput => Ok(EffectiveReceptiveField::FullInput),
            Self::CenteredRectangle {
                width_px,
                height_px,
            } => Ok(EffectiveReceptiveField::CenteredRectangle {
                width_px: width_px.into_rational()?,
                height_px: height_px.into_rational()?,
            }),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum WireBoundaryPolicy {
    FullyContainedOnly,
    Reflect,
    ConstantRgb { rgb: [u8; 3] },
}

impl From<PatchBoundaryPolicy> for WireBoundaryPolicy {
    fn from(value: PatchBoundaryPolicy) -> Self {
        match value {
            PatchBoundaryPolicy::FullyContainedOnly => Self::FullyContainedOnly,
            PatchBoundaryPolicy::Reflect => Self::Reflect,
            PatchBoundaryPolicy::ConstantRgb(rgb) => Self::ConstantRgb { rgb },
        }
    }
}

impl WireBoundaryPolicy {
    fn into_domain(self) -> PatchBoundaryPolicy {
        match self {
            Self::FullyContainedOnly => PatchBoundaryPolicy::FullyContainedOnly,
            Self::Reflect => PatchBoundaryPolicy::Reflect,
            Self::ConstantRgb { rgb } => PatchBoundaryPolicy::ConstantRgb(rgb),
        }
    }
}

fn parse_bits(value: &str) -> Result<u64, MultiscaleEmbeddingError> {
    if value.len() != 16
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(MultiscaleEmbeddingError::InvalidCanonicalJson);
    }
    u64::from_str_radix(value, 16).map_err(|_| MultiscaleEmbeddingError::InvalidCanonicalJson)
}
