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

use crate::common::animation::{Animated, AnimatedDecibels};
use crate::common::definitions::{Samples, METADATA_QUEUE_CAPACITY};
use crate::common::gains::GainVariant;
use crate::common::units::{Decibels, LinearGain};
use std::collections::VecDeque;

/// Represents a queued gain change event in the renderer's DSP timeline.
///
/// A `GainUpdate` is created from a user-requested `Gain` metadata update
/// and a target duration. It tracks the progress of the gain transition
/// (whether constant, multi-channel, or animated) as audio samples are processed.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GainUpdate {
    pub gain: GainVariant,
    pub duration: Samples,
    pub elapsed: u32,
}

/// Manages the live rendering state and pending update queue for a single gain parameter.
///
/// Each gain control (e.g., a specific element's gain or a group's loudness gain)
/// has an associated `GainParameter`. It queues incoming `GainUpdate`s and
/// ensures smooth transitions between them during rendering.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GainParameter {
    pub id: u32,
    pub updates: VecDeque<GainUpdate>,
    pub last_value: f32, // holds linear gain
}

impl GainParameter {
    /// Creates a new `GainParameter` with a default gain of 1.0 (0 dB) and an empty update queue.
    pub fn new(id: u32) -> Self {
        GainParameter {
            id,
            updates: VecDeque::with_capacity(METADATA_QUEUE_CAPACITY),
            last_value: 1.0, // default gain is 1.0 (0 dB)
        }
    }
}

/// Fills `out_slice` with linear amplitude multipliers for a linear decibel ramp.
///
/// Because a constant step in decibels (`start + i * delta / duration`) corresponds to a
/// constant ratio in linear amplitude (`10^(step_db / 20)`), we evaluate `exp2` only twice per
/// slice in `f64` precision and advance per sample with a single multiplication (`cur *= step`).
/// Over an audio frame (`<= 1024` samples), `f64` rounding drift stays below `1e-12`, preserving
/// exact `f32` accuracy while eliminating per-sample transcendental calls.
///
/// # Arguments
/// * `elapsed` - Number of samples already processed in this ramp prior to `out_slice`.
/// * `duration` - Total ramp duration in samples (`> 0`).
/// * `out_slice` - Destination slice of non-zero length to populate with linear gain multipliers.
fn fill_linear_db_ramp(
    start_db: Decibels,
    end_db: Decibels,
    elapsed: u32,
    duration: u32,
    out_slice: &mut [f32],
) -> f32 {
    const DB_TO_LOG2_SCALE: f64 = 0.05 * std::f64::consts::LOG2_10;
    let delta_db = (end_db.value() - start_db.value()) as f64;
    let inv_duration = 1.0 / (duration as f64);
    let init_db = (start_db.value() as f64) + delta_db * (elapsed as f64) * inv_duration;
    let step_db = delta_db * inv_duration;

    let mut cur = (init_db * DB_TO_LOG2_SCALE).exp2();
    let step = (step_db * DB_TO_LOG2_SCALE).exp2();
    let mut last_gain = cur as f32;

    for slot in out_slice.iter_mut() {
        let g = cur as f32;
        *slot = g;
        last_gain = g;
        cur *= step;
    }

    last_gain
}

/// Fills `out_slice` with linear amplitude multipliers for a quadratic Bezier decibel curve.
///
/// Precomputes `1.0 / duration` once per slice and iterates over `out_slice` directly to avoid
/// per-sample integer-to-float divisions and slice bounds checks.
///
/// # Arguments
/// * `elapsed` - Number of samples already processed in this curve prior to `out_slice`.
/// * `duration` - Total curve duration in samples (`> 0`).
/// * `out_slice` - Destination slice of non-zero length to populate with linear gain multipliers.
fn fill_bezier_db_curve(
    anim: &AnimatedDecibels,
    elapsed: u32,
    duration: u32,
    out_slice: &mut [f32],
) -> f32 {
    let inv_duration = 1.0 / (duration as f32);
    let mut last_gain = 1.0;

    for (i, slot) in out_slice.iter_mut().enumerate() {
        let t = (elapsed + i as u32) as f32 * inv_duration;
        let g = LinearGain::from(anim.sample(t)).value();
        *slot = g;
        last_gain = g;
    }

    last_gain
}

