use std::{fmt, mem::size_of};

use marklab_data::{
    CohortHierarchy, CoordinateFrameId, CoordinateRegistry, CoordinateSpace, CoordinateUnit,
    HierarchyId, ImageCoordinateConvention, SlideId, SpatialAxis, TransformId,
};
use marklab_project::ContentDigest;

use crate::{PatchBoundaryPolicy, PositiveRational};

use super::{
    digest::LogicalDigest,
    error::MultiscaleEmbeddingError,
    json::{canonical_json_len, encode_canonical_json, matches_canonical_json},
};

mod wire;
use wire::{convention_name, WireContext};

const CONTEXT_FORMAT: &str = "marklab.patch_embedding_context";
const CONTEXT_VERSION: u32 = 1;
const CONTEXT_DOMAIN: &[u8] = b"marklab-patch-embedding-context-logical-v1";
const MAX_CONTEXT_BYTES: usize = 64 * 1024;

/// Closed effective receptive field within one input patch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectiveReceptiveField {
    /// The complete input patch contributes to the extracted vector.
    FullInput,
    /// A centered exact rectangle contributes to the extracted vector.
    CenteredRectangle {
        /// Exact receptive-field width in input pixels.
        width_px: PositiveRational,
        /// Exact receptive-field height in input pixels.
        height_px: PositiveRational,
    },
}

/// Exact per-slide, per-scale patch extraction context.
#[derive(Clone, Eq, PartialEq)]
pub struct PatchEmbeddingContext {
    owning_slide_id: SlideId,
    image_frame_id: CoordinateFrameId,
    physical_frame_id: CoordinateFrameId,
    pixel_convention: ImageCoordinateConvention,
    transform_id: TransformId,
    matrix_bits: [u64; 6],
    mpp_x: PositiveRational,
    mpp_y: PositiveRational,
    source_image_px: [u64; 2],
    patch_px: [u32; 2],
    stride_px: [u32; 2],
    overlap_px: [u32; 2],
    effective_receptive_field: EffectiveReceptiveField,
    boundary_policy: PatchBoundaryPolicy,
    logical_digest: ContentDigest,
    encoded_len: usize,
}

impl fmt::Debug for PatchEmbeddingContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PatchEmbeddingContext")
            .field("patch_px", &self.patch_px)
            .field("stride_px", &self.stride_px)
            .field("overlap_px", &self.overlap_px)
            .field("logical_digest", &self.logical_digest)
            .finish()
    }
}

