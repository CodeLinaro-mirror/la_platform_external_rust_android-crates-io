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

//! Partitioned FFT overlap-add FIR (finite impulse response) convolution filter.
//!
//! Breaks up long FIR filters into time/frequency chunks to operate with low latency.

use super::dsp_utils::ceil_to_multiple_of_frames_per_buffer;
use super::fft_manager::FftManager;
use crate::common::audio_buffer::{AudioBuffer, ChannelView, ChannelViewMut};
use crate::renderer::obr::common::constants::NUM_MONO_CHANNELS;

/// Overlap-add partitioned FFT convolving filter.
///
/// Splits long FIR filter impulse responses into smaller block partitions, performing convolving
/// operations in the frequency domain. This allows low-latency rendering of large filter sets
/// (such as head-related impulse responses, HRIRs).
#[derive(Debug, Clone, PartialEq)]
pub struct PartitionedFftFilter {
    /// Number of points in the FFT (double the next power-of-two of `frames_per_buffer`),
    /// defining the length of each frequency-domain channel.
    fft_size: usize,
    /// Number of audio samples per input block.
    frames_per_buffer: usize,
    /// Maximum supported filter length in samples, bounding buffer capacity for dynamic resizing.
    max_filter_size: usize,
    /// Maximum number of partitions allocated in the frequency-domain buffers.
    max_num_partitions: usize,
    /// Active time-domain filter length in samples (rounded up to a multiple of
    /// `frames_per_buffer`).
    filter_size: usize,
    /// Active number of partitions (`filter_size / frames_per_buffer`) convolved during rendering.
    num_partitions: usize,
    /// Frequency-domain spectra for each partition of the impulse response kernel ($H_0 \dots
    /// H_{P-1}$).
    kernel_freq_domain_buffer: AudioBuffer,
    /// Ring-buffer index into `freq_domain_buffer` pointing to the current (most recent) input
    /// block.
    curr_front_buffer: usize,
    /// Circular history buffer storing the frequency-domain spectra of the last $P$ input blocks
    /// ($X_m \dots X_{m-(P-1)}$).
    freq_domain_buffer: AudioBuffer,
    /// Pre-allocated single-channel scratch buffer used to zero-pad time-domain kernel chunks
    /// before FFT transformation.
    temp_kernel_chunk_buffer: AudioBuffer,
}

impl PartitionedFftFilter {
    /// Pre-allocates memory for a filter where the target size equals the maximum supported size.
    ///
    /// # Parameters
    /// * `filter_size` - Length of the time-domain filter in samples.
    /// * `frames_per_buffer` - Number of audio samples per processing block.
    /// * `fft_manager` - FFT manager providing the configured FFT size.
    pub fn new_constant_size(
        filter_size: usize,
        frames_per_buffer: usize,
        fft_manager: &FftManager,
    ) -> Self {
        Self::new(filter_size, frames_per_buffer, filter_size, fft_manager)
    }

    /// Pre-allocates memory based on the `max_filter_size` and initial `filter_size`.
    ///
    /// # Parameters
    /// * `filter_size` - Initial length of the time-domain filter in samples.
    /// * `frames_per_buffer` - Number of audio samples per processing block.
    /// * `max_filter_size` - Maximum filter length in samples to accommodate dynamic resizing.
    /// * `fft_manager` - FFT manager providing the configured FFT size.
    pub fn new(
        filter_size: usize,
        frames_per_buffer: usize,
        max_filter_size: usize,
        fft_manager: &FftManager,
    ) -> Self {
        let fft_size = fft_manager.get_fft_size();
        let chunk_size = fft_size / 2;
        assert!(frames_per_buffer <= chunk_size);
        assert!(filter_size <= max_filter_size);

        let max_filter_size =
            ceil_to_multiple_of_frames_per_buffer(max_filter_size, frames_per_buffer);
        let max_num_partitions = max_filter_size / frames_per_buffer;
        let filter_size = ceil_to_multiple_of_frames_per_buffer(filter_size, frames_per_buffer);
        let num_partitions = filter_size / frames_per_buffer;

        let mut filter = Self {
            fft_size,
            frames_per_buffer,
            max_filter_size,
            max_num_partitions,
            filter_size,
            num_partitions,
            kernel_freq_domain_buffer: AudioBuffer::new(max_num_partitions, fft_size),
            curr_front_buffer: 0,
            freq_domain_buffer: AudioBuffer::new(max_num_partitions, fft_size),
            temp_kernel_chunk_buffer: AudioBuffer::new(NUM_MONO_CHANNELS, frames_per_buffer),
        };
        assert_eq!(filter.num_partitions * filter.frames_per_buffer, filter.filter_size);
        assert_eq!(filter.max_num_partitions * filter.frames_per_buffer, filter.max_filter_size);
        filter.clear();
        filter
    }

    /// Resets all partition buffers and convolving history to zero.
    pub fn clear(&mut self) {
        for i in 0..self.num_partitions {
            self.kernel_freq_domain_buffer.channel_mut(i).clear();
            self.freq_domain_buffer.channel_mut(i).clear();
        }
    }

