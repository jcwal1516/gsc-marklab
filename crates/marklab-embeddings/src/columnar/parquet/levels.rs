//! Validation of Parquet definition and repetition levels with bit width one.
//!
//! Levels are stored in the RLE/bit-packed hybrid encoding. These checks decode a stream
//! without allocating and compare every level with the value the profile requires.

/// The level each value must have.
#[derive(Clone, Copy)]
pub(in crate::columnar::parquet) enum LevelPattern {
    /// Every level is one: a required value inside an optional column, or a present value.
    AllOne,
    /// Repetition levels of fixed-length lists: zero at each list start, one elsewhere.
    FixedListRepetition { dimension: usize },
}

impl LevelPattern {
    pub(in crate::columnar::parquet) fn level(self, index: usize) -> Option<u8> {
        match self {
            Self::AllOne => Some(1),
            Self::FixedListRepetition { dimension } => {
                (dimension != 0).then(|| u8::from(!index.is_multiple_of(dimension)))
            }
        }
    }
}

/// Why a level stream was rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::columnar::parquet) enum LevelStreamError {
    /// The stream is malformed or a level differs from the required value.
    Invalid,
    /// A length or offset does not fit in `usize`.
    Overflow,
}

/// Check that `bytes` decodes to exactly `expected_values` levels and that level `index`
/// equals `expected(index)`. `expected` returns `None` when no level is valid there.
pub(in crate::columnar::parquet) fn check_level_stream(
    bytes: &[u8],
    expected_values: usize,
    expected: impl Fn(usize) -> Option<u8>,
) -> Result<(), LevelStreamError> {
    use LevelStreamError::{Invalid, Overflow};
    let matches = |index: usize, value: u8| expected(index).is_some_and(|level| level == value);
    let mut cursor = 0_usize;
    let mut produced = 0_usize;
    while cursor < bytes.len() {
        if produced == expected_values {
            return Err(Invalid);
        }
        let (header, header_bytes) = read_level_varint(bytes.get(cursor..).ok_or(Invalid)?)?;
        cursor = cursor.checked_add(header_bytes).ok_or(Overflow)?;
        if header == 0 {
            return Err(Invalid);
        }
        let remaining = expected_values.checked_sub(produced).ok_or(Overflow)?;
        if header & 1 == 0 {
            // Run-length encoded: one byte repeated `run` times.
            let run = header >> 1;
            if run == 0 || run > remaining {
                return Err(Invalid);
            }
            let value = *bytes.get(cursor).ok_or(Invalid)?;
            cursor = cursor.checked_add(1).ok_or(Overflow)?;
            if value > 1 || !(produced..produced + run).all(|index| matches(index, value)) {
                return Err(Invalid);
            }
            produced = produced.checked_add(run).ok_or(Overflow)?;
        } else {
            // Bit-packed: `groups` bytes of eight levels each; padding bits must be zero.
            let groups = header >> 1;
            let run = groups.checked_mul(8).ok_or(Overflow)?;
            if groups == 0 || run > remaining.saturating_add(7) {
                return Err(Invalid);
            }
            let packed = bytes
                .get(cursor..cursor.checked_add(groups).ok_or(Overflow)?)
                .ok_or(Invalid)?;
            for index in 0..run {
                let value = (packed[index / 8] >> (index % 8)) & 1;
                let valid = if index < remaining {
                    matches(produced + index, value)
                } else {
                    value == 0
                };
                if !valid {
                    return Err(Invalid);
                }
            }
            cursor = cursor.checked_add(groups).ok_or(Overflow)?;
            produced = produced.checked_add(run.min(remaining)).ok_or(Overflow)?;
        }
    }
    if produced != expected_values {
        return Err(Invalid);
    }
    Ok(())
}

/// Read one ULEB128 run header of at most 32 bits; returns the value and its length.
pub(in crate::columnar::parquet) fn read_level_varint(
    bytes: &[u8],
) -> Result<(usize, usize), LevelStreamError> {
    use LevelStreamError::{Invalid, Overflow};
    let mut value = 0_u32;
    for index in 0..5_usize {
        let byte = *bytes.get(index).ok_or(Invalid)?;
        let payload = u32::from(byte & 0x7f);
        if index == 4 && payload > 0x0f {
            return Err(Invalid);
        }
        value |= payload
            .checked_shl(u32::try_from(index * 7).map_err(|_| Overflow)?)
            .ok_or(Invalid)?;
        if byte & 0x80 == 0 {
            if index != 0 && payload == 0 {
                return Err(Invalid);
            }
            return Ok((usize::try_from(value).map_err(|_| Overflow)?, index + 1));
        }
    }
    Err(Invalid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_rle_and_bit_packed_runs_and_rejects_each_malformation() {
        let all_one = |_| Some(1);
        let lists_of_three =
            |index| LevelPattern::FixedListRepetition { dimension: 3 }.level(index);
        // RLE: header 2 * run, then the value byte.
        assert_eq!(check_level_stream(&[6, 1], 3, all_one), Ok(()));
        // Bit-packed: header 2 * groups + 1; levels 0,1,1,0,1,1 then two zero padding bits.
        assert_eq!(
            check_level_stream(&[3, 0b0011_0110], 6, lists_of_three),
            Ok(())
        );
        assert_eq!(check_level_stream(&[], 0, all_one), Ok(()));
        for (bytes, expected_values) in [
            (&[6, 1][..], 2),           // run longer than the column
            (&[6, 1][..], 4),           // stream ends early
            (&[0][..], 1),              // zero header
            (&[2, 2][..], 1),           // value above one
            (&[2, 0][..], 1),           // level differs from the pattern
            (&[3, 0b1111_1111][..], 6), // nonzero padding bit
            (&[3][..], 6),              // missing packed byte
            (&[0x80][..], 1),           // truncated varint
            (&[2, 1, 2, 1][..], 1),     // data after the last level
        ] {
            assert_eq!(
                check_level_stream(bytes, expected_values, all_one),
                Err(LevelStreamError::Invalid),
                "{bytes:?}"
            );
        }
        assert_eq!(
            LevelPattern::FixedListRepetition { dimension: 0 }.level(0),
            None
        );
    }

    #[test]
    fn varints_must_be_minimal_and_fit_in_32_bits() {
        assert_eq!(read_level_varint(&[0x7f]), Ok((127, 1)));
        assert_eq!(read_level_varint(&[0x80, 0x01]), Ok((128, 2)));
        assert_eq!(
            read_level_varint(&[0x80, 0x00]),
            Err(LevelStreamError::Invalid)
        );
        assert_eq!(
            read_level_varint(&[0xff, 0xff, 0xff, 0xff, 0x1f]),
            Err(LevelStreamError::Invalid)
        );
    }
}
