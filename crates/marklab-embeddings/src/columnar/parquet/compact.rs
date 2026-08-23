use std::io::{self, Cursor, Read, Write};

use parquet::thrift::TSerializable;
use thrift::{
    protocol::{
        TCompactOutputProtocol, TFieldIdentifier, TInputProtocol, TListIdentifier, TMapIdentifier,
        TMessageIdentifier, TSetIdentifier, TStructIdentifier, TType,
    },
    Error, ProtocolError, ProtocolErrorKind,
};

#[derive(Clone, Copy)]
pub(super) struct CompactLimits {
    pub(super) maximum_depth: usize,
    pub(super) maximum_fields: usize,
    pub(super) maximum_collection_elements: usize,
    pub(super) maximum_total_elements: usize,
    pub(super) maximum_string_bytes: usize,
    pub(super) maximum_total_string_bytes: usize,
}

struct StructState {
    previous_field_id: i16,
}

/// Compact-Thrift input that rejects untrusted sizes before any generated allocation.
pub(super) struct BoundedCompactProtocol<'a> {
    input: Cursor<&'a [u8]>,
    limits: CompactLimits,
    structs: Vec<StructState>,
    pending_bool: Option<bool>,
    observed_fields: usize,
    observed_elements: usize,
    observed_string_bytes: usize,
}

impl<'a> BoundedCompactProtocol<'a> {
    pub(super) fn new(bytes: &'a [u8], limits: CompactLimits) -> Self {
        Self {
            input: Cursor::new(bytes),
            limits,
            structs: Vec::new(),
            pending_bool: None,
            observed_fields: 0,
            observed_elements: 0,
            observed_string_bytes: 0,
        }
    }

    pub(super) fn consumed_bytes(&self) -> thrift::Result<usize> {
        usize::try_from(self.input.position())
            .map_err(|_| protocol_error(ProtocolErrorKind::SizeLimit))
    }

    fn read_raw_byte(&mut self) -> thrift::Result<u8> {
        let mut byte = [0_u8; 1];
        self.input.read_exact(&mut byte).map_err(Error::from)?;
        Ok(byte[0])
    }

    fn read_unsigned_varint(&mut self, maximum_bytes: usize) -> thrift::Result<u64> {
        let mut value = 0_u64;
        for index in 0..maximum_bytes {
            let byte = self.read_raw_byte()?;
            let payload = u64::from(byte & 0x7f);
            let maximum_final_payload = match maximum_bytes {
                3 => 0x03,
                5 => 0x0f,
                10 => 0x01,
                _ => return Err(protocol_error(ProtocolErrorKind::InvalidData)),
            };
            if index + 1 == maximum_bytes && payload > maximum_final_payload {
                return Err(protocol_error(ProtocolErrorKind::InvalidData));
            }
            value |= payload
                .checked_shl(
                    u32::try_from(
                        index
                            .checked_mul(7)
                            .ok_or_else(|| protocol_error(ProtocolErrorKind::SizeLimit))?,
                    )
                    .map_err(|_| protocol_error(ProtocolErrorKind::SizeLimit))?,
                )
                .ok_or_else(|| protocol_error(ProtocolErrorKind::InvalidData))?;
            if byte & 0x80 == 0 {
                if index != 0 && payload == 0 {
                    return Err(protocol_error(ProtocolErrorKind::InvalidData));
                }
                return Ok(value);
            }
        }
        Err(protocol_error(ProtocolErrorKind::InvalidData))
    }

    fn read_zigzag(&mut self, maximum_bytes: usize) -> thrift::Result<i64> {
        let encoded = self.read_unsigned_varint(maximum_bytes)?;
        Ok(((encoded >> 1) as i64) ^ -((encoded & 1) as i64))
    }

    fn read_collection_header(&mut self) -> thrift::Result<(TType, i32)> {
        let header = self.read_raw_byte()?;
        let element_type = collection_type(header & 0x0f)?;
        let short_count = usize::from(header >> 4);
        let count = if short_count == 15 {
            usize::try_from(self.read_unsigned_varint(5)?)
                .map_err(|_| protocol_error(ProtocolErrorKind::SizeLimit))?
        } else {
            short_count
        };
        self.charge_elements(count)?;
        Ok((
            element_type,
            i32::try_from(count).map_err(|_| protocol_error(ProtocolErrorKind::SizeLimit))?,
        ))
    }

