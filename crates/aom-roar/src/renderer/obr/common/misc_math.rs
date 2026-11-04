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

//! Mathematical utilities and coordinate representations.
//!
//! Provides functions for basic geometry and arithmetic (factorial, double factorial,
//! integer power, fast reciprocal square root, greatest common divisor) and coordinates
//! (world position, world rotation, azimuth/elevation transformations).

/// Finds the greatest common divisor between two integer values.
///
/// Uses the Euclidean algorithm. Always returns a positive integer.
pub fn find_gcd(mut a: i32, mut b: i32) -> i32 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        let temp_value = b;
        b = a % b;
        a = temp_value;
    }
    a
}

/// Finds the next power of two greater than or equal to the input value.
///
/// Works with values representable by unsigned 32-bit integers.
pub fn next_pow_two(mut input: usize) -> usize {
    assert!((input as u64) < u32::MAX as u64);
    if input == 0 {
        return 1;
    }
    input -= 1;
    let mut number = input as u32;
    number |= number >> 1;
    number |= number >> 2;
    number |= number >> 4;
    number |= number >> 8;
    number |= number >> 16;
    number += 1;
    number as usize
}

/// Returns the factorial (!) of x.
///
/// Returns 0.0 if x < 0.
pub fn factorial(mut x: i32) -> f32 {
    if x < 0 {
        return 0.0;
    }
    let mut result = 1.0_f32;
    while x > 0 {
        result *= x as f32;
        x -= 1;
    }
    result
}

/// Returns the double factorial (!!) of x.
///
/// For odd x:  1 * 3 * 5 * ... * (x - 2) * x.
/// For even x: 2 * 4 * 6 * ... * (x - 2) * x.
/// Returns 0.0 if x < 0.
pub fn double_factorial(mut x: i32) -> f32 {
    if x < 0 {
        return 0.0;
    }
    let mut result = 1.0_f32;
    while x > 0 {
        result *= x as f32;
        x -= 2;
    }
    result
}

// ===== Tests =====

#[cfg(test)]
mod test {

    use super::*;
    use googletest::prelude::*;

    #[gtest]
    fn test_find_gcd() {
        expect_eq!(find_gcd(12, 18), 6);
        expect_eq!(find_gcd(5, 7), 1);
        expect_eq!(find_gcd(-12, 18), 6);
        expect_eq!(find_gcd(12, 0), 12);
        expect_eq!(find_gcd(0, 0), 0);
    }

    #[gtest]
    fn test_next_pow_two() {
        expect_eq!(next_pow_two(0), 1);
        expect_eq!(next_pow_two(1), 1);
        expect_eq!(next_pow_two(2), 2);
        expect_eq!(next_pow_two(3), 4);
        expect_eq!(next_pow_two(4), 4);
        expect_eq!(next_pow_two(5), 8);
        expect_eq!(next_pow_two(127), 128);
        expect_eq!(next_pow_two(128), 128);
    }

    #[gtest]
    fn test_factorial() {
        expect_eq!(factorial(0), 1.0);
        expect_eq!(factorial(1), 1.0);
        expect_eq!(factorial(2), 2.0);
        expect_eq!(factorial(3), 6.0);
        expect_eq!(factorial(5), 120.0);
        expect_eq!(factorial(-1), 0.0);
    }

    #[gtest]
    fn test_double_factorial() {
        expect_eq!(double_factorial(0), 1.0);
        expect_eq!(double_factorial(1), 1.0);
        expect_eq!(double_factorial(2), 2.0);
        expect_eq!(double_factorial(3), 3.0);
        expect_eq!(double_factorial(4), 8.0);
        expect_eq!(double_factorial(5), 15.0);
        expect_eq!(double_factorial(-1), 0.0);
    }
}
