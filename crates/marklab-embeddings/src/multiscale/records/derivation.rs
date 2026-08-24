#[cfg(feature = "parquet")]
use std::io::Read;
use std::{fmt, mem::size_of};

use marklab_project::ContentDigest;
use serde::{Deserialize, Serialize};

use super::codec::{
    preflight_json_strings, require_decoded, require_retained, valid_token,
    MAX_RAW_SMALL_JSON_STRING_BYTES, MAX_SMALL_RECORD_BYTES,
};
#[cfg(feature = "parquet")]
use crate::multiscale::json::{compare_canonical_json_reader, CanonicalJsonReaderError};
use crate::multiscale::{
    digest::LogicalDigest,
    error::MultiscaleEmbeddingError,
    json::{canonical_json_len, encode_canonical_json, matches_canonical_json},
};

const FORMAT: &str = "marklab.multiscale_embedding_derivation";
const VERSION: u32 = 1;
const DOMAIN: &[u8] = b"marklab-multiscale-embedding-derivation-logical-v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DerivationAlgorithm {
    WeightedMean,
    ArithmeticMean,
}

impl DerivationAlgorithm {
    fn wire_name(self) -> &'static str {
        match self {
            Self::WeightedMean => "weighted_mean",
            Self::ArithmeticMean => "arithmetic_mean",
        }
    }

    fn parse(value: &str) -> Result<Self, MultiscaleEmbeddingError> {
        match value {
            "weighted_mean" => Ok(Self::WeightedMean),
            "arithmetic_mean" => Ok(Self::ArithmeticMean),
            _ => Err(MultiscaleEmbeddingError::InvalidMultiscaleEmbeddingDerivation),
        }
    }
}

/// Closed deterministic version-one aggregation contract for derived region or slide embeddings.
#[derive(Clone, Eq, PartialEq)]
pub struct MultiscaleEmbeddingDerivationContract {
    algorithm: DerivationAlgorithm,
    algorithm_version: Box<str>,
    logical_digest: ContentDigest,
    encoded_len: usize,
}

impl fmt::Debug for MultiscaleEmbeddingDerivationContract {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MultiscaleEmbeddingDerivationContract")
            .field("algorithm", &self.algorithm)
            .field("logical_digest", &self.logical_digest)
            .finish()
    }
}

impl MultiscaleEmbeddingDerivationContract {
    /// Freeze deterministic declared-fraction weighted-mean aggregation for region derivation.
    pub fn weighted_mean(
        algorithm_version: impl Into<String>,
        maximum_retained_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        Self::from_owned_version(
            DerivationAlgorithm::WeightedMean,
            algorithm_version.into(),
            maximum_retained_bytes,
        )
    }

    /// Freeze deterministic unweighted arithmetic-mean aggregation for slide derivation.
    pub fn arithmetic_mean(
        algorithm_version: impl Into<String>,
        maximum_retained_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        Self::from_owned_version(
            DerivationAlgorithm::ArithmeticMean,
            algorithm_version.into(),
            maximum_retained_bytes,
        )
    }

    /// Decode and revalidate the exact canonical JSON fixed point under caller budgets.
    pub fn from_canonical_json(
        bytes: &[u8],
        maximum_encoded_bytes: usize,
        maximum_decoded_bytes: usize,
        maximum_retained_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        let effective_maximum = maximum_encoded_bytes.min(MAX_SMALL_RECORD_BYTES);
        if bytes.len() > effective_maximum {
            return Err(MultiscaleEmbeddingError::EncodedByteBudgetExceeded {
                observed: bytes.len(),
                maximum: effective_maximum,
            });
        }
        preflight_json_strings(bytes, MAX_RAW_SMALL_JSON_STRING_BYTES)?;
        let decoded_required = size_of::<WireBorrowed<'_>>()
            .checked_add(bytes.len())
            .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
        require_decoded(decoded_required, maximum_decoded_bytes)?;
        let wire: WireBorrowed<'_> = serde_json::from_slice(bytes)
            .map_err(|_| MultiscaleEmbeddingError::InvalidCanonicalJson)?;
        if wire.format != FORMAT
            || wire.version != VERSION
            || wire.kind != "deterministic"
            || wire.missing_policy != "exclude_non_present_require_one"
            || wire.accumulator_dtype != "f64"
            || wire.output_dtype != "f32"
        {
            return Err(MultiscaleEmbeddingError::InvalidMultiscaleEmbeddingDerivation);
        }
        let algorithm = DerivationAlgorithm::parse(wire.algorithm)?;
        let derivation =
            Self::from_borrowed_version(algorithm, wire.algorithm_version, maximum_retained_bytes)?;
        if !matches_canonical_json(&derivation.wire(), bytes) {
            return Err(MultiscaleEmbeddingError::InvalidCanonicalJson);
        }
        Ok(derivation)
    }