    fn charge_elements(&mut self, count: usize) -> thrift::Result<()> {
        if count > self.limits.maximum_collection_elements {
            return Err(protocol_error(ProtocolErrorKind::SizeLimit));
        }
        self.observed_elements = self
            .observed_elements
            .checked_add(count)
            .ok_or_else(|| protocol_error(ProtocolErrorKind::SizeLimit))?;
        if self.observed_elements > self.limits.maximum_total_elements {
            return Err(protocol_error(ProtocolErrorKind::SizeLimit));
        }
        Ok(())
    }
}

/// Require the parsed value to reproduce the exact bounded compact-Thrift bytes.
pub(super) fn is_canonical_compact<T: TSerializable>(
    value: &T,
    bytes: &[u8],
) -> thrift::Result<bool> {
    let mut output = BoundedCompactOutput::new(bytes.len())?;
    {
        let mut protocol = TCompactOutputProtocol::new(&mut output);
        value.write_to_out_protocol(&mut protocol)?;
    }
    Ok(output.bytes == bytes)
}

/// Measure canonical compact-Thrift output while enforcing a hard byte ceiling.
pub(super) fn canonical_compact_len<T: TSerializable>(
    value: &T,
    maximum_bytes: usize,
) -> thrift::Result<usize> {
    let mut output = BoundedCompactCounter {
        byte_len: 0,
        maximum_bytes,
    };
    {
        let mut protocol = TCompactOutputProtocol::new(&mut output);
        value.write_to_out_protocol(&mut protocol)?;
    }
    Ok(output.byte_len)
}

struct BoundedCompactOutput {
    bytes: Vec<u8>,
    maximum_bytes: usize,
}

impl BoundedCompactOutput {
    fn new(maximum_bytes: usize) -> thrift::Result<Self> {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(maximum_bytes)
            .map_err(|_| protocol_error(ProtocolErrorKind::SizeLimit))?;
        Ok(Self {
            bytes,
            maximum_bytes,
        })
    }
}

