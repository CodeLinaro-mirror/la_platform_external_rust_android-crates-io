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

//! DBAP 3D gain calculator.
//!
//! This module calculates loudspeaker gains for a 3D setup using DBAP.
//! DBAP is used to pan sources when they are near or outside the boundaries
//! of the active speaker layout.

use crate::common::definitions::{
    CartesianCoordinate, Degrees, Distance, OarError, PolarCoordinate,
};
use crate::renderer::olr::convex_hull::SpeakerNode;
use crate::renderer::olr::gain_calculator::GainCalculator;
use crate::utility::oar_utils::normalized_polar_to_cartesian_float32;

/// A DBAP gain calculator.
#[derive(Debug, Clone)]
pub struct DbapPanner {
    /// The loudspeaker nodes.
    speakers: Vec<SpeakerNode>,
    /// Pre-allocated 3D Cartesian positions of the speakers to avoid dynamic allocation.
    speaker_cartesian: Vec<CartesianCoordinate>,
    /// Speaker indices mapping.
    speaker_indices: Vec<usize>,
}

impl DbapPanner {
    /// Creates a new `DbapPanner` instance.
    ///
    /// # Arguments
    ///
    /// * `speakers` - A list of speaker nodes.
    ///
    /// # Returns
    ///
    /// A new `DbapPanner` instance with pre-calculated speaker Cartesian coordinates.
    pub fn new(speakers: Vec<SpeakerNode>) -> Self {
        let speaker_cartesian = speakers
            .iter()
            .map(|sp| {
                normalized_polar_to_cartesian_float32(
                    Degrees::new(sp.x).unwrap(),
                    Degrees::new(sp.y).unwrap(),
                )
            })
            .collect::<Vec<_>>();
        let speaker_indices = (0..speakers.len()).collect::<Vec<_>>();

        DbapPanner { speakers, speaker_cartesian, speaker_indices }
    }
}

impl GainCalculator for DbapPanner {
    fn calculate_gains(
        &self,
        azimuth: Degrees,
        elevation: Degrees,
        distance: Distance,
        gains: &mut [f32],
    ) -> Result<(), OarError> {
        if gains.len() < self.speakers.len() {
            return Err(OarError::InvalidParameter);
        }

        if self.speakers.is_empty() {
            return Ok(());
        }

        // Convert the polar source coordinate to 3D Cartesian coordinates
        let source_polar = PolarCoordinate::new(azimuth, elevation, distance)?;
        let source_cart: CartesianCoordinate = source_polar.try_into()?;

        // Calculate Euclidean distance and weight for each speaker.
        let mut sum_of_weights = 0.0;
        self.speaker_cartesian.iter().zip(gains.iter_mut()).for_each(|(&sp_cart, gain)| {
            let dx = sp_cart.x() - source_cart.x();
            let dy = sp_cart.y() - source_cart.y();
            let dz = sp_cart.z() - source_cart.z();
            let dist = (dx * dx + dy * dy + dz * dz).sqrt();
            let dist_clamped = dist.max(0.1);
            let weight = 1.0 / (dist_clamped * dist_clamped);
            *gain = weight;
            sum_of_weights += weight;
        });

        // Normalize weights relative to the sum of weights
        if sum_of_weights > 0.0 {
            gains.iter_mut().take(self.speakers.len()).for_each(|g| {
                *g /= sum_of_weights;
            });
        } else {
            gains[..self.speakers.len()].fill(0.0);
            return Ok(());
        }

        // L2 normalize for energy conservation
        let l2_norm = gains.iter().take(self.speakers.len()).map(|&g| g * g).sum::<f32>().sqrt();
        if l2_norm > 1e-10 {
            gains.iter_mut().take(self.speakers.len()).for_each(|g| {
                *g /= l2_norm;
            });
        } else {
            gains[..self.speakers.len()].fill(0.0);
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
        false
    }
}

// ===== Tests =====

#[cfg(test)]
mod test {

    use super::*;
    use crate::renderer::olr::convex_hull::SpeakerNode;
    use googletest::prelude::*;

    const EPSILON: f32 = 1e-5;

    fn make_speaker(azimuth: f32, elevation: f32, index: u32) -> SpeakerNode {
        SpeakerNode { x: azimuth, y: elevation, index }
    }

    #[gtest]
    fn test_dbap_panner_energy_conservation_across_source_positions() {
        let speakers = vec![
            make_speaker(0.0, 0.0, 0),
            make_speaker(90.0, 0.0, 1),
            make_speaker(180.0, 0.0, 2),
            make_speaker(-90.0, 0.0, 3),
        ];
        let panner = DbapPanner::new(speakers);
        let test_positions =
            vec![(0.0, 0.0, 1.0), (45.0, 10.0, 2.0), (-30.0, -15.0, 0.5), (135.0, 45.0, 5.0)];

        for (az, el, dist) in test_positions {
            let mut gains = vec![0.0f32; 4];
            panner
                .calculate_gains(
                    Degrees::new(az).unwrap(),
                    Degrees::new(el).unwrap(),
                    Distance::new(dist).unwrap(),
                    &mut gains,
                )
                .unwrap();

            let power_sum: f32 = gains.iter().map(|&val| val * val).sum();
            expect_near!(power_sum, 1.0, EPSILON);
        }
    }

    #[gtest]
    fn test_dbap_panner_weights_closer_speakers_higher() {
        let speakers = vec![
            make_speaker(0.0, 0.0, 0),
            make_speaker(90.0, 0.0, 1),
            make_speaker(180.0, 0.0, 2),
            make_speaker(-90.0, 0.0, 3),
        ];
        let panner = DbapPanner::new(speakers);
        let mut gains = vec![0.0f32; 4];

        panner
            .calculate_gains(Degrees::ZERO, Degrees::ZERO, Distance::new(0.1).unwrap(), &mut gains)
            .unwrap();

        expect_gt!(gains[0], gains[1]);
        expect_gt!(gains[0], gains[2]);
        expect_gt!(gains[0], gains[3]);
    }
}
