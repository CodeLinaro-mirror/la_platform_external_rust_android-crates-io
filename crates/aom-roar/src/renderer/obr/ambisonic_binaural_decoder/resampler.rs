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

//! Rational audio sample rate resampler.
//!
//! Provides the resampler `Resampler` which uses a polyphase filter structure to convert
//! audio sample rates cleanly between different target rates.

use super::dsp_utils::generate_hann_window;
use crate::common::audio_buffer::AudioBuffer;
use crate::renderer::obr::common::constants::{
    EPSILON_FLOAT, MAX_SUPPORTED_AMBISONIC_ORDER, MAX_SUPPORTED_NUM_FRAMES, NUM_MONO_CHANNELS,
};
use crate::renderer::obr::common::misc_math::find_gcd;

const TRANSITION_BANDWIDTH_RATIO: usize = 13;
const MAX_NUM_CHANNELS: usize =
    ((MAX_SUPPORTED_AMBISONIC_ORDER + 1) * (MAX_SUPPORTED_AMBISONIC_ORDER + 1)) as usize;

/// Polyphase rational sample rate converter.
///
/// Pre-allocates and designs FIR interpolating filters at rate-change events to ensure
/// zero heap allocations on real-time execution loops.
#[derive(Debug, Clone, PartialEq)]
pub struct Resampler {
    up_rate: usize,
    down_rate: usize,
    time_modulo_up_rate: usize,
    last_processed_sample: usize,
    num_channels: usize,
    coeffs_per_phase: usize,
    transposed_filter_coeffs: AudioBuffer,
    temporary_filter_coeffs: AudioBuffer,
    state: AudioBuffer,
}

impl Default for Resampler {
    fn default() -> Self {
        Self::new()
    }
}

impl Resampler {
    /// Constructs a new `Resampler` instance.
    pub fn new() -> Self {
        let mut state = AudioBuffer::new(MAX_NUM_CHANNELS, MAX_SUPPORTED_NUM_FRAMES);
        state.clear();
        Self {
            up_rate: 0,
            down_rate: 0,
            time_modulo_up_rate: 0,
            last_processed_sample: 0,
            num_channels: 0,
            coeffs_per_phase: 0,
            transposed_filter_coeffs: AudioBuffer::new(NUM_MONO_CHANNELS, MAX_SUPPORTED_NUM_FRAMES),
            temporary_filter_coeffs: AudioBuffer::new(NUM_MONO_CHANNELS, MAX_SUPPORTED_NUM_FRAMES),
            state,
        }
    }

