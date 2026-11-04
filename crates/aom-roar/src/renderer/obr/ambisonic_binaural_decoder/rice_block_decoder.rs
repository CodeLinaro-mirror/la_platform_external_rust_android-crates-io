// Copyright (c) 2026, Alliance for Open Media. All rights reserved
//
// This source code is subject to the terms of the BSD 3-Clause Clear License
// and the Alliance for Open Media Patent License 1.0. If the BSD 3-Clause Clear
// License was not distributed with this source code in the LICENSE file, you
// can obtain it at www.aomedia.org/license/software-license/bsd-3-c-c. If the
// Alliance for Open Media Patent License 1.0 was not distributed with this
// source code in the PATENTS file, you can obtain it at
// www.aomedia.org/license/patent.

#![forbid(unsafe_code)]

//! Low-level Rice block decoder and bit reader for 16-bit audio samples.

/// Number of audio samples per Rice-encoded block.
pub const BLOCK_SIZE: usize = 256;

/// Maximum valid Rice exponent `k` (`M = 1 << k`) for 16-bit signed audio samples.
pub const MAX_RICE_EXPONENT: u8 = 15;

/// Sequential MSB-first bit reader over a byte slice.
pub struct BitReader<'a> {
    bytes: &'a [u8],
    byte_index: usize,
    bit_offset: u8,
}

/// Decodes a single byte-aligned block of [`BLOCK_SIZE`] Rice-encoded 16-bit audio samples.
pub struct RiceBlockDecoder;

impl<'a> BitReader<'a> {
    /// Creates a new `BitReader` positioned at the first bit of `bytes`.
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, byte_index: 0, bit_offset: 0 }
    }

    /// Reads the next single bit (`0` or `1`) from the stream, or `None` if exhausted.
    pub fn read_bit(&mut self) -> Option<u32> {
        let current_byte = *self.bytes.get(self.byte_index)?;
        let bit = u32::from((current_byte >> (7 - self.bit_offset)) & 1);
        self.bit_offset += 1;
        if self.bit_offset == 8 {
            self.bit_offset = 0;
            self.byte_index += 1;
        }
        Some(bit)
    }

    /// Reads an unsigned integer of `num_bits` bits (MSB-first) from the stream.
    ///
    /// # Arguments
    /// * `num_bits` - Number of bits to read (`0..=16`). Returns `Some(0)` when `num_bits == 0`.
    pub fn read_bits(&mut self, num_bits: u8) -> Option<u32> {
        if num_bits > 16 {
            return None;
        }
        let mut value = 0_u32;
        for _ in 0..num_bits {
            let bit = self.read_bit()?;
            value = (value << 1) | bit;
        }
        Some(value)
    }

    /// Returns the total number of bytes touched so far, rounding up any partially read byte.
    pub fn bytes_consumed(&self) -> usize {
        if self.bit_offset == 0 {
            self.byte_index
        } else {
            self.byte_index + 1
        }
    }
}

impl RiceBlockDecoder {
    /// Converts a ZigZag-mapped unsigned integer back into a signed 16-bit integer.
    ///
    /// Even values map to non-negative integers (`0 -> 0, 2 -> 1, 4 -> 2, ...`) and odd values map
    /// to negative integers (`1 -> -1, 3 -> -2, ..., 65535 -> -32768`).
    ///
    /// # Arguments
    /// * `mapped` - Unsigned integer in the range `0..=65535`. Returns `None` if out of range.
    pub fn unmap_to_signed(mapped: u32) -> Option<i16> {
        if mapped > u32::from(u16::MAX) {
            return None;
        }
        let signed_wide = if mapped.is_multiple_of(2) {
            (mapped / 2) as i32
        } else {
            -(((mapped + 1) / 2) as i32)
        };
        Some(signed_wide as i16)
    }

