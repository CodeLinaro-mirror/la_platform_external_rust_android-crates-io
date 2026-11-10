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

//! 2D (horizontal) vector-based amplitude panning (VBAP) gain calculator.
//!
//! This module implements 2D VBAP panning, which computes loudspeaker gains for
//! source directions restricted to a single horizontal layout layer or ring of
//! speakers. It groups adjacent speakers into 2D panning regions.

use crate::common::definitions::{
    CartesianCoordinate, Degrees, Distance, OarError, PolarCoordinate,
};
use crate::renderer::olr::convex_hull::{solve_convex_hull, SpeakerNode};
use crate::renderer::olr::gain_calculator::{GainCalculator, VogPanner};
use crate::renderer::olr::SENTINEL_ANGLE_DEGREES;
use crate::utility::oar_utils::normalized_polar_to_cartesian_float32;

/// Represents a 2D VBAP panning region formed by two adjacent speakers.
#[derive(Debug)]
pub struct VbapRegion2D {
    speaker_indices: [usize; 2],
    inv_matrix: [f32; 4],
}

impl VbapRegion2D {
    /// Creates a new 2D panning region.
    ///
    /// # Errors
    ///
    /// Returns `OarError::InvalidParameter` if the speaker vectors are collinear (singular matrix).
    pub fn new(
        speaker_indices: [usize; 2],
        speaker_positions: [CartesianCoordinate; 2],
    ) -> Result<Self, OarError> {
        let matrix = [
            speaker_positions[0].x(),
            speaker_positions[0].y(),
            speaker_positions[1].x(),
            speaker_positions[1].y(),
        ];
        let mut inv_matrix = [0.0; 4];
        let det = matrix[0] * matrix[3] - matrix[1] * matrix[2];
        if det.abs() < 1e-10 {
            return Err(OarError::InvalidParameter);
        }
        let inv_det = 1.0 / det;
        inv_matrix[0] = matrix[3] * inv_det;
        inv_matrix[1] = -matrix[1] * inv_det;
        inv_matrix[2] = -matrix[2] * inv_det;
        inv_matrix[3] = matrix[0] * inv_det;

        Ok(VbapRegion2D { speaker_indices, inv_matrix })
    }
}

// TODO: Replace the argument with a PolarCoordinate.
impl GainCalculator for VbapRegion2D {
    fn calculate_gains(
        &self,
        azimuth: Degrees,
        elevation: Degrees,
        distance: Distance,
        gains: &mut [f32],
    ) -> Result<(), OarError> {
        let cart: CartesianCoordinate =
            PolarCoordinate::new(azimuth, elevation, distance)?.try_into()?;
        let xyz = [cart.x(), cart.y()];

        if gains.len() < 2 {
            return Err(OarError::InvalidParameter);
        }

        gains[0] = xyz[0] * self.inv_matrix[0] + xyz[1] * self.inv_matrix[2];
        gains[1] = xyz[0] * self.inv_matrix[1] + xyz[1] * self.inv_matrix[3];

        let mut has_negative = false;
        for g in gains.iter_mut().take(2) {
            if *g < 0.0 {
                *g = 0.0;
                has_negative = true;
            }
        }

        if has_negative {
            Err(OarError::InvalidParameter)
        } else {
            Ok(())
        }
    }

    fn gains_count(&self) -> usize {
        2
    }

    fn speaker_indices(&self) -> &[usize] {
        &self.speaker_indices
    }

    fn is_convex_hull(&self) -> bool {
        false
    }
}

/// A 2D VBAP panner that divides the horizontal plane into regions and calculates speaker gains.
#[derive(Debug)]
pub struct VbapPanner2D {
    speakers: Vec<SpeakerNode>, // Polar coordinates (x=azimuth, y=elevation)
    regions: Vec<Box<dyn GainCalculator>>,
    is_convex_hull: bool,
    speaker_indices: Vec<usize>,
}

