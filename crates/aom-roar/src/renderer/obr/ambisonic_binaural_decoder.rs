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

//! Higher-Order Ambisonic (HOA) frequency-domain binaural decoder.
//!
//! Convolves spatial audio channels represented in spherical harmonics with custom HRIR filter sets
//! to yield a stereo binaural headphone signal.

pub mod binaural_filters;
pub mod dsp_utils;
pub mod fft_manager;
pub mod partitioned_fft_filter;
pub mod resampler;
pub mod rice_block_decoder;
pub mod rice_coding;
pub mod sh_hrir_creator;

use crate::common::audio_buffer::{AudioBuffer, ChannelViewMut};
use crate::renderer::obr::common::constants::{
    NUM_BINAURAL_CHANNELS, NUM_MONO_CHANNELS, NUM_STEREO_CHANNELS,
};
pub use fft_manager::FftManager;
use partitioned_fft_filter::PartitionedFftFilter;
pub use resampler::Resampler;
pub use sh_hrir_creator::create_sh_hrirs_from_assets;

/// HOA frequency-domain binaural decoder.
///
/// Converts input Ambisonic buffers into the frequency domain, performs convolution
/// with spherical harmonic Head-Related Impulse Response (HRIR) kernels across all channels
/// in the frequency domain, and translates the accumulated signals back to the time domain
/// with only two inverse FFTs (one per binaural channel).
#[derive(Debug, Clone, PartialEq)]
pub struct AmbisonicBinauralDecoder {
    sh_hrir_filters_l: Vec<PartitionedFftFilter>,
    sh_hrir_filters_r: Vec<PartitionedFftFilter>,
    freq_input: AudioBuffer,
    freq_accumulator_l: AudioBuffer,
    freq_accumulator_r: AudioBuffer,
    filtered_time_domain_buffers_l: AudioBuffer,
    filtered_time_domain_buffers_r: AudioBuffer,
    temp_zeropad_buffer: AudioBuffer,
    buffer_selector: usize,
    frames_per_buffer: usize,
    chunk_size: usize,
}

impl AmbisonicBinauralDecoder {
    /// Constructs a new `AmbisonicBinauralDecoder` instance.
    pub fn new(
        sh_hrirs_l: &AudioBuffer,
        sh_hrirs_r: &AudioBuffer,
        frames_per_buffer: usize,
        fft_manager: &mut FftManager,
    ) -> Self {
        assert!(frames_per_buffer > 0);
        let num_channels = sh_hrirs_l.num_channels();
        let filter_size = sh_hrirs_l.num_frames();
        assert!(num_channels > 0 && filter_size > 0);
        assert_eq!(sh_hrirs_r.num_channels(), num_channels);
        assert_eq!(sh_hrirs_r.num_frames(), filter_size);

        let mut sh_hrir_filters_l = Vec::with_capacity(num_channels);
        for i in 0..num_channels {
            let mut filter = PartitionedFftFilter::new_constant_size(
                filter_size,
                frames_per_buffer,
                fft_manager,
            );
            filter.set_time_domain_kernel(&sh_hrirs_l.channel(i), fft_manager);
            sh_hrir_filters_l.push(filter);
        }

        let mut sh_hrir_filters_r = Vec::with_capacity(num_channels);
        for i in 0..num_channels {
            let mut filter = PartitionedFftFilter::new_constant_size(
                filter_size,
                frames_per_buffer,
                fft_manager,
            );
            filter.set_time_domain_kernel(&sh_hrirs_r.channel(i), fft_manager);
            sh_hrir_filters_r.push(filter);
        }

        let fft_size = fft_manager.get_fft_size();
        let chunk_size = fft_size / 2;
        Self {
            sh_hrir_filters_l,
            sh_hrir_filters_r,
            freq_input: AudioBuffer::new(NUM_MONO_CHANNELS, fft_size),
            freq_accumulator_l: AudioBuffer::new(NUM_MONO_CHANNELS, fft_size),
            freq_accumulator_r: AudioBuffer::new(NUM_MONO_CHANNELS, fft_size),
            filtered_time_domain_buffers_l: AudioBuffer::new(NUM_STEREO_CHANNELS, fft_size),
            filtered_time_domain_buffers_r: AudioBuffer::new(NUM_STEREO_CHANNELS, fft_size),
            temp_zeropad_buffer: AudioBuffer::new(NUM_MONO_CHANNELS, chunk_size),
            buffer_selector: 0,
            frames_per_buffer,
            chunk_size,
        }
    }

