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

//! Generic animation types for gains and objects.

use crate::common::coordinates::{CartesianCoordinate, PolarCoordinate};
use crate::common::definitions::{Distance, OarError};
use crate::common::units::{Decibels, Radians};

/// Trait for types that can be animated with Step, Linear, and Bezier interpolation.
pub trait Animatable: Interpolate + Copy {
    /// The type representing the control point relative times (pacing) for Bezier.
    type ControlRelativeTime: Copy;

    /// Performs Bezier quadratic interpolation for this type.
    fn bezier_interpolate(
        start: Self,
        end: Self,
        control: Self,
        control_relative_time: Self::ControlRelativeTime,
        t: f32,
    ) -> Self;
}

/// Generic animation representation, supporting Step, Linear, and Bezier interpolation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Animated<T: Animatable> {
    /// Constant value throughout the animation.
    Step { value: T },
    /// Linear interpolation between start and end.
    Linear { start: T, end: T },
    /// Bezier interpolation between start and end with control point and relative time.
    Bezier { start: T, end: T, control: T, control_relative_time: T::ControlRelativeTime },
}

// Type aliases for convenience.
pub type AnimatedFloat32 = Animated<f32>;
pub type AnimatedDecibels = Animated<Decibels>;
pub type AnimatedPolar = Animated<PolarCoordinate>;
pub type AnimatedCartesian = Animated<CartesianCoordinate>;

/// Trait for types that can be linearly interpolated.
pub trait Interpolate {
    /// Linearly interpolate between `self` and `other` by a factor of `t` [0.0, 1.0].
    fn lerp(self, other: Self, t: f32) -> Self;
}

impl<T> Animated<T>
where
    T: Animatable,
{
    /// Samples the animated value at a normalized time `t` [0.0, 1.0].
    pub fn sample(&self, t: f32) -> T {
        let t = t.clamp(0.0, 1.0);

        match *self {
            Animated::Step { value } => value,
            Animated::Linear { start, end } => start.lerp(end, t),
            Animated::Bezier { start, end, control, control_relative_time } => {
                T::bezier_interpolate(start, end, control, control_relative_time, t)
            }
        }
    }
}

// Implementations of Interpolate for core types.

impl Interpolate for f32 {
    fn lerp(self, other: Self, t: f32) -> Self {
        self + t * (other - self)
    }
}

impl Interpolate for Decibels {
    fn lerp(self, other: Self, t: f32) -> Self {
        Decibels::new(self.value() + t * (other.value() - self.value())).unwrap_or(self)
    }
}

impl Interpolate for CartesianCoordinate {
    fn lerp(self, other: Self, t: f32) -> Self {
        CartesianCoordinate::new(
            self.x() + t * (other.x() - self.x()),
            self.y() + t * (other.y() - self.y()),
            self.z() + t * (other.z() - self.z()),
        )
        .unwrap_or(self)
    }
}

impl Interpolate for PolarCoordinate {
    fn lerp(self, other: Self, t: f32) -> Self {
        slerp_polar(self, other, t).unwrap_or(self)
    }
}

// Implementations of Animatable for core types.

impl Animatable for f32 {
    type ControlRelativeTime = f32;
    fn bezier_interpolate(
        start: Self,
        end: Self,
        control: Self,
        control_relative_time: Self::ControlRelativeTime,
        t: f32,
    ) -> Self {
        let factor = quadratic_bezier_time_to_parameter(control_relative_time, t);
        let q0 = start + factor * (control - start);
        let q1 = control + factor * (end - control);
        q0 + factor * (q1 - q0)
    }
}

impl Animatable for Decibels {
    type ControlRelativeTime = f32;
    fn bezier_interpolate(
        start: Self,
        end: Self,
        control: Self,
        control_relative_time: Self::ControlRelativeTime,
        t: f32,
    ) -> Self {
        let factor = quadratic_bezier_time_to_parameter(control_relative_time, t);
        let q0 = start.lerp(control, factor);
        let q1 = control.lerp(end, factor);
        q0.lerp(q1, factor)
    }
}

