//! Decoding helpers shared by the workflow nodes' artifact codecs.

use std::io;

use marklab_data::MeasurementStatus;

/// An `InvalidData` error for a malformed or inconsistent artifact.
pub(crate) fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

/// Whether `value` is a finite number in [0, 1].
pub(crate) fn unit_interval(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

/// Whether a decoded value matches its recomputation: equal, or finite and within 16 ulps
/// of the larger magnitude (at least 1).
pub(crate) fn nearly_equal(left: f64, right: f64) -> bool {
    left == right
        || (left.is_finite()
            && right.is_finite()
            && (left - right).abs() <= 16.0 * f64::EPSILON * left.abs().max(right.abs()).max(1.0))
}

/// The measurement status named in an artifact.
pub(crate) fn parse_status(value: &str) -> io::Result<MeasurementStatus> {
    crate::measurement_status_wire::parse(value)
        .ok_or_else(|| invalid("invalid measurement status"))
}
