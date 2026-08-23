use std::{fmt, io::Read, mem::size_of};

use marklab_project::ContentDigest;
use serde::{Deserialize, Serialize};

use super::codec::{
    preflight_json_strings, require_decoded, require_retained, valid_decimal,
    MAX_NORMALIZATION_BYTES,
};
use crate::multiscale::{
    digest::LogicalDigest,
    error::MultiscaleEmbeddingError,
    json::{
        canonical_json_len, compare_canonical_json_reader, encode_canonical_json,
        matches_canonical_json, CanonicalJsonReaderError,
    },
};

const FORMAT: &str = "marklab.patch_embedding_input_normalization";
const VERSION: u32 = 1;
const DOMAIN: &[u8] = b"marklab-patch-input-normalization-logical-v1";

/// Exact 1–64 byte ASCII fixed-point decimal for C-05 input normalization.
#[derive(Clone, Eq, PartialEq)]
pub struct PatchNormalizationDecimal(Box<str>);

impl fmt::Debug for PatchNormalizationDecimal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PatchNormalizationDecimal")
            .finish_non_exhaustive()
    }
}

impl PatchNormalizationDecimal {
    /// Validate the bounded canonical fixed-point grammar without floating-point conversion.
    pub fn new(value: impl Into<String>) -> Result<Self, MultiscaleEmbeddingError> {
        let value = value.into();
        if !valid_decimal(&value) {
            return Err(MultiscaleEmbeddingError::InvalidPatchNormalizationDecimal);
        }
        Ok(Self(value.into_boxed_str()))
    }

    /// Exact canonical decimal text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn is_positive(&self) -> bool {
        !self.0.starts_with('-') && self.0.as_ref() != "0"
    }
}

/// Closed RGB sRGB H&E input-normalization record.
#[derive(Clone, Eq, PartialEq)]
pub struct PatchEmbeddingInputNormalization {
    channel_mean: [PatchNormalizationDecimal; 3],
    channel_std: [PatchNormalizationDecimal; 3],
    logical_digest: ContentDigest,
    encoded_len: usize,
}

impl fmt::Debug for PatchEmbeddingInputNormalization {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PatchEmbeddingInputNormalization")
            .field("logical_digest", &self.logical_digest)
            .finish()
    }
}

impl PatchEmbeddingInputNormalization {
    /// Construct the only admitted version-one H&E brightfield sRGB profile.
    pub fn he_srgb(
        channel_mean: [PatchNormalizationDecimal; 3],
        channel_std: [PatchNormalizationDecimal; 3],
        maximum_retained_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        if channel_std.iter().any(|value| !value.is_positive()) {
            return Err(MultiscaleEmbeddingError::InvalidPatchInputNormalization);
        }
        let required = retained_bytes(&channel_mean, &channel_std)?;
        require_retained(required, maximum_retained_bytes)?;
        let logical_digest = normalization_digest(&channel_mean, &channel_std)?;
        let mut normalization = Self {
            channel_mean,
            channel_std,
            logical_digest,
            encoded_len: 0,
        };
        let encoded_len = canonical_json_len(&normalization.wire())?;
        if encoded_len > MAX_NORMALIZATION_BYTES {
            return Err(MultiscaleEmbeddingError::EncodedByteBudgetExceeded {
                observed: encoded_len,
                maximum: MAX_NORMALIZATION_BYTES,
            });
        }
        normalization.encoded_len = encoded_len;
        Ok(normalization)
    }

    /// Decode and revalidate the exact canonical JSON fixed point after budget preflight.
    pub fn from_canonical_json(
        bytes: &[u8],
        maximum_encoded_bytes: usize,
        maximum_decoded_bytes: usize,
        maximum_retained_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        let effective_maximum = maximum_encoded_bytes.min(MAX_NORMALIZATION_BYTES);
        if bytes.len() > effective_maximum {
            return Err(MultiscaleEmbeddingError::EncodedByteBudgetExceeded {
                observed: bytes.len(),
                maximum: effective_maximum,
            });
        }
        preflight_json_strings(bytes, 6 * 64)?;
        let decoded_required = size_of::<OwnedWire>()
            .checked_add(bytes.len())
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        require_decoded(decoded_required, maximum_decoded_bytes)?;
        let wire: OwnedWire = serde_json::from_slice(bytes)
            .map_err(|_| MultiscaleEmbeddingError::InvalidCanonicalJson)?;
        if wire.format != FORMAT
            || wire.version != VERSION
            || wire.modality != "he_brightfield"
            || wire.color_space != "srgb"
            || wire.channel_order != ["r", "g", "b"]
            || wire.value_range.minimum != "0"
            || wire.value_range.maximum != "1"
            || wire.stain_normalization.kind != "none"
        {
            return Err(MultiscaleEmbeddingError::InvalidPatchInputNormalization);
        }
        let [mean_r, mean_g, mean_b] = wire.channel_mean;
        let [std_r, std_g, std_b] = wire.channel_std;
        let normalization = Self::he_srgb(
            [
                PatchNormalizationDecimal::new(mean_r)?,
                PatchNormalizationDecimal::new(mean_g)?,
                PatchNormalizationDecimal::new(mean_b)?,
            ],
            [
                PatchNormalizationDecimal::new(std_r)?,
                PatchNormalizationDecimal::new(std_g)?,
                PatchNormalizationDecimal::new(std_b)?,
            ],
            maximum_retained_bytes,
        )?;
        if !matches_canonical_json(&normalization.wire(), bytes) {
            return Err(MultiscaleEmbeddingError::InvalidCanonicalJson);
        }
        Ok(normalization)
    }

