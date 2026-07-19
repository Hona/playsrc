use std::ops::Range;

use crate::{Classification, CodecError, ErrorCode, failure};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BitSpan {
    pub start: usize,
    pub length: usize,
}

impl BitSpan {
    pub fn range(self) -> Range<usize> {
        self.start..self.start + self.length
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BitPayload {
    pub bytes: Vec<u8>,
    pub bit_length: usize,
}

impl BitPayload {
    pub fn reader(&self) -> BitReader<'_> {
        BitReader::new(&self.bytes, self.bit_length).expect("owned bit payload is valid")
    }
}

#[derive(Clone, Debug)]
pub struct BitReader<'a> {
    bytes: &'a [u8],
    bit: usize,
    end: usize,
    message_ordinal: Option<usize>,
}

impl<'a> BitReader<'a> {
    pub fn new(bytes: &'a [u8], bit_length: usize) -> Result<Self, CodecError> {
        if bit_length > bytes.len().saturating_mul(8) {
            return Err(failure(
                Classification::Malformed,
                ErrorCode::BitLimit,
                bytes.len() * 8..bit_length,
                None,
                "bit_payload.length",
                Some(bit_length as i64),
                Some((bytes.len() * 8) as u64),
                None,
            ));
        }
        Ok(Self {
            bytes,
            bit: 0,
            end: bit_length,
            message_ordinal: None,
        })
    }

    pub(crate) fn with_message_ordinal(mut self, ordinal: usize) -> Self {
        self.message_ordinal = Some(ordinal);
        self
    }

    pub(crate) fn set_message_ordinal(&mut self, ordinal: usize) {
        self.message_ordinal = Some(ordinal);
    }

    pub fn position(&self) -> usize {
        self.bit
    }

    pub fn bits_left(&self) -> usize {
        self.end - self.bit
    }

    pub fn end(&self) -> usize {
        self.end
    }

    pub fn seek(&mut self, position: usize) -> Result<(), CodecError> {
        if position > self.end {
            return Err(self.truncated(position..position, "bit_reader.seek", 0));
        }
        self.bit = position;
        Ok(())
    }

    pub fn read_bit(&mut self, field: &'static str) -> Result<bool, CodecError> {
        Ok(self.read_unsigned(1, field)? != 0)
    }

    pub fn read_unsigned(&mut self, width: usize, field: &'static str) -> Result<u32, CodecError> {
        if width > 32 {
            return Err(failure(
                Classification::Malformed,
                ErrorCode::BitLimit,
                self.bit..self.bit,
                self.message_ordinal,
                field,
                Some(width as i64),
                None,
                Some(32),
            ));
        }
        let start = self.bit;
        let end = start
            .checked_add(width)
            .ok_or_else(|| self.truncated(start..start, field, width))?;
        if end > self.end {
            return Err(self.truncated(start..end, field, width));
        }
        let mut value = 0_u32;
        for output_bit in 0..width {
            let source_bit = self.bit + output_bit;
            let bit = (self.bytes[source_bit / 8] >> (source_bit % 8)) & 1;
            value |= u32::from(bit) << output_bit;
        }
        self.bit = end;
        Ok(value)
    }

    pub fn read_signed(&mut self, width: usize, field: &'static str) -> Result<i32, CodecError> {
        let value = self.read_unsigned(width, field)?;
        if width == 0 || width == 32 {
            return Ok(value as i32);
        }
        let sign = 1_u32 << (width - 1);
        Ok(if value & sign == 0 {
            value as i32
        } else {
            value.wrapping_sub(sign).wrapping_sub(sign) as i32
        })
    }

    pub fn read_u8(&mut self, field: &'static str) -> Result<u8, CodecError> {
        Ok(self.read_unsigned(8, field)? as u8)
    }

    pub fn read_u16(&mut self, field: &'static str) -> Result<u16, CodecError> {
        Ok(self.read_unsigned(16, field)? as u16)
    }

    pub fn read_i16(&mut self, field: &'static str) -> Result<i16, CodecError> {
        Ok(self.read_unsigned(16, field)? as u16 as i16)
    }

