use marklab_data::{CoordinateFrameId, CoordinateRegistry, ImageCoordinateConvention, TransformId};
use serde::{Deserialize, Serialize};

use super::{EmbeddingSpatialContext, PatchBoundaryPolicy, PositiveRational};
use crate::EmbeddingError;

const CONTEXT_FORMAT: &str = "marklab.cell_embedding_spatial_context";
const CONTEXT_VERSION: u32 = 1;
const MAX_CONTEXT_BYTES: usize = 16 * 1024;

impl EmbeddingSpatialContext {
    /// Encode deterministic strict JSON with one final newline.
    pub fn to_canonical_json(&self) -> Result<Vec<u8>, EmbeddingError> {
        let wire = WireContext::from(self);
        let mut bytes =
            serde_json::to_vec(&wire).map_err(|_| EmbeddingError::InvalidCanonicalJson)?;
        bytes.push(b'\n');
        if bytes.len() > MAX_CONTEXT_BYTES {
            return Err(EmbeddingError::EncodedByteBudgetExceeded {
                observed: bytes.len(),
                maximum: MAX_CONTEXT_BYTES,
            });
        }
        Ok(bytes)
    }

    /// Decode canonical JSON and require exact equality with registry values.
    pub fn from_canonical_json(
        bytes: &[u8],
        registry: &CoordinateRegistry,
    ) -> Result<Self, EmbeddingError> {
        if bytes.len() > MAX_CONTEXT_BYTES {
            return Err(EmbeddingError::EncodedByteBudgetExceeded {
                observed: bytes.len(),
                maximum: MAX_CONTEXT_BYTES,
            });
        }
        let wire: WireContext =
            serde_json::from_slice(bytes).map_err(|_| EmbeddingError::InvalidCanonicalJson)?;
        let context = wire.into_context(registry)?;
        if context.to_canonical_json()?.as_slice() != bytes {
            return Err(EmbeddingError::InvalidCanonicalJson);
        }
        Ok(context)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireContext {
    format: String,
    version: u32,
    image_frame_id: String,
    image_axes: [String; 2],
    image_space: String,
    image_unit: String,
    pixel_convention: String,
    physical_frame_id: String,
    physical_axes: [String; 2],
    physical_space: String,
    physical_unit: String,
    pixel_to_physical_transform_id: String,
    transform_uncertainty: WireNone,
    pixel_to_physical_matrix_f64_bits: [String; 6],
    mpp_x: WireRational,
    mpp_y: WireRational,
    patch_width_px: u32,
    patch_height_px: u32,
    overlap_x_px: u32,
    overlap_y_px: u32,
    stride_x_px: u32,
    stride_y_px: u32,
    token_patch_width_px: u32,
    token_patch_height_px: u32,
    patch_boundary_policy: WireBoundary,
    effective_context: String,
}

impl From<&EmbeddingSpatialContext> for WireContext {
    fn from(context: &EmbeddingSpatialContext) -> Self {
        Self {
            format: CONTEXT_FORMAT.to_owned(),
            version: CONTEXT_VERSION,
            image_frame_id: context.image_frame_id.as_str().to_owned(),
            image_axes: ["x".to_owned(), "y".to_owned()],
            image_space: "image".to_owned(),
            image_unit: "pixel".to_owned(),
            pixel_convention: convention_name(context.pixel_convention).to_owned(),
            physical_frame_id: context.physical_frame_id.as_str().to_owned(),
            physical_axes: ["x".to_owned(), "y".to_owned()],
            physical_space: "physical".to_owned(),
            physical_unit: "micrometer".to_owned(),
            pixel_to_physical_transform_id: context.transform_id.as_str().to_owned(),
            transform_uncertainty: WireNone {
                kind: "none".to_owned(),
            },
            pixel_to_physical_matrix_f64_bits: context
                .matrix_bits
                .map(|bits| format!("{bits:016x}")),
            mpp_x: WireRational::from(context.mpp_x),
            mpp_y: WireRational::from(context.mpp_y),
            patch_width_px: context.patch_px[0],
            patch_height_px: context.patch_px[1],
            overlap_x_px: context.overlap_px[0],
            overlap_y_px: context.overlap_px[1],
            stride_x_px: context.stride_px[0],
            stride_y_px: context.stride_px[1],
            token_patch_width_px: context.token_patch_px[0],
            token_patch_height_px: context.token_patch_px[1],
            patch_boundary_policy: WireBoundary::from(context.boundary_policy),
            effective_context: "nucleus_bbox_intersecting_encoder_tokens_within_patch".to_owned(),
        }
    }
}

impl WireContext {
    fn into_context(
        self,
        registry: &CoordinateRegistry,
    ) -> Result<EmbeddingSpatialContext, EmbeddingError> {
        if self.format != CONTEXT_FORMAT
            || self.version != CONTEXT_VERSION
            || self.image_axes != ["x", "y"]
            || self.image_space != "image"
            || self.image_unit != "pixel"
            || self.physical_axes != ["x", "y"]
            || self.physical_space != "physical"
            || self.physical_unit != "micrometer"
            || self.transform_uncertainty.kind != "none"
            || self.effective_context != "nucleus_bbox_intersecting_encoder_tokens_within_patch"
        {
            return Err(EmbeddingError::InvalidCanonicalJson);
        }
        let convention = parse_convention(&self.pixel_convention)?;
        let image = CoordinateFrameId::new(self.image_frame_id)
            .map_err(|_| EmbeddingError::InvalidCanonicalJson)?;
        let physical = CoordinateFrameId::new(self.physical_frame_id)
            .map_err(|_| EmbeddingError::InvalidCanonicalJson)?;
        let transform = TransformId::new(self.pixel_to_physical_transform_id)
            .map_err(|_| EmbeddingError::InvalidCanonicalJson)?;
        let parsed_bits = self
            .pixel_to_physical_matrix_f64_bits
            .map(|value| parse_bits(&value))
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;
        let context = EmbeddingSpatialContext::new(
            registry,
            image,
            physical,
            transform,
            self.mpp_x.into_rational()?,
            self.mpp_y.into_rational()?,
            [self.patch_width_px, self.patch_height_px],
            [self.overlap_x_px, self.overlap_y_px],
            [self.token_patch_width_px, self.token_patch_height_px],
            self.patch_boundary_policy.into_policy(),
        )?;
        if convention != context.pixel_convention
            || self.stride_x_px != context.stride_px[0]
            || self.stride_y_px != context.stride_px[1]
            || parsed_bits.as_slice() != context.matrix_bits
        {
            return Err(EmbeddingError::InvalidCanonicalJson);
        }
        Ok(context)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireNone {
    kind: String,
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
            numerator: value.numerator,
            denominator: value.denominator,
        }
    }
}

impl WireRational {
    fn into_rational(self) -> Result<PositiveRational, EmbeddingError> {
        PositiveRational::new(self.numerator, self.denominator)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum WireBoundary {
    FullyContainedOnly,
    Reflect,
    ConstantRgb { value: [u8; 3] },
}

impl From<PatchBoundaryPolicy> for WireBoundary {
    fn from(value: PatchBoundaryPolicy) -> Self {
        match value {
            PatchBoundaryPolicy::FullyContainedOnly => Self::FullyContainedOnly,
            PatchBoundaryPolicy::Reflect => Self::Reflect,
            PatchBoundaryPolicy::ConstantRgb(value) => Self::ConstantRgb { value },
        }
    }
}

impl WireBoundary {
    fn into_policy(self) -> PatchBoundaryPolicy {
        match self {
            Self::FullyContainedOnly => PatchBoundaryPolicy::FullyContainedOnly,
            Self::Reflect => PatchBoundaryPolicy::Reflect,
            Self::ConstantRgb { value } => PatchBoundaryPolicy::ConstantRgb(value),
        }
    }
}

fn convention_name(value: ImageCoordinateConvention) -> &'static str {
    match value {
        ImageCoordinateConvention::PixelCenterAtInteger => "pixel_center_at_integer",
        ImageCoordinateConvention::PixelCornerAtInteger => "pixel_corner_at_integer",
    }
}

fn parse_convention(value: &str) -> Result<ImageCoordinateConvention, EmbeddingError> {
    match value {
        "pixel_center_at_integer" => Ok(ImageCoordinateConvention::PixelCenterAtInteger),
        "pixel_corner_at_integer" => Ok(ImageCoordinateConvention::PixelCornerAtInteger),
        _ => Err(EmbeddingError::InvalidCanonicalJson),
    }
}

fn parse_bits(value: &str) -> Result<u64, EmbeddingError> {
    if value.len() != 16
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(EmbeddingError::InvalidCanonicalJson);
    }
    u64::from_str_radix(value, 16).map_err(|_| EmbeddingError::InvalidCanonicalJson)
}
