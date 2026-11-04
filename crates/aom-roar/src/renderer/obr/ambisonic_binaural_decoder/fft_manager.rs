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

//! Fast Fourier Transform (FFT) manager.
//!
//! Wraps real-to-complex FFT routines and handles conversion between time domain signals
//! and canonical frequency-domain representations without real-time allocations.

use crate::common::audio_buffer::{ChannelView, ChannelViewMut};
use crate::renderer::obr::common::misc_math::next_pow_two;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use std::sync::Arc;

pub const MIN_FFT_SIZE: usize = 32;

/// Wrapper around `realfft` providing forward and inverse real FFT transforms.
///
/// Pre-allocates scratch arrays at initialisation to guarantee that real-time execution
/// does not trigger dynamic heap allocations.
pub struct FftManager {
    /// Number of points in the real-to-complex FFT, matching `2 *
    /// next_pow_two(frames_per_buffer)`.
    fft_size: usize,
    /// Number of audio frames per processing buffer block.
    frames_per_buffer: usize,
    /// Precomputed inverse FFT normalization factor (`1.0 / fft_size`) applied during
    /// frequency-domain convolution.
    inverse_fft_scale: f32,
    /// Pre-planned forward real-to-complex FFT transform instance.
    /// Stored as `Arc<dyn ...>` to match `RealFftPlanner::plan_fft_forward`'s cached trait-object
    /// return type (ROAR itself is single-threaded and does not require cross-thread sharing).
    r2c: Arc<dyn RealToComplex<f32>>,
    /// Pre-planned inverse complex-to-real FFT transform instance.
    /// Stored as `Arc<dyn ...>` to match `RealFftPlanner::plan_fft_inverse`'s cached trait-object
    /// return type (ROAR itself is single-threaded and does not require cross-thread sharing).
    c2r: Arc<dyn ComplexToReal<f32>>,
    /// Pre-allocated time-domain scratch buffer of length `fft_size`, used to stage zero-padded
    /// inputs and IFFT outputs without real-time heap allocations.
    scratch_time: Vec<f32>,
    /// Pre-allocated complex frequency-domain scratch buffer of length `fft_size / 2 + 1`,
    /// converting between `realfft` representation and canonical packed layout.
    scratch_complex: Vec<Complex<f32>>,
    /// Pre-allocated scratch buffer required internally by `realfft`'s `process_with_scratch`
    /// routines.
    fft_scratch: Vec<Complex<f32>>,
}

impl std::fmt::Debug for FftManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FftManager")
            .field("fft_size", &self.fft_size)
            .field("frames_per_buffer", &self.frames_per_buffer)
            .finish()
    }
}

impl FftManager {
    /// Constructs a new `FftManager` instance.
    pub fn new(frames_per_buffer: usize) -> Self {
        assert!(frames_per_buffer > 0);
        let fft_size = (next_pow_two(frames_per_buffer) * 2).max(MIN_FFT_SIZE);
        assert!(fft_size.is_power_of_two());

        let mut planner = RealFftPlanner::<f32>::new();
        let r2c = planner.plan_fft_forward(fft_size);
        let c2r = planner.plan_fft_inverse(fft_size);

        let max_scratch_len = r2c.get_scratch_len().max(c2r.get_scratch_len());
        let fft_scratch = vec![Complex { re: 0.0, im: 0.0 }; max_scratch_len];

        Self {
            fft_size,
            frames_per_buffer,
            inverse_fft_scale: 1.0 / (fft_size as f32),
            r2c,
            c2r,
            scratch_time: vec![0.0; fft_size],
            scratch_complex: vec![Complex { re: 0.0, im: 0.0 }; fft_size / 2 + 1],
            fft_scratch,
        }
    }

    /// Returns the number of points in the FFT.
    pub fn get_fft_size(&self) -> usize {
        self.fft_size
    }

    /// Transforms a single time-domain channel into canonical frequency-domain format.
    pub fn freq_from_time_domain(
        &mut self,
        time_channel: &ChannelView<'_>,
        freq_channel: &mut ChannelViewMut<'_>,
    ) {
        assert_eq!(freq_channel.size(), self.fft_size);
        assert!(time_channel.size() <= self.fft_size);

        self.scratch_time.fill(0.0);
        let len = time_channel.size();
        self.scratch_time[..len].copy_from_slice(&time_channel.as_slice()[..len]);

        self.r2c
            .process_with_scratch(
                &mut self.scratch_time,
                &mut self.scratch_complex,
                &mut self.fft_scratch,
            )
            .expect("Realfft forward transform failed");

        let out = freq_channel.as_mut_slice();
        out[0] = self.scratch_complex[0].re;
        out[1] = self.scratch_complex[self.fft_size / 2].re;

        let complex_bins = &self.scratch_complex[1..self.fft_size / 2];
        let out_chunks = out[2..self.fft_size].chunks_exact_mut(2);

        for (c, out_chunk) in complex_bins.iter().zip(out_chunks) {
            out_chunk[0] = c.re;
            out_chunk[1] = c.im;
        }
    }