    /// Processes planar Ambisonic sound field `input` and renders the binauralised stereo `output`.
    pub fn process_audio_buffer(
        &mut self,
        input: &AudioBuffer,
        output: &mut AudioBuffer,
        fft_manager: &mut FftManager,
    ) {
        assert_eq!(input.num_channels(), self.sh_hrir_filters_l.len());
        assert_eq!(input.num_channels(), self.sh_hrir_filters_r.len());
        assert_eq!(input.num_frames(), output.num_frames());
        assert_eq!(output.num_channels(), NUM_BINAURAL_CHANNELS);
        output.clear();
        self.freq_accumulator_l.clear();
        self.freq_accumulator_r.clear();
        // 1. Transform each input channel to frequency domain and accumulate
        // convolution products across all channels into the left & right frequency accumulators.
        for channel in 0..input.num_channels() {
            let src = input.channel(channel);
            // Compute Forward FFT once for this Ambisonic channel
            let mut freq_dst = self.freq_input.channel_mut(0);
            fft_manager.freq_from_time_domain(&src, &mut freq_dst);
            let freq_src = self.freq_input.channel(0);
            // Accumulate frequency-domain convolution for Left ear
            let mut acc_l = self.freq_accumulator_l.channel_mut(0);
            self.sh_hrir_filters_l[channel].filter_and_accumulate(
                &freq_src,
                &mut acc_l,
                fft_manager,
            );
            // Accumulate frequency-domain convolution for Right ear
            let mut acc_r = self.freq_accumulator_r.channel_mut(0);
            self.sh_hrir_filters_r[channel].filter_and_accumulate(
                &freq_src,
                &mut acc_r,
                fft_manager,
            );
        }

        // 2. Perform exactly 2 Inverse FFTs: one for Left, one for Right.
        self.buffer_selector = (self.buffer_selector + 1) % NUM_STEREO_CHANNELS;
        let acc_l = self.freq_accumulator_l.channel(0);
        let mut time_l = self.filtered_time_domain_buffers_l.channel_mut(self.buffer_selector);
        fft_manager.time_from_freq_domain(&acc_l, &mut time_l);
        let acc_r = self.freq_accumulator_r.channel(0);
        let mut time_r = self.filtered_time_domain_buffers_r.channel_mut(self.buffer_selector);
        fft_manager.time_from_freq_domain(&acc_r, &mut time_r);

        // 3. Overlap-add into Left and Right output channels.
        let curr_buffer = self.buffer_selector;
        let prev_buffer = (self.buffer_selector + 1) % NUM_STEREO_CHANNELS;
        Self::overlap_add(
            &self.filtered_time_domain_buffers_l,
            curr_buffer,
            prev_buffer,
            self.frames_per_buffer,
            self.chunk_size,
            &mut self.temp_zeropad_buffer,
            &mut output.channel_mut(0),
        );
        Self::overlap_add(
            &self.filtered_time_domain_buffers_r,
            curr_buffer,
            prev_buffer,
            self.frames_per_buffer,
            self.chunk_size,
            &mut self.temp_zeropad_buffer,
            &mut output.channel_mut(1),
        );
    }

