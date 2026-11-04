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

//! Processing blocks for OLR gain application.

use crate::common::definitions::{OarError, MAX_OUTPUT_CHANNEL_COUNT};

fn get_overlap(
    first_sample: u64,
    last_sample: u64,
    start_sample: u64,
    num_samples: usize,
) -> Option<(std::ops::Range<usize>, std::ops::Range<usize>)> {
    let end_sample = start_sample + num_samples as u64;
    let overlay_start_sample = start_sample.max(first_sample);
    let overlay_end_sample = end_sample.min(last_sample);

    if overlay_start_sample < overlay_end_sample {
        let samples_start = (overlay_start_sample - start_sample) as usize;
        let samples_end = (overlay_end_sample - start_sample) as usize;
        let states_start = (overlay_start_sample - first_sample) as usize;
        let states_end = (overlay_end_sample - first_sample) as usize;
        Some((samples_start..samples_end, states_start..states_end))
    } else {
        None
    }
}

/// A processing block that applies constant speaker gains over its duration.
#[derive(Debug, Clone)]
pub struct FixedGainsBlock {
    first_sample: u64,
    last_sample: u64,
    gains: [f32; MAX_OUTPUT_CHANNEL_COUNT],
    num_gains: usize,
}

impl FixedGainsBlock {
    pub fn new(start_sample: u64, end_sample: u64, gains_slice: &[f32]) -> Self {
        let num_gains = gains_slice.len();
        let mut gains = [0.0f32; MAX_OUTPUT_CHANNEL_COUNT];
        gains[..num_gains].copy_from_slice(gains_slice);
        FixedGainsBlock { first_sample: start_sample, last_sample: end_sample, gains, num_gains }
    }

    pub fn process(
        &mut self,
        start_sample: u64,
        input: &[f32],
        num_samples: usize,
        channels: usize,
        output: &mut [f32],
    ) -> Result<(), OarError> {
        let (samples_range, _) =
            match get_overlap(self.first_sample, self.last_sample, start_sample, num_samples) {
                Some(r) => r,
                None => return Ok(()),
            };

        if output.len() < self.num_gains * num_samples {
            return Err(OarError::InvalidParameter);
        }

        for i in 0..self.num_gains {
            let gain = self.gains[i];
            for j in samples_range.clone() {
                for k in 0..channels {
                    output[i * num_samples + j] += input[k * num_samples + j] * gain;
                }
            }
        }

        Ok(())
    }
}

/// A processing block that linearly interpolates between two sets of gains.
#[derive(Debug, Clone)]
pub struct InterpGainsBlock {
    first_sample: u64,
    last_sample: u64,
    gains_start: [f32; MAX_OUTPUT_CHANNEL_COUNT],
    gains_end: [f32; MAX_OUTPUT_CHANNEL_COUNT],
    has_start: bool,
    has_end: bool,
    num_gains: usize,
}

impl InterpGainsBlock {
    pub fn new(
        start_sample: u64,
        end_sample: u64,
        gains_start_opt: Option<&[f32]>,
        gains_end_opt: Option<&[f32]>,
        num_gains: usize,
    ) -> Self {
        let mut gains_start = [0.0f32; MAX_OUTPUT_CHANNEL_COUNT];
        let mut has_start = false;
        if let Some(gs) = gains_start_opt {
            gains_start[..num_gains].copy_from_slice(gs);
            has_start = true;
        }

        let mut gains_end = [0.0f32; MAX_OUTPUT_CHANNEL_COUNT];
        let mut has_end = false;
        if let Some(ge) = gains_end_opt {
            gains_end[..num_gains].copy_from_slice(ge);
            has_end = true;
        }

        InterpGainsBlock {
            first_sample: start_sample,
            last_sample: end_sample,
            gains_start,
            gains_end,
            has_start,
            has_end,
            num_gains,
        }
    }