impl VbapPanner2D {
    /// Creates a new `VbapPanner2D` for the given list of speakers.
    pub fn new(speakers: Vec<SpeakerNode>) -> Self {
        let n = speakers.len();
        let mut regions: Vec<Box<dyn GainCalculator>> = Vec::new();
        let speaker_indices = (0..n).collect::<Vec<_>>();

        let is_convex_hull = if n > 0 {
            let mut min_azi = f32::MAX;
            let mut max_azi = f32::MIN;
            for sp in &speakers {
                min_azi = min_azi.min(sp.x);
                max_azi = max_azi.max(sp.x);
            }
            max_azi > 90.0 && min_azi < -90.0
        } else {
            false
        };

        if n > 1 {
            let cartesian_positions = speakers
                .iter()
                .map(|sp| {
                    normalized_polar_to_cartesian_float32(
                        Degrees::new(sp.x).unwrap(),
                        Degrees::new(sp.y).unwrap(),
                    )
                })
                .collect::<Vec<_>>();

            if is_convex_hull {
                let cart_nodes = cartesian_positions
                    .iter()
                    .enumerate()
                    .map(|(i, cp)| SpeakerNode { x: cp.x(), y: cp.y(), index: i as u32 })
                    .collect::<Vec<_>>();

                if let Ok(hull) = solve_convex_hull(cart_nodes) {
                    let m = hull.len();
                    for i in 0..m {
                        let idx1 = hull[i].index as usize;
                        let idx2 = hull[(i + 1) % m].index as usize;

                        // TODO(b/525080422): This code duplicates the same casting of cartesian to
                        // polar coordinates for the purpose of maintaining bit-exactness with the C
                        // reference implementation. In C, cartesian_position_t was cast to
                        // polar_position_t, causing pp[i]->azimuth to read the cartesian x
                        // coordinate. We compare x coordinates here to match that behavior.
                        let (ordered_idx1, ordered_idx2) =
                            if cartesian_positions[idx1].x() < cartesian_positions[idx2].x() {
                                (idx1, idx2)
                            } else {
                                (idx2, idx1)
                            };

                        if let Ok(region) = VbapRegion2D::new(
                            [ordered_idx1, ordered_idx2],
                            [cartesian_positions[ordered_idx1], cartesian_positions[ordered_idx2]],
                        ) {
                            regions.push(Box::new(region));
                        }
                    }
                }
            } else {
                let mut indexed_speakers = speakers.iter().enumerate().collect::<Vec<_>>();
                indexed_speakers
                    .sort_by(|a, b| a.1.x.partial_cmp(&b.1.x).unwrap_or(std::cmp::Ordering::Equal));

                for i in 0..n - 1 {
                    let idx1 = indexed_speakers[i].0;
                    let idx2 = indexed_speakers[i + 1].0;

                    if let Ok(region) = VbapRegion2D::new(
                        [idx1, idx2],
                        [cartesian_positions[idx1], cartesian_positions[idx2]],
                    ) {
                        regions.push(Box::new(region));
                    }
                }
            }
        } else if n == 1 {
            regions.push(Box::new(VogPanner));
        }

        VbapPanner2D { speakers, regions, is_convex_hull, speaker_indices }
    }

    /// Returns whether this panner forms a closed convex hull.
    pub fn is_convex_hull(&self) -> bool {
        self.is_convex_hull
    }
}

impl GainCalculator for VbapPanner2D {
    fn calculate_gains(
        &self,
        azimuth: Degrees,
        elevation: Degrees,
        distance: Distance,
        gains: &mut [f32],
    ) -> Result<(), OarError> {
        let n = self.speakers.len();
        if gains.len() < n {
            return Err(OarError::InvalidParameter);
        }
        gains[..n].fill(0.0);

        let mut success = false;
        let mut r_gains = [0.0; 2];

        for region in &self.regions {
            let s = region.gains_count();
            if region.calculate_gains(azimuth, elevation, distance, &mut r_gains[..s]).is_ok() {
                let norm = if s == 2 {
                    (r_gains[0] * r_gains[0] + r_gains[1] * r_gains[1]).sqrt()
                } else {
                    r_gains[0]
                };

                if norm > 0.0 {
                    let indices = region.speaker_indices();
                    for j in 0..s {
                        gains[indices[j]] += r_gains[j] / norm;
                    }
                    success = true;
                    break;
                }
            }
        }

        if !success {
            let mut min_diff = SENTINEL_ANGLE_DEGREES;
            let mut closest_spk_idx = 0;
            for (j, sp) in self.speakers.iter().enumerate() {
                let az_diff = (sp.x - azimuth.value()).abs();
                if az_diff < min_diff {
                    min_diff = az_diff;
                    closest_spk_idx = j;
                }
            }
            if n > 0 {
                gains[closest_spk_idx] = 1.0;
            }
        }

        Ok(())
    }

