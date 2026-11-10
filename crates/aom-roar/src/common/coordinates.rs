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

use super::definitions::OarError;
use super::units::{Degrees, Radians};
use std::ops::{Add, Mul, Sub};

/// Represents a unitless distance from the listener, restricted to non-negative values.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Distance(f32);

/// Represents a 3D spherical polar coordinate centered on the listener, equivalent of C API
/// `roar_polar_t`.
///
/// Coordinate system details:
/// - `azimuth`: Horizontal angle in degrees. Range: `[-180.0, 180.0]`. 0° = front, 90° = left, -90°
///   = right, ±180° = behind.
/// - `elevation`: Vertical angle in degrees. Range: `[-90.0, 90.0]`. 0° = horizon, 90° = above,
///   -90° = below.
/// - `distance`: Distance from the listener. Range: `[0.0, f32::MAX]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PolarCoordinate {
    azimuth: Degrees,
    elevation: Degrees,
    distance: Distance,
}

/// Represents a 3D cartesian coordinate, equivalent of C API `roar_cartesian_t`.
///
/// Uses the ADM object coordinate system centered on the listener:
/// - **X-axis**: Left (negative) / Right (positive).
/// - **Y-axis**: Back (negative) / Front (positive).
/// - **Z-axis**: Down (negative) / Up (positive).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CartesianCoordinate {
    x: f32,
    y: f32,
    z: f32,
}

// ===== Distance =====

impl Distance {
    /// Represents a distance of 1.0.
    pub const ONE: Self = Self(1.0);

    /// Creates a new Distance. Returns error if value is not finite or negative.
    pub fn new(val: f32) -> Result<Self, OarError> {
        if val.is_finite() && val >= 0.0 {
            Ok(Self(val))
        } else {
            Err(OarError::InvalidParameter)
        }
    }

    /// Returns the raw f32 value.
    pub const fn value(&self) -> f32 {
        self.0
    }
}

// ===== PolarCoordinate =====

impl PolarCoordinate {
    /// Default unit-distance front position (`azimuth = 0.0°`, `elevation = 0.0°`,
    /// `distance = 1.0`).
    pub const FRONT: Self =
        Self { azimuth: Degrees::ZERO, elevation: Degrees::ZERO, distance: Distance::ONE };

    /// Creates a new PolarCoordinate.
    ///
    /// Validates and normalizes coordinate values:
    /// - Elevation must be in `[-90.0, 90.0]`.
    /// - Distance must be non-negative (`>= 0.0`).
    /// - Azimuth is normalized to `[-180.0, 180.0]`.
    pub fn new(azimuth: Degrees, elevation: Degrees, distance: Distance) -> Result<Self, OarError> {
        let mut az_val = azimuth.value();
        let mut el_val = elevation.value();

        el_val %= 360.0;
        while el_val > 90.0 {
            el_val = 180.0 - el_val;
            az_val += 180.0;
        }
        while el_val < -90.0 {
            el_val = -180.0 - el_val;
            az_val += 180.0;
        }

        // Normalize azimuth to (-180, 180]
        az_val %= 360.0;
        if az_val <= -180.0 {
            az_val += 360.0;
        }
        if az_val > 180.0 {
            az_val -= 360.0;
        }
        if az_val == -180.0 {
            az_val = 180.0;
        }

        Ok(PolarCoordinate {
            azimuth: Degrees::new(az_val)?,
            elevation: Degrees::new(el_val)?,
            distance,
        })
    }

    /// Helper to create a new PolarCoordinate from raw floats.
    pub fn new_from_floats(azimuth: f32, elevation: f32, distance: f32) -> Result<Self, OarError> {
        let az = Degrees::new(azimuth)?;
        let el = Degrees::new(elevation)?;
        let dist = Distance::new(distance)?;
        Self::new(az, el, dist)
    }

    /// Returns the normalized azimuth angle.
    pub fn azimuth(&self) -> Degrees {
        self.azimuth
    }

    /// Returns the elevation angle.
    pub fn elevation(&self) -> Degrees {
        self.elevation
    }

    /// Returns the distance value.
    pub fn distance(&self) -> Distance {
        self.distance
    }
}

impl TryFrom<CartesianCoordinate> for PolarCoordinate {
    type Error = OarError;
    fn try_from(c: CartesianCoordinate) -> Result<Self, OarError> {
        let dist_val = c.x.hypot(c.y).hypot(c.z);
        // Avoid division by zero.
        if dist_val < 1e-10_f32 {
            return Self::new(Degrees::ZERO, Degrees::ZERO, Distance::new(0.0)?);
        }
        let distance = Distance::new(dist_val)?;
        let azimuth = Degrees::new((-c.x).atan2(c.y).to_degrees())?;
        // Normalize z by distance before calling asin()
        let elevation = Degrees::new((c.z / dist_val).asin().to_degrees())?;
        Self::new(azimuth, elevation, distance)
    }
}