    /// Adjusts `num_partitions` and resizes valid frequency-domain partition ranges.
    ///
    /// # Parameters
    /// * `new_filter_size` - New length of the time-domain filter in samples.
    pub fn reset_freq_domain_buffers(&mut self, new_filter_size: usize) {
        assert!(new_filter_size > 0);
        self.filter_size =
            ceil_to_multiple_of_frames_per_buffer(new_filter_size, self.frames_per_buffer);
        assert!(self.filter_size <= self.max_filter_size);

        let old_num_partitions = self.num_partitions;
        self.num_partitions = self.filter_size / self.frames_per_buffer;
        let min_num_partitions = old_num_partitions.min(self.num_partitions);

        if self.curr_front_buffer > 0 {
            let mut temp = AudioBuffer::new(min_num_partitions, self.fft_size);
            for i in 0..min_num_partitions {
                let idx = (self.curr_front_buffer + i) % old_num_partitions;
                let src = self.freq_domain_buffer.channel(idx);
                temp.channel_mut(i).as_mut_slice().copy_from_slice(src.as_slice());
            }
            for i in 0..min_num_partitions {
                let src = temp.channel(i);
                self.freq_domain_buffer
                    .channel_mut(i)
                    .as_mut_slice()
                    .copy_from_slice(src.as_slice());
            }
            self.curr_front_buffer = 0;
        }

        for i in old_num_partitions..self.num_partitions {
            self.freq_domain_buffer.channel_mut(i).clear();
        }
    }

    /// Initialises the FIR filter partitions from a time-domain `kernel`.
    ///
    /// # Parameters
    /// * `kernel` - Time-domain filter impulse response.
    /// * `fft_manager` - FFT manager used to transform each partition into the frequency domain.
    pub fn set_time_domain_kernel(
        &mut self,
        kernel: &ChannelView<'_>,
        fft_manager: &mut FftManager,
    ) {
        let new_num_partitions =
            ceil_to_multiple_of_frames_per_buffer(kernel.size(), self.frames_per_buffer)
                / self.frames_per_buffer;

        for partition in 0..new_num_partitions {
            assert!(partition * self.frames_per_buffer <= kernel.size());
            let offset = partition * self.frames_per_buffer;
            let num_frames_to_copy = (kernel.size() - offset).min(self.frames_per_buffer);

            {
                let mut padded = self.temp_kernel_chunk_buffer.channel_mut(0);
                let slice = padded.as_mut_slice();
                slice[..num_frames_to_copy]
                    .copy_from_slice(&kernel.as_slice()[offset..offset + num_frames_to_copy]);
                for sample in slice[num_frames_to_copy..self.frames_per_buffer].iter_mut() {
                    *sample = 0.0;
                }
            }

            let src = self.temp_kernel_chunk_buffer.channel(0);
            let mut dst = self.kernel_freq_domain_buffer.channel_mut(partition);
            fft_manager.freq_from_time_domain(&src, &mut dst);
        }

        if new_num_partitions != self.num_partitions {
            let new_filter_size = new_num_partitions * self.frames_per_buffer;
            self.reset_freq_domain_buffers(new_filter_size);
        }
    }

    /// Performs frequency-domain partitioned convolution of `input` with this filter kernel,
    /// adding the frequency-domain result onto `accumulator`.
    ///
    /// Updates the internal frequency-domain input history without performing an IFFT or
    /// overlap-add.
    ///
    /// # Parameters
    /// * `input` - Frequency-domain input channel (`fft_size` samples long).
    /// * `accumulator` - Frequency-domain accumulator channel (`fft_size` samples long) onto which
    ///   the convolved frequency-domain partitions are accumulated.
    /// * `fft_manager` - FFT manager providing frequency-domain convolution.
    pub fn filter_and_accumulate(
        &mut self,
        input: &ChannelView<'_>,
        accumulator: &mut ChannelViewMut<'_>,
        fft_manager: &FftManager,
    ) {
        assert_eq!(input.size(), self.fft_size);
        assert_eq!(accumulator.size(), self.fft_size);
        {
            let mut dest = self.freq_domain_buffer.channel_mut(self.curr_front_buffer);
            dest.as_mut_slice().copy_from_slice(input.as_slice());
        }

        for i in 0..self.num_partitions {
            let modulo_idx = (self.curr_front_buffer + i) % self.num_partitions;
            let src_a = self.freq_domain_buffer.channel(modulo_idx);
            let src_b = self.kernel_freq_domain_buffer.channel(i);
            fft_manager.freq_domain_convolution(&src_a, &src_b, accumulator);
        }

        self.curr_front_buffer =
            (self.curr_front_buffer + self.num_partitions - 1) % self.num_partitions;
    }
}

// ===== Tests =====

#[cfg(test)]
mod test {
    use super::*;
    use crate::renderer::obr::common::constants::NUM_STEREO_CHANNELS;
    use crate::renderer::obr::common::misc_math::next_pow_two;
    use googletest::prelude::*;

    const FFT_EPSILON: f32 = 1e-3;
    const LENGTH: usize = 32;

    /// Test harness managing frequency accumulation, IFFT, and overlap-add for unit testing
    /// `PartitionedFftFilter`.
    struct TestFilterRunner {
        filter: PartitionedFftFilter,
        freq_domain_accumulator: AudioBuffer,
        filtered_time_domain_buffers: AudioBuffer,
        temp_zeropad_buffer: AudioBuffer,
        buffer_selector: usize,
        chunk_size: usize,
        frames_per_buffer: usize,
    }