/// Control relative time pacing parameters for Cartesian coordinates Bezier animation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CartesianControlRelativeTime {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Animatable for CartesianCoordinate {
    type ControlRelativeTime = CartesianControlRelativeTime;
    fn bezier_interpolate(
        start: Self,
        end: Self,
        control: Self,
        control_relative_time: Self::ControlRelativeTime,
        t: f32,
    ) -> Self {
        let factor_x = quadratic_bezier_time_to_parameter(control_relative_time.x, t);
        let factor_y = quadratic_bezier_time_to_parameter(control_relative_time.y, t);
        let factor_z = quadratic_bezier_time_to_parameter(control_relative_time.z, t);

        let lerp_component = |start_val: f32, end_val: f32, ctrl_val: f32, factor: f32| {
            let q0 = start_val + factor * (ctrl_val - start_val);
            let q1 = ctrl_val + factor * (end_val - ctrl_val);
            q0 + factor * (q1 - q0)
        };

        CartesianCoordinate::new(
            lerp_component(start.x(), end.x(), control.x(), factor_x),
            lerp_component(start.y(), end.y(), control.y(), factor_y),
            lerp_component(start.z(), end.z(), control.z(), factor_z),
        )
        .unwrap_or(start)
    }
}

/// Control relative time pacing parameters for Polar coordinates Bezier animation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PolarControlRelativeTime {
    pub azimuth: f32,
    pub elevation: f32,
    pub distance: f32,
}

impl Animatable for PolarCoordinate {
    type ControlRelativeTime = PolarControlRelativeTime;
    fn bezier_interpolate(
        start: Self,
        _end: Self,
        _control: Self,
        _control_relative_time: Self::ControlRelativeTime,
        _t: f32,
    ) -> Self {
        // Bezier is not supported for polar coordinates, return start.
        start
    }
}

/// Calculates the Bezier parameter `s` corresponding to a given normalized time `t`.
///
/// This solves the quadratic equation mapping the Bezier parameter to time, assuming
/// a 1D Bezier curve from 0.0 to 1.0 with a control point at `control_relative_time`.
///
/// # Parameters
/// * `control_relative_time`: The position of the control point relative to start (0.0) and end
///   (1.0). Determines the pacing of the animation.
/// * `t`: The target normalized time in [0.0, 1.0].
fn quadratic_bezier_time_to_parameter(control_relative_time: f32, t: f32) -> f32 {
    let alpha = 1.0 - 2.0 * control_relative_time;
    let beta = 2.0 * control_relative_time;
    let gamma = -t;

    // If alpha is near zero, the quadratic term is negligible, and we solve
    // the remaining linear equation: beta * s + gamma = 0.
    if alpha.abs() < 1e-6 {
        if beta.abs() < 1e-6 {
            return 0.0;
        }
        return -gamma / beta;
    }

    let discriminant = beta * beta - 4.0 * alpha * gamma;
    if discriminant < 0.0 {
        return 0.0;
    }

    (-beta + discriminant.sqrt()) / (2.0 * alpha)
}

/// Calculates the angle in radians between two Cartesian coordinates.
///
/// # Returns
/// The angle in radians, or 0.0 if either coordinate is at or very close to the origin.
fn angle_between_cartesian(
    p1: CartesianCoordinate,
    p2: CartesianCoordinate,
) -> Result<Radians, OarError> {
    let dot_product = p1.x() * p2.x() + p1.y() * p2.y() + p1.z() * p2.z();
    let mag1 = (p1.x() * p1.x() + p1.y() * p1.y() + p1.z() * p1.z()).sqrt();
    let mag2 = (p2.x() * p2.x() + p2.y() * p2.y() + p2.z() * p2.z()).sqrt();
    // Avoid division by zero if either vector is near-zero.
    if mag1 < 1e-10_f32 || mag2 < 1e-10_f32 {
        return Ok(Radians::ZERO);
    }
    // Clamp to avoid NaN from acos due to floating point precision errors.
    let cos_angle = (dot_product / (mag1 * mag2)).clamp(-1.0, 1.0);
    Radians::new(cos_angle.acos())
}

/// Performs spherical linear interpolation (Slerp) on a single coordinate component.
///
/// # Parameters
/// * `rad`: The angle in radians between the two vectors being interpolated.
/// * `c`: The interpolation factor in [0.0, 1.0].
fn slerp_component(start: f32, end: f32, rad: Radians, c: f32) -> f32 {
    if !c.is_finite() || !start.is_finite() || !end.is_finite() {
        return 0.0;
    }
    let rad_val = rad.value();
    // Fall back to linear interpolation if the angle is too small, to avoid
    // division by zero (sin(0) = 0).
    let sin_rad = rad_val.sin();
    if sin_rad.abs() < 1e-6 {
        return start + c * (end - start);
    }
    (((1.0 - c) * rad_val).sin() * start + (c * rad_val).sin() * end) / sin_rad
}

