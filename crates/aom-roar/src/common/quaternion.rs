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
use std::ops::Mul;

/// Represents a 3D spatial rotation quaternion, equivalent of C API `roar_quaternion_t`.
///
/// The quaternion head tracking input uses the ADM object coordinate system to orient the
/// listener's head:
/// - **X-axis**: Left (negative) / Right (positive)
/// - **Y-axis**: Back (negative) / Front (positive)
/// - **Z-axis**: Down (negative) / Up (positive)
///
/// Expected to be a unit quaternion (i.e. `w^2 + x^2 + y^2 + z^2` is close to 1.0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quaternion {
    w: f32,
    x: f32,
    y: f32,
    z: f32,
}

/// Represents a 3x3 rotation matrix.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RotationMatrix(pub [[f32; 3]; 3]);

impl std::ops::Index<usize> for RotationMatrix {
    type Output = [f32; 3];
    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl RotationMatrix {
    /// Returns an iterator over the rows of the rotation matrix.
    pub fn iter(&self) -> std::slice::Iter<'_, [f32; 3]> {
        self.0.iter()
    }
}

impl Default for Quaternion {
    fn default() -> Self {
        Self::identity()
    }
}

impl Quaternion {
    /// Creates a new Quaternion. Returns error if any component is not finite.
    pub fn new(w: f32, x: f32, y: f32, z: f32) -> Result<Self, OarError> {
        if w.is_finite() && x.is_finite() && y.is_finite() && z.is_finite() {
            Ok(Quaternion { w, x, y, z })
        } else {
            Err(OarError::InvalidParameter)
        }
    }

    /// Creates a new unit identity quaternion.
    pub fn identity() -> Self {
        Self { w: 1.0, x: 0.0, y: 0.0, z: 0.0 }
    }

    /// Returns the w component.
    pub const fn w(&self) -> f32 {
        self.w
    }
    /// Returns the x component.
    pub const fn x(&self) -> f32 {
        self.x
    }
    /// Returns the y component.
    pub const fn y(&self) -> f32 {
        self.y
    }
    /// Returns the z component.
    pub const fn z(&self) -> f32 {
        self.z
    }

    /// Returns the conjugate of the quaternion (`w, -x, -y, -z`).
    pub const fn conjugate(&self) -> Self {
        Self { w: self.w, x: -self.x, y: -self.y, z: -self.z }
    }

    /// Returns the conjugate/inverse of the quaternion assuming unit norm.
    pub fn inverse(&self) -> Self {
        let sq_norm = self.w * self.w + self.x * self.x + self.y * self.y + self.z * self.z;
        if sq_norm > 0.0 {
            let inv_sq = 1.0 / sq_norm;
            Self {
                w: self.w * inv_sq,
                x: -self.x * inv_sq,
                y: -self.y * inv_sq,
                z: -self.z * inv_sq,
            }
        } else {
            Self::identity()
        }
    }

    /// Returns the shortest arc between two `Quaternion`s in radians.
    pub fn angular_difference_rad(&self, other: &Self) -> f32 {
        let difference = self.inverse() * (*other);
        let v_norm = (difference.x * difference.x
            + difference.y * difference.y
            + difference.z * difference.z)
            .sqrt();
        2.0 * v_norm.atan2(difference.w.abs())
    }

    /// Converts the quaternion to a 3x3 rotation matrix `RotationMatrix`.
    pub fn to_rotation_matrix(&self) -> RotationMatrix {
        let (w, x, y, z) = (self.w, self.x, self.y, self.z);
        RotationMatrix([
            [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y - z * w), 2.0 * (x * z + y * w)],
            [2.0 * (x * y + z * w), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z - x * w)],
            [2.0 * (x * z - y * w), 2.0 * (y * z + x * w), 1.0 - 2.0 * (x * x + y * y)],
        ])
    }

    /// Spherical linear interpolation between `self` and `other` by parameter `t` in `[0, 1]`.
    pub fn slerp(&self, t: f32, other: &Self) -> Self {
        let mut dot = self.w * other.w + self.x * other.x + self.y * other.y + self.z * other.z;
        let mut q2 = *other;
        if dot < 0.0 {
            dot = -dot;
            q2 = Self { w: -q2.w, x: -q2.x, y: -q2.y, z: -q2.z };
        }
        if dot > 0.9995 {
            // Linear approximation
            let q = Self {
                w: self.w + t * (q2.w - self.w),
                x: self.x + t * (q2.x - self.x),
                y: self.y + t * (q2.y - self.y),
                z: self.z + t * (q2.z - self.z),
            };
            let norm = (q.w * q.w + q.x * q.x + q.y * q.y + q.z * q.z).sqrt();
            if norm > 0.0 {
                return Self { w: q.w / norm, x: q.x / norm, y: q.y / norm, z: q.z / norm };
            } else {
                return *self;
            }
        }
        let theta = dot.acos();
        let sin_theta = theta.sin();
        let w1 = ((1.0 - t) * theta).sin() / sin_theta;
        let w2 = (t * theta).sin() / sin_theta;
        Self {
            w: w1 * self.w + w2 * q2.w,
            x: w1 * self.x + w2 * q2.x,
            y: w1 * self.y + w2 * q2.y,
            z: w1 * self.z + w2 * q2.z,
        }
    }
}

impl Mul for Quaternion {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        Self {
            w: self.w * rhs.w - self.x * rhs.x - self.y * rhs.y - self.z * rhs.z,
            x: self.w * rhs.x + self.x * rhs.w + self.y * rhs.z - self.z * rhs.y,
            y: self.w * rhs.y - self.x * rhs.z + self.y * rhs.w + self.z * rhs.x,
            z: self.w * rhs.z + self.x * rhs.y - self.y * rhs.x + self.z * rhs.w,
        }
    }
}

