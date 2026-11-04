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

//! Digital signal processing (DSP) utilities.
//!
//! Provides small helper functions (buffer padding alignment, windowing function generation)
//! used by the binaural decoder and resampler components.

/// Rounds the given size up to the next integer multiple of the frame block size.
pub fn ceil_to_multiple_of_frames_per_buffer(size: usize, frames_per_buffer: usize) -> usize {
    assert!(frames_per_buffer > 0);
    let remainder = size % frames_per_buffer;
    if remainder == 0 {
        size.max(frames_per_buffer)
    } else {
        size + frames_per_buffer - remainder
    }
}

pub fn generate_hann_window(full_window: bool, window_length: usize, buffer: &mut [f32]) {
    assert!(window_length <= buffer.len());
    if window_length <= 1 {
        if window_length == 1 {
            buffer[0] = 1.0;
        }
        return;
    }
    let full_window_scaling_factor = std::f32::consts::TAU / (window_length as f32 - 1.0);
    let half_window_scaling_factor = std::f32::consts::TAU / (2.0 * window_length as f32 - 1.0);
    let scaling_factor =
        if full_window { full_window_scaling_factor } else { half_window_scaling_factor };
    for (i, value) in buffer.iter_mut().enumerate().take(window_length) {
        *value = 0.5 * (1.0 - (scaling_factor * i as f32).cos());
    }
}

// ===== Tests =====

#[cfg(test)]
mod test {

    use super::*;
    use googletest::prelude::*;

    #[gtest]
    fn test_ceil_to_multiple_of_frames_per_buffer() {
        expect_eq!(ceil_to_multiple_of_frames_per_buffer(0, 4), 4);
        expect_eq!(ceil_to_multiple_of_frames_per_buffer(3, 4), 4);
        expect_eq!(ceil_to_multiple_of_frames_per_buffer(4, 4), 4);
        expect_eq!(ceil_to_multiple_of_frames_per_buffer(5, 4), 8);
    }

    #[gtest]
    fn test_generate_hann_window_full() {
        let mut buffer = [0.0; 5];
        generate_hann_window(true, 5, &mut buffer);

        // Hann window values should be symmetric and 0.0 at edges
        expect_near!(buffer[0], 0.0, 1e-6);
        expect_near!(buffer[2], 1.0, 1e-6); // Center of 5 samples is 1.0
        expect_near!(buffer[4], 0.0, 1e-6);
        expect_near!(buffer[1], buffer[3], 1e-6);
    }

    #[gtest]
    fn test_generate_hann_window_half() {
        let mut buffer = [0.0; 5];
        generate_hann_window(false, 5, &mut buffer);

        // Half window values are cosine transition on the first half of 2 * window_length
        expect_near!(buffer[0], 0.0, 1e-6);
        expect_gt!(buffer[1], 0.0);
        expect_lt!(buffer[4], 1.0); // Doesn't reach 1.0 within window_length
    }
}