    /// Encode the exact version-one canonical JSON with one final newline.
    pub fn to_canonical_json(&self) -> Result<Vec<u8>, MultiscaleEmbeddingError> {
        encode_canonical_json(&self.wire(), self.encoded_len, MAX_NORMALIZATION_BYTES)
    }

    pub(in crate::multiscale) fn compare_canonical_json_reader<R: Read + ?Sized>(
        &self,
        reader: &mut R,
    ) -> Result<(), CanonicalJsonReaderError> {
        compare_canonical_json_reader(
            &self.wire(),
            self.encoded_len,
            MAX_NORMALIZATION_BYTES,
            reader,
        )
    }

    /// Exact channel means in r/g/b order.
    pub fn channel_mean(&self) -> &[PatchNormalizationDecimal; 3] {
        &self.channel_mean
    }

    /// Exact positive channel standard deviations in r/g/b order.
    pub fn channel_std(&self) -> &[PatchNormalizationDecimal; 3] {
        &self.channel_std
    }

    /// Format-independent logical identity.
    pub fn logical_digest(&self) -> ContentDigest {
        self.logical_digest
    }

    fn wire(&self) -> WireRef<'_> {
        WireRef {
            format: FORMAT,
            version: VERSION,
            modality: "he_brightfield",
            color_space: "srgb",
            channel_order: ["r", "g", "b"],
            value_range: ValueRangeRef {
                minimum: "0",
                maximum: "1",
            },
            channel_mean: self.channel_mean.each_ref().map(|value| value.as_str()),
            channel_std: self.channel_std.each_ref().map(|value| value.as_str()),
            stain_normalization: StainRef { kind: "none" },
        }
    }
}

fn retained_bytes(
    mean: &[PatchNormalizationDecimal; 3],
    std: &[PatchNormalizationDecimal; 3],
) -> Result<usize, MultiscaleEmbeddingError> {
    mean.iter().chain(std).try_fold(
        size_of::<PatchEmbeddingInputNormalization>(),
        |total, value| {
            total
                .checked_add(value.as_str().len())
                .ok_or(MultiscaleEmbeddingError::SizeOverflow)
        },
    )
}

fn normalization_digest(
    mean: &[PatchNormalizationDecimal; 3],
    std: &[PatchNormalizationDecimal; 3],
) -> Result<ContentDigest, MultiscaleEmbeddingError> {
    let mut digest = LogicalDigest::new(DOMAIN);
    digest.text(FORMAT);
    digest.u32(VERSION);
    digest.text("he_brightfield");
    digest.text("srgb");
    digest.array_len(3)?;
    for channel in ["r", "g", "b"] {
        digest.text(channel);
    }
    digest.text("0");
    digest.text("1");
    digest.array_len(3)?;
    for value in mean {
        digest.text(value.as_str());
    }
    digest.array_len(3)?;
    for value in std {
        digest.text(value.as_str());
    }
    digest.text("none");
    Ok(digest.finish())
}

#[derive(Serialize)]
struct WireRef<'a> {
    format: &'static str,
    version: u32,
    modality: &'static str,
    color_space: &'static str,
    channel_order: [&'static str; 3],
    value_range: ValueRangeRef<'a>,
    channel_mean: [&'a str; 3],
    channel_std: [&'a str; 3],
    stain_normalization: StainRef,
}

#[derive(Serialize)]
struct ValueRangeRef<'a> {
    minimum: &'a str,
    maximum: &'a str,
}

#[derive(Serialize)]
struct StainRef {
    kind: &'static str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnedWire {
    format: String,
    version: u32,
    modality: String,
    color_space: String,
    channel_order: [String; 3],
    value_range: OwnedValueRange,
    channel_mean: [String; 3],
    channel_std: [String; 3],
    stain_normalization: OwnedStain,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnedValueRange {
    minimum: String,
    maximum: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnedStain {
    kind: String,
}