impl Write for BoundedCompactOutput {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let attempted = self
            .bytes
            .len()
            .checked_add(buffer.len())
            .ok_or_else(|| io::Error::other("bounded compact-Thrift output rejected"))?;
        if attempted > self.maximum_bytes {
            return Err(io::Error::other("bounded compact-Thrift output rejected"));
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct BoundedCompactCounter {
    byte_len: usize,
    maximum_bytes: usize,
}

impl Write for BoundedCompactCounter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let attempted = self
            .byte_len
            .checked_add(buffer.len())
            .ok_or_else(|| io::Error::other("bounded compact-Thrift output rejected"))?;
        if attempted > self.maximum_bytes {
            return Err(io::Error::other("bounded compact-Thrift output rejected"));
        }
        self.byte_len = attempted;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl TInputProtocol for BoundedCompactProtocol<'_> {
    fn read_message_begin(&mut self) -> thrift::Result<TMessageIdentifier> {
        Err(protocol_error(ProtocolErrorKind::NotImplemented))
    }

    fn read_message_end(&mut self) -> thrift::Result<()> {
        Err(protocol_error(ProtocolErrorKind::NotImplemented))
    }

    fn read_struct_begin(&mut self) -> thrift::Result<Option<TStructIdentifier>> {
        if self.structs.len() >= self.limits.maximum_depth {
            return Err(protocol_error(ProtocolErrorKind::DepthLimit));
        }
        self.structs.push(StructState {
            previous_field_id: 0,
        });
        Ok(None)
    }

    fn read_struct_end(&mut self) -> thrift::Result<()> {
        self.structs
            .pop()
            .ok_or_else(|| protocol_error(ProtocolErrorKind::InvalidData))?;
        Ok(())
    }

    fn read_field_begin(&mut self) -> thrift::Result<TFieldIdentifier> {
        let header = self.read_raw_byte()?;
        let compact_type = header & 0x0f;
        let field_type = compact_type_to_type(compact_type)?;
        if field_type == TType::Stop {
            return Ok(
                TFieldIdentifier::new::<Option<String>, String, Option<i16>>(
                    None,
                    TType::Stop,
                    None,
                ),
            );
        }
        self.observed_fields = self
            .observed_fields
            .checked_add(1)
            .ok_or_else(|| protocol_error(ProtocolErrorKind::SizeLimit))?;
        if self.observed_fields > self.limits.maximum_fields {
            return Err(protocol_error(ProtocolErrorKind::SizeLimit));
        }
        self.pending_bool = match compact_type {
            1 => Some(true),
            2 => Some(false),
            _ => None,
        };
        let delta = i16::from(header >> 4);
        let previous = self
            .structs
            .last()
            .ok_or_else(|| protocol_error(ProtocolErrorKind::InvalidData))?
            .previous_field_id;
        let field_id = if delta == 0 {
            self.read_i16()?
        } else {
            previous
                .checked_add(delta)
                .ok_or_else(|| protocol_error(ProtocolErrorKind::InvalidData))?
        };
        if field_id <= previous || field_id <= 0 {
            return Err(protocol_error(ProtocolErrorKind::InvalidData));
        }
        self.structs
            .last_mut()
            .ok_or_else(|| protocol_error(ProtocolErrorKind::InvalidData))?
            .previous_field_id = field_id;
        Ok(
            TFieldIdentifier::new::<Option<String>, String, Option<i16>>(
                None,
                field_type,
                Some(field_id),
            ),
        )
    }

    fn read_field_end(&mut self) -> thrift::Result<()> {
        Ok(())
    }

    fn read_bool(&mut self) -> thrift::Result<bool> {
        if let Some(value) = self.pending_bool.take() {
            return Ok(value);
        }
        match self.read_raw_byte()? {
            1 => Ok(true),
            2 => Ok(false),
            _ => Err(protocol_error(ProtocolErrorKind::InvalidData)),
        }
    }

    fn read_bytes(&mut self) -> thrift::Result<Vec<u8>> {
        let length = usize::try_from(self.read_unsigned_varint(5)?)
            .map_err(|_| protocol_error(ProtocolErrorKind::SizeLimit))?;
        if length > self.limits.maximum_string_bytes {
            return Err(protocol_error(ProtocolErrorKind::SizeLimit));
        }
        self.observed_string_bytes = self
            .observed_string_bytes
            .checked_add(length)
            .ok_or_else(|| protocol_error(ProtocolErrorKind::SizeLimit))?;
        if self.observed_string_bytes > self.limits.maximum_total_string_bytes {
            return Err(protocol_error(ProtocolErrorKind::SizeLimit));
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| protocol_error(ProtocolErrorKind::SizeLimit))?;
        bytes.resize(length, 0);
        self.input.read_exact(&mut bytes).map_err(Error::from)?;
        Ok(bytes)
    }

    fn read_i8(&mut self) -> thrift::Result<i8> {
        Ok(self.read_raw_byte()? as i8)
    }

    fn read_i16(&mut self) -> thrift::Result<i16> {
        i16::try_from(self.read_zigzag(3)?)
            .map_err(|_| protocol_error(ProtocolErrorKind::InvalidData))
    }

    fn read_i32(&mut self) -> thrift::Result<i32> {
        i32::try_from(self.read_zigzag(5)?)
            .map_err(|_| protocol_error(ProtocolErrorKind::InvalidData))
    }

    fn read_i64(&mut self) -> thrift::Result<i64> {
        self.read_zigzag(10)
    }

    fn read_double(&mut self) -> thrift::Result<f64> {
        let mut bytes = [0_u8; 8];
        self.input.read_exact(&mut bytes).map_err(Error::from)?;
        Ok(f64::from_le_bytes(bytes))
    }

    fn read_string(&mut self) -> thrift::Result<String> {
        String::from_utf8(self.read_bytes()?)
            .map_err(|_| protocol_error(ProtocolErrorKind::InvalidData))
    }

    fn read_list_begin(&mut self) -> thrift::Result<TListIdentifier> {
        let (element_type, count) = self.read_collection_header()?;
        Ok(TListIdentifier::new(element_type, count))
    }

    fn read_list_end(&mut self) -> thrift::Result<()> {
        Ok(())
    }

    fn read_set_begin(&mut self) -> thrift::Result<TSetIdentifier> {
        let (element_type, count) = self.read_collection_header()?;
        Ok(TSetIdentifier::new(element_type, count))
    }

    fn read_set_end(&mut self) -> thrift::Result<()> {
        Ok(())
    }

    fn read_map_begin(&mut self) -> thrift::Result<TMapIdentifier> {
        let count = usize::try_from(self.read_unsigned_varint(5)?)
            .map_err(|_| protocol_error(ProtocolErrorKind::SizeLimit))?;
        self.charge_elements(count)?;
        let count =
            i32::try_from(count).map_err(|_| protocol_error(ProtocolErrorKind::SizeLimit))?;
        if count == 0 {
            return Ok(TMapIdentifier::new(None, None, 0));
        }
        let types = self.read_raw_byte()?;
        Ok(TMapIdentifier::new(
            Some(collection_type(types >> 4)?),
            Some(collection_type(types & 0x0f)?),
            count,
        ))
    }

    fn read_map_end(&mut self) -> thrift::Result<()> {
        Ok(())
    }

    fn read_byte(&mut self) -> thrift::Result<u8> {
        self.read_raw_byte()
    }

    fn skip(&mut self, _field_type: TType) -> thrift::Result<()> {
        Err(protocol_error(ProtocolErrorKind::InvalidData))
    }
}

fn compact_type_to_type(value: u8) -> thrift::Result<TType> {
    match value {
        0 => Ok(TType::Stop),
        1 | 2 => Ok(TType::Bool),
        3 => Ok(TType::I08),
        4 => Ok(TType::I16),
        5 => Ok(TType::I32),
        6 => Ok(TType::I64),
        7 => Ok(TType::Double),
        8 => Ok(TType::String),
        9 => Ok(TType::List),
        10 => Ok(TType::Set),
        11 => Ok(TType::Map),
        12 => Ok(TType::Struct),
        _ => Err(protocol_error(ProtocolErrorKind::InvalidData)),
    }
}

fn collection_type(value: u8) -> thrift::Result<TType> {
    if value == 1 {
        Ok(TType::Bool)
    } else {
        compact_type_to_type(value)
    }
}

fn protocol_error(kind: ProtocolErrorKind) -> Error {
    Error::Protocol(ProtocolError {
        kind,
        message: "bounded compact-Thrift input rejected".to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> CompactLimits {
        CompactLimits {
            maximum_depth: 2,
            maximum_fields: 4,
            maximum_collection_elements: 4,
            maximum_total_elements: 6,
            maximum_string_bytes: 4,
            maximum_total_string_bytes: 6,
        }
    }

    #[test]
    fn rejects_truncated_nonminimal_and_overflowing_varints() {
        for bytes in [&[0x80][..], &[0x84, 0x00], &[0xff, 0xff, 0xff, 0xff, 0x10]] {
            let mut protocol = BoundedCompactProtocol::new(bytes, limits());
            assert!(protocol.read_i32().is_err(), "accepted {bytes:?}");
        }
    }

    #[test]
    fn rejects_per_value_and_aggregate_string_limits_before_allocation() {
        let mut too_large = BoundedCompactProtocol::new(&[5], limits());
        assert!(too_large.read_bytes().is_err());

        let bytes = [4, b'a', b'b', b'c', b'd', 3, b'e', b'f', b'g'];
        let mut aggregate = BoundedCompactProtocol::new(&bytes, limits());
        assert_eq!(aggregate.read_bytes().expect("first string"), b"abcd");
        assert!(aggregate.read_bytes().is_err());
    }

    #[test]
    fn rejects_giant_collections_duplicate_fields_depth_and_skip() {
        let mut collection = BoundedCompactProtocol::new(&[0xf5, 5], limits());
        assert!(collection.read_list_begin().is_err());

        let mut duplicate = BoundedCompactProtocol::new(&[0x15, 0x00, 0x05, 0x02], limits());
        duplicate.read_struct_begin().expect("root struct");
        assert_eq!(duplicate.read_field_begin().expect("field").id, Some(1));
        assert_eq!(duplicate.read_i32().expect("value"), 0);
        assert!(duplicate.read_field_begin().is_err());

        let mut depth = BoundedCompactProtocol::new(
            &[],
            CompactLimits {
                maximum_depth: 1,
                ..limits()
            },
        );
        depth.read_struct_begin().expect("root struct");
        assert!(depth.read_struct_begin().is_err());

        let mut skipped = BoundedCompactProtocol::new(&[], limits());
        assert!(skipped.skip(TType::String).is_err());
    }
}