impl PatchEmbeddingContext {
    /// Validate exact hierarchy, frames, calibration, patch grid, receptive field, and boundary.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        hierarchy: &CohortHierarchy,
        registry: &CoordinateRegistry,
        owning_slide_id: SlideId,
        image_frame_id: CoordinateFrameId,
        physical_frame_id: CoordinateFrameId,
        transform_id: TransformId,
        mpp_x: PositiveRational,
        mpp_y: PositiveRational,
        source_image_px: [u64; 2],
        patch_px: [u32; 2],
        stride_px: [u32; 2],
        overlap_px: [u32; 2],
        effective_receptive_field: EffectiveReceptiveField,
        boundary_policy: PatchBoundaryPolicy,
        maximum_retained_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        if !hierarchy.contains(&HierarchyId::from(owning_slide_id.clone())) {
            return Err(MultiscaleEmbeddingError::HierarchyOwnershipMismatch);
        }
        if source_image_px.contains(&0) || patch_px.contains(&0) || stride_px.contains(&0) {
            return Err(MultiscaleEmbeddingError::ZeroDimension);
        }
        let retained_bytes = context_retained_bytes(
            &owning_slide_id,
            &image_frame_id,
            &physical_frame_id,
            &transform_id,
        )?;
        if retained_bytes > maximum_retained_bytes {
            return Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded {
                required: retained_bytes,
                maximum: maximum_retained_bytes,
            });
        }
        for axis in 0..2 {
            if overlap_px[axis] >= patch_px[axis]
                || stride_px[axis]
                    .checked_add(overlap_px[axis])
                    .ok_or(MultiscaleEmbeddingError::SizeOverflow)?
                    != patch_px[axis]
            {
                return Err(MultiscaleEmbeddingError::InvalidPatchContext);
            }
        }
        validate_receptive_field(effective_receptive_field, patch_px)?;

        let image = registry
            .frame(&image_frame_id)
            .ok_or(MultiscaleEmbeddingError::InvalidPatchContext)?;
        let pixel_convention = match image.space() {
            CoordinateSpace::Image(convention)
                if image.axes() == [SpatialAxis::X, SpatialAxis::Y]
                    && image.unit() == CoordinateUnit::Pixel =>
            {
                convention
            }
            _ => return Err(MultiscaleEmbeddingError::InvalidPatchContext),
        };
        let physical = registry
            .frame(&physical_frame_id)
            .ok_or(MultiscaleEmbeddingError::InvalidPatchContext)?;
        if physical.axes() != [SpatialAxis::X, SpatialAxis::Y]
            || physical.space() != CoordinateSpace::Physical
            || physical.unit() != CoordinateUnit::Micrometer
        {
            return Err(MultiscaleEmbeddingError::InvalidPatchContext);
        }
        let transform = registry
            .transform(&transform_id)
            .ok_or(MultiscaleEmbeddingError::InvalidPatchContext)?;
        if transform.source() != &image_frame_id
            || transform.target() != &physical_frame_id
            || transform.uncertainty().is_some()
        {
            return Err(MultiscaleEmbeddingError::InvalidPatchContext);
        }
        let coefficients: [f64; 6] = transform
            .matrix()
            .coefficients()
            .try_into()
            .map_err(|_| MultiscaleEmbeddingError::InvalidPatchContext)?;
        let mpp_x_value = rational_as_f64(mpp_x);
        let mpp_y_value = rational_as_f64(mpp_y);
        if coefficients[1].to_bits() != 0
            || coefficients[3].to_bits() != 0
            || !coefficients[0].is_finite()
            || !coefficients[4].is_finite()
            || coefficients[0] <= 0.0
            || coefficients[4] <= 0.0
            || coefficients[0].to_bits() != mpp_x_value.to_bits()
            || coefficients[4].to_bits() != mpp_y_value.to_bits()
        {
            return Err(MultiscaleEmbeddingError::InvalidPatchContext);
        }
        let matrix_bits = [
            coefficients[0].to_bits(),
            0,
            canonical_f64_bits(coefficients[2]),
            0,
            coefficients[4].to_bits(),
            canonical_f64_bits(coefficients[5]),
        ];
        let logical_digest = context_digest(
            &owning_slide_id,
            &image_frame_id,
            pixel_convention,
            &physical_frame_id,
            &transform_id,
            matrix_bits,
            mpp_x,
            mpp_y,
            source_image_px,
            patch_px,
            stride_px,
            overlap_px,
            effective_receptive_field,
            boundary_policy,
        )?;
        let mut context = Self {
            owning_slide_id,
            image_frame_id,
            physical_frame_id,
            pixel_convention,
            transform_id,
            matrix_bits,
            mpp_x,
            mpp_y,
            source_image_px,
            patch_px,
            stride_px,
            overlap_px,
            effective_receptive_field,
            boundary_policy,
            logical_digest,
            encoded_len: 0,
        };
        let encoded_len = canonical_json_len(&WireContext::from(&context))?;
        if encoded_len > MAX_CONTEXT_BYTES {
            return Err(MultiscaleEmbeddingError::EncodedByteBudgetExceeded {
                observed: encoded_len,
                maximum: MAX_CONTEXT_BYTES,
            });
        }
        context.encoded_len = encoded_len;
        Ok(context)
    }

    /// Decode and revalidate the exact canonical JSON against hierarchy and registry values.
    pub fn from_canonical_json(
        bytes: &[u8],
        hierarchy: &CohortHierarchy,
        registry: &CoordinateRegistry,
        maximum_encoded_bytes: usize,
        maximum_decoded_bytes: usize,
        maximum_retained_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        let effective_maximum = maximum_encoded_bytes.min(MAX_CONTEXT_BYTES);
        if bytes.len() > effective_maximum {
            return Err(MultiscaleEmbeddingError::EncodedByteBudgetExceeded {
                observed: bytes.len(),
                maximum: effective_maximum,
            });
        }
        let decoded_required = size_of::<WireContext>()
            .checked_add(bytes.len())
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        if decoded_required > maximum_decoded_bytes {
            return Err(MultiscaleEmbeddingError::DecodedByteBudgetExceeded {
                required: decoded_required,
                maximum: maximum_decoded_bytes,
            });
        }
        let wire: WireContext = serde_json::from_slice(bytes)
            .map_err(|_| MultiscaleEmbeddingError::InvalidCanonicalJson)?;
        let context = wire.into_context(hierarchy, registry, maximum_retained_bytes)?;
        if !matches_canonical_json(&WireContext::from(&context), bytes) {
            return Err(MultiscaleEmbeddingError::InvalidCanonicalJson);
        }
        Ok(context)
    }

    /// Encode exact version-one canonical JSON with one final newline.
    pub fn to_canonical_json(&self) -> Result<Vec<u8>, MultiscaleEmbeddingError> {
        encode_canonical_json(
            &WireContext::from(self),
            self.encoded_len,
            MAX_CONTEXT_BYTES,
        )
    }

    /// Owning slide identity.
    pub fn owning_slide_id(&self) -> &SlideId {
        &self.owning_slide_id
    }

    /// Exact image coordinate frame containing patch origins and cell anchors.
    pub fn image_frame_id(&self) -> &CoordinateFrameId {
        &self.image_frame_id
    }

    /// Exact source-image pixel extent in X/Y order.
    pub fn source_image_px(&self) -> [u64; 2] {
        self.source_image_px
    }

    /// Exact input patch extent in X/Y order.
    pub fn patch_px(&self) -> [u32; 2] {
        self.patch_px
    }

    /// Exact extraction-grid stride in X/Y order.
    pub fn stride_px(&self) -> [u32; 2] {
        self.stride_px
    }

    /// Exact extraction-grid overlap in X/Y order.
    pub fn overlap_px(&self) -> [u32; 2] {
        self.overlap_px
    }

    /// Frozen effective receptive field.
    pub fn effective_receptive_field(&self) -> EffectiveReceptiveField {
        self.effective_receptive_field
    }

    /// Closed source-boundary policy.
    pub fn boundary_policy(&self) -> PatchBoundaryPolicy {
        self.boundary_policy
    }

    /// Format-independent logical identity.
    pub fn logical_digest(&self) -> ContentDigest {
        self.logical_digest
    }
}