    pub fn read_u32(&mut self, field: &'static str) -> Result<u32, CodecError> {
        self.read_unsigned(32, field)
    }

    pub fn read_i32(&mut self, field: &'static str) -> Result<i32, CodecError> {
        Ok(self.read_u32(field)? as i32)
    }

    pub fn read_var_u32(&mut self, field: &'static str) -> Result<u32, CodecError> {
        let start = self.bit;
        let mut value = 0_u32;
        for index in 0..5 {
            let byte = self.read_u8(field)?;
            if index == 4 && byte > 0x0f {
                return Err(failure(
                    Classification::Malformed,
                    ErrorCode::InvalidVarint,
                    start..self.bit,
                    self.message_ordinal,
                    field,
                    None,
                    None,
                    None,
                ));
            }
            value |= u32::from(byte & 0x7f) << (index * 7);
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(failure(
            Classification::Malformed,
            ErrorCode::InvalidVarint,
            start..self.bit,
            self.message_ordinal,
            field,
            None,
            None,
            None,
        ))
    }

    pub fn read_ubit_var(&mut self, field: &'static str) -> Result<u32, CodecError> {
        let first = self.read_unsigned(6, field)?;
        let encoding = first & 3;
        if encoding == 0 {
            return Ok(first >> 2);
        }
        self.bit -= 4;
        let width = match encoding {
            1 => 8,
            2 => 12,
            3 => 32,
            _ => unreachable!(),
        };
        self.read_unsigned(width, field)
    }

    pub fn read_c_string(
        &mut self,
        max_bytes: usize,
        field: &'static str,
    ) -> Result<Vec<u8>, CodecError> {
        let start = self.bit;
        let mut value = Vec::new();
        loop {
            if self.bits_left() < 8 {
                return Err(failure(
                    Classification::Malformed,
                    ErrorCode::UnterminatedString,
                    start..self.end,
                    self.message_ordinal,
                    field,
                    None,
                    Some(value.len() as u64),
                    Some(max_bytes as u64),
                ));
            }
            let byte = self.read_u8(field)?;
            if byte == 0 {
                return Ok(value);
            }
            if value.len() >= max_bytes {
                return Err(failure(
                    Classification::Malformed,
                    ErrorCode::StringLimit,
                    start..self.bit,
                    self.message_ordinal,
                    field,
                    Some((value.len() + 1) as i64),
                    None,
                    Some(max_bytes as u64),
                ));
            }
            value.push(byte);
        }
    }

    pub fn skip(&mut self, bit_length: usize, field: &'static str) -> Result<BitSpan, CodecError> {
        let span = BitSpan {
            start: self.bit,
            length: bit_length,
        };
        let end = self
            .bit
            .checked_add(bit_length)
            .ok_or_else(|| self.truncated(self.bit..self.bit, field, bit_length))?;
        if end > self.end {
            return Err(self.truncated(self.bit..end, field, bit_length));
        }
        self.bit = end;
        Ok(span)
    }

    pub fn read_payload(
        &mut self,
        bit_length: usize,
        field: &'static str,
    ) -> Result<BitPayload, CodecError> {
        let span = self.skip(bit_length, field)?;
        Ok(self.copy_span(span))
    }

    pub fn copy_span(&self, span: BitSpan) -> BitPayload {
        let mut bytes = vec![0_u8; span.length.div_ceil(8)];
        for output_bit in 0..span.length {
            let source_bit = span.start + output_bit;
            let bit = (self.bytes[source_bit / 8] >> (source_bit % 8)) & 1;
            bytes[output_bit / 8] |= bit << (output_bit % 8);
        }
        BitPayload {
            bytes,
            bit_length: span.length,
        }
    }

    fn truncated(&self, range: Range<usize>, field: &'static str, width: usize) -> CodecError {
        failure(
            Classification::Malformed,
            ErrorCode::Truncated,
            range,
            self.message_ordinal,
            field,
            Some(width as i64),
            Some(self.bits_left() as u64),
            None,
        )
    }
}