/// Generates a slice of gain multipliers for a block of audio samples by consuming
/// or progressing the updates in the `GainParameter`.
///
/// Animated gains are interpolated in the decibel (dB) domain and converted to linear
/// multipliers. Constant and step gains convert once per slice, linear dB ramps use a geometric
/// multiplier recurrence, and Bezier dB curves evaluate per sample via `exp2`.
///
/// # Arguments
/// * `param` - The active gain parameter state to progress.
/// * `samples` - The number of samples to generate multipliers for.
/// * `out_multipliers` - The output slice to fill with the calculated gain multipliers.
pub(crate) fn generate_gain_multipliers(
    param: &mut GainParameter,
    samples: usize,
    out_multipliers: &mut [f32],
) {
    let mut idx = 0;
    while idx < samples {
        if let Some(update) = param.updates.front_mut() {
            let dur = u32::from(update.duration);
            let remaining_in_update = (dur - update.elapsed) as usize;
            let count = remaining_in_update.min(samples - idx);
            let out_slice = &mut out_multipliers[idx..idx + count];

            if !out_slice.is_empty() {
                match &update.gain {
                    GainVariant::Constant(g) => {
                        out_slice.fill(g.value());
                        param.last_value = g.value();
                    }
                    GainVariant::Multiple(g_arr) => {
                        let start = update.elapsed as usize;
                        for (slot, gain) in out_slice.iter_mut().zip(&g_arr[start..start + count]) {
                            *slot = gain.value();
                        }
                        param.last_value = g_arr[start + count - 1].value();
                    }
                    GainVariant::Animated(anim) => match anim {
                        Animated::Step { value } => {
                            let g = LinearGain::from(*value).value();
                            out_slice.fill(g);
                            param.last_value = g;
                        }
                        Animated::Linear { start, end } => {
                            param.last_value =
                                fill_linear_db_ramp(*start, *end, update.elapsed, dur, out_slice);
                        }
                        Animated::Bezier { .. } => {
                            param.last_value =
                                fill_bezier_db_curve(anim, update.elapsed, dur, out_slice);
                        }
                    },
                }
            }

            update.elapsed += count as u32;
            idx += count;
            if update.elapsed >= dur {
                param.updates.pop_front();
            }
        } else {
            // Queue empty, hold last value
            out_multipliers[idx..samples].fill(param.last_value);
            break;
        }
    }
}

// ===== Tests =====

#[cfg(test)]
mod test {

    use super::*;
    use crate::common::animation::Animated;
    use crate::common::units::LinearGain;
    use googletest::prelude::*;

    #[gtest]
    fn test_new_parameter() {
        let param = GainParameter::new(42);

        expect_eq!(param.id, 42);
        expect_true!(param.updates.is_empty());
        expect_eq!(param.last_value, 1.0);
    }

    #[gtest]
    fn test_generate_multipliers_empty_queue() {
        let mut param = GainParameter::new(42);
        param.last_value = 0.5;
        let mut multipliers = vec![0.0f32; 10];

        generate_gain_multipliers(&mut param, 10, &mut multipliers);

        expect_eq!(multipliers, vec![0.5f32; 10]);
        expect_eq!(param.last_value, 0.5);
    }

    #[gtest]
    fn test_generate_multipliers_constant_gain() {
        let mut param = GainParameter::new(42);
        param.updates.push_back(GainUpdate {
            gain: GainVariant::Constant(LinearGain::new(0.8).unwrap()),
            duration: Samples::new(5).unwrap(),
            elapsed: 0,
        });
        let mut multipliers = vec![0.0f32; 10];

        generate_gain_multipliers(&mut param, 10, &mut multipliers);

        // First 5 samples should be 0.8, remaining should hold last_value (0.8)
        expect_eq!(multipliers, vec![0.8f32; 10]);
        expect_true!(param.updates.is_empty());
        expect_eq!(param.last_value, 0.8);
    }

    #[gtest]
    fn test_generate_multipliers_constant_gain_split() {
        let mut param = GainParameter::new(42);
        param.updates.push_back(GainUpdate {
            gain: GainVariant::Constant(LinearGain::new(0.8).unwrap()),
            duration: Samples::new(5).unwrap(),
            elapsed: 0,
        });
        param.updates.push_back(GainUpdate {
            gain: GainVariant::Constant(LinearGain::new(0.3).unwrap()),
            duration: Samples::new(5).unwrap(),
            elapsed: 0,
        });
        let mut multipliers = vec![0.0f32; 8];

        generate_gain_multipliers(&mut param, 8, &mut multipliers);

        // First 5 samples should be 0.8, next 3 should be 0.3
        expect_eq!(multipliers, vec![0.8, 0.8, 0.8, 0.8, 0.8, 0.3, 0.3, 0.3]);
        expect_eq!(param.updates.len(), 1);
        expect_eq!(param.updates[0].elapsed, 3);
        expect_eq!(param.last_value, 0.3);
    }