    /// Transforms canonical frequency-domain input into time-domain output.
    pub fn time_from_freq_domain(
        &mut self,
        freq_channel: &ChannelView<'_>,
        time_channel: &mut ChannelViewMut<'_>,
    ) {
        assert_eq!(freq_channel.size(), self.fft_size);
        let out_len = time_channel.size();
        assert!(out_len == self.fft_size || out_len == self.frames_per_buffer);

        let input = freq_channel.as_slice();
        self.scratch_complex[0] = Complex { re: input[0], im: 0.0 };
        self.scratch_complex[self.fft_size / 2] = Complex { re: input[1], im: 0.0 };

        let in_chunks = input[2..self.fft_size].chunks_exact(2);
        let complex_bins = &mut self.scratch_complex[1..self.fft_size / 2];

        for (in_chunk, c) in in_chunks.zip(complex_bins.iter_mut()) {
            c.re = in_chunk[0];
            c.im = in_chunk[1];
        }

        self.c2r
            .process_with_scratch(
                &mut self.scratch_complex,
                &mut self.scratch_time,
                &mut self.fft_scratch,
            )
            .expect("Realfft inverse transform failed");

        let dst = time_channel.as_mut_slice();
        dst[..out_len].copy_from_slice(&self.scratch_time[..out_len]);
    }

    /// Performs pointwise complex multiplication of two canonical frequency-domain buffers,
    /// accumulating in `scaled_output`.
    pub fn freq_domain_convolution(
        &self,
        input_a: &ChannelView<'_>,
        input_b: &ChannelView<'_>,
        scaled_output: &mut ChannelViewMut<'_>,
    ) {
        assert_eq!(input_a.size(), self.fft_size);
        assert_eq!(input_b.size(), self.fft_size);
        assert_eq!(scaled_output.size(), self.fft_size);

        let a = input_a.as_slice();
        let b = input_b.as_slice();
        let out = scaled_output.as_mut_slice();

        let scale = self.inverse_fft_scale;
        out[0] += a[0] * b[0] * scale;
        out[1] += a[1] * b[1] * scale;

        let a_chunks = a[2..self.fft_size].chunks_exact(2);
        let b_chunks = b[2..self.fft_size].chunks_exact(2);
        let out_chunks = out[2..self.fft_size].chunks_exact_mut(2);

        for ((a_chunk, b_chunk), out_chunk) in a_chunks.zip(b_chunks).zip(out_chunks) {
            let re_a = a_chunk[0];
            let im_a = a_chunk[1];
            let re_b = b_chunk[0];
            let im_b = b_chunk[1];
            out_chunk[0] += (re_a * re_b - im_a * im_b) * scale;
            out_chunk[1] += (re_a * im_b + im_a * re_b) * scale;
        }
    }
}

// ===== Tests =====

#[cfg(test)]
mod test {

    use super::*;
    use crate::common::audio_buffer::AudioBuffer;
    use googletest::prelude::*;

    #[gtest]
    fn test_fft_round_trip() {
        let frames = 16;
        let mut manager = FftManager::new(frames);
        let fft_size = manager.get_fft_size(); // should be 32

        let mut input_buf = AudioBuffer::new(1, frames);
        input_buf.channel_mut(0).as_mut_slice()[0] = 1.0; // Impulse

        let mut freq_buf = AudioBuffer::new(1, fft_size);
        let mut output_buf = AudioBuffer::new(1, frames);

        // Forward
        {
            let in_view = input_buf.channel(0);
            let mut freq_view = freq_buf.channel_mut(0);
            manager.freq_from_time_domain(&in_view, &mut freq_view);
        }

        // Inverse
        {
            let freq_view = freq_buf.channel(0);
            let mut out_view = output_buf.channel_mut(0);
            manager.time_from_freq_domain(&freq_view, &mut out_view);
        }

        // Output should be scaled by fft_size (32)
        let scale = fft_size as f32;
        let expected = 1.0 * scale;

        expect_near!(output_buf[0][0], expected, 1e-5);
        for i in 1..frames {
            expect_near!(output_buf[0][i], 0.0, 1e-5);
        }
    }

    #[gtest]
    fn test_convolution_with_impulse() {
        let frames = 16;
        let manager = FftManager::new(frames);
        let fft_size = manager.get_fft_size();

        let mut input_a = AudioBuffer::new(1, fft_size);
        let mut input_b = AudioBuffer::new(1, fft_size);
        let mut output_conv = AudioBuffer::new(1, fft_size);

        input_a.channel_mut(0).as_mut_slice().fill(0.5);

        // Freq domain of impulse:
        let mut b_view = input_b.channel_mut(0);
        let b_slice = b_view.as_mut_slice();
        b_slice[0] = 1.0;
        b_slice[1] = 1.0;
        for k in 1..(fft_size / 2) {
            b_slice[2 * k] = 1.0;
            b_slice[2 * k + 1] = 0.0;
        }

        // Perform convolution
        {
            let a_view = input_a.channel(0);
            let b_view = input_b.channel(0);
            let mut out_view = output_conv.channel_mut(0);
            manager.freq_domain_convolution(&a_view, &b_view, &mut out_view);
        }

        // a * b * (1/N) = 0.5 * 1.0 * (1/32) = 0.015625
        let expected = 0.5 / fft_size as f32;
        let out_slice = output_conv.channel(0).as_slice();
        for val in out_slice {
            expect_near!(*val, expected, 1e-5);
        }
    }
}