    impl TestFilterRunner {
        fn new(filter: PartitionedFftFilter) -> Self {
            let fft_size = filter.fft_size;
            let chunk_size = fft_size / 2;
            let frames_per_buffer = filter.frames_per_buffer;
            Self {
                filter,
                freq_domain_accumulator: AudioBuffer::new(NUM_MONO_CHANNELS, fft_size),
                filtered_time_domain_buffers: AudioBuffer::new(NUM_STEREO_CHANNELS, fft_size),
                temp_zeropad_buffer: AudioBuffer::new(NUM_MONO_CHANNELS, chunk_size),
                buffer_selector: 0,
                chunk_size,
                frames_per_buffer,
            }
        }

        fn set_time_domain_kernel(
            &mut self,
            kernel: &ChannelView<'_>,
            fft_manager: &mut FftManager,
        ) {
            self.filter.set_time_domain_kernel(kernel, fft_manager);
        }

        fn filter_and_get_output(
            &mut self,
            input: &ChannelView<'_>,
            fft_manager: &mut FftManager,
            output: &mut ChannelViewMut<'_>,
        ) {
            self.freq_domain_accumulator.clear();
            let mut acc = self.freq_domain_accumulator.channel_mut(0);
            self.filter.filter_and_accumulate(input, &mut acc, fft_manager);

            self.buffer_selector = (self.buffer_selector + 1) % NUM_STEREO_CHANNELS;
            let acc_src = self.freq_domain_accumulator.channel(0);
            let mut time_dst = self.filtered_time_domain_buffers.channel_mut(self.buffer_selector);
            fft_manager.time_from_freq_domain(&acc_src, &mut time_dst);

            let curr_buffer = self.buffer_selector;
            let prev_buffer = (self.buffer_selector + 1) % NUM_STEREO_CHANNELS;
            let curr = self.filtered_time_domain_buffers.channel(curr_buffer);
            let prev = self.filtered_time_domain_buffers.channel(prev_buffer);
            let curr_slice = curr.as_slice();
            let prev_slice = prev.as_slice();

            if self.frames_per_buffer == self.chunk_size {
                let out_slice = output.as_mut_slice();
                let chunk_size = self.chunk_size;
                for ((out, &c), &p) in out_slice[..chunk_size]
                    .iter_mut()
                    .zip(curr_slice.iter())
                    .zip(prev_slice[chunk_size..].iter())
                {
                    *out = c + p;
                }
            } else {
                let mut temp_ch = self.temp_zeropad_buffer.channel_mut(0);
                let temp_slice = temp_ch.as_mut_slice();
                let fpb = self.frames_per_buffer;
                for ((temp, &c), &p) in temp_slice[..fpb]
                    .iter_mut()
                    .zip(curr_slice.iter())
                    .zip(prev_slice[fpb..].iter())
                {
                    *temp = c + p;
                }
                output.as_mut_slice().copy_from_slice(&temp_slice[..fpb]);
            }
        }
    }

    // ===== Helper Functions for Tests =====

    fn generate_dirac_impulse_filter(delay_samples: usize, output: &mut [f32]) {
        output.fill(0.0);
        if delay_samples < output.len() {
            output[delay_samples] = 1.0;
        }
    }

    fn generate_silence(output: &mut [f32]) {
        output.fill(0.0);
    }

    fn generate_increasing_signal(output: &mut [f32]) {
        let len_f32 = output.len() as f32;
        for (i, out) in output.iter_mut().enumerate() {
            *out = (i as f32) / len_f32 * 2.0 - 1.0;
        }
    }

    fn generate_saw_tooth_signal(tooth_length_samples: usize, output: &mut [f32]) {
        assert!(tooth_length_samples > 0);
        let tooth_length_f32 = tooth_length_samples as f32;
        for (i, out) in output.iter_mut().enumerate() {
            *out = ((i % tooth_length_samples) as f32) / tooth_length_f32 * 2.0 - 1.0;
        }
    }

    fn process_filter_with_impulse_signal(
        runner: &mut TestFilterRunner,
        fft_manager: &mut FftManager,
        zeros_iteration: usize,
        output_signal: &mut Vec<f32>,
    ) {
        let mut signal_buffer = AudioBuffer::new(NUM_MONO_CHANNELS, LENGTH);
        generate_dirac_impulse_filter(0, signal_buffer.channel_mut(0).as_mut_slice());
        let mut output_buffer = AudioBuffer::new(NUM_MONO_CHANNELS, LENGTH);
        let mut freq_domain_buffer = AudioBuffer::new(NUM_MONO_CHANNELS, LENGTH * 2);

        fft_manager.freq_from_time_domain(
            &signal_buffer.channel(0),
            &mut freq_domain_buffer.channel_mut(0),
        );
        runner.filter_and_get_output(
            &freq_domain_buffer.channel(0),
            fft_manager,
            &mut output_buffer.channel_mut(0),
        );
        output_signal.extend_from_slice(output_buffer.channel(0).as_slice());

        generate_silence(signal_buffer.channel_mut(0).as_mut_slice());
        for _ in 0..zeros_iteration {
            fft_manager.freq_from_time_domain(
                &signal_buffer.channel(0),
                &mut freq_domain_buffer.channel_mut(0),
            );
            runner.filter_and_get_output(
                &freq_domain_buffer.channel(0),
                fft_manager,
                &mut output_buffer.channel_mut(0),
            );
            output_signal.extend_from_slice(output_buffer.channel(0).as_slice());
        }
    }

