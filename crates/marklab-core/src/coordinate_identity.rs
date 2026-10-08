use std::{fmt, sync::Arc};

use thiserror::Error;

use crate::identity::MAX_ID_BYTES;

/// Kind of one coordinate-substrate identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CoordinateIdentityKind {
    /// Coordinate-frame identity.
    CoordinateFrame,
    /// Directed transform identity.
    Transform,
    /// Uncertainty-reference identity.
    Uncertainty,
}

impl fmt::Display for CoordinateIdentityKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::CoordinateFrame => "coordinate frame",
            Self::Transform => "transform",
            Self::Uncertainty => "uncertainty",
        })
    }
}

macro_rules! define_coordinate_id {
    ($name:ident, $kind:ident, $docs:literal) => {
        #[doc = $docs]
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(Arc<str>);

        impl $name {
            /// Validate and retain one opaque identity value.
            pub fn new(value: impl AsRef<str>) -> Result<Self, CoordinateIdentityError> {
                validate_id(CoordinateIdentityKind::$kind, value.as_ref()).map(Self)
            }

            /// Borrow the exact validated identity text.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

define_coordinate_id!(
    CoordinateFrameId,
    CoordinateFrame,
    "A named coordinate-frame identity."
);
define_coordinate_id!(
    TransformId,
    Transform,
    "A directed frame-transform identity."
);
define_coordinate_id!(
    UncertaintyId,
    Uncertainty,
    "A coordinate-uncertainty reference identity."
);

/// Rejection reasons for one opaque coordinate-substrate identity.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum CoordinateIdentityError {
    /// The value is empty or consists only of whitespace.
    #[error("{kind} ID must not be blank")]
    Blank {
        /// Rejected identity kind.
        kind: CoordinateIdentityKind,
    },
    /// The value exceeds the fixed UTF-8 byte bound.
    #[error("{kind} ID has {byte_len} UTF-8 bytes, exceeding {MAX_ID_BYTES}")]
    TooLong {
        /// Rejected identity kind.
        kind: CoordinateIdentityKind,
        /// Observed UTF-8 byte length.
        byte_len: usize,
    },
    /// The value contains at least one Unicode control character.
    #[error("{kind} ID must not contain control characters")]
    ControlCharacter {
        /// Rejected identity kind.
        kind: CoordinateIdentityKind,
    },
    /// The value changes when surrounding Unicode whitespace is trimmed.
    #[error("{kind} ID must not contain surrounding whitespace")]
    SurroundingWhitespace {
        /// Rejected identity kind.
        kind: CoordinateIdentityKind,
    },
}

fn validate_id(
    kind: CoordinateIdentityKind,
    value: &str,
) -> Result<Arc<str>, CoordinateIdentityError> {
    if value.trim().is_empty() {
        return Err(CoordinateIdentityError::Blank { kind });
    }
    if value.len() > MAX_ID_BYTES {
        return Err(CoordinateIdentityError::TooLong {
            kind,
            byte_len: value.len(),
        });
    }
    if value.chars().any(char::is_control) {
        return Err(CoordinateIdentityError::ControlCharacter { kind });
    }
    if value.trim() != value {
        return Err(CoordinateIdentityError::SurroundingWhitespace { kind });
    }
    Ok(Arc::from(value))
}