    fn process(
        &mut self,
        start_sample: u64,
        input: &[f32],
        num_samples: usize,
        channels: usize,
        output: &mut [f32],
    ) -> Result<(), OarError> {
        let (samples_range, states_range) =
            match get_overlap(self.first_sample, self.last_sample, start_sample, num_samples) {
                Some(r) => r,
                None => return Ok(()),
            };

        if output.len() < self.num_gains * num_samples {
            return Err(OarError::InvalidParameter);
        }

        let transition_len = (self.last_sample - self.first_sample) as f32;
        let inv_transition_len = if transition_len > 0.0 { 1.0 / transition_len } else { 0.0 };

        // TODO(b/525080422): Optimize by merging the has_start and has_end loops when both are true
        // to reduce loop overhead and redundant multiplication operations.
        if self.has_start {
            for i in 0..self.num_gains {
                let g_start = self.gains_start[i];
                for (m, j) in (states_range.start..).zip(samples_range.clone()) {
                    let interp_val = m as f32 * inv_transition_len;
                    let weight = 1.0 - interp_val;
                    for k in 0..channels {
                        output[i * num_samples + j] +=
                            input[k * num_samples + j] * weight * g_start;
                    }
                }
            }
        }

        if self.has_end {
            for i in 0..self.num_gains {
                let g_end = self.gains_end[i];
                for (m, j) in (states_range.start..).zip(samples_range.clone()) {
                    let interp_val = m as f32 * inv_transition_len;
                    for k in 0..channels {
                        output[i * num_samples + j] +=
                            input[k * num_samples + j] * interp_val * g_end;
                    }
                }
            }
        }

        Ok(())
    }
}

/// Enum wrapping the concrete processing block types to avoid dynamic boxing.
#[derive(Debug, Clone)]
pub enum ProcessingBlockVariant {
    Fixed(FixedGainsBlock),
    Interp(InterpGainsBlock),
}

impl ProcessingBlockVariant {
    #[cfg(test)]
    pub fn first_sample(&self) -> u64 {
        match self {
            ProcessingBlockVariant::Fixed(b) => b.first_sample,
            ProcessingBlockVariant::Interp(b) => b.first_sample,
        }
    }

    pub fn last_sample(&self) -> u64 {
        match self {
            ProcessingBlockVariant::Fixed(b) => b.last_sample,
            ProcessingBlockVariant::Interp(b) => b.last_sample,
        }
    }

    pub fn process(
        &mut self,
        start_sample: u64,
        input: &[f32],
        num_samples: usize,
        channels: usize,
        output: &mut [f32],
    ) -> Result<(), OarError> {
        match self {
            ProcessingBlockVariant::Fixed(b) => {
                b.process(start_sample, input, num_samples, channels, output)
            }
            ProcessingBlockVariant::Interp(b) => {
                b.process(start_sample, input, num_samples, channels, output)
            }
        }
    }
}

// ===== Tests =====

#[cfg(test)]
mod test {

    use super::*;
    use googletest::prelude::*;

    const EPSILON: f32 = 1e-4;

    #[gtest]
    fn test_fixed_gains_block_applies_gains_to_overlapping_samples() {
        let mut block = FixedGainsBlock::new(0, 10, &[0.5, 0.2]);
        let input = [1.0, 1.0, 1.0, 1.0];
        let mut output = [0.0; 8];

        let result = block.process(0, &input, 4, 1, &mut output);

        assert_ok!(result);
        expect_that!(output[0], near(0.5, EPSILON));
        expect_that!(output[1], near(0.5, EPSILON));
        expect_that!(output[2], near(0.5, EPSILON));
        expect_that!(output[3], near(0.5, EPSILON));
        expect_that!(output[4], near(0.2, EPSILON));
        expect_that!(output[5], near(0.2, EPSILON));
        expect_that!(output[6], near(0.2, EPSILON));
        expect_that!(output[7], near(0.2, EPSILON));
    }

    #[gtest]
    fn test_fixed_gains_block_no_overlap_does_not_modify_output() {
        let mut block = FixedGainsBlock::new(0, 10, &[0.5, 0.2]);
        let input = [1.0, 1.0, 1.0, 1.0];
        let mut output = [0.0; 8];

        let result = block.process(10, &input, 4, 1, &mut output);

        assert_ok!(result);
        expect_true!(output.iter().all(|&v| v == 0.0));
    }

    #[gtest]
    fn test_interp_gains_block_interpolates_gains_linearly() {
        let mut block = InterpGainsBlock::new(0, 10, Some(&[0.0]), Some(&[1.0]), 1);
        let input = [1.0; 5];
        let mut output = [0.0; 5];

        let result = block.process(0, &input, 5, 1, &mut output);

        assert_ok!(result);
        expect_that!(output[0], near(0.0, EPSILON));
        expect_that!(output[1], near(0.1, EPSILON));
        expect_that!(output[2], near(0.2, EPSILON));
        expect_that!(output[3], near(0.3, EPSILON));
        expect_that!(output[4], near(0.4, EPSILON));
    }
}