    // ===== Test Cases =====

    /// Verifies that filter_and_accumulate accumulates onto existing data rather than overwriting.
    #[gtest]
    fn filter_and_accumulate_accumulates_onto_existing_accumulator() {
        let buffer_size = 16;
        let mut fft_manager = FftManager::new(buffer_size);
        let fft_size = fft_manager.get_fft_size();

        let mut kernel = AudioBuffer::new(NUM_MONO_CHANNELS, buffer_size);
        kernel.channel_mut(0).as_mut_slice()[0] = 1.0;
        let mut filter =
            PartitionedFftFilter::new(buffer_size, buffer_size, buffer_size, &fft_manager);
        filter.set_time_domain_kernel(&kernel.channel(0), &mut fft_manager);

        let mut input_time = AudioBuffer::new(NUM_MONO_CHANNELS, buffer_size);
        input_time.channel_mut(0).as_mut_slice()[0] = 2.0;
        let mut input_freq = AudioBuffer::new(NUM_MONO_CHANNELS, fft_size);
        fft_manager.freq_from_time_domain(&input_time.channel(0), &mut input_freq.channel_mut(0));

        let mut acc_baseline = AudioBuffer::new(NUM_MONO_CHANNELS, fft_size);
        acc_baseline.clear();
        filter.filter_and_accumulate(
            &input_freq.channel(0),
            &mut acc_baseline.channel_mut(0),
            &fft_manager,
        );

        let initial_offset = 7.5;
        let mut acc_with_initial = AudioBuffer::new(NUM_MONO_CHANNELS, fft_size);
        acc_with_initial.channel_mut(0).as_mut_slice().fill(initial_offset);
        filter.filter_and_accumulate(
            &input_freq.channel(0),
            &mut acc_with_initial.channel_mut(0),
            &fft_manager,
        );

        for (&base, &with_init) in
            acc_baseline.channel(0).as_slice().iter().zip(acc_with_initial.channel(0).as_slice())
        {
            expect_that!(with_init, near(base + initial_offset, FFT_EPSILON));
        }
    }

    /// Verifies that the filter can handle various non-power-of-two buffer and filter lengths.
    #[gtest]
    fn filter_with_non_pow2_buffer_and_filter_lengths_produces_expected_gain_ramp() {
        let min_non_pow_two = 13;
        let max_non_pow_two = 41;
        let mut buffer_length = min_non_pow_two;

        while buffer_length <= max_non_pow_two {
            let mut filter_length = LENGTH;
            while filter_length <= 3 * LENGTH {
                let mut kernel_buffer = AudioBuffer::new(NUM_MONO_CHANNELS, filter_length);
                let mut output_signal = Vec::new();
                generate_increasing_signal(kernel_buffer.channel_mut(0).as_mut_slice());
                for i in 0..filter_length {
                    kernel_buffer.channel_mut(0).as_mut_slice()[i] =
                        (kernel_buffer.channel_mut(0).as_mut_slice()[i] + 1.0)
                            * (filter_length as f32)
                            / 8.0;
                }
                let mut fft_manager = FftManager::new(buffer_length);
                let filter = PartitionedFftFilter::new(
                    filter_length,
                    buffer_length,
                    filter_length,
                    &fft_manager,
                );
                let mut runner = TestFilterRunner::new(filter);
                runner.set_time_domain_kernel(&kernel_buffer.channel(0), &mut fft_manager);

                let mut signal_buffer = AudioBuffer::new(NUM_MONO_CHANNELS, buffer_length);
                generate_dirac_impulse_filter(0, signal_buffer.channel_mut(0).as_mut_slice());
                let mut output_buffer = AudioBuffer::new(NUM_MONO_CHANNELS, buffer_length);
                let mut freq_domain_buffer =
                    AudioBuffer::new(NUM_MONO_CHANNELS, next_pow_two(buffer_length) * 2);
                freq_domain_buffer.clear();
                fft_manager.freq_from_time_domain(
                    &signal_buffer.channel(0),
                    &mut freq_domain_buffer.channel_mut(0),
                );

                runner.filter_and_get_output(
                    &freq_domain_buffer.channel(0),
                    &mut fft_manager,
                    &mut output_buffer.channel_mut(0),
                );
                output_signal.extend_from_slice(output_buffer.channel(0).as_slice());

                while output_signal.len() < kernel_buffer.num_frames() {
                    freq_domain_buffer.clear();
                    output_buffer.clear();
                    runner.filter_and_get_output(
                        &freq_domain_buffer.channel(0),
                        &mut fft_manager,
                        &mut output_buffer.channel_mut(0),
                    );
                    output_signal.extend_from_slice(output_buffer.channel(0).as_slice());
                }

                for (i, out) in output_signal.iter().enumerate().take(kernel_buffer.num_frames()) {
                    expect_that!(out, near(kernel_buffer.channel(0).as_slice()[i], 4.0e-5));
                }
                filter_length += LENGTH;
            }
            buffer_length += 2;
        }
    }