    #[gtest]
    fn test_generate_multipliers_multiple_gains() {
        let mut param = GainParameter::new(42);
        param.updates.push_back(GainUpdate {
            gain: GainVariant::Multiple(vec![
                LinearGain::new(0.1).unwrap(),
                LinearGain::new(0.2).unwrap(),
                LinearGain::new(0.3).unwrap(),
            ]),
            duration: Samples::new(3).unwrap(),
            elapsed: 0,
        });
        let mut multipliers = vec![0.0f32; 5];

        generate_gain_multipliers(&mut param, 5, &mut multipliers);

        expect_eq!(multipliers, vec![0.1, 0.2, 0.3, 0.3, 0.3]);
        expect_true!(param.updates.is_empty());
        expect_eq!(param.last_value, 0.3);
    }

    #[gtest]
    fn test_generate_multipliers_animated_step_converts_db_to_linear() {
        let mut param = GainParameter::new(42);
        param.updates.push_back(GainUpdate {
            gain: GainVariant::Animated(Animated::Step { value: Decibels::new(-20.0).unwrap() }),
            duration: Samples::new(3).unwrap(),
            elapsed: 0,
        });
        let mut multipliers = vec![0.0f32; 5];

        generate_gain_multipliers(&mut param, 5, &mut multipliers);

        expect_that!(multipliers[0], near(0.1f32, 1e-6));
        expect_that!(multipliers[1], near(0.1f32, 1e-6));
        expect_that!(multipliers[2], near(0.1f32, 1e-6));
        expect_that!(multipliers[3], near(0.1f32, 1e-6));
        expect_that!(multipliers[4], near(0.1f32, 1e-6));
        expect_that!(param.last_value, near(0.1f32, 1e-6));
    }

    #[gtest]
    fn test_generate_multipliers_animated_linear_interpolates_in_db() {
        let mut param = GainParameter::new(42);
        param.updates.push_back(GainUpdate {
            gain: GainVariant::Animated(Animated::Linear {
                start: Decibels::ZERO,
                end: Decibels::new(-20.0).unwrap(),
            }),
            duration: Samples::new(5).unwrap(),
            elapsed: 0,
        });
        let mut multipliers = vec![0.0f32; 7];

        generate_gain_multipliers(&mut param, 7, &mut multipliers);

        // Duration is 5: samples at 0 dB, -4 dB, -8 dB, -12 dB, -16 dB, holding last_value.
        expect_that!(multipliers[0], near(1.0f32, 1e-6));
        expect_that!(multipliers[1], near(0.63095737f32, 1e-6));
        expect_that!(multipliers[2], near(0.39810717f32, 1e-6));
        expect_that!(multipliers[3], near(0.25118864f32, 1e-6));
        expect_that!(multipliers[4], near(0.15848932f32, 1e-6));
        expect_that!(multipliers[5], near(0.15848932f32, 1e-6));
        expect_that!(multipliers[6], near(0.15848932f32, 1e-6));
    }

    #[gtest]
    fn test_generate_multipliers_animated_bezier_interpolates_in_db() {
        let mut param = GainParameter::new(42);
        let db_neg20 = Decibels::new(-20.0).unwrap();
        param.updates.push_back(GainUpdate {
            gain: GainVariant::Animated(Animated::Bezier {
                start: Decibels::ZERO,
                end: db_neg20,
                control: db_neg20,
                control_relative_time: 0.5,
            }),
            duration: Samples::new(2).unwrap(),
            elapsed: 0,
        });
        let mut multipliers = vec![0.0f32; 2];

        generate_gain_multipliers(&mut param, 2, &mut multipliers);

        // At t = 0.0 -> 0 dB (1.0); at t = 0.5 -> q0 = -10 dB, q1 = -20 dB -> -15 dB (0.17782794).
        expect_that!(multipliers[0], near(1.0f32, 1e-6));
        expect_that!(multipliers[1], near(0.17782794f32, 1e-6));
    }

    #[gtest]
    fn test_generate_multipliers_animated_linear_split_across_blocks_preserves_continuity() {
        let mut param = GainParameter::new(42);
        param.updates.push_back(GainUpdate {
            gain: GainVariant::Animated(Animated::Linear {
                start: Decibels::ZERO,
                end: Decibels::new(-20.0).unwrap(),
            }),
            duration: Samples::new(5).unwrap(),
            elapsed: 0,
        });
        let mut block_a = vec![0.0f32; 2];
        let mut block_b = vec![0.0f32; 3];

        generate_gain_multipliers(&mut param, 2, &mut block_a);
        generate_gain_multipliers(&mut param, 3, &mut block_b);

        expect_that!(block_a[0], near(1.0f32, 1e-6));
        expect_that!(block_a[1], near(0.63095737f32, 1e-6));
        expect_that!(block_b[0], near(0.39810717f32, 1e-6));
        expect_that!(block_b[1], near(0.25118864f32, 1e-6));
        expect_that!(block_b[2], near(0.15848932f32, 1e-6));
        expect_true!(param.updates.is_empty());
    }
}