    /// Resamples `input` and writes the results into `output`.
    pub fn process(&mut self, input: &AudioBuffer, output: &mut AudioBuffer) {
        assert_eq!(input.num_channels(), self.num_channels);
        let input_length = input.num_frames();
        assert!(output.num_frames() >= self.get_next_output_length(input_length));
        assert!(output.num_frames() <= self.get_max_output_length(input_length));
        assert_eq!(output.num_channels(), self.num_channels);
        output.clear();

        if self.up_rate == self.down_rate {
            for c in 0..self.num_channels {
                output.channel_mut(c).as_mut_slice().copy_from_slice(input.channel(c).as_slice());
            }
            return;
        }

        let mut input_sample = self.last_processed_sample;
        let mut output_sample = 0;
        let filter_ch = self.transposed_filter_coeffs.channel(0);
        let filter_coefficients = filter_ch.as_slice();

        // TODO(b/525080422): Optimize by swapping loops to process channel-by-channel and using
        // local accumulation to avoid calling channel_mut in the inner loop.
        while input_sample < input_length {
            let mut filter_index = self.time_modulo_up_rate * self.coeffs_per_phase;
            let mut offset_input_index = if input_sample >= self.coeffs_per_phase.saturating_sub(1)
            {
                input_sample + 1 - self.coeffs_per_phase
            } else {
                0
            };
            let offset = -(input_sample as isize - self.coeffs_per_phase as isize + 1);

            if offset > 0 {
                let state_num_frames = (self.coeffs_per_phase - 1) as isize;
                let mut state_index = (state_num_frames - offset) as usize;
                while (state_index as isize) < state_num_frames {
                    for channel in 0..self.num_channels {
                        output.channel_mut(channel)[output_sample] +=
                            self.state[channel][state_index] * filter_coefficients[filter_index];
                    }
                    state_index += 1;
                    filter_index += 1;
                }
            }

            while offset_input_index <= input_sample {
                for channel in 0..self.num_channels {
                    output.channel_mut(channel)[output_sample] +=
                        input[channel][offset_input_index] * filter_coefficients[filter_index];
                }
                offset_input_index += 1;
                filter_index += 1;
            }
            output_sample += 1;

            self.time_modulo_up_rate += self.down_rate;
            input_sample += self.time_modulo_up_rate / self.up_rate;
            self.time_modulo_up_rate %= self.up_rate;
        }
        assert!(input_sample >= input_length);
        self.last_processed_sample = input_sample - input_length;

        let samples_left_in_input = (self.coeffs_per_phase as isize - 1) - input_length as isize;
        if samples_left_in_input > 0 {
            let left_usize = samples_left_in_input as usize;
            for channel in 0..self.num_channels {
                let mut state_ch = self.state.channel_mut(channel);
                let state_slice = state_ch.as_mut_slice();
                let state_len = self.coeffs_per_phase - 1;
                // TODO(b/525080422): Optimize by using copy_within to copy elements within the same
                // slice.
                for i in 0..left_usize {
                    state_slice[i] = state_slice[state_len - left_usize + i];
                }
                let src = input.channel(channel);
                let src_slice = src.as_slice();
                state_slice[left_usize..(left_usize + input_length)]
                    .copy_from_slice(&src_slice[..input_length]);
            }
        } else {
            for channel in 0..self.num_channels {
                assert!(self.coeffs_per_phase > 0);
                let state_len = self.coeffs_per_phase - 1;
                let src = input.channel(channel);
                assert!(src.size() >= state_len);
                let src_slice = src.as_slice();
                let start_idx = src.size() - state_len;
                let mut state_ch = self.state.channel_mut(channel);
                let state_slice = state_ch.as_mut_slice();
                state_slice[..state_len]
                    .copy_from_slice(&src_slice[start_idx..(start_idx + state_len)]);
            }
        }
    }

    /// Returns the maximum possible length of the resampled output block.
    pub fn get_max_output_length(&self, input_length: usize) -> usize {
        if self.up_rate == self.down_rate {
            return input_length;
        }
        assert!(self.down_rate > 0);
        (input_length * self.up_rate) / self.down_rate + 1
    }

    /// Returns the exact length of the next resampled output block.
    pub fn get_next_output_length(&self, input_length: usize) -> usize {
        if self.up_rate == self.down_rate {
            return input_length;
        }
        let max_len = self.get_max_output_length(input_length);
        if (self.time_modulo_up_rate + self.up_rate * self.last_processed_sample)
            >= ((input_length * self.up_rate) % self.down_rate)
        {
            if max_len > 0 {
                max_len - 1
            } else {
                0
            }
        } else {
            max_len
        }
    }

    /// Configures the resampler rates and input channel count.
    pub fn set_rate_and_num_channels(
        &mut self,
        source_frequency: i32,
        destination_frequency: i32,
        num_channels: usize,
    ) {
        assert!(source_frequency > 0);
        assert!(destination_frequency > 0);
        assert!(num_channels > 0);

        let gcd = find_gcd(destination_frequency, source_frequency) as usize;
        let dest = (destination_frequency as usize) / gcd;
        let src = (source_frequency as usize) / gcd;

        let old_state_size = if self.coeffs_per_phase > 0 { self.coeffs_per_phase - 1 } else { 0 };
        if dest != self.up_rate || src != self.down_rate {
            self.up_rate = dest;
            self.down_rate = src;
            if self.up_rate != self.down_rate {
                self.generate_interpolating_filter(source_frequency);
            }
            self.time_modulo_up_rate = 0;
        }

        if self.num_channels != num_channels {
            self.num_channels = num_channels;
            self.initialize_state_buffer(old_state_size);
        }
    }

    /// Returns whether the rate conversion from `source` to `destination` is supported.
    pub fn are_sample_rates_supported(source: i32, destination: i32) -> bool {
        assert!(source > 0 && destination > 0);
        let max_rate = source.max(destination) / find_gcd(source, destination);
        let mut filter_length = (max_rate as usize) * TRANSITION_BANDWIDTH_RATIO;
        filter_length += filter_length % 2;
        filter_length <= MAX_SUPPORTED_NUM_FRAMES
    }