    /// Verifies correct filtering output with a specific non-power-of-two buffer size (15).
    #[gtest]
    fn filter_with_non_pow2_buffer_size_matches_ideal_output() {
        let buffer_size = 15;
        let kernel =
            vec![1.0, 3.0, 0.0, 2.0, 5.0, 1.0, 3.0, 2.0, 0.0, 4.0, 1.0, 3.0, 0.0, 2.0, 1.0, 2.0];
        let signal =
            vec![2.0, 3.0, 3.0, 4.0, 0.0, 0.0, 2.0, 1.0, 2.0, 1.0, 3.0, 2.0, 4.0, 0.0, 2.0];
        let ideal_output = vec![
            2.0, 9.0, 12.0, 17.0, 28.0, 23.0, 34.0, 43.0, 24.0, 37.0, 40.0, 43.0, 57.0, 49.0, 50.0,
            57.0, 65.0, 57.0, 60.0, 61.0, 47.0, 74.0, 59.0, 55.0, 48.0, 62.0, 51.0, 69.0, 51.0,
            54.0, 55.0, 56.0, 45.0, 43.0, 33.0, 24.0, 40.0, 16.0, 31.0, 11.0, 22.0, 8.0, 12.0, 2.0,
            4.0,
        ];

        let mut kernel_buffer = AudioBuffer::new(NUM_MONO_CHANNELS, kernel.len());
        kernel_buffer.channel_mut(0).as_mut_slice().copy_from_slice(&kernel);

        let mut signal_buffer = AudioBuffer::new(NUM_MONO_CHANNELS, signal.len());
        signal_buffer.channel_mut(0).as_mut_slice().copy_from_slice(&signal);

        let mut output_buffer = AudioBuffer::new(NUM_MONO_CHANNELS, signal.len());
        let mut output_signal = Vec::with_capacity(kernel.len() + signal.len() * 3);

        let mut fft_manager = FftManager::new(buffer_size);
        let filter =
            PartitionedFftFilter::new(kernel.len(), buffer_size, kernel.len(), &fft_manager);
        let mut runner = TestFilterRunner::new(filter);
        runner.set_time_domain_kernel(&kernel_buffer.channel(0), &mut fft_manager);

        let mut freq_domain_buffer = AudioBuffer::new(1, next_pow_two(buffer_size) * 2);

        // Process first block of signal
        fft_manager.freq_from_time_domain(
            &signal_buffer.channel(0),
            &mut freq_domain_buffer.channel_mut(0),
        );
        runner.filter_and_get_output(
            &freq_domain_buffer.channel(0),
            &mut fft_manager,
            &mut output_buffer.channel_mut(0),
        );
        output_signal.extend_from_slice(output_buffer.channel(0).as_slice());

        // Process second block (same signal again, to test continuity)
        runner.filter_and_get_output(
            &freq_domain_buffer.channel(0),
            &mut fft_manager,
            &mut output_buffer.channel_mut(0),
        );
        output_signal.extend_from_slice(output_buffer.channel(0).as_slice());

        // Process third block of silence to flush the filter tail
        signal_buffer.clear();
        fft_manager.freq_from_time_domain(
            &signal_buffer.channel(0),
            &mut freq_domain_buffer.channel_mut(0),
        );
        runner.filter_and_get_output(
            &freq_domain_buffer.channel(0),
            &mut fft_manager,
            &mut output_buffer.channel_mut(0),
        );
        output_signal.extend_from_slice(output_buffer.channel(0).as_slice());

        for (sample, &expected) in ideal_output.iter().enumerate() {
            expect_that!(output_signal[sample], near(expected, FFT_EPSILON));
        }
    }

    /// Verifies that the filter output is invariant across different internal FFT/partition sizes.
    #[gtest]
    fn filter_with_different_partition_sizes_produces_identical_output() {
        let fft_sizes = [32, 64, 128];
        let max_fft_size = fft_sizes[fft_sizes.len() - 1];
        let mut kernel_buffer = AudioBuffer::new(NUM_MONO_CHANNELS, max_fft_size);
        let mut signal_buffer = AudioBuffer::new(NUM_MONO_CHANNELS, max_fft_size);
        for i in 0..max_fft_size {
            kernel_buffer.channel_mut(0).as_mut_slice()[i] = ((i as i32) % 13 - 7) as f32;
            signal_buffer.channel_mut(0).as_mut_slice()[i] = ((i as i32) % 17 - 9) as f32;
        }

        let mut output_signal = vec![Vec::new(); fft_sizes.len()];

        for (fft_idx, &current_fft_size) in fft_sizes.iter().enumerate() {
            let chunk_size = current_fft_size / 2;
            let mut fft_manager = FftManager::new(chunk_size);
            let filter =
                PartitionedFftFilter::new(max_fft_size, chunk_size, max_fft_size, &fft_manager);
            let mut runner = TestFilterRunner::new(filter);
            runner.set_time_domain_kernel(&kernel_buffer.channel(0), &mut fft_manager);

            let mut output_chunk = AudioBuffer::new(NUM_MONO_CHANNELS, chunk_size);
            let mut freq_domain_buffer = AudioBuffer::new(1, current_fft_size);

            for chunk in 0..(max_fft_size / chunk_size) {
                let mut signal_block = AudioBuffer::new(NUM_MONO_CHANNELS, chunk_size);
                let src_slice = &signal_buffer.channel(0).as_slice()
                    [(chunk * chunk_size)..(chunk * chunk_size + chunk_size)];
                signal_block.channel_mut(0).as_mut_slice().copy_from_slice(src_slice);

                fft_manager.freq_from_time_domain(
                    &signal_block.channel(0),
                    &mut freq_domain_buffer.channel_mut(0),
                );
                runner.filter_and_get_output(
                    &freq_domain_buffer.channel(0),
                    &mut fft_manager,
                    &mut output_chunk.channel_mut(0),
                );
                output_signal[fft_idx].extend_from_slice(output_chunk.channel(0).as_slice());
            }
        }

        for i in 1..fft_sizes.len() {
            for (&s0, &si) in output_signal[0].iter().zip(&output_signal[i]) {
                expect_that!(s0, near(si, FFT_EPSILON));
            }
        }
    }