    /// Performs time-domain overlap-add synthesis between the current and previous IFFT blocks,
    /// writing `frames_per_buffer` samples to `output`.
    ///
    /// Sums the head of `curr_buffer` with the tail of `prev_buffer`. When `frames_per_buffer <
    /// chunk_size` (non-power-of-two frame sizes), `temp_zeropad_buffer` stages the unpadded
    /// result.
    fn overlap_add(
        time_buffers: &AudioBuffer,
        curr_buffer: usize,
        prev_buffer: usize,
        frames_per_buffer: usize,
        chunk_size: usize,
        temp_zeropad_buffer: &mut AudioBuffer,
        output: &mut ChannelViewMut<'_>,
    ) {
        assert_eq!(output.size(), frames_per_buffer);
        let curr = time_buffers.channel(curr_buffer);
        let prev = time_buffers.channel(prev_buffer);
        let curr_slice = curr.as_slice();
        let prev_slice = prev.as_slice();

        if frames_per_buffer == chunk_size {
            let out_slice = output.as_mut_slice();
            for ((out, &c), &p) in out_slice[..chunk_size]
                .iter_mut()
                .zip(curr_slice.iter())
                .zip(prev_slice[chunk_size..].iter())
            {
                *out = c + p;
            }
        } else {
            let mut temp_ch = temp_zeropad_buffer.channel_mut(0);
            let temp_slice = temp_ch.as_mut_slice();
            for ((temp, &c), &p) in temp_slice[..frames_per_buffer]
                .iter_mut()
                .zip(curr_slice.iter())
                .zip(prev_slice[frames_per_buffer..].iter())
            {
                *temp = c + p;
            }
            output.as_mut_slice().copy_from_slice(&temp_slice[..frames_per_buffer]);
        }
    }
}

// ===== Tests =====

#[cfg(test)]
mod test {

    use super::*;
    use crate::common::definitions::SampleRate;
    use googletest::prelude::*;

    #[gtest]
    fn test_decoder_creation_and_processing() {
        let mut resampler = Resampler::new();
        // Use 1OA (Order 1, which has (1+1)^2 = 4 channels)
        let order = 1;
        let output_channel_count = 2;
        let num_input_channels = (order + 1) * (order + 1);
        let sample_rate = 48000;
        let frames_per_buffer = 256;

        resampler.set_rate_and_num_channels(sample_rate, sample_rate, num_input_channels);

        let sh_hrirs_l = create_sh_hrirs_from_assets(
            "1OADirectL",
            SampleRate::new(sample_rate as u32).unwrap(),
            &mut resampler,
        )
        .expect("Failed to load 1OADirectL");
        let sh_hrirs_r = create_sh_hrirs_from_assets(
            "1OADirectR",
            SampleRate::new(sample_rate as u32).unwrap(),
            &mut resampler,
        )
        .expect("Failed to load 1OADirectR");

        let mut fft_manager = FftManager::new(frames_per_buffer);
        let mut decoder = AmbisonicBinauralDecoder::new(
            &sh_hrirs_l,
            &sh_hrirs_r,
            frames_per_buffer,
            &mut fft_manager,
        );

        // Input: 4 channels, 256 frames
        let mut input = AudioBuffer::new(num_input_channels, frames_per_buffer);
        // Output: 2 channels (binaural), 256 frames
        let mut output = AudioBuffer::new(output_channel_count, frames_per_buffer);

        // Test with zero input -> should yield zero output
        input.clear();
        decoder.process_audio_buffer(&input, &mut output, &mut fft_manager);

        for c in 0..output_channel_count {
            for i in 0..frames_per_buffer {
                expect_that!(output[c][i], eq(0.0));
            }
        }

        // Test with some impulse input -> should yield non-zero output
        // Set an impulse in the first channel
        input[0][0] = 1.0;
        decoder.process_audio_buffer(&input, &mut output, &mut fft_manager);

        // Verify output is not all zeros (it should have impulse response)
        let mut has_non_zero = false;
        for c in 0..output_channel_count {
            for i in 0..frames_per_buffer {
                if output[c][i].abs() > 1e-6 {
                    has_non_zero = true;
                    break;
                }
            }
        }
        expect_true!(has_non_zero);
    }