    /// Resets the internal state and history buffers.
    pub fn reset_state(&mut self) {
        self.time_modulo_up_rate = 0;
        self.last_processed_sample = 0;
        self.state.clear();
    }

    fn initialize_state_buffer(&mut self, old_state_num_frames: usize) {
        if self.up_rate == self.down_rate || self.num_channels == 0 {
            return;
        }
        let new_state_num_frames =
            if self.coeffs_per_phase > 0 { self.coeffs_per_phase - 1 } else { 0 };
        if old_state_num_frames != new_state_num_frames {
            let min_size = new_state_num_frames.min(old_state_num_frames);
            let max_size = new_state_num_frames.max(old_state_num_frames);
            for channel in 0..self.num_channels {
                let mut state_ch = self.state.channel_mut(channel);
                let slice = state_ch.as_mut_slice();
                assert!(slice.len() >= max_size);
                for value in slice.iter_mut().take(max_size).skip(min_size) {
                    *value = 0.0;
                }
            }
        }
    }

    fn generate_interpolating_filter(&mut self, sample_rate: i32) {
        let max_rate = self.up_rate.max(self.down_rate);
        let cutoff_frequency = (sample_rate as f32) / (2 * max_rate) as f32;
        let mut filter_length = max_rate * TRANSITION_BANDWIDTH_RATIO;
        filter_length += filter_length % 2;

        self.temporary_filter_coeffs.clear();
        self.generate_sinc_filter(cutoff_frequency, sample_rate as f32, filter_length);

        // Pad out the filter length so that it can be arranged in polyphase fashion.
        // The polyphase decomposition has `self.up_rate` branches, so the per-branch
        // length must be derived from `self.up_rate`. Deriving it from `max_rate`
        // would drop the tail of the filter whenever `down_rate > up_rate`.
        let transposed_length = filter_length + self.up_rate - (filter_length % self.up_rate);
        self.coeffs_per_phase = transposed_length / self.up_rate;
        self.arrange_filter_as_polyphase(filter_length);
    }

    fn arrange_filter_as_polyphase(&mut self, filter_length: usize) {
        self.transposed_filter_coeffs.clear();
        for i in 0..self.up_rate {
            for j in 0..self.coeffs_per_phase {
                if j * self.up_rate + i < filter_length {
                    let coeff_idx = (self.coeffs_per_phase - 1 - j) + i * self.coeffs_per_phase;
                    self.transposed_filter_coeffs.channel_mut(0)[coeff_idx] =
                        self.temporary_filter_coeffs[0][j * self.up_rate + i];
                }
            }
        }
    }

    fn generate_sinc_filter(
        &mut self,
        cutoff_frequency: f32,
        sample_rate: f32,
        filter_length: usize,
    ) {
        assert!(sample_rate > 0.0);
        let angular_cutoff_frequency = std::f32::consts::TAU * cutoff_frequency / sample_rate;
        let half_filter_length = filter_length / 2;

        {
            let mut filter_ch = self.temporary_filter_coeffs.channel_mut(0);
            let slice = filter_ch.as_mut_slice();
            generate_hann_window(true, filter_length, slice);

            for i in 0..filter_length {
                if i == half_filter_length {
                    slice[half_filter_length] *= angular_cutoff_frequency;
                } else {
                    let denominator = i as f32 - (filter_length as f32 / 2.0);
                    assert!(denominator.abs() > EPSILON_FLOAT);
                    slice[i] *= (angular_cutoff_frequency * denominator).sin() / denominator;
                }
            }

            let sum: f32 = slice[..filter_length].iter().sum();
            let normalizing_factor = (self.up_rate as f32) / sum;
            for value in slice.iter_mut().take(filter_length) {
                *value *= normalizing_factor;
            }
        }
    }
}

// ===== Tests =====

#[cfg(test)]
mod test {

    use super::*;
    use googletest::prelude::*;
    use rstest::rstest;

    #[gtest]
    fn test_resampler_identity_passthrough() {
        let mut r = Resampler::new();
        r.set_rate_and_num_channels(48000, 48000, 1);

        let mut input = AudioBuffer::new(1, 10);
        input.copy_from_2d(&[vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0]]);

        let next_len = r.get_next_output_length(10);
        assert_eq!(next_len, 10);

        let mut output = AudioBuffer::new(1, 10);
        r.process(&input, &mut output);