    /// Verifies correct filtering output with standard power-of-two sizes.
    #[gtest]
    fn filter_with_standard_pow2_size_matches_ideal_output() {
        let buffer_size = 32;
        let kernel = vec![
            1.0, 3.0, 0.0, 2.0, 5.0, 1.0, 3.0, 2.0, 0.0, 4.0, 1.0, 3.0, 0.0, 2.0, 1.0, 2.0, 2.0,
            1.0, 0.0, 3.0, 5.0, 2.0, 3.0, 0.0, 1.0, 4.0, 2.0, 0.0, 1.0, 0.0, 2.0, 1.0,
        ];
        let signal = vec![
            2.0, 1.0, 3.0, 3.0, 2.0, 4.0, 2.0, 1.0, 3.0, 4.0, 5.0, 3.0, 2.0, 2.0, 5.0, 4.0, 5.0,
            3.0, 3.0, 4.0, 0.0, 0.0, 2.0, 1.0, 2.0, 1.0, 3.0, 2.0, 4.0, 0.0, 2.0, 1.0,
        ];
        let ideal = vec![
            2.0, 7.0, 6.0, 16.0, 23.0, 23.0, 42.0, 36.0, 38.0, 62.0, 51.0, 66.0, 67.0, 72.0, 88.0,
            90.0, 90.0, 95.0, 104.0, 118.0, 127.0, 113.0, 123.0, 131.0, 116.0, 144.0, 119.0, 126.0,
            138.0, 138.0, 153.0, 142.0, 130.0, 125.0, 137.0, 142.0, 121.0, 113.0, 99.0, 112.0,
            84.0, 89.0, 68.0, 66.0, 74.0, 54.0, 54.0, 59.0, 53.0, 42.0, 42.0, 26.0, 32.0, 28.0,
            18.0, 15.0, 19.0, 9.0, 12.0, 5.0, 4.0, 4.0, 1.0,
        ];

        let mut kernel_buffer = AudioBuffer::new(NUM_MONO_CHANNELS, kernel.len());
        kernel_buffer.channel_mut(0).as_mut_slice().copy_from_slice(&kernel);

        let mut signal_buffer = AudioBuffer::new(NUM_MONO_CHANNELS, signal.len());
        signal_buffer.channel_mut(0).as_mut_slice().copy_from_slice(&signal);

        let mut output_buffer = AudioBuffer::new(NUM_MONO_CHANNELS, kernel.len());
        let mut output_signal = Vec::with_capacity(kernel.len() + signal.len());

        let mut fft_manager = FftManager::new(buffer_size);
        let filter =
            PartitionedFftFilter::new(kernel.len(), buffer_size, kernel.len(), &fft_manager);
        let mut runner = TestFilterRunner::new(filter);
        runner.set_time_domain_kernel(&kernel_buffer.channel(0), &mut fft_manager);

        let mut freq_domain_buffer = AudioBuffer::new(NUM_MONO_CHANNELS, buffer_size * 2);

        // Process first block of signal
        fft_manager.freq_from_time_domain(
            &signal_buffer.channel(0),
            &mut freq_domain_buffer.channel_mut(0),
        );
        runner.filter_and_get_output(
            &freq_domain_buffer.channel(0),
            &mut fft_manager,
            &mut output_buffer.channel_mut(0),
        );
        output_signal.extend_from_slice(output_buffer.channel(0).as_slice());

        // Flush tail with silence
        signal_buffer.clear();
        fft_manager.freq_from_time_domain(
            &signal_buffer.channel(0),
            &mut freq_domain_buffer.channel_mut(0),
        );
        runner.filter_and_get_output(
            &freq_domain_buffer.channel(0),
            &mut fft_manager,
            &mut output_buffer.channel_mut(0),
        );
        output_signal.extend_from_slice(output_buffer.channel(0).as_slice());

        for (sample, &expected) in ideal.iter().enumerate() {
            expect_that!(output_signal[sample], near(expected, FFT_EPSILON));
        }
    }

