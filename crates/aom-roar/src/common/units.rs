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

use crate::common::definitions::OarError;
use std::ops::Mul;

// ===== Types =====

/// Represents an angle in degrees.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Degrees(f32);

/// Represents an angle in radians.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Radians(f32);

/// Represents a value in decibels (dB).
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Decibels(f32);

/// Sample rate in Hz.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SampleRate(u32);

/// Represents a duration in milliseconds.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Milliseconds(f32);

/// Represents a duration or offset measured in audio samples.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Samples(u32);

// ===== Degrees =====

impl Degrees {
    /// Zero degrees (`0.0°`).
    pub const ZERO: Self = Self(0.0);

    /// Creates a new `Degrees` value if it is finite.
    pub fn new(val: f32) -> Result<Self, OarError> {
        if val.is_finite() {
            Ok(Self(val))
        } else {
            Err(OarError::InvalidParameter)
        }
    }

    /// Creates a new `Degrees` value in a `const` context, panicking if `val` is not finite.
    pub(crate) const fn from_const(val: f32) -> Self {
        assert!(val.is_finite());
        Self(val)
    }

    /// Returns the raw `f32` value in degrees.
    pub const fn value(&self) -> f32 {
        self.0
    }
}

// ===== Decibels =====

impl Decibels {
    /// Minimum decibel floor representing silence/zero gain.
    pub const MIN_DECIBELS: f32 = -120.0;

    /// Zero decibels (`0.0 dB`, unity gain).
    pub const ZERO: Self = Self(0.0);

    /// Creates a new `Decibels` value if it is finite.
    pub fn new(val: f32) -> Result<Self, OarError> {
        if val.is_finite() {
            Ok(Self(val))
        } else {
            Err(OarError::InvalidParameter)
        }
    }

    /// Creates a new `Decibels` value in a `const` context, panicking if `val` is not finite.
    pub(crate) const fn from_const(val: f32) -> Self {
        assert!(val.is_finite());
        Self(val)
    }

    /// Returns the raw `f32` value in decibels.
    pub const fn value(&self) -> f32 {
        self.0
    }
}
// ===== SampleRate =====

impl SampleRate {
    /// Creates a new `SampleRate` if `val` is positive.
    pub fn new(val: u32) -> Result<Self, OarError> {
        if val > 0 {
            Ok(SampleRate(val))
        } else {
            Err(OarError::InvalidParameter)
        }
    }

    /// Returns the raw `u32` sample rate in Hz.
    pub const fn value(&self) -> u32 {
        self.0
    }
}

// Multiply SampleRate by Milliseconds to get duration in Samples.
impl Mul<Milliseconds> for SampleRate {
    type Output = Samples;

    fn mul(self, rhs: Milliseconds) -> Self::Output {
        Samples(((self.0 as f64 * rhs.0 as f64) / 1000.0).round() as u32)
    }
}

// ===== Radians =====

impl Radians {
    /// Zero radians (`0.0 rad`).
    pub const ZERO: Self = Self(0.0);

    /// Creates a new `Radians` value if it is finite.
    pub fn new(val: f32) -> Result<Self, OarError> {
        if val.is_finite() {
            Ok(Self(val))
        } else {
            Err(OarError::InvalidParameter)
        }
    }

    /// Returns the raw `f32` value in radians.
    pub const fn value(&self) -> f32 {
        self.0
    }
}

/// Represents a linear gain/amplitude factor.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct LinearGain(f32);

impl From<Degrees> for Radians {
    fn from(deg: Degrees) -> Self {
        Radians(deg.0.to_radians())
    }
}

impl From<Radians> for Degrees {
    fn from(rad: Radians) -> Self {
        Degrees(rad.0.to_degrees())
    }
}

// ===== LinearGain =====

impl LinearGain {
    /// Zero linear gain (`0.0`, silence).
    pub const ZERO: Self = Self(0.0);

    /// Unity linear gain (`1.0`).
    pub const ONE: Self = Self(1.0);

