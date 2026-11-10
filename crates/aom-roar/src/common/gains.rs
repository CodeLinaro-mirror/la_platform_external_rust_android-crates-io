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

use super::animation::{Animated, AnimatedDecibels, AnimatedFloat32};
use super::units::{Decibels, LinearGain};
use crate::common::definitions::OarError;

/// Dynamic gain control, enforcing Decibels for creation.
///
/// Static (`Constant` and `Multiple`) gains are converted upfront to `LinearGain`,
/// while `Animated` gains retain `Decibels` so interpolation occurs in the dB domain.
#[derive(Debug, Clone, PartialEq)]
pub struct Gain {
    pub(crate) variant: GainVariant,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum GainVariant {
    Constant(LinearGain),
    Multiple(Vec<LinearGain>),
    Animated(AnimatedDecibels),
}

impl Gain {
    /// Creates a new constant gain metadata from Decibels.
    pub fn new_constant(val: Decibels) -> Result<Self, OarError> {
        Ok(Gain { variant: GainVariant::Constant(LinearGain::from(val)) })
    }

    /// Creates a new per-channel static gain metadata from Decibels.
    pub fn new_multiple(vals: Vec<Decibels>) -> Result<Self, OarError> {
        let lin_vals = vals.into_iter().map(LinearGain::from).collect();
        Ok(Gain { variant: GainVariant::Multiple(lin_vals) })
    }

    /// Creates a new animated gain metadata where the animation start/end/control are in Decibels.
    pub fn new_animated(anim: AnimatedFloat32) -> Result<Self, OarError> {
        let variant = match anim {
            Animated::Step { value } => Animated::Step { value: Decibels::new(value)? },
            Animated::Linear { start, end } => {
                Animated::Linear { start: Decibels::new(start)?, end: Decibels::new(end)? }
            }
            Animated::Bezier { start, end, control, control_relative_time } => {
                if !control_relative_time.is_finite()
                    || !(0.0..=1.0).contains(&control_relative_time)
                {
                    return Err(OarError::InvalidParameter);
                }
                Animated::Bezier {
                    start: Decibels::new(start)?,
                    end: Decibels::new(end)?,
                    control: Decibels::new(control)?,
                    control_relative_time,
                }
            }
        };
        Ok(Gain { variant: GainVariant::Animated(variant) })
    }
}

// ===== Tests =====

#[cfg(test)]
mod test {

    use super::*;
    use googletest::prelude::*;

    #[gtest]
    fn test_gain_new_constant_stores_linear_gain() {
        let gain = Gain::new_constant(Decibels::ZERO);

        expect_ok!(&gain);
        expect_eq!(gain.unwrap(), Gain { variant: GainVariant::Constant(LinearGain::ONE) });
    }

    #[gtest]
    fn test_gain_new_multiple_stores_linear_gains() {
        let db_neg3 = Decibels::new(-3.0).unwrap();
        let db_pos3 = Decibels::new(3.0).unwrap();
        let gain = Gain::new_multiple(vec![db_neg3, Decibels::ZERO, db_pos3]);

        expect_ok!(&gain);
        expect_eq!(
            gain.unwrap(),
            Gain {
                variant: GainVariant::Multiple(vec![
                    LinearGain::from(db_neg3),
                    LinearGain::ONE,
                    LinearGain::from(db_pos3),
                ]),
            }
        );
    }

    #[gtest]
    fn test_gain_new_animated_stores_decibels() {
        let gain = Gain::new_animated(Animated::Linear { start: 0.0, end: -12.0 });

        expect_ok!(&gain);
        expect_eq!(
            gain.unwrap(),
            Gain {
                variant: GainVariant::Animated(Animated::Linear {
                    start: Decibels::ZERO,
                    end: Decibels::new(-12.0).unwrap(),
                }),
            }
        );
    }

    #[gtest]
    fn test_gain_new_animated_bezier_rejects_out_of_range_control_time() {
        let gain = Gain::new_animated(Animated::Bezier {
            start: 0.0,
            end: -12.0,
            control: -6.0,
            control_relative_time: 1.5,
        });

        expect_eq!(gain.unwrap_err(), OarError::InvalidParameter);
    }
}
