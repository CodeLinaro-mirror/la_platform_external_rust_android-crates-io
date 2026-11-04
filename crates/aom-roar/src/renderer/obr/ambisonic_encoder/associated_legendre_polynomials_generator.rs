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

//! Associated Legendre Polynomials (ALP) generator.
//!
//! Provides the generator `AssociatedLegendrePolynomialsGenerator` to compute
//! Legendre polynomial recurrence relations used for spherical harmonic evaluations.

use crate::renderer::obr::common::misc_math::{double_factorial, factorial};

/// Recurrence-based Associated Legendre Polynomials (ALP) generator.
///
/// Computes polynomial coefficients for real spherical harmonic calculations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssociatedLegendrePolynomialsGenerator {
    max_degree: i32,
    condon_shortley_phase: bool,
    compute_negative_order: bool,
}

impl AssociatedLegendrePolynomialsGenerator {
    /// Constructs a new `AssociatedLegendrePolynomialsGenerator` instance.
    pub fn new(max_degree: i32, condon_shortley_phase: bool, compute_negative_order: bool) -> Self {
        assert!(max_degree >= 0);
        Self { max_degree, condon_shortley_phase, compute_negative_order }
    }

    /// Returns the number of polynomial values this generator computes.
    pub fn get_num_values(&self) -> usize {
        if self.compute_negative_order {
            ((self.max_degree + 1) * (self.max_degree + 1)) as usize
        } else {
            (((self.max_degree + 1) * (self.max_degree + 2)) / 2) as usize
        }
    }

    /// Checks index validity for `(degree, order)`.
    fn check_index_validity(&self, degree: i32, order: i32) {
        assert!(degree >= 0 && degree <= self.max_degree);
        if self.compute_negative_order {
            assert!(-degree <= order && order <= degree);
        } else {
            assert!(order >= 0 && order <= degree);
        }
    }

    /// Computes the 1D index into the output slice corresponding to the `(degree, order)` pair.
    pub fn get_index(&self, degree: i32, order: i32) -> usize {
        self.check_index_validity(degree, order);
        let result = if self.compute_negative_order {
            degree * (degree + 1) + order
        } else {
            (degree * (degree + 1)) / 2 + order
        };
        assert!(result >= 0);
        let res_usize = result as usize;
        assert!(res_usize < self.get_num_values());
        res_usize
    }

    /// Computes the ALP for `(degree, order)` at `x` assuming prerequisites in `values`.
    fn compute_value(&self, degree: i32, order: i32, x: f32, values: &[f32]) -> f32 {
        self.check_index_validity(degree, order);
        if degree == 0 && order == 0 {
            1.0
        } else if degree == 1 && order == 0 {
            x
        } else if degree == order {
            (-1.0_f32).powi(degree)
                * double_factorial(2 * degree - 1)
                * (1.0 - x * x).powf(0.5 * degree as f32)
        } else if order == degree - 1 {
            x * (2 * degree - 1) as f32 * values[self.get_index(degree - 1, degree - 1)]
        } else if order < 0 {
            (-1.0_f32).powi(order) * factorial(degree + order) / factorial(degree - order)
                * values[self.get_index(degree, -order)]
        } else {
            ((2 * degree - 1) as f32 * x * values[self.get_index(degree - 1, order)]
                - (degree - 1 + order) as f32 * values[self.get_index(degree - 2, order)])
                / (degree - order) as f32
        }
    }

    /// Generates the associated Legendre polynomials into a pre-allocated slice.
    ///
    /// # Parameters
    /// * `x` - Input parameter in the range `[-1.0, 1.0]`.
    /// * `values` - Output slice to store the generated polynomials.
    pub fn generate_into(&self, x: f32, values: &mut [f32]) {
        let num_vals = self.get_num_values();
        assert!(values.len() >= num_vals);

        values[self.get_index(0, 0)] = self.compute_value(0, 0, x, values);
        if self.max_degree >= 1 {
            let idx = self.get_index(1, 0);
            values[idx] = self.compute_value(1, 0, x, values);
        }

        for degree in 2..=self.max_degree {
            let order = 0;
            let idx = self.get_index(degree, order);
            values[idx] = self.compute_value(degree, order, x, values);
        }

        for degree in 1..=self.max_degree {
            let order = degree;
            let idx = self.get_index(degree, order);
            values[idx] = self.compute_value(degree, order, x, values);
        }

        for degree in 2..=self.max_degree {
            let order = degree - 1;
            let idx = self.get_index(degree, order);
            values[idx] = self.compute_value(degree, order, x, values);
        }

        for degree in 3..=self.max_degree {
            for order in 1..=degree - 2 {
                let idx = self.get_index(degree, order);
                values[idx] = self.compute_value(degree, order, x, values);
            }
        }

        if self.compute_negative_order {
            for degree in 1..=self.max_degree {
                for order in 1..=degree {
                    let idx = self.get_index(degree, -order);
                    values[idx] = self.compute_value(degree, -order, x, values);
                }
            }
        }

        if !self.condon_shortley_phase {
            for degree in 1..=self.max_degree {
                let start_order = if self.compute_negative_order { -degree } else { 0 };
                for order in start_order..=degree {
                    let idx = self.get_index(degree, order);
                    values[idx] *= (-1.0_f32).powi(order);
                }
            }
        }
    }
}

// ===== Tests =====

#[cfg(test)]
mod test {

    use super::*;
    use googletest::prelude::*;

    const EPSILON: f32 = 1e-5;