    #[gtest]
    fn test_superposition_linearity() {
        let mut resampler = Resampler::new();
        let order = 1;
        let num_input_channels = (order + 1) * (order + 1);
        let sample_rate = 48000;
        let frames_per_buffer = 256;

        resampler.set_rate_and_num_channels(sample_rate, sample_rate, num_input_channels);

        let sh_hrirs_l = create_sh_hrirs_from_assets(
            "1OADirectL",
            SampleRate::new(sample_rate as u32).unwrap(),
            &mut resampler,
        )
        .expect("Failed to load 1OADirectL");
        let sh_hrirs_r = create_sh_hrirs_from_assets(
            "1OADirectR",
            SampleRate::new(sample_rate as u32).unwrap(),
            &mut resampler,
        )
        .expect("Failed to load 1OADirectR");

        let mut fft_manager = FftManager::new(frames_per_buffer);

        // Decoder 1: Processes channel 0 only
        let mut decoder_ch0 = AmbisonicBinauralDecoder::new(
            &sh_hrirs_l,
            &sh_hrirs_r,
            frames_per_buffer,
            &mut fft_manager,
        );
        let mut in_ch0 = AudioBuffer::new(num_input_channels, frames_per_buffer);
        in_ch0[0][0] = 0.6;
        in_ch0[0][10] = -0.3;
        let mut out_ch0 = AudioBuffer::new(2, frames_per_buffer);
        decoder_ch0.process_audio_buffer(&in_ch0, &mut out_ch0, &mut fft_manager);

        // Decoder 2: Processes channel 1 only
        let mut decoder_ch1 = AmbisonicBinauralDecoder::new(
            &sh_hrirs_l,
            &sh_hrirs_r,
            frames_per_buffer,
            &mut fft_manager,
        );
        let mut in_ch1 = AudioBuffer::new(num_input_channels, frames_per_buffer);
        in_ch1[1][5] = 0.4;
        in_ch1[1][20] = 0.8;
        let mut out_ch1 = AudioBuffer::new(2, frames_per_buffer);
        decoder_ch1.process_audio_buffer(&in_ch1, &mut out_ch1, &mut fft_manager);

        // Decoder Combined: Processes channel 0 + channel 1 combined
        let mut decoder_combined = AmbisonicBinauralDecoder::new(
            &sh_hrirs_l,
            &sh_hrirs_r,
            frames_per_buffer,
            &mut fft_manager,
        );
        let mut in_combined = AudioBuffer::new(num_input_channels, frames_per_buffer);
        in_combined[0][0] = 0.6;
        in_combined[0][10] = -0.3;
        in_combined[1][5] = 0.4;
        in_combined[1][20] = 0.8;
        let mut out_combined = AudioBuffer::new(2, frames_per_buffer);
        decoder_combined.process_audio_buffer(&in_combined, &mut out_combined, &mut fft_manager);

        // Verify linearity: out_combined == out_ch0 + out_ch1
        for ch in 0..2 {
            for frame in 0..frames_per_buffer {
                let expected = out_ch0[ch][frame] + out_ch1[ch][frame];
                expect_that!(out_combined[ch][frame], near(expected, 1e-5));
            }
        }
    }