// ===== CartesianCoordinate =====

impl Default for CartesianCoordinate {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0, z: 0.0 }
    }
}

impl CartesianCoordinate {
    /// Creates a new CartesianCoordinate. Returns error if any value is not finite.
    pub fn new(x: f32, y: f32, z: f32) -> Result<Self, OarError> {
        if x.is_finite() && y.is_finite() && z.is_finite() {
            Ok(Self { x, y, z })
        } else {
            Err(OarError::InvalidParameter)
        }
    }

    /// Creates a new `CartesianCoordinate` in a `const` context, panicking if any component is not
    /// finite.
    pub(crate) const fn from_const(x: f32, y: f32, z: f32) -> Self {
        assert!(x.is_finite() && y.is_finite() && z.is_finite());
        Self { x, y, z }
    }

    /// Returns the x coordinate.
    pub const fn x(&self) -> f32 {
        self.x
    }
    /// Returns the y coordinate.
    pub const fn y(&self) -> f32 {
        self.y
    }
    /// Returns the z coordinate.
    pub const fn z(&self) -> f32 {
        self.z
    }
}

impl Add for CartesianCoordinate {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self { x: self.x + rhs.x, y: self.y + rhs.y, z: self.z + rhs.z }
    }
}

impl Sub for CartesianCoordinate {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self { x: self.x - rhs.x, y: self.y - rhs.y, z: self.z - rhs.z }
    }
}

impl Mul<f32> for CartesianCoordinate {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self::Output {
        Self { x: self.x * rhs, y: self.y * rhs, z: self.z * rhs }
    }
}

impl TryFrom<PolarCoordinate> for CartesianCoordinate {
    type Error = OarError;
    fn try_from(polar: PolarCoordinate) -> Result<Self, OarError> {
        let azimuth_rad = Radians::from(polar.azimuth).value();
        let elevation_rad = Radians::from(polar.elevation).value();
        let dist = polar.distance.value();
        CartesianCoordinate::new(
            dist * (-azimuth_rad).sin() * elevation_rad.cos(),
            dist * (-azimuth_rad).cos() * elevation_rad.cos(),
            dist * elevation_rad.sin(),
        )
    }
}

// ===== Tests =====

#[cfg(test)]
mod test {

    use super::*;
    use googletest::prelude::*;

    #[gtest]
    fn test_distance_constant() {
        expect_that!(Distance::ONE.value(), eq(1.0));
    }

    #[gtest]
    fn test_polar_coordinate_valid_creation() {
        let p1 = PolarCoordinate::new_from_floats(30.0, 45.0, 0.5);

        expect_ok!(&p1);

        let p1 = p1.unwrap();
        expect_that!(p1.azimuth().value(), eq(30.0));
        expect_that!(p1.elevation().value(), eq(45.0));
        expect_that!(p1.distance().value(), eq(0.5));
    }

    #[gtest]
    fn test_polar_coordinate_pole_mirroring() {
        // Elevation > 90
        let p2 = PolarCoordinate::new_from_floats(30.0, 95.0, 0.5);
        expect_ok!(&p2);

        let p2 = p2.unwrap();
        expect_that!(p2.elevation().value(), eq(85.0));
        expect_that!(p2.azimuth().value(), eq(-150.0));
    }

    #[gtest]
    fn test_polar_coordinate_extreme_pole_mirroring() {
        // Elevation > 90
        let p2 = PolarCoordinate::new_from_floats(-30.0, 500.0, 0.5);
        expect_ok!(&p2);

        let p2 = p2.unwrap();
        expect_that!(p2.elevation().value(), eq(40.0));
        expect_that!(p2.azimuth().value(), eq(150.0));
    }

    #[gtest]
    fn test_polar_coordinate_pole_invalid_distance() {
        // Invalid distance
        let p3 = PolarCoordinate::new_from_floats(30.0, 45.0, -0.1);
        expect_true!(p3.is_err());
    }

    #[gtest]
    fn test_polar_coordinate_azimuth_normalization() {
        // Azimuth normalization
        let p4 = PolarCoordinate::new_from_floats(190.0, 45.0, 0.5).unwrap();
        expect_that!(p4.azimuth().value(), eq(-170.0));

        let p5 = PolarCoordinate::new_from_floats(-190.0, 45.0, 0.5).unwrap();
        expect_that!(p5.azimuth().value(), eq(170.0));
    }

    #[gtest]
    fn test_cartesian_coordinate_new_success() {
        let c = CartesianCoordinate::new(1.0, 2.0, 3.0);
        expect_ok!(&c);
        let c = c.unwrap();
        expect_that!(c.x(), eq(1.0));
        expect_that!(c.y(), eq(2.0));
        expect_that!(c.z(), eq(3.0));
    }

    #[gtest]
    fn test_cartesian_coordinate_new_non_finite() {
        let c = CartesianCoordinate::new(f32::NAN, 2.0, 3.0);
        expect_true!(c.is_err());
    }