    /// Decodes [`BLOCK_SIZE`] signed 16-bit samples from the start of `bytes` into `output`.
    ///
    /// # Arguments
    /// * `bytes` - Slice starting at the block's 1-byte exponent `k`.
    /// * `output` - Destination array for the decoded 16-bit PCM samples.
    ///
    /// Returns the total number of bytes consumed by this block (including the 1-byte exponent `k`
    /// and any byte-alignment padding bits), or `None` if `bytes` is malformed or truncated.
    pub fn decode_block(bytes: &[u8], output: &mut [i16; BLOCK_SIZE]) -> Option<usize> {
        let &k = bytes.first()?;
        if k > MAX_RICE_EXPONENT {
            return None;
        }

        let mut reader = BitReader::new(&bytes[1..]);
        for sample in output.iter_mut() {
            let mut quotient = 0_u32;
            while reader.read_bit()? == 0 {
                quotient = quotient.checked_add(1)?;
                // Guard against corrupted bitstreams without a stop bit.
                if quotient > u32::from(u16::MAX) {
                    return None;
                }
            }

            let remainder = reader.read_bits(k)?;
            let mapped = quotient.checked_shl(u32::from(k))?.checked_add(remainder)?;
            *sample = Self::unmap_to_signed(mapped)?;
        }

        Some(1 + reader.bytes_consumed())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use googletest::prelude::*;

    #[gtest]
    fn bit_reader_reads_msb_first_bits_and_tracks_consumed_bytes() {
        let data = [0b1011_0010, 0b0100_0000];
        let mut reader = BitReader::new(&data);

        let first_bit = reader.read_bit();
        let next_three_bits = reader.read_bits(3);
        let next_five_bits = reader.read_bits(5);

        expect_that!(first_bit, eq(Some(1)));
        expect_that!(next_three_bits, eq(Some(0b011)));
        expect_that!(next_five_bits, eq(Some(0b0010_0)));
        expect_that!(reader.bytes_consumed(), eq(2));
    }

    #[gtest]
    fn bit_reader_rejects_reading_more_than_16_bits() {
        let data = [0xFF; 4];
        let mut reader = BitReader::new(&data);
        expect_that!(reader.read_bits(17), eq(None));
    }

    #[gtest]
    fn unmap_to_signed_restores_zero_positive_negative_and_extremes() {
        expect_that!(RiceBlockDecoder::unmap_to_signed(0), eq(Some(0)));
        expect_that!(RiceBlockDecoder::unmap_to_signed(1), eq(Some(-1)));
        expect_that!(RiceBlockDecoder::unmap_to_signed(2), eq(Some(1)));
        expect_that!(RiceBlockDecoder::unmap_to_signed(3), eq(Some(-2)));
        expect_that!(RiceBlockDecoder::unmap_to_signed(4), eq(Some(2)));
        expect_that!(RiceBlockDecoder::unmap_to_signed(65534), eq(Some(32767)));
        expect_that!(RiceBlockDecoder::unmap_to_signed(65535), eq(Some(-32768)));
        expect_that!(RiceBlockDecoder::unmap_to_signed(65536), eq(None));
    }

    #[gtest]
    fn decode_block_with_k_zero_and_all_stop_bits_decodes_256_zeros() {
        // k = 0 means M = 1 (0 remainder bits). Each '1' bit is a stop bit for quotient = 0 ->
        // sample 0. 256 ones = 32 bytes of 0xFF, preceded by k = 0.
        let mut encoded = vec![0_u8];
        encoded.extend_from_slice(&[0xFF; 32]);
        let mut output = [99_i16; BLOCK_SIZE];

        let consumed = RiceBlockDecoder::decode_block(&encoded, &mut output);

        expect_that!(consumed, eq(Some(33)));
        expect_that!(output.as_slice(), eq(&[0_i16; BLOCK_SIZE]));
    }

    #[gtest]
    fn decode_block_decodes_known_rice_coded_first_samples() {
        let mut encoded = vec![1_u8, 0xD3];
        encoded.extend_from_slice(&[0xAA; 63]);
        encoded.push(0x80);
        let mut output = [0_i16; BLOCK_SIZE];

        let consumed = RiceBlockDecoder::decode_block(&encoded, &mut output);

        expect_that!(consumed, eq(Some(66)));
        expect_that!(output[0], eq(-1));
        expect_that!(output[1], eq(1));
        expect_that!(output[2], eq(-2));
        expect_that!(output[3], eq(0));
        expect_that!(output[255], eq(0));
    }
}
