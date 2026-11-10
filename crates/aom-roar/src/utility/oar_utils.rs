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

//! Geometry, Math Utilities, and Coordinate conversions for the OAR library.

use crate::common::definitions::{
    CartesianCoordinate, Degrees, OarError, PolarCoordinate, Radians,
};

/// Converts normalized spherical polar angles (azimuth, elevation with distance = 1.0) to 3D
/// cartesian coordinates.
///
/// # Arguments
/// * `azimuth` - The horizontal angle in degrees.
/// * `elevation` - The vertical angle in degrees.
///
/// # Returns
/// The CartesianCoordinate representing the 3D position.
pub fn normalized_polar_to_cartesian_float32(
    azimuth: Degrees,
    elevation: Degrees,
) -> CartesianCoordinate {
    let azimuth_rad = Radians::from(azimuth).value();
    let elevation_rad = Radians::from(elevation).value();

    CartesianCoordinate::new(
        (-azimuth_rad).sin() * elevation_rad.cos(),
        (-azimuth_rad).cos() * elevation_rad.cos(),
        elevation_rad.sin(),
    )
    .unwrap()
}

/// Structure representing a sector mapping entry.
struct SectorMapping {
    azimuth: f32,
    position: CartesianCoordinate,
}

/// Static sector mapping array of 5 elements matching legacy C mapping.
const SECTOR_MAPPINGS: [SectorMapping; 5] = [
    SectorMapping { azimuth: 0.0, position: CartesianCoordinate::from_const(0.0, 1.0, 0.0) },
    SectorMapping { azimuth: -30.0, position: CartesianCoordinate::from_const(1.0, 1.0, 0.0) },
    SectorMapping { azimuth: -110.0, position: CartesianCoordinate::from_const(1.0, -1.0, 0.0) },
    SectorMapping { azimuth: 110.0, position: CartesianCoordinate::from_const(-1.0, -1.0, 0.0) },
    SectorMapping { azimuth: 30.0, position: CartesianCoordinate::from_const(-1.0, 1.0, 0.0) },
];

fn _azimuth(positions: CartesianCoordinate) -> f32 {
    (-positions.x()).atan2(positions.y()).to_degrees()
}

/// Helper: checks if angle `x` is inside the range [`start`, `end`] given clockwise layout and
/// tolerance.
fn _inside_angle_range(mut x: f32, start: f32, mut end: f32, tol: f32) -> bool {
    // end is clockwise from start; if end is start + 360, this rotation is
    // preserved; this makes sure that a range of (-180, 180) or (0, 360) means
    // any angle, while (-180, -180) or (0, 0) means a single angle.
    while end - 360.0 > start {
        end -= 360.0;
    }
    while end < start {
        end += 360.0;
    }

    // assume that x is clockwise from start - tol; if x is exactly
    // start-tol+360, this is resolved to start-tol, so that the comparison with
    // start-tol is >= rather than >
    let start_tol = start - tol;
    while x - 360.0 >= start_tol {
        x -= 360.0;
    }
    while x < start_tol {
        x += 360.0;
    }

    // x is greater than equal to start-tol, so we only need to compare against the end.
    x <= end + tol
}

/// Helper: finds which sector standard azimuth `az` falls into and returns its left and right
/// indices.
fn _find_cart_sector(az: f32) -> Result<(usize, usize), OarError> {
    (0..5)
        .find(|&i| {
            let j = (i + 1) % 5;
            let start = _azimuth(SECTOR_MAPPINGS[j].position);
            let end = _azimuth(SECTOR_MAPPINGS[i].position);
            _inside_angle_range(az, start, end, 0.0)
        })
        .map(|i| (i, (i + 1) % 5))
        .ok_or(OarError::InvalidParameter)
}

/// Helper: normalizes y relative to x.
fn _relative_angle(x: f32, mut y: f32) -> f32 {
    while y - 360.0 >= x {
        y -= 360.0;
    }
    while y < x {
        y += 360.0;
    }
    y
}

/// Helper: maps a linear position between sectors to an azimuth angle using sin/cos panning laws.
fn _map_linear_to_az(left_az: f32, right_az: f32, x: f32) -> f32 {
    let mid_az = (left_az + right_az) / 2.0;
    let az_range = right_az - mid_az;
    let gain_l_ = (x * (std::f32::consts::PI / 2.0)).cos();
    let gain_r_ = (x * (std::f32::consts::PI / 2.0)).sin();
    let gain_r = gain_r_ / (gain_l_ + gain_r_);
    let rel_az = ((2.0 * (gain_r - 0.5) * az_range.to_radians().tan()).atan()).to_degrees();
    mid_az + rel_az
}

/// Helper: performs matrix inversion on a 2x2 matrix. Returns `Err(OarError::InvalidParameter)`
/// if determinant is less than 1e-10.
fn _linalg_inv_2x2(a: &[f32; 4]) -> Result<[f32; 4], OarError> {
    let det = a[0] * a[3] - a[1] * a[2];
    if det.abs() < 1e-10_f32 {
        return Err(OarError::InvalidParameter);
    }
    let inv_det = 1.0 / det;
    Ok([a[3] * inv_det, -a[1] * inv_det, -a[2] * inv_det, a[0] * inv_det])
}

