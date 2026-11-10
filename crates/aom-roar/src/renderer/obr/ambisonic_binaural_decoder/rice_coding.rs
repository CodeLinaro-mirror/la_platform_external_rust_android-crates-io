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

//! Block-variable Rice decoder for embedded planar 16-bit binaural HRIR filters.
//!
//! Each filter asset begins with a 2-byte header `[num_channels, blocks_per_channel]`, followed by
//! `num_channels * blocks_per_channel` byte-aligned blocks in planar order (all blocks for channel
//! 0, then channel 1, and so on). Each block represents [`BLOCK_SIZE`] (256) signed 16-bit PCM
//! samples and begins with a 1-byte Rice parameter `k` (where the divisor is `M = 1 << k`),
//! followed by the bit-packed Rice-encoded samples padded with zero bits to the next byte boundary.

use super::rice_block_decoder::{RiceBlockDecoder, BLOCK_SIZE};
use crate::common::audio_buffer::simd_utils::float_from_int16;
use crate::common::audio_buffer::AudioBuffer;

/// Size in bytes of the stream header (`[num_channels, blocks_per_channel]`).
pub const HEADER_SIZE: usize = 2;

/// Decodes a complete Rice-encoded planar binaural filter asset into an [`AudioBuffer`].
pub struct RiceFilterDecoder;

impl RiceFilterDecoder {
    /// Decodes a planar Rice-encoded filter asset byte stream into a planar `f32` [`AudioBuffer`].
    ///
    /// # Arguments
    /// * `bytes` - Complete encoded asset starting with `[num_channels, blocks_per_channel]`.
    ///
    /// Returns `None` if the header is invalid, any block fails to decode, or trailing bytes
    /// remain after decoding all expected blocks.
    pub fn decode_to_audio_buffer(bytes: &[u8]) -> Option<AudioBuffer> {
        if bytes.len() < HEADER_SIZE {
            return None;
        }

        let num_channels = usize::from(bytes[0]);
        let blocks_per_channel = usize::from(bytes[1]);
        if num_channels == 0 || blocks_per_channel == 0 {
            return None;
        }

        let num_frames = blocks_per_channel * BLOCK_SIZE;
        let mut audio_buffer = AudioBuffer::new(num_channels, num_frames);
        let mut cursor = HEADER_SIZE;
        let mut decoded_block = [0_i16; BLOCK_SIZE];

        for ch in 0..num_channels {
            let mut channel_view = audio_buffer.channel_mut(ch);
            let channel_slice = channel_view.as_mut_slice();
            for block_idx in 0..blocks_per_channel {
                let consumed =
                    RiceBlockDecoder::decode_block(&bytes[cursor..], &mut decoded_block)?;
                cursor += consumed;
                let frame_start = block_idx * BLOCK_SIZE;
                let frame_end = frame_start + BLOCK_SIZE;
                float_from_int16(&decoded_block, &mut channel_slice[frame_start..frame_end]);
            }
        }

        if cursor != bytes.len() {
            return None;
        }

        Some(audio_buffer)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use googletest::prelude::*;

    #[gtest]
    fn decode_to_audio_buffer_rejects_invalid_headers() {
        expect_true!(RiceFilterDecoder::decode_to_audio_buffer(&[]).is_none());
        expect_true!(RiceFilterDecoder::decode_to_audio_buffer(&[0, 1]).is_none());
        expect_true!(RiceFilterDecoder::decode_to_audio_buffer(&[1, 0]).is_none());
    }

    #[gtest]
    fn decode_to_audio_buffer_rejects_trailing_bytes() {
        // Valid 1-channel, 1-block of zeros (2 header bytes + 1 k-byte + 32 data bytes = 35 bytes),
        // plus 1 unexpected trailing byte.
        let mut with_trailing = vec![1_u8, 1_u8, 0_u8];
        with_trailing.extend_from_slice(&[0xFF; 32]);
        with_trailing.push(0x00);

        expect_true!(RiceFilterDecoder::decode_to_audio_buffer(&with_trailing).is_none());
    }

    #[gtest]
    fn decode_to_audio_buffer_decodes_single_channel_single_block() {
        // 1 channel, 1 block = 256 samples of zeros.
        let mut encoded = vec![1_u8, 1_u8, 0_u8];
        encoded.extend_from_slice(&[0xFF; 32]);

        let audio_buffer = RiceFilterDecoder::decode_to_audio_buffer(&encoded);
        expect_true!(audio_buffer.is_some());
        let buffer = audio_buffer.unwrap();

        expect_that!(buffer.num_channels(), eq(1));
        expect_that!(buffer.num_frames(), eq(BLOCK_SIZE));
        expect_that!(buffer.channel(0).as_slice(), each(eq(&0.0_f32)));
    }

    #[gtest]
    fn decode_to_audio_buffer_decodes_real_filter_asset() {
        use crate::renderer::obr::ambisonic_binaural_decoder::binaural_filters::BinauralFiltersWrapper;

        let wrapper = BinauralFiltersWrapper::new();
        let asset_bytes = wrapper.get_file("1OADirectL");
        expect_true!(asset_bytes.is_some());

        let audio_buffer = RiceFilterDecoder::decode_to_audio_buffer(asset_bytes.unwrap());
        expect_true!(audio_buffer.is_some());
        let buffer = audio_buffer.unwrap();

        // 1OA direct has 4 channels and 256 samples per channel.
        expect_that!(buffer.num_channels(), eq(4));
        expect_that!(buffer.num_frames(), eq(BLOCK_SIZE));

        // Ensure non-trivial audio was decoded (not all silence).
        let max_amplitude = (0..buffer.num_channels())
            .flat_map(|ch| buffer.channel(ch).as_slice().iter().copied().collect::<Vec<_>>())
            .fold(0.0_f32, |acc, val| acc.max(val.abs()));
        expect_that!(max_amplitude, gt(0.0_f32));
    }
}