    /// Verifies that zero input to the filter yields zero output.
    #[gtest]
    fn filter_with_zero_input_produces_zero_output() {
        let chunk_size = 16;
        let kernel =
            vec![1.0, 3.0, 0.0, 2.0, 5.0, 1.0, 3.0, 2.0, 0.0, 4.0, 1.0, 3.0, 0.0, 2.0, 1.0, 2.0];

        let mut kernel_buffer = AudioBuffer::new(NUM_MONO_CHANNELS, kernel.len());
        kernel_buffer.channel_mut(0).as_mut_slice().copy_from_slice(&kernel);
        let mut output_signal = Vec::with_capacity(kernel.len() * 2);
        let mut fft_manager = FftManager::new(chunk_size);
        let filter = PartitionedFftFilter::new(
            kernel_buffer.channel(0).size(),
            chunk_size,
            kernel_buffer.channel(0).size(),
            &fft_manager,
        );
        let mut runner = TestFilterRunner::new(filter);
        runner.set_time_domain_kernel(&kernel_buffer.channel(0), &mut fft_manager);
        let mut output_chunk = AudioBuffer::new(NUM_MONO_CHANNELS, chunk_size);
        let mut zero_signal = AudioBuffer::new(NUM_MONO_CHANNELS, chunk_size);
        zero_signal.clear();
        let mut freq_domain_buffer = AudioBuffer::new(NUM_MONO_CHANNELS, chunk_size * 2);

        for _ in 0..(2 * kernel.len() / chunk_size) {
            fft_manager.freq_from_time_domain(
                &zero_signal.channel(0),
                &mut freq_domain_buffer.channel_mut(0),
            );
            runner.filter_and_get_output(
                &freq_domain_buffer.channel(0),
                &mut fft_manager,
                &mut output_chunk.channel_mut(0),
            );
            output_signal.extend_from_slice(output_chunk.channel(0).as_slice());
        }

        for &sample in &output_signal {
            expect_that!(sample, eq(0.0));
        }
    }

    /// Tests filtering with a Dirac impulse kernel.
    #[gtest]
    fn filter_with_delayed_dirac_impulse_delays_signal() {
        let filter_size = 32;
        let num_blocks = 4;
        let signal_size = filter_size * num_blocks;
        let mut test_signal = AudioBuffer::new(NUM_MONO_CHANNELS, signal_size);
        generate_saw_tooth_signal(5, test_signal.channel_mut(0).as_mut_slice());
        let mut fft_manager = FftManager::new(filter_size);
        let fft_filter =
            PartitionedFftFilter::new(filter_size, filter_size, filter_size, &fft_manager);
        let mut runner = TestFilterRunner::new(fft_filter);
        let mut kernel = AudioBuffer::new(NUM_MONO_CHANNELS, filter_size);
        let delay = filter_size / 2;
        generate_dirac_impulse_filter(delay, kernel.channel_mut(0).as_mut_slice());
        runner.set_time_domain_kernel(&kernel.channel(0), &mut fft_manager);
        let mut filtered_signal = Vec::with_capacity(signal_size);
        let mut filtered_block = AudioBuffer::new(NUM_MONO_CHANNELS, filter_size);
        let mut freq_domain_buffer = AudioBuffer::new(NUM_MONO_CHANNELS, filter_size * 2);

        for b in 0..num_blocks {
            let mut signal_block = AudioBuffer::new(NUM_MONO_CHANNELS, filter_size);
            let src_slice = &test_signal.channel(0).as_slice()
                [(b * filter_size)..(b * filter_size + filter_size)];
            signal_block.channel_mut(0).as_mut_slice().copy_from_slice(src_slice);

            fft_manager.freq_from_time_domain(
                &signal_block.channel(0),
                &mut freq_domain_buffer.channel_mut(0),
            );
            runner.filter_and_get_output(
                &freq_domain_buffer.channel(0),
                &mut fft_manager,
                &mut filtered_block.channel_mut(0),
            );
            filtered_signal.extend_from_slice(filtered_block.channel(0).as_slice());
        }

        for &val in filtered_signal.iter().take(delay) {
            expect_that!(val, near(0.0, 1e-5));
        }
        for (&f_val, &t_val) in
            filtered_signal[delay..].iter().zip(test_signal.channel(0).as_slice())
        {
            expect_that!(f_val, near(t_val, 1e-5));
        }
    }

    fn set_freq_domain_buffer(filter: &mut PartitionedFftFilter) {
        for i in 0..filter.num_partitions {
            let mut chan = filter.freq_domain_buffer.channel_mut(i);
            chan.clear();
            chan.as_mut_slice()[0] = (i + 1) as f32;
        }
    }

