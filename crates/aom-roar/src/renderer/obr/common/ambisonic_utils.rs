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

//! Ambisonic rendering utilities.
//!
//! Provides conversion functions between channels, degree, order, and normalization
//! constants for spherical harmonics.

use super::misc_math::factorial;

/// Computes the Ambisonic Channel Number (ACN) sequence index from a degree and order.
pub fn acn_sequence(degree: i32, order: i32) -> i32 {
    assert!(degree >= 0);
    assert!(-degree <= order && order <= degree);
    degree * degree + degree + order
}

/// Computes the normalization factor for Schmidt semi-normalized harmonics (`sn3d`).
pub fn sn3d_normalization(degree: i32, order: i32) -> f32 {
    assert!(degree >= 0);
    assert!(-degree <= order && order <= degree);
    let factor = if order == 0 { 1.0_f32 } else { 0.0_f32 };
    ((2.0 - factor) * factorial(degree - order.abs()) / factorial(degree + order.abs())).sqrt()
}

/// Returns the number of spherical harmonics components for a periphonic Ambisonic
/// sound field of `ambisonic_order`.
pub fn get_num_periphonic_components(ambisonic_order: i32) -> usize {
    ((ambisonic_order + 1) * (ambisonic_order + 1)) as usize
}

/// Returns the number of periphonic spherical harmonics (SH) components for a
/// particular Ambisonic order.
pub fn get_num_nth_order_periphonic_components(ambisonic_order: i32) -> usize {
    if ambisonic_order == 0 {
        1
    } else {
        get_num_periphonic_components(ambisonic_order)
            - get_num_periphonic_components(ambisonic_order - 1)
    }
}

/// Returns whether the given channel count corresponds to a valid Ambisonic order configuration.
pub fn is_valid_ambisonic_order(num_channels: usize) -> bool {
    if num_channels == 0 {
        return false;
    }
    let sqrt_num_channels = (num_channels as f64).sqrt() as usize;
    num_channels == sqrt_num_channels * sqrt_num_channels
}

// ===== Tests =====

#[cfg(test)]
mod test {

    use super::*;
    use googletest::prelude::*;

    #[gtest]
    fn test_acn_sequence() {
        expect_eq!(acn_sequence(0, 0), 0); // W
        expect_eq!(acn_sequence(1, -1), 1); // Y
        expect_eq!(acn_sequence(1, 0), 2); // Z
        expect_eq!(acn_sequence(1, 1), 3); // X
        expect_eq!(acn_sequence(2, -2), 4); // V
        expect_eq!(acn_sequence(2, 2), 8); // R
    }

    #[gtest]
    fn test_sn3d_normalization() {
        expect_near!(sn3d_normalization(0, 0), 1.0, 1e-6);
        expect_near!(sn3d_normalization(1, 0), 1.0, 1e-6);
        expect_near!(sn3d_normalization(1, 1), 1.0, 1e-6);
        expect_near!(sn3d_normalization(2, 1), (1.0 / 3.0_f32).sqrt(), 1e-6);
    }

    #[gtest]
    fn test_get_num_periphonic_components() {
        expect_eq!(get_num_periphonic_components(0), 1);
        expect_eq!(get_num_periphonic_components(1), 4);
        expect_eq!(get_num_periphonic_components(2), 9);
        expect_eq!(get_num_periphonic_components(3), 16);
    }

    #[gtest]
    fn test_get_num_nth_order_periphonic_components() {
        expect_eq!(get_num_nth_order_periphonic_components(0), 1);
        expect_eq!(get_num_nth_order_periphonic_components(1), 3);
        expect_eq!(get_num_nth_order_periphonic_components(2), 5);
        expect_eq!(get_num_nth_order_periphonic_components(3), 7);
    }

    #[gtest]
    fn test_is_valid_ambisonic_order() {
        expect_true!(is_valid_ambisonic_order(1));
        expect_true!(is_valid_ambisonic_order(4));
        expect_true!(is_valid_ambisonic_order(9));
        expect_true!(is_valid_ambisonic_order(16));
        expect_false!(is_valid_ambisonic_order(0));
        expect_false!(is_valid_ambisonic_order(2));
        expect_false!(is_valid_ambisonic_order(5));
    }
}