/// Helper: performs matrix-vector dot product for 2D vectors and matrices.
fn _dot_n_nxn_2(n_vec: &[f32; 2], matrix: &[f32; 4]) -> [f32; 2] {
    let mut out = [0.0; 2];
    out.iter_mut().enumerate().for_each(|(i, val)| {
        *val = (0..2).map(|j| n_vec[j] * matrix[j * 2 + i]).sum();
    });
    out
}

/// Converts 3D cartesian coordinates to spherical polar coordinates using sector-mapping.
///
/// Ported deep sector-mapping coordinate calculation from legacy C.
/// If matrix inversion fails (determinant < 1e-10), falls back gracefully
/// to standard `cartesian_to_polar_float32` conversion without panicking.
///
/// # Arguments
/// * `cartesian` - The input CartesianCoordinate.
///
/// # Returns
/// The PolarCoordinate mapped through sector boundaries.
pub fn cartesian_to_polar_sector_float32(
    cartesian: CartesianCoordinate,
) -> Result<PolarCoordinate, OarError> {
    let pos = CartesianCoordinate::new(cartesian.x(), cartesian.y(), 0.0)?;
    const MIN_FLOAT_THRESHOLD: f32 = 1e-10;

    if cartesian.x().abs() < MIN_FLOAT_THRESHOLD && cartesian.y().abs() < MIN_FLOAT_THRESHOLD {
        if cartesian.z().abs() < MIN_FLOAT_THRESHOLD {
            return PolarCoordinate::new_from_floats(0.0, 0.0, 0.0);
        } else {
            return PolarCoordinate::new_from_floats(
                0.0,
                cartesian.z().signum() * 90.0,
                cartesian.z().abs(),
            );
        }
    }

    let az_val = _azimuth(pos);

    let (left_idx, right_idx) = match _find_cart_sector(az_val) {
        Ok(indices) => indices,
        Err(_) => {
            // If sector mapping fails, fall back to standard conversion.
            return cartesian.try_into();
        }
    };

    let left_az = SECTOR_MAPPINGS[left_idx].azimuth;
    let right_az = SECTOR_MAPPINGS[right_idx].azimuth;
    let left_pos = &SECTOR_MAPPINGS[left_idx].position;
    let right_pos = &SECTOR_MAPPINGS[right_idx].position;

    let _2x2 = [left_pos.x(), left_pos.y(), right_pos.x(), right_pos.y()];
    let _xy = [pos.x(), pos.y()];

    let _2x2_inv = match _linalg_inv_2x2(&_2x2) {
        Ok(inv) => inv,
        Err(_) => {
            // Fallback to standard conversion if matrix is singular
            return cartesian.try_into();
        }
    };

    let g_lr = _dot_n_nxn_2(&_xy, &_2x2_inv);

    let r_xy = g_lr[0] + g_lr[1];
    if r_xy.abs() < 1e-10_f32 {
        return PolarCoordinate::new_from_floats(
            0.0,
            0.0,
            (cartesian.x() * cartesian.x()
                + cartesian.y() * cartesian.y()
                + cartesian.z() * cartesian.z())
            .sqrt(),
        );
    }

    let rel_left_az = _relative_angle(right_az, left_az);
    let mut az = _map_linear_to_az(rel_left_az, right_az, g_lr[1] / r_xy);
    az = _relative_angle(-180.0, az);

    let el_tilde = ((cartesian.z() / r_xy).atan()).to_degrees();

    let el: f32;
    let d: f32;
    if el_tilde.abs() > 45.0 {
        let abs_el = 30.0 + (90.0 - 30.0) * (el_tilde.abs() - 45.0) / (90.0 - 45.0);
        el = el_tilde.signum() * abs_el;
        d = cartesian.z().abs();
    } else {
        el = 30.0 * el_tilde / 45.0;
        d = r_xy;
    }

    PolarCoordinate::new_from_floats(az, el, d)
}

// ===== Tests =====

#[cfg(test)]
mod test {

    use super::*;
    use crate::common::definitions::CartesianCoordinate;
    use googletest::prelude::*;

    #[gtest]
    fn cartesian_to_polar_sector_origin_correct() {
        let origin = CartesianCoordinate::new(0.0, 0.0, 0.0).unwrap();

        let polar_origin = cartesian_to_polar_sector_float32(origin).unwrap();

        expect_that!(polar_origin.azimuth().value(), eq(0.0));
        expect_that!(polar_origin.elevation().value(), eq(0.0));
        expect_that!(polar_origin.distance().value(), eq(0.0));
    }

    #[gtest]
    fn cartesian_to_polar_sector_z_axis_correct() {
        let z_only = CartesianCoordinate::new(0.0, 0.0, 2.5).unwrap();
        let polar_z = cartesian_to_polar_sector_float32(z_only).unwrap();
        expect_that!(polar_z.azimuth().value(), eq(0.0));
        expect_that!(polar_z.elevation().value(), eq(90.0));
        expect_that!(polar_z.distance().value(), eq(2.5));
    }

    #[gtest]
    fn cartesian_to_polar_sector_y_axis_correct() {
        let cart_sec1 = CartesianCoordinate::new(0.0, 1.0, 0.0).unwrap();
        let polar_sec1 = cartesian_to_polar_sector_float32(cart_sec1).unwrap();
        expect_that!(polar_sec1.azimuth().value(), near(0.0f32, 1e-3));
        expect_that!(polar_sec1.elevation().value(), near(0.0f32, 1e-3));
        expect_that!(polar_sec1.distance().value(), near(1.0f32, 1e-3));
    }
}