    fn generate_expected_values_fourth_degree(x: f32) -> Vec<f32> {
        vec![
            1.0,                                                             // (0, 0)
            0.5 * (1.0 - x * x).sqrt(),                                      // (1, -1)
            x,                                                               // (1, 0)
            -(1.0 - x * x).sqrt(),                                           // (1, 1)
            1.0 / 8.0 * (1.0 - x * x),                                       // (2, -2)
            0.5 * x * (1.0 - x * x).sqrt(),                                  // (2, -1)
            0.5 * (3.0 * x * x - 1.0),                                       // (2, 0)
            -3.0 * x * (1.0 - x * x).sqrt(),                                 // (2, 1)
            3.0 * (1.0 - x * x),                                             // (2, 2)
            15.0 / 720.0 * (1.0 - x * x).powf(1.5),                          // (3, -3)
            15.0 / 120.0 * x * (1.0 - x * x),                                // (3, -2)
            3.0 / 24.0 * (5.0 * x * x - 1.0) * (1.0 - x * x).sqrt(),         // (3, -1)
            0.5 * (5.0 * x.powi(3) - 3.0 * x),                               // (3, 0)
            -3.0 / 2.0 * (5.0 * x * x - 1.0) * (1.0 - x * x).sqrt(),         // (3, 1)
            15.0 * x * (1.0 - x * x),                                        // (3, 2)
            -15.0 * (1.0 - x * x).powf(1.5),                                 // (3, 3)
            105.0 / 40320.0 * (1.0 - x * x).powi(2),                         // (4, -4)
            105.0 / 5040.0 * x * (1.0 - x * x).powf(1.5),                    // (4, -3)
            15.0 / 720.0 * (7.0 * x * x - 1.0) * (1.0 - x * x),              // (4, -2)
            5.0 / 40.0 * (7.0 * x.powi(3) - 3.0 * x) * (1.0 - x * x).sqrt(), // (4, -1)
            1.0 / 8.0 * (35.0 * x.powi(4) - 30.0 * x * x + 3.0),             // (4, 0)
            -5.0 / 2.0 * (7.0 * x.powi(3) - 3.0 * x) * (1.0 - x * x).sqrt(), // (4, 1)
            15.0 / 2.0 * (7.0 * x * x - 1.0) * (1.0 - x * x),                // (4, 2)
            -105.0 * x * (1.0 - x * x).powf(1.5),                            // (4, 3)
            105.0 * (1.0 - x * x).powi(2),                                   // (4, 4)
        ]
    }

    #[gtest]
    fn test_get_index_successive_indices() {
        let max_degree = 5;
        let alp_generator = AssociatedLegendrePolynomialsGenerator::new(max_degree, false, true);
        let mut last_index = -1;
        for degree in 0..=max_degree {
            for order in -degree..=degree {
                let index = alp_generator.get_index(degree, order) as i32;
                expect_that!(last_index + 1, eq(index));
                last_index = index;
            }
        }
    }

    #[gtest]
    fn test_generate_zeroth_element_is_one() {
        let max_degree = 10;
        for max_deg in 0..=max_degree {
            for condon_shortley in [false, true] {
                for compute_neg in [false, true] {
                    let alp_generator = AssociatedLegendrePolynomialsGenerator::new(
                        max_deg,
                        condon_shortley,
                        compute_neg,
                    );
                    let mut x = -1.0;
                    while x <= 1.0 {
                        let mut values = vec![0.0; alp_generator.get_num_values()];
                        alp_generator.generate_into(x, &mut values);
                        expect_that!(values[0], near(1.0, EPSILON));
                        x += 0.2;
                    }
                }
            }
        }
    }

    #[gtest]
    fn test_generate_correct_fourth_degree() {
        let max_degree = 4;
        let condon_shortley_phase = true;
        let compute_negative_order = true;
        let alp_generator = AssociatedLegendrePolynomialsGenerator::new(
            max_degree,
            condon_shortley_phase,
            compute_negative_order,
        );

        let mut x = -1.0;
        while x <= 1.0 {
            let mut generated_values = vec![0.0; alp_generator.get_num_values()];
            alp_generator.generate_into(x, &mut generated_values);
            let expected_values = generate_expected_values_fourth_degree(x);
            assert_eq!(expected_values.len(), generated_values.len());
            for i in 0..expected_values.len() {
                expect_that!(generated_values[i], near(expected_values[i], EPSILON));
            }
            x += 0.05;
        }
    }

    #[gtest]
    fn test_generate_condon_shortley_phase() {
        let max_degree = 10;
        let value = 0.12345;
        for max_deg in 0..=max_degree {
            for compute_neg in [false, true] {
                let alp_generator_without_phase =
                    AssociatedLegendrePolynomialsGenerator::new(max_deg, false, compute_neg);
                let mut values_without_phase =
                    vec![0.0; alp_generator_without_phase.get_num_values()];
                alp_generator_without_phase.generate_into(value, &mut values_without_phase);

                let alp_generator_with_phase =
                    AssociatedLegendrePolynomialsGenerator::new(max_deg, true, compute_neg);
                let mut values_with_phase = vec![0.0; alp_generator_with_phase.get_num_values()];
                alp_generator_with_phase.generate_into(value, &mut values_with_phase);

                assert_eq!(values_with_phase.len(), values_without_phase.len());
                for degree in 0..=max_deg {
                    let start_order = if compute_neg { -degree } else { 0 };
                    for order in start_order..=degree {
                        let index = alp_generator_without_phase.get_index(degree, order);
                        let expected = values_without_phase[index] * (-1.0_f32).powi(order);
                        expect_that!(values_with_phase[index], near(expected, EPSILON));
                    }
                }
            }
        }
    }
}