/// Performs spherical linear interpolation (Slerp) on polar coordinates.
///
/// The direction is interpolated spherically (Slerp) while the distance is
/// interpolated linearly.
///
/// # Parameters
/// * `c`: The interpolation factor in [0.0, 1.0].
fn slerp_polar(
    start: PolarCoordinate,
    end: PolarCoordinate,
    c: f32,
) -> Result<PolarCoordinate, OarError> {
    if c.is_nan() || c.is_infinite() {
        return Err(OarError::InvalidParameter);
    }
    let v0 = crate::utility::oar_utils::normalized_polar_to_cartesian_float32(
        start.azimuth(),
        start.elevation(),
    );
    let v2 = crate::utility::oar_utils::normalized_polar_to_cartesian_float32(
        end.azimuth(),
        end.elevation(),
    );
    let omega = angle_between_cartesian(v0, v2)?;
    let v_x = slerp_component(v0.x(), v2.x(), omega, c);
    let v_y = slerp_component(v0.y(), v2.y(), omega, c);
    let v_z = slerp_component(v0.z(), v2.z(), omega, c);
    let v = CartesianCoordinate::new(v_x, v_y, v_z)?;
    let v_polar: PolarCoordinate = v.try_into()?;
    let start_d = start.distance().value();
    let end_d = end.distance().value();
    let interp_distance = start_d + c * (end_d - start_d);

    PolarCoordinate::new(v_polar.azimuth(), v_polar.elevation(), Distance::new(interp_distance)?)
}

// ===== Tests =====

#[cfg(test)]
mod test {

    use super::*;
    use googletest::prelude::*;

    #[gtest]
    fn test_animated_step() {
        let anim = Animated::Step { value: 5.0f32 };
        expect_eq!(anim.sample(0.0), 5.0);
        expect_eq!(anim.sample(0.5), 5.0);
        expect_eq!(anim.sample(1.0), 5.0);
    }

    #[gtest]
    fn test_animated_linear() {
        let anim = Animated::Linear { start: 0.0f32, end: 10.0f32 };
        expect_eq!(anim.sample(0.0), 0.0);
        expect_eq!(anim.sample(0.5), 5.0);
        expect_eq!(anim.sample(1.0), 10.0);
        // Clamp checks
        expect_eq!(anim.sample(-0.5), 0.0);
        expect_eq!(anim.sample(1.5), 10.0);
    }

    #[gtest]
    fn test_animated_bezier() {
        // Linear Bezier mapping (control relative time = 0.5, control = midpoint)
        // should behave like linear.
        let anim = Animated::Bezier {
            start: 0.0f32,
            end: 10.0f32,
            control: 5.0f32,
            control_relative_time: 0.5,
        };
        expect_eq!(anim.sample(0.0), 0.0);
        expect_eq!(anim.sample(0.5), 5.0);
        expect_eq!(anim.sample(1.0), 10.0);
    }

    #[gtest]
    fn test_quadratic_bezier_time_to_parameter() {
        expect_that!(quadratic_bezier_time_to_parameter(0.5, 0.5), near(0.5, 1e-6));
        expect_that!(quadratic_bezier_time_to_parameter(0.5, 0.0), near(0.0, 1e-6));
        expect_that!(quadratic_bezier_time_to_parameter(0.5, 1.0), near(1.0, 1e-6));

        // Edge case: control_relative_time = 0.5 (alpha is 0)
        expect_that!(quadratic_bezier_time_to_parameter(0.5, 0.25), near(0.25, 1e-6));
    }

    #[gtest]
    fn test_angle_between_cartesian() {
        let p1 = CartesianCoordinate::new(1.0, 2.0, 3.0).unwrap();
        let identical_angle = angle_between_cartesian(p1, p1).unwrap();
        expect_that!(identical_angle.value(), lt(1e-3));

        let p_x = CartesianCoordinate::new(1.0, 0.0, 0.0).unwrap();
        let p_y = CartesianCoordinate::new(0.0, 1.0, 0.0).unwrap();
        let angle = angle_between_cartesian(p_x, p_y).unwrap();
        expect_that!(angle.value(), near(std::f32::consts::PI / 2.0, 1e-6));

        let p_zero = CartesianCoordinate::new(1e-11, 1e-11, 1e-11).unwrap();
        expect_that!(angle_between_cartesian(p1, p_zero).unwrap().value(), eq(0.0));
    }