    /// Verifies that resetting the frequency domain buffers (during resizing) correctly
    /// copies over previous active state and pads new space with zero.
    #[gtest]
    fn reset_freq_domain_buffers_preserves_and_rotates_active_partitions() {
        let buffer_size = 32;
        let initial_size_factor = 8;
        let bigger_size_factor = 10;
        let smaller_size_factor = 5;
        let initial_num_partitions = 2 * initial_size_factor;
        let bigger_num_partitions = 2 * bigger_size_factor;
        let smaller_num_partitions = 2 * smaller_size_factor;
        let fft_manager = FftManager::new(buffer_size);
        let mut filter = PartitionedFftFilter::new(
            initial_size_factor * buffer_size * 2,
            buffer_size,
            bigger_size_factor * buffer_size * 2,
            &fft_manager,
        );

        set_freq_domain_buffer(&mut filter);
        let initial_freq_buf = filter.freq_domain_buffer.clone();
        for i in 0..initial_num_partitions {
            expect_that!(initial_freq_buf.channel(i).as_slice()[0], eq((i + 1) as f32));
        }

        filter.curr_front_buffer = filter.num_partitions / 2;
        filter.reset_freq_domain_buffers(bigger_size_factor * fft_manager.get_fft_size());

        expect_that!(filter.curr_front_buffer, eq(0));
        let bigger_freq_buf = filter.freq_domain_buffer.clone();
        for i in 0..initial_num_partitions {
            let expected_val = initial_freq_buf
                .channel((initial_size_factor + i) % initial_num_partitions)
                .as_slice()[0];
            expect_that!(bigger_freq_buf.channel(i).as_slice()[0], eq(expected_val));
        }
        for i in initial_num_partitions..bigger_num_partitions {
            expect_that!(bigger_freq_buf.channel(i).as_slice()[0], eq(0.0));
        }

        set_freq_domain_buffer(&mut filter);
        let bigger_freq_buf_ref = filter.freq_domain_buffer.clone();

        filter.curr_front_buffer = filter.num_partitions / 2;
        filter.reset_freq_domain_buffers(smaller_size_factor * fft_manager.get_fft_size());

        expect_that!(filter.curr_front_buffer, eq(0));
        let smaller_freq_buf = filter.freq_domain_buffer.clone();
        for i in 0..smaller_num_partitions {
            let expected_val = bigger_freq_buf_ref
                .channel((bigger_size_factor + i) % bigger_num_partitions)
                .as_slice()[0];
            expect_that!(smaller_freq_buf.channel(i).as_slice()[0], eq(expected_val));
        }
    }

    /// Verifies that resetting the time-domain kernel with a different size (longer or shorter)
    /// works correctly and doesn't cause artifacts.
    #[gtest]
    fn set_time_domain_kernel_with_resized_kernels_updates_filter_length() {
        let buffer_size = LENGTH;
        let mut small_kernel_buffer = AudioBuffer::new(NUM_MONO_CHANNELS, buffer_size);
        let mut big_kernel_buffer = AudioBuffer::new(NUM_MONO_CHANNELS, 2 * buffer_size);
        let mut total_output_signal = Vec::new();
        generate_increasing_signal(small_kernel_buffer.channel_mut(0).as_mut_slice());
        generate_increasing_signal(big_kernel_buffer.channel_mut(0).as_mut_slice());
        for i in 0..buffer_size {
            small_kernel_buffer.channel_mut(0).as_mut_slice()[i] =
                (small_kernel_buffer.channel_mut(0).as_mut_slice()[i] + 1.0) * (buffer_size as f32)
                    / 8.0;
        }
        for i in 0..2 * buffer_size {
            big_kernel_buffer.channel_mut(0).as_mut_slice()[i] =
                (big_kernel_buffer.channel_mut(0).as_mut_slice()[i] + 1.0)
                    * (buffer_size as f32)
                    * 2.0
                    / 8.0;
        }
        let mut fft_manager = FftManager::new(buffer_size);
        let filter =
            PartitionedFftFilter::new(buffer_size, buffer_size, 2 * buffer_size, &fft_manager);
        let mut runner = TestFilterRunner::new(filter);

        runner.set_time_domain_kernel(&small_kernel_buffer.channel(0), &mut fft_manager);
        process_filter_with_impulse_signal(
            &mut runner,
            &mut fft_manager,
            2,
            &mut total_output_signal,
        );

        runner.set_time_domain_kernel(&big_kernel_buffer.channel(0), &mut fft_manager);
        process_filter_with_impulse_signal(
            &mut runner,
            &mut fft_manager,
            3,
            &mut total_output_signal,
        );

        runner.set_time_domain_kernel(&small_kernel_buffer.channel(0), &mut fft_manager);
        process_filter_with_impulse_signal(
            &mut runner,
            &mut fft_manager,
            2,
            &mut total_output_signal,
        );

        for i in 0..buffer_size {
            expect_that!(total_output_signal[i], near((i as f32) / 4.0, FFT_EPSILON));
            expect_that!(total_output_signal[i + buffer_size], near(0.0, FFT_EPSILON));
            expect_that!(total_output_signal[i + 2 * buffer_size], near(0.0, FFT_EPSILON));

            expect_that!(
                total_output_signal[i + 3 * buffer_size],
                near((i as f32) / 4.0, FFT_EPSILON)
            );
            expect_that!(
                total_output_signal[i + 4 * buffer_size],
                near(((i + buffer_size) as f32) / 4.0, FFT_EPSILON)
            );
            expect_that!(total_output_signal[i + 5 * buffer_size], near(0.0, FFT_EPSILON));
            expect_that!(total_output_signal[i + 6 * buffer_size], near(0.0, FFT_EPSILON));

            expect_that!(
                total_output_signal[i + 7 * buffer_size],
                near((i as f32) / 4.0, FFT_EPSILON)
            );
            expect_that!(total_output_signal[i + 8 * buffer_size], near(0.0, FFT_EPSILON));
            expect_that!(total_output_signal[i + 9 * buffer_size], near(0.0, FFT_EPSILON));
        }
    }
}