    /// Creates a new `LinearGain` value if it is finite and non-negative.
    pub fn new(val: f32) -> Result<Self, OarError> {
        if val.is_finite() && val >= 0.0 {
            Ok(Self(val))
        } else {
            Err(OarError::InvalidParameter)
        }
    }

    /// Returns the raw `f32` linear amplitude factor.
    pub const fn value(&self) -> f32 {
        self.0
    }
}

impl std::ops::Sub for Decibels {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Decibels(self.0 - rhs.0)
    }
}

impl From<Decibels> for LinearGain {
    fn from(db: Decibels) -> Self {
        // Evaluate `10^(db / 20)` as `2^(db * 0.05 * log2(10))` so per-sample dB gain
        // interpolation calls single-argument `exp2f` rather than general `powf`.
        const DB_TO_LOG2_SCALE: f32 = 0.05 * std::f32::consts::LOG2_10;
        LinearGain((db.0 * DB_TO_LOG2_SCALE).exp2())
    }
}

impl From<LinearGain> for Decibels {
    fn from(lin: LinearGain) -> Self {
        if lin.0 <= 0.0 {
            Decibels(Decibels::MIN_DECIBELS)
        } else {
            Decibels(20.0 * lin.0.log10())
        }
    }
}

impl std::ops::Add for LinearGain {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        LinearGain(self.0 + rhs.0)
    }
}

impl std::ops::Sub for LinearGain {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        LinearGain(self.0 - rhs.0)
    }
}

impl std::ops::Mul<f32> for LinearGain {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self {
        LinearGain(self.0 * rhs)
    }
}

impl std::ops::MulAssign<LinearGain> for f32 {
    fn mul_assign(&mut self, rhs: LinearGain) {
        *self *= rhs.0;
    }
}

impl std::ops::Sub<f32> for LinearGain {
    type Output = f32;
    fn sub(self, rhs: f32) -> Self::Output {
        self.0 - rhs
    }
}

// ===== Milliseconds =====

impl Milliseconds {
    /// Creates a new `Milliseconds` value if it is finite and positive.
    pub fn new(val: f32) -> Result<Self, OarError> {
        if val.is_finite() && val > 0.0 {
            Ok(Milliseconds(val))
        } else {
            Err(OarError::InvalidParameter)
        }
    }

    /// Creates a new `Milliseconds` value in a `const` context, panicking if `val` is not finite
    /// and positive.
    pub(crate) const fn from_const(val: f32) -> Self {
        assert!(val.is_finite() && val > 0.0);
        Self(val)
    }

    /// Returns the raw `f32` duration in milliseconds.
    pub const fn value(&self) -> f32 {
        self.0
    }
}

impl Mul<SampleRate> for Milliseconds {
    type Output = Samples;

    fn mul(self, rhs: SampleRate) -> Self::Output {
        rhs * self
    }
}

// ===== Samples =====

impl Samples {
    /// Creates a new `Samples` value if it is within `1..=i32::MAX`.
    pub fn new(val: u32) -> Result<Self, OarError> {
        if val > 0 && val <= i32::MAX as u32 {
            Ok(Samples(val))
        } else {
            Err(OarError::InvalidParameter)
        }
    }

    /// Returns the raw `u32` sample count.
    pub const fn value(&self) -> u32 {
        self.0
    }
}

impl From<Samples> for u32 {
    fn from(s: Samples) -> Self {
        s.0
    }
}

impl From<Samples> for usize {
    fn from(s: Samples) -> Self {
        s.0 as usize
    }
}

// ===== Tests =====

#[cfg(test)]
mod test {

    use super::*;
    use googletest::prelude::*;

    #[gtest]
    fn test_angle_conversions() {
        let deg = Degrees(180.0);
        let rad = Radians::from(deg);
        expect_that!(rad.value(), near(std::f32::consts::PI, 1e-6));

        let round_trip = Degrees::from(rad);
        expect_that!(round_trip.value(), near(180.0, 1e-6));
    }

    #[gtest]
    fn test_gain_conversions() {
        let db = Decibels(-6.0206);
        let lin = LinearGain::from(db);
        expect_that!(lin.value(), near(0.5, 1e-4));

        let round_trip = Decibels::from(lin);
        expect_that!(round_trip.value(), near(-6.0206, 1e-4));

        let zero_lin = LinearGain(0.0);
        let zero_db = Decibels::from(zero_lin);
        expect_that!(zero_db.value(), eq(Decibels::MIN_DECIBELS));
    }