    #[gtest]
    fn test_slerp_component() {
        let start = 1.0;
        let end = 2.0;
        let rad = Radians::new(1.0).unwrap();

        expect_that!(slerp_component(start, end, rad, 0.0), near(start, 1e-6));
        expect_that!(slerp_component(start, end, rad, 1.0), near(end, 1e-6));

        let mid = slerp_component(start, end, rad, 0.5);
        expect_that!(mid, near(1.709235f32, 1e-5));

        let mid_small = slerp_component(start, end, Radians::new(1e-7).unwrap(), 0.5);
        expect_that!(mid_small, near(1.5f32, 1e-6));

        expect_that!(slerp_component(start, end, rad, f32::NAN), eq(0.0));
        expect_that!(slerp_component(start, end, rad, f32::INFINITY), eq(0.0));
    }

    #[gtest]
    fn test_slerp_polar() {
        let start_polar = PolarCoordinate::new_from_floats(0.0, 0.0, 1.0).unwrap();
        let end_polar = PolarCoordinate::new_from_floats(90.0, 0.0, 1.0).unwrap();

        let result_0 = slerp_polar(start_polar, end_polar, 0.0).unwrap();
        expect_that!(result_0.azimuth().value(), near(0.0f32, 1e-5));
        expect_that!(result_0.elevation().value(), near(0.0f32, 1e-5));
        expect_that!(result_0.distance().value(), near(1.0f32, 1e-5));

        let result_1 = slerp_polar(start_polar, end_polar, 1.0).unwrap();
        expect_that!(result_1.azimuth().value(), near(90.0f32, 1e-5));
        expect_that!(result_1.elevation().value(), near(0.0f32, 1e-5));
        expect_that!(result_1.distance().value(), near(1.0f32, 1e-5));

        let result_mid = slerp_polar(start_polar, end_polar, 0.5).unwrap();
        expect_that!(result_mid.azimuth().value(), near(45.0f32, 1e-5));
        expect_that!(result_mid.elevation().value(), near(0.0f32, 1e-5));
        expect_that!(result_mid.distance().value(), near(1.0f32, 1e-5));
    }

    #[gtest]
    fn test_animated_cartesian_bezier_mixed_pacing() {
        let anim = AnimatedCartesian::Bezier {
            start: CartesianCoordinate::new(0.0, 0.0, 0.0).unwrap(),
            end: CartesianCoordinate::new(10.0, 10.0, 10.0).unwrap(),
            control: CartesianCoordinate::new(5.0, 5.0, 5.0).unwrap(),
            control_relative_time: CartesianControlRelativeTime { x: 0.5, y: 0.0, z: 1.0 },
        };

        let sample = anim.sample(0.5);
        expect_that!(sample.x(), near(5.0f32, 1e-5));
        expect_that!(sample.y(), near(7.071068f32, 1e-5));
        expect_that!(sample.z(), near(2.928932f32, 1e-5));
    }

    #[gtest]
    fn test_animated_decibels_linear_interpolates_db_values() {
        let anim =
            AnimatedDecibels::Linear { start: Decibels::ZERO, end: Decibels::new(-12.0).unwrap() };

        let sample_start = anim.sample(0.0);
        let sample_mid = anim.sample(0.5);
        let sample_end = anim.sample(1.0);

        expect_that!(sample_start.value(), near(0.0f32, 1e-6));
        expect_that!(sample_mid.value(), near(-6.0f32, 1e-6));
        expect_that!(sample_end.value(), near(-12.0f32, 1e-6));
    }

    #[gtest]
    fn test_animated_decibels_bezier_interpolates_db_values() {
        let db_neg20 = Decibels::new(-20.0).unwrap();
        let anim = AnimatedDecibels::Bezier {
            start: Decibels::ZERO,
            end: db_neg20,
            control: db_neg20,
            control_relative_time: 0.5,
        };

        let sample_mid = anim.sample(0.5);

        expect_that!(sample_mid.value(), near(-15.0f32, 1e-6));
    }
}