    /// Encode the exact canonical JSON document with one final newline.
    pub fn to_canonical_json(&self) -> Result<Vec<u8>, MultiscaleEmbeddingError> {
        encode_canonical_json(&self.wire(), self.encoded_len, MAX_SMALL_RECORD_BYTES)
    }

    #[cfg(feature = "parquet")]
    pub(in crate::multiscale) fn compare_canonical_json_reader<R: Read + ?Sized>(
        &self,
        reader: &mut R,
    ) -> Result<(), CanonicalJsonReaderError> {
        compare_canonical_json_reader(
            &self.wire(),
            self.encoded_len,
            MAX_SMALL_RECORD_BYTES,
            reader,
        )
    }

    /// Closed algorithm token (`weighted_mean` or `arithmetic_mean`).
    pub fn algorithm(&self) -> &'static str {
        self.algorithm.wire_name()
    }

    /// Explicit bounded implementation/version token.
    pub fn algorithm_version(&self) -> &str {
        &self.algorithm_version
    }

    /// Format-independent logical identity.
    pub fn logical_digest(&self) -> ContentDigest {
        self.logical_digest
    }

    fn from_owned_version(
        algorithm: DerivationAlgorithm,
        algorithm_version: String,
        maximum_retained_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        if !valid_token(&algorithm_version) {
            return Err(MultiscaleEmbeddingError::InvalidMultiscaleEmbeddingDerivation);
        }
        let required = retained_bytes(algorithm_version.capacity())?;
        require_retained(required, maximum_retained_bytes)?;
        Self::finish(algorithm, algorithm_version.into_boxed_str())
    }

    fn from_borrowed_version(
        algorithm: DerivationAlgorithm,
        algorithm_version: &str,
        maximum_retained_bytes: usize,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        if !valid_token(algorithm_version) {
            return Err(MultiscaleEmbeddingError::InvalidMultiscaleEmbeddingDerivation);
        }
        require_retained(
            retained_bytes(algorithm_version.len())?,
            maximum_retained_bytes,
        )?;
        Self::finish(algorithm, algorithm_version.into())
    }

    fn finish(
        algorithm: DerivationAlgorithm,
        algorithm_version: Box<str>,
    ) -> Result<Self, MultiscaleEmbeddingError> {
        let logical_digest = derivation_digest(algorithm, &algorithm_version);
        let mut derivation = Self {
            algorithm,
            algorithm_version,
            logical_digest,
            encoded_len: 0,
        };
        let encoded_len = canonical_json_len(&derivation.wire())?;
        if encoded_len > MAX_SMALL_RECORD_BYTES {
            return Err(MultiscaleEmbeddingError::EncodedByteBudgetExceeded {
                observed: encoded_len,
                maximum: MAX_SMALL_RECORD_BYTES,
            });
        }
        derivation.encoded_len = encoded_len;
        Ok(derivation)
    }

    fn wire(&self) -> WireRef<'_> {
        WireRef {
            format: FORMAT,
            version: VERSION,
            kind: "deterministic",
            algorithm: self.algorithm(),
            algorithm_version: &self.algorithm_version,
            missing_policy: "exclude_non_present_require_one",
            accumulator_dtype: "f64",
            output_dtype: "f32",
        }
    }
}

fn retained_bytes(version_bytes: usize) -> Result<usize, MultiscaleEmbeddingError> {
    size_of::<MultiscaleEmbeddingDerivationContract>()
        .checked_add(version_bytes)
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)
}

fn derivation_digest(algorithm: DerivationAlgorithm, algorithm_version: &str) -> ContentDigest {
    let mut digest = LogicalDigest::new(DOMAIN);
    digest.text(FORMAT);
    digest.u32(VERSION);
    digest.text("deterministic");
    digest.text(algorithm.wire_name());
    digest.text(algorithm_version);
    digest.text("exclude_non_present_require_one");
    digest.text("f64");
    digest.text("f32");
    digest.finish()
}

#[derive(Serialize)]
struct WireRef<'a> {
    format: &'static str,
    version: u32,
    kind: &'static str,
    algorithm: &'static str,
    algorithm_version: &'a str,
    missing_policy: &'static str,
    accumulator_dtype: &'static str,
    output_dtype: &'static str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireBorrowed<'a> {
    format: &'a str,
    version: u32,
    kind: &'a str,
    algorithm: &'a str,
    algorithm_version: &'a str,
    missing_policy: &'a str,
    accumulator_dtype: &'a str,
    output_dtype: &'a str,
}