fn validate_receptive_field(
    receptive_field: EffectiveReceptiveField,
    patch_px: [u32; 2],
) -> Result<(), MultiscaleEmbeddingError> {
    let EffectiveReceptiveField::CenteredRectangle {
        width_px,
        height_px,
    } = receptive_field
    else {
        return Ok(());
    };
    for (extent, patch) in [(width_px, patch_px[0]), (height_px, patch_px[1])] {
        if u128::from(extent.numerator()) > u128::from(patch) * u128::from(extent.denominator()) {
            return Err(MultiscaleEmbeddingError::InvalidPatchContext);
        }
    }
    Ok(())
}

fn rational_as_f64(value: PositiveRational) -> f64 {
    value.numerator() as f64 / value.denominator() as f64
}

fn context_retained_bytes(
    owning_slide_id: &SlideId,
    image_frame_id: &CoordinateFrameId,
    physical_frame_id: &CoordinateFrameId,
    transform_id: &TransformId,
) -> Result<usize, MultiscaleEmbeddingError> {
    size_of::<PatchEmbeddingContext>()
        .checked_add(owning_slide_id.as_str().len())
        .and_then(|value| value.checked_add(image_frame_id.as_str().len()))
        .and_then(|value| value.checked_add(physical_frame_id.as_str().len()))
        .and_then(|value| value.checked_add(transform_id.as_str().len()))
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)
}

fn canonical_f64_bits(value: f64) -> u64 {
    if value == 0.0 {
        0
    } else {
        value.to_bits()
    }
}

#[allow(clippy::too_many_arguments)]
fn context_digest(
    owning_slide_id: &SlideId,
    image_frame_id: &CoordinateFrameId,
    pixel_convention: ImageCoordinateConvention,
    physical_frame_id: &CoordinateFrameId,
    transform_id: &TransformId,
    matrix_bits: [u64; 6],
    mpp_x: PositiveRational,
    mpp_y: PositiveRational,
    source_image_px: [u64; 2],
    patch_px: [u32; 2],
    stride_px: [u32; 2],
    overlap_px: [u32; 2],
    receptive_field: EffectiveReceptiveField,
    boundary_policy: PatchBoundaryPolicy,
) -> Result<ContentDigest, MultiscaleEmbeddingError> {
    let mut digest = LogicalDigest::new(CONTEXT_DOMAIN);
    digest.text(CONTEXT_FORMAT);
    digest.u32(CONTEXT_VERSION);
    digest.text(owning_slide_id.as_str());
    digest.text(image_frame_id.as_str());
    digest.array_len(2)?;
    digest.text("x");
    digest.text("y");
    digest.text("image");
    digest.text("pixel");
    digest.text(convention_name(pixel_convention));
    digest.text(physical_frame_id.as_str());
    digest.array_len(2)?;
    digest.text("x");
    digest.text("y");
    digest.text("physical");
    digest.text("micrometer");
    digest.text(transform_id.as_str());
    digest.text(image_frame_id.as_str());
    digest.text(physical_frame_id.as_str());
    digest.u8(0);
    digest.array_len(matrix_bits.len())?;
    for bits in matrix_bits {
        digest.f64_bits(bits);
    }
    digest_rational(&mut digest, mpp_x);
    digest_rational(&mut digest, mpp_y);
    digest.array_len(source_image_px.len())?;
    for value in source_image_px {
        digest.u64(value);
    }
    for values in [patch_px, stride_px, overlap_px] {
        digest.array_len(values.len())?;
        for value in values {
            digest.u32(value);
        }
    }
    match receptive_field {
        EffectiveReceptiveField::FullInput => digest.text("full_input"),
        EffectiveReceptiveField::CenteredRectangle {
            width_px,
            height_px,
        } => {
            digest.text("centered_rectangle");
            digest_rational(&mut digest, width_px);
            digest_rational(&mut digest, height_px);
        }
    }
    match boundary_policy {
        PatchBoundaryPolicy::FullyContainedOnly => digest.text("fully_contained_only"),
        PatchBoundaryPolicy::Reflect => digest.text("reflect"),
        PatchBoundaryPolicy::ConstantRgb(rgb) => {
            digest.text("constant_rgb");
            digest.array_len(rgb.len())?;
            for value in rgb {
                digest.u8(value);
            }
        }
    }
    Ok(digest.finish())
}

fn digest_rational(digest: &mut LogicalDigest, value: PositiveRational) {
    digest.u64(value.numerator());
    digest.u64(value.denominator());
}