    fn gains_count(&self) -> usize {
        self.speakers.len()
    }

    fn speaker_indices(&self) -> &[usize] {
        &self.speaker_indices
    }

    fn is_convex_hull(&self) -> bool {
        self.is_convex_hull()
    }
}

// ===== Tests =====

#[cfg(test)]
mod test {

    use super::*;
    use googletest::prelude::*;
    use std::f32::consts::FRAC_1_SQRT_2;

    const EPSILON: f32 = 1e-4;

    #[gtest]
    fn test_vbap_2d_panner_exact_alignment_on_speaker_0() {
        let speakers = vec![
            SpeakerNode { x: 30.0, y: 0.0, index: 0 },
            SpeakerNode { x: -30.0, y: 0.0, index: 1 },
        ];
        let panner = VbapPanner2D::new(speakers);
        let mut gains = [0.0; 2];

        assert_ok!(panner.calculate_gains(
            Degrees::new(30.0).unwrap(),
            Degrees::ZERO,
            Distance::ONE,
            &mut gains
        ));

        expect_that!(gains[0], near(1.0, EPSILON));
        expect_that!(gains[1], near(0.0, EPSILON));
    }

    #[gtest]
    fn test_vbap_2d_panner_exact_alignment_on_speaker_1() {
        let speakers = vec![
            SpeakerNode { x: 30.0, y: 0.0, index: 0 },
            SpeakerNode { x: -30.0, y: 0.0, index: 1 },
        ];
        let panner = VbapPanner2D::new(speakers);
        let mut gains = [0.0; 2];

        assert_ok!(panner.calculate_gains(
            Degrees::new(-30.0).unwrap(),
            Degrees::ZERO,
            Distance::ONE,
            &mut gains
        ));

        expect_that!(gains[0], near(0.0, EPSILON));
        expect_that!(gains[1], near(1.0, EPSILON));
    }

    #[gtest]
    fn test_vbap_2d_panner_midpoint_energy_conservation() {
        let speakers = vec![
            SpeakerNode { x: 30.0, y: 0.0, index: 0 },
            SpeakerNode { x: -30.0, y: 0.0, index: 1 },
        ];
        let panner = VbapPanner2D::new(speakers);
        let mut gains = [0.0; 2];

        assert_ok!(panner.calculate_gains(Degrees::ZERO, Degrees::ZERO, Distance::ONE, &mut gains));

        expect_that!(gains[0], near(FRAC_1_SQRT_2, EPSILON));
        expect_that!(gains[1], near(FRAC_1_SQRT_2, EPSILON));
    }

    #[gtest]
    fn test_vbap_2d_fallback_closest_speaker_0() {
        let speakers = vec![
            SpeakerNode { x: 30.0, y: 0.0, index: 0 },
            SpeakerNode { x: -30.0, y: 0.0, index: 1 },
        ];
        let panner = VbapPanner2D::new(speakers);
        let mut gains = [0.0; 2];

        assert_ok!(panner.calculate_gains(
            Degrees::new(90.0).unwrap(),
            Degrees::ZERO,
            Distance::ONE,
            &mut gains
        ));

        expect_that!(gains[0], near(1.0, EPSILON));
        expect_that!(gains[1], near(0.0, EPSILON));
    }

    #[gtest]
    fn test_vbap_2d_fallback_closest_speaker_1() {
        let speakers = vec![
            SpeakerNode { x: 30.0, y: 0.0, index: 0 },
            SpeakerNode { x: -30.0, y: 0.0, index: 1 },
        ];
        let panner = VbapPanner2D::new(speakers);
        let mut gains = [0.0; 2];

        assert_ok!(panner.calculate_gains(
            Degrees::new(-90.0).unwrap(),
            Degrees::ZERO,
            Distance::ONE,
            &mut gains
        ));

        expect_that!(gains[0], near(0.0, EPSILON));
        expect_that!(gains[1], near(1.0, EPSILON));
    }
}