        for i in 0..10 {
            expect_that!(output[0][i], eq(input[0][i]));
        }
    }

    #[gtest]
    fn test_resampler_supported_rates() {
        // Supported
        assert!(Resampler::are_sample_rates_supported(48000, 44100));
        assert!(Resampler::are_sample_rates_supported(16000, 48000));
        // Mismatched ratios requiring massive filter sizes should return false
        // e.g. 44100 to 44101 (gcd = 1, requires huge filter)
        assert!(!Resampler::are_sample_rates_supported(44100, 44101));
    }

    #[gtest]
    fn test_resampler_upsample_constant_dc_signal() {
        let mut r = Resampler::new();
        r.set_rate_and_num_channels(24000, 48000, 1); // 1:2 upsampling

        let input_len = 64;
        let mut input = AudioBuffer::new(1, input_len);
        input.copy_from_2d(&[vec![1.0; input_len]]);

        let next_len = r.get_next_output_length(input_len);
        let mut output = AudioBuffer::new(1, next_len);

        r.process(&input, &mut output);

        // Verify the middle of the buffer (to avoid filter boundary transient ripple effects)
        // is close to 1.0. Sinc interpolation of constant signal yields constant signal.
        let mid_start = next_len / 4;
        let mid_end = 3 * next_len / 4;
        for i in mid_start..mid_end {
            expect_that!(output[0][i], near(1.0, 0.02)); // 2% tolerance for boundary leakage /
                                                         // ripple
        }
    }

    const TONE_FREQUENCY_HZ: f32 = 1000.0;

    /// Helper to generate a sine wave with the given frequency, sample rate, and number of samples.
    fn generate_sine_wave(frequency_hz: f32, sample_rate: i32, num_samples: usize) -> AudioBuffer {
        let mut buffer = AudioBuffer::new(NUM_MONO_CHANNELS, num_samples);
        let mut ch = buffer.channel_mut(0);
        for (i, sample) in ch.as_mut_slice().iter_mut().enumerate() {
            let phase = (i as f32) * std::f32::consts::TAU / (sample_rate as f32) * frequency_hz;
            *sample = phase.sin();
        }
        buffer
    }

    /// Computes the sine wave amplitude from the RMS of the middle half of the buffer,
    /// skipping filter warm-up and tail transients.
    fn middle_half_sine_amplitude(buffer: &AudioBuffer) -> f64 {
        let begin = buffer.num_frames() / 4;
        let end = 3 * buffer.num_frames() / 4;
        let sum_squares: f64 =
            buffer.channel(0).as_slice()[begin..end].iter().map(|&s| (s as f64) * (s as f64)).sum();
        (2.0 * sum_squares / (end - begin) as f64).sqrt()
    }

    // Verifies that changing the sample rate does not change the volume of an audible (1 kHz)
    // tone: a sine wave with amplitude 1.0 in should still have amplitude ~1.0 out.
    #[rstest]
    #[case(44100, 48000)]
    #[case(48000, 44100)]
    #[case(88200, 48000)]
    #[case(96000, 48000)]
    #[case(176400, 48000)]
    #[case(192000, 48000)]
    #[case(48000, 96000)]
    #[case(48000, 32000)]
    #[googletest::test]
    fn passband_gain_is_unity_across_rate_pairs(
        #[case] source_rate: i32,
        #[case] destination_rate: i32,
    ) {
        assert!(Resampler::are_sample_rates_supported(source_rate, destination_rate));
        let input_length = (source_rate / 10) as usize;
        let input = generate_sine_wave(TONE_FREQUENCY_HZ, source_rate, input_length);
        let mut resampler = Resampler::new();
        resampler.set_rate_and_num_channels(source_rate, destination_rate, NUM_MONO_CHANNELS);
        let mut output =
            AudioBuffer::new(NUM_MONO_CHANNELS, resampler.get_next_output_length(input_length));

        resampler.process(&input, &mut output);

        // +-0.012 is roughly +-0.1 dB.
        let amplitude = middle_half_sine_amplitude(&output);
        expect_that!(amplitude, near(1.0, 0.012));
    }

    // Verifies that when converting to a lower sample rate, frequencies too high for the target
    // rate to represent (above half the target sample rate, the "Nyquist" limit) are filtered out
    // to near-silence rather than folding back ("aliasing") into the output as audible artifacts.
    // Each test tone sits at 1.5x the destination Nyquist frequency.
    #[rstest]
    #[case(96000, 48000, 36000.0)]
    #[case(192000, 48000, 72000.0)]
    #[case(88200, 44100, 33000.0)]
    #[case(88200, 48000, 36000.0)]
    #[googletest::test]
    fn downsampling_attenuates_above_destination_nyquist(
        #[case] source_rate: i32,
        #[case] destination_rate: i32,
        #[case] tone_frequency_hz: f32,
    ) {
        let input_length = (source_rate / 10) as usize;
        let input = generate_sine_wave(tone_frequency_hz, source_rate, input_length);
        let mut resampler = Resampler::new();
        resampler.set_rate_and_num_channels(source_rate, destination_rate, NUM_MONO_CHANNELS);
        let mut output =
            AudioBuffer::new(NUM_MONO_CHANNELS, resampler.get_next_output_length(input_length));

        resampler.process(&input, &mut output);

        // 0.02 is roughly -34 dB.
        let amplitude = middle_half_sine_amplitude(&output);
        expect_that!(amplitude, lt(0.02));
    }

    // Verifies that feeding audio to the resampler in small streaming chunks produces the exact
    // same output samples as resampling the entire buffer in a single call, confirming that the
    // resampler's internal history buffer (`self.state`, active length `coeffs_per_phase - 1`)
    // preserves continuity across chunk boundaries.
    #[rstest]
    // 2:1 downsampling (active state = 26 samples): chunk (16) is shorter than the filter state,
    // exercising the state-buffer shift-and-append path in `process`.
    #[case(96000, 48000, 16)]
    // 1:2 upsampling (active state = 13 samples): chunk (8) is shorter than the filter state.
    #[case(48000, 96000, 8)]
    // 4:1 downsampling (active state = 52 samples): chunk (160) is longer than the filter state,
    // exercising the full state-buffer overwrite path in `process`.
    #[case(192000, 48000, 160)]
    // Non-integer 160:147 upsampling (active state = 13 samples): chunk (63) > state.
    #[case(44100, 48000, 63)]
    // Non-integer 147:160 downsampling (active state = 14 samples): chunk (441) > state.
    #[case(48000, 44100, 441)]
    // 1:2 upsampling (active state = 13 samples): chunk (100) is longer than the filter state.
    #[case(48000, 96000, 100)]
    #[googletest::test]
    fn chunked_processing_matches_one_shot(
        #[case] source_rate: i32,
        #[case] destination_rate: i32,
        #[case] chunk_size: usize,
    ) {
        let input_length = 4410;
        let input = generate_sine_wave(TONE_FREQUENCY_HZ, source_rate, input_length);
        let mut one_shot = Resampler::new();
        one_shot.set_rate_and_num_channels(source_rate, destination_rate, NUM_MONO_CHANNELS);
        let mut one_shot_output =
            AudioBuffer::new(NUM_MONO_CHANNELS, one_shot.get_next_output_length(input_length));
        let mut chunked = Resampler::new();
        chunked.set_rate_and_num_channels(source_rate, destination_rate, NUM_MONO_CHANNELS);
        let mut chunked_output = Vec::new();

        // Resample the entire input in a single pass as the reference output.
        one_shot.process(&input, &mut one_shot_output);
        // Resample the same input incrementally in `chunk_size` blocks, concatenating each block's
        // output into `chunked_output`.
        let mut offset = 0;
        while offset < input_length {
            let this_chunk = chunk_size.min(input_length - offset);
            let mut chunk = AudioBuffer::new(NUM_MONO_CHANNELS, this_chunk);
            chunk
                .channel_mut(0)
                .as_mut_slice()
                .copy_from_slice(&input.channel(0).as_slice()[offset..offset + this_chunk]);
            let mut chunk_output =
                AudioBuffer::new(NUM_MONO_CHANNELS, chunked.get_next_output_length(this_chunk));
            chunked.process(&chunk, &mut chunk_output);
            chunked_output.extend_from_slice(chunk_output.channel(0).as_slice());
            offset += this_chunk;
        }

        // The concatenated chunked output must match the one-shot output sample-for-sample.
        assert_eq!(one_shot_output.num_frames(), chunked_output.len());
        for (i, &actual) in chunked_output.iter().enumerate() {
            expect_that!(actual, near(one_shot_output[0][i], EPSILON_FLOAT));
        }
    }
}