// ===== Tests =====

#[cfg(test)]
mod test {

    use super::*;
    use googletest::prelude::*;
    use std::f32::consts::PI;

    #[gtest]
    fn test_quaternion_new_success() {
        let q = Quaternion::new(1.0, 1.5, -2.0, -3.25);
        expect_ok!(&q);
        let q = q.unwrap();
        expect_that!(q.w, eq(1.0));
        expect_that!(q.x, eq(1.5));
        expect_that!(q.y, eq(-2.0));
        expect_that!(q.z, eq(-3.25));
    }

    #[gtest]
    fn test_quaternion_new_non_finite() {
        let q = Quaternion::new(f32::NAN, 0.0, 0.0, 0.0);
        expect_true!(q.is_err());
    }

    #[gtest]
    fn test_quaternion_identity() {
        let q = Quaternion::identity();
        expect_that!(q.w, eq(1.0));
        expect_that!(q.x, eq(0.0));
        expect_that!(q.y, eq(0.0));
        expect_that!(q.z, eq(0.0));
    }

    #[gtest]
    fn test_quaternion_inverse() {
        // Rotates 90 deg around Z: w = cos(45) = sqrt(2)/2, z = sin(45) = sqrt(2)/2
        let val = (PI / 4.0).cos();
        let q = Quaternion::new(val, 0.0, 0.0, val).unwrap();
        let inv = q.inverse();
        expect_near!(inv.w, val, 1e-6);
        expect_near!(inv.x, 0.0, 1e-6);
        expect_near!(inv.y, 0.0, 1e-6);
        expect_near!(inv.z, -val, 1e-6);

        // Inverse of identity is identity
        let q_id = Quaternion::identity();
        expect_that!(q_id.inverse(), eq(q_id));
    }

    #[gtest]
    fn test_quaternion_multiplication() {
        // Multiply two 90 degree Z axis rotations to get 180 degree Z axis rotation.
        let val = (PI / 4.0).cos();
        let q1 = Quaternion::new(val, 0.0, 0.0, val).unwrap();
        let q2 = Quaternion::new(val, 0.0, 0.0, val).unwrap();
        let product = q1 * q2;
        // 180 deg around Z: w = cos(90) = 0, z = sin(90) = 1
        expect_near!(product.w, 0.0, 1e-6);
        expect_near!(product.x, 0.0, 1e-6);
        expect_near!(product.y, 0.0, 1e-6);
        expect_near!(product.z, 1.0, 1e-6);
    }

    #[gtest]
    fn test_quaternion_angular_difference() {
        let q1 = Quaternion::identity();
        let val = (PI / 4.0).cos();
        let q2 = Quaternion::new(val, 0.0, 0.0, val).unwrap();
        // Difference should be 90 deg (PI/2 rad)
        expect_near!(q1.angular_difference_rad(&q2), PI / 2.0, 1e-5);
    }

    #[gtest]
    fn test_quaternion_angular_difference_ensures_smallest_angle() {
        let q1 = Quaternion::identity();
        // A valid unit quaternion with a negative 'w' component.
        // Length: sqrt((-0.5)^2 + 0.5^2 + 0.5^2 + 0.5^2) = sqrt(1.0) = 1.0
        let q2 = Quaternion { w: -0.5, x: 0.5, y: 0.5, z: 0.5 };

        let angle = q1.angular_difference_rad(&q2);

        // The shortest arc between any two 3D rotations can never exceed PI (180 degrees).
        assert!(angle <= std::f32::consts::PI);
    }

    #[gtest]
    fn test_quaternion_slerp() {
        let q1 = Quaternion::identity();
        let val = (PI / 4.0).cos();
        let q2 = Quaternion::new(val, 0.0, 0.0, val).unwrap();
        // Slerp halfway should be 45 deg Z rotation:
        // w = cos(22.5) = 0.9238795, z = sin(22.5) = 0.3826834
        let halfway = q1.slerp(0.5, &q2);
        expect_near!(halfway.w, 0.9238795, 1e-6);
        expect_near!(halfway.x, 0.0, 1e-6);
        expect_near!(halfway.y, 0.0, 1e-6);
        expect_near!(halfway.z, 0.3826834, 1e-6);
    }

    #[gtest]
    fn test_to_rotation_matrix_and_indexing() {
        // 90 deg around Z axis
        let val = (PI / 4.0).cos();
        let q = Quaternion::new(val, 0.0, 0.0, val).unwrap();
        let r = q.to_rotation_matrix();
        // Rotation matrix should be:
        // [ 0 -1  0]
        // [ 1  0  0]
        // [ 0  0  1]
        expect_near!(r[0][0], 0.0, 1e-6);
        expect_near!(r[0][1], -1.0, 1e-6);
        expect_near!(r[0][2], 0.0, 1e-6);

        expect_near!(r[1][0], 1.0, 1e-6);
        expect_near!(r[1][1], 0.0, 1e-6);
        expect_near!(r[1][2], 0.0, 1e-6);

        expect_near!(r[2][0], 0.0, 1e-6);
        expect_near!(r[2][1], 0.0, 1e-6);
        expect_near!(r[2][2], 1.0, 1e-6);
    }

    #[gtest]
    fn test_rotation_matrix_iter() {
        let q = Quaternion::identity();
        let r = q.to_rotation_matrix();
        let mut count = 0;
        for (i, row) in r.iter().enumerate() {
            count += 1;
            expect_that!(row[i], eq(1.0)); // Identity matrix diagonal should be 1.0
        }
        expect_that!(count, eq(3));
    }
}