    #[gtest]
    fn test_cartesian_coordinate_default() {
        let c = CartesianCoordinate::default();
        expect_that!(c.x, eq(0.0));
        expect_that!(c.y, eq(0.0));
        expect_that!(c.z, eq(0.0));
    }

    #[gtest]
    fn test_cartesian_coordinate_add() {
        let c1 = CartesianCoordinate::new(1.0, 2.0, 3.0).unwrap();
        let c2 = CartesianCoordinate::new(4.0, 5.0, 6.0).unwrap();
        let sum = c1 + c2;
        expect_that!(sum.x, eq(5.0));
        expect_that!(sum.y, eq(7.0));
        expect_that!(sum.z, eq(9.0));
    }

    #[gtest]
    fn test_cartesian_coordinate_sub() {
        let c1 = CartesianCoordinate::new(1.0, 2.0, 3.0).unwrap();
        let c2 = CartesianCoordinate::new(4.0, 5.0, 6.0).unwrap();
        let diff = c1 - c2;
        expect_that!(diff.x, eq(-3.0));
        expect_that!(diff.y, eq(-3.0));
        expect_that!(diff.z, eq(-3.0));
    }

    #[gtest]
    fn test_cartesian_coordinate_mul() {
        let c1 = CartesianCoordinate::new(1.0, 2.0, 3.0).unwrap();
        let scaled = c1 * 2.5;
        expect_that!(scaled.x, eq(2.5));
        expect_that!(scaled.y, eq(5.0));
        expect_that!(scaled.z, eq(7.5));
    }

    #[gtest]
    fn cartesian_to_polar_very_small_values_returns_origin() {
        let cart = CartesianCoordinate { x: 1e-11, y: 1e-11, z: 1e-11 };

        let polar: PolarCoordinate = cart.try_into().unwrap();

        expect_that!(polar.azimuth().value(), eq(0.0));
        expect_that!(polar.elevation().value(), eq(0.0));
        expect_that!(polar.distance().value(), eq(0.0));
    }

    #[gtest]
    fn cartesian_to_polar_zero_azimuth_and_elevation() {
        let cart = CartesianCoordinate { x: 0.0, y: 1.0, z: 0.0 };

        let polar: PolarCoordinate = cart.try_into().unwrap();

        expect_that!(polar.azimuth().value(), near(0.0f32, 1e-5));
        expect_that!(polar.elevation().value(), near(0.0f32, 1e-5));
        expect_that!(polar.distance().value(), near(1.0f32, 1e-5));
    }

    #[gtest]
    fn cartesian_to_polar_along_z_axis() {
        let cart = CartesianCoordinate { x: 0.0, y: 0.0, z: 5.0 };

        let polar: PolarCoordinate = cart.try_into().unwrap();

        expect_that!(polar.distance().value(), near(5.0f32, 1e-5));
        expect_that!(polar.elevation().value(), near(90.0f32, 1e-5));
        expect_that!(polar.azimuth().value(), near(0.0f32, 1e-5));
    }

    #[gtest]
    fn polar_to_cartesian_zero_distance_returns_origin() {
        let polar = PolarCoordinate::new_from_floats(45.0, 45.0, 0.0).unwrap();

        let cart: CartesianCoordinate = polar.try_into().unwrap();

        expect_that!(cart.x, eq(0.0));
        expect_that!(cart.y, eq(0.0));
        expect_that!(cart.z, eq(0.0));
    }

    #[gtest]
    fn polar_to_cartesian_azimuth_zero_degrees_is_y_axis() {
        let polar = PolarCoordinate::new_from_floats(0.0, 0.0, 1.0).unwrap();

        let cart: CartesianCoordinate = polar.try_into().unwrap();

        expect_that!(cart.x, near(0.0f32, 1e-6));
        expect_that!(cart.y, near(1.0f32, 1e-6));
        expect_that!(cart.z, near(0.0f32, 1e-6));
    }

    #[gtest]
    fn polar_to_cartesian_negative_90_azimuth_is_positive_x_axis() {
        let polar = PolarCoordinate::new_from_floats(-90.0, 0.0, 1.0).unwrap();

        let cart: CartesianCoordinate = polar.try_into().unwrap();

        expect_that!(cart.x, near(1.0f32, 1e-6));
        expect_that!(cart.y, near(0.0f32, 1e-6));
        expect_that!(cart.z, near(0.0f32, 1e-6));
    }

    #[gtest]
    fn polar_to_cartesian_90_elevation_is_positive_z_axis() {
        let polar = PolarCoordinate::new_from_floats(0.0, 90.0, 1.0).unwrap();

        let cart: CartesianCoordinate = polar.try_into().unwrap();

        expect_that!(cart.x, near(0.0f32, 1e-6));
        expect_that!(cart.y, near(0.0f32, 1e-6));
        expect_that!(cart.z, near(1.0f32, 1e-6));
    }
}