    #[gtest]
    fn test_multi_block_continuous_processing() {
        let mut resampler = Resampler::new();
        let order = 1;
        let num_input_channels = (order + 1) * (order + 1);
        let sample_rate = 48000;
        let frames_per_buffer = 128;

        resampler.set_rate_and_num_channels(sample_rate, sample_rate, num_input_channels);

        let sh_hrirs_l = create_sh_hrirs_from_assets(
            "1OADirectL",
            SampleRate::new(sample_rate as u32).unwrap(),
            &mut resampler,
        )
        .expect("Failed to load 1OADirectL");
        let sh_hrirs_r = create_sh_hrirs_from_assets(
            "1OADirectR",
            SampleRate::new(sample_rate as u32).unwrap(),
            &mut resampler,
        )
        .expect("Failed to load 1OADirectR");

        let mut fft_manager = FftManager::new(frames_per_buffer);
        let mut decoder = AmbisonicBinauralDecoder::new(
            &sh_hrirs_l,
            &sh_hrirs_r,
            frames_per_buffer,
            &mut fft_manager,
        );

        let mut input = AudioBuffer::new(num_input_channels, frames_per_buffer);
        let mut output = AudioBuffer::new(2, frames_per_buffer);

        // Block 1: Impulse in channel 0
        input[0][0] = 1.0;
        decoder.process_audio_buffer(&input, &mut output, &mut fft_manager);
        let block1_has_signal = output[0].iter().any(|&s| s.abs() > 1e-6);
        expect_true!(block1_has_signal);

        // Block 2: Silence input (flushing filter tail across partition overlap-add)
        input.clear();
        decoder.process_audio_buffer(&input, &mut output, &mut fft_manager);
        let block2_has_tail = output[0].iter().any(|&s| s.abs() > 1e-6);
        expect_true!(block2_has_tail);
    }

    #[gtest]
    fn overlap_add_with_non_pow2_buffer_size_sums_correct_slices() {
        let chunk_size = 4;
        let frames_per_buffer = 3;
        let fft_size = 8;
        let curr_buffer = 0;
        let prev_buffer = 1;
        let mut time_buffers = AudioBuffer::new(NUM_STEREO_CHANNELS, fft_size);
        time_buffers
            .channel_mut(curr_buffer)
            .as_mut_slice()
            .copy_from_slice(&[10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0]);
        time_buffers
            .channel_mut(prev_buffer)
            .as_mut_slice()
            .copy_from_slice(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        let mut temp_zeropad_buffer = AudioBuffer::new(NUM_MONO_CHANNELS, chunk_size);
        let mut output = AudioBuffer::new(NUM_MONO_CHANNELS, frames_per_buffer);

        AmbisonicBinauralDecoder::overlap_add(
            &time_buffers,
            curr_buffer,
            prev_buffer,
            frames_per_buffer,
            chunk_size,
            &mut temp_zeropad_buffer,
            &mut output.channel_mut(0),
        );

        let out = output.channel(0).as_slice();
        expect_that!(out[0], eq(14.0));
        expect_that!(out[1], eq(25.0));
        expect_that!(out[2], eq(36.0));
    }

    #[gtest]
    fn overlap_add_with_matching_buffer_and_chunk_size_sums_correct_slices() {
        let chunk_size = 4;
        let frames_per_buffer = 4;
        let fft_size = 8;
        let curr_buffer = 0;
        let prev_buffer = 1;
        let mut time_buffers = AudioBuffer::new(NUM_STEREO_CHANNELS, fft_size);
        time_buffers
            .channel_mut(curr_buffer)
            .as_mut_slice()
            .copy_from_slice(&[10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0]);
        time_buffers
            .channel_mut(prev_buffer)
            .as_mut_slice()
            .copy_from_slice(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        let mut temp_zeropad_buffer = AudioBuffer::new(NUM_MONO_CHANNELS, chunk_size);
        let mut output = AudioBuffer::new(NUM_MONO_CHANNELS, frames_per_buffer);

        AmbisonicBinauralDecoder::overlap_add(
            &time_buffers,
            curr_buffer,
            prev_buffer,
            frames_per_buffer,
            chunk_size,
            &mut temp_zeropad_buffer,
            &mut output.channel_mut(0),
        );

        let out = output.channel(0).as_slice();
        expect_that!(out[0], eq(15.0));
        expect_that!(out[1], eq(26.0));
        expect_that!(out[2], eq(37.0));
        expect_that!(out[3], eq(48.0));
    }
}