    #[gtest]
    fn test_degrees_validation() {
        expect_ok!(Degrees::new(0.0));
        expect_ok!(Degrees::new(360.0));
        expect_ok!(Degrees::new(-180.0));

        expect_eq!(Degrees::new(f32::NAN).unwrap_err(), OarError::InvalidParameter);
        expect_eq!(Degrees::new(f32::INFINITY).unwrap_err(), OarError::InvalidParameter);
    }

    #[gtest]
    fn test_radians_validation() {
        expect_ok!(Radians::new(0.0));
        expect_ok!(Radians::new(std::f32::consts::PI));

        expect_eq!(Radians::new(f32::NAN).unwrap_err(), OarError::InvalidParameter);
        expect_eq!(Radians::new(f32::INFINITY).unwrap_err(), OarError::InvalidParameter);
    }

    #[gtest]
    fn test_decibels_validation() {
        expect_ok!(Decibels::new(0.0));
        expect_ok!(Decibels::new(-120.0));
        expect_ok!(Decibels::new(12.0));

        expect_eq!(Decibels::new(f32::NAN).unwrap_err(), OarError::InvalidParameter);
        expect_eq!(Decibels::new(f32::INFINITY).unwrap_err(), OarError::InvalidParameter);
    }

    #[gtest]
    fn test_lineargain_validation() {
        expect_ok!(LinearGain::new(0.0));
        expect_ok!(LinearGain::new(1.0));

        expect_eq!(LinearGain::new(-0.1).unwrap_err(), OarError::InvalidParameter);
        expect_eq!(LinearGain::new(f32::NAN).unwrap_err(), OarError::InvalidParameter);
        expect_eq!(LinearGain::new(f32::INFINITY).unwrap_err(), OarError::InvalidParameter);
    }

    #[gtest]
    fn test_milliseconds_validation() {
        expect_ok!(Milliseconds::new(1.0));
        expect_ok!(Milliseconds::new(100.0));

        expect_eq!(Milliseconds::new(0.0).unwrap_err(), OarError::InvalidParameter);
        expect_eq!(Milliseconds::new(-1.0).unwrap_err(), OarError::InvalidParameter);
        expect_eq!(Milliseconds::new(f32::NAN).unwrap_err(), OarError::InvalidParameter);
        expect_eq!(Milliseconds::new(f32::INFINITY).unwrap_err(), OarError::InvalidParameter);
    }

    #[gtest]
    fn test_sample_rate_validation() {
        expect_ok!(SampleRate::new(1));
        expect_ok!(SampleRate::new(48_000));

        expect_eq!(SampleRate::new(0).unwrap_err(), OarError::InvalidParameter);
    }

    #[gtest]
    fn test_sample_rate_times_milliseconds() {
        let sr = SampleRate(48_000);
        let ms = Milliseconds(1000.0);
        expect_eq!(sr * ms, Samples(48_000));
        expect_eq!(ms * sr, Samples(48_000));

        let ms = Milliseconds(5.0);
        expect_eq!(sr * ms, Samples(240));
        expect_eq!(ms * sr, Samples(240));
    }

    #[gtest]
    fn test_lineargain_operators() {
        let g1 = LinearGain(0.5);
        let g2 = LinearGain(0.25);

        expect_that!((g1 + g2).value(), near(0.75, 1e-6));
        expect_that!((g1 - g2).value(), near(0.25, 1e-6));
        expect_that!((g1 * 2.0).value(), near(1.0, 1e-6));
    }

    #[gtest]
    fn test_samples_validation() {
        expect_eq!(Samples::new(0).unwrap_err(), OarError::InvalidParameter);
        expect_ok!(Samples::new(1));
        expect_ok!(Samples::new(i32::MAX as u32));
        expect_eq!(Samples::new(i32::MAX as u32 + 1).unwrap_err(), OarError::InvalidParameter);
    }
}
