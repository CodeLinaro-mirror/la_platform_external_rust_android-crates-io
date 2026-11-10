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
// TODO(b/525080422): This 3D VBAP implementation is currently unused because OLR uses layerwise 2D
// panning. Likely delete in the future.
#![allow(dead_code)]

//! Vector-based Amplitude Panning (VBAP) 3D Gain Calculator.
//!
//! This module calculates loudspeaker gains for a 3D setup using
//! triplet-wise VBAP. It maps source directions to active triangles of
//! loudspeakers and solves the panning equations using Cramer's rule.

use crate::common::definitions::{CartesianCoordinate, Degrees, OarError};
use crate::renderer::olr::convex_hull::SpeakerNode;
use crate::utility::oar_utils::normalized_polar_to_cartesian_float32;

/// A 3D VBAP gain calculator.
#[derive(Debug, Clone)]
pub struct VbapPanner {
    /// The loudspeaker nodes.
    speakers: Vec<SpeakerNode>,
    /// The active triangles representing triplets of speaker indices.
    active_triangles: Vec<(usize, usize, usize)>,
    /// Pre-allocated 3D Cartesian positions of the speakers to avoid dynamic allocation.
    speaker_cartesian: Vec<CartesianCoordinate>,
}

impl VbapPanner {
    /// Creates a new `VbapPanner` instance.
    ///
    /// # Arguments
    ///
    /// * `speakers` - A list of speaker nodes.
    /// * `active_triangles` - A list of active triangles (triplets of indices into `speakers`).
    ///
    /// # Returns
    ///
    /// A new `VbapPanner` instance with pre-calculated speaker Cartesian coordinates.
    pub fn new(speakers: Vec<SpeakerNode>, active_triangles: Vec<(usize, usize, usize)>) -> Self {
        let speaker_cartesian = speakers
            .iter()
            .map(|sp| {
                normalized_polar_to_cartesian_float32(
                    Degrees::new(sp.x).unwrap(),
                    Degrees::new(sp.y).unwrap(),
                )
            })
            .collect::<Vec<_>>();

        VbapPanner { speakers, active_triangles, speaker_cartesian }
    }

    /// Calculates loudspeaker gains for a target source direction.
    ///
    /// This function uses Cramer's rule to solve the VBAP 3D matrix system
    /// for each active triangle. If a triangle contains the source direction
    /// (i.e. all solved gains are non-negative), the gains are normalised via
    /// the L2 norm and stored in the provided `gains` slice. All other speaker
    /// gains are set to 0.0.
    ///
    /// # Arguments
    ///
    /// * `azimuth` - The target source azimuth angle.
    /// * `elevation` - The target source elevation angle.
    /// * `gains` - A mutable slice to store the calculated gains. Must be equal in length to the
    ///   number of speakers.
    pub fn calculate_gains(
        &self,
        azimuth: Degrees,
        elevation: Degrees,
        gains: &mut [f32],
    ) -> Result<(), OarError> {
        if gains.len() != self.speakers.len() {
            return Err(OarError::InvalidParameter);
        }

        // Initialise all gains to 0.0
        gains.fill(0.0);

        // Convert the target polar source direction to a Cartesian unit vector
        let p = normalized_polar_to_cartesian_float32(azimuth, elevation);

        // Search for the active triangle containing the source direction
        for &(i1, i2, i3) in &self.active_triangles {
            if i1 >= self.speaker_cartesian.len()
                || i2 >= self.speaker_cartesian.len()
                || i3 >= self.speaker_cartesian.len()
            {
                continue;
            }

            let v1 = &self.speaker_cartesian[i1];
            let v2 = &self.speaker_cartesian[i2];
            let v3 = &self.speaker_cartesian[i3];

            // Calculate determinant of M = [v1, v2, v3]
            let det = det3x3(v1, v2, v3);
            if det.abs() < 1e-6 {
                continue;
            }

            // Solve M * g = p using Cramer's rule
            let det1 = det3x3(&p, v2, v3);
            let det2 = det3x3(v1, &p, v3);
            let det3 = det3x3(v1, v2, &p);

            let g1 = det1 / det;
            let g2 = det2 / det;
            let g3 = det3 / det;

            // Check if solved gains are all non-negative (within epsilon tolerance)
            if g1 >= -1e-5 && g2 >= -1e-5 && g3 >= -1e-5 {
                let g1_clamped = g1.max(0.0);
                let g2_clamped = g2.max(0.0);
                let g3_clamped = g3.max(0.0);

                let norm =
                    (g1_clamped * g1_clamped + g2_clamped * g2_clamped + g3_clamped * g3_clamped)
                        .sqrt();
                if norm > 1e-10 {
                    gains[i1] = g1_clamped / norm;
                    gains[i2] = g2_clamped / norm;
                    gains[i3] = g3_clamped / norm;
                }
                break;
            }
        }
        Ok(())
    }
}

/// Helper to calculate the determinant of a 3x3 matrix from three column vectors.
fn det3x3(
    col1: &CartesianCoordinate,
    col2: &CartesianCoordinate,
    col3: &CartesianCoordinate,
) -> f32 {
    col1.x() * (col2.y() * col3.z() - col2.z() * col3.y())
        - col1.y() * (col2.x() * col3.z() - col2.z() * col3.x())
        + col1.z() * (col2.x() * col3.y() - col2.y() * col3.x())
}

// ===== Tests =====

#[cfg(test)]
mod test {

    use super::*;
    use crate::renderer::olr::convex_hull::SpeakerNode;
    use googletest::prelude::*;

    const EPSILON: f32 = 1e-4;
    const POWER_EPSILON: f32 = 1e-5;

    fn make_speaker(azimuth: f32, elevation: f32, index: u32) -> SpeakerNode {
        SpeakerNode { x: azimuth, y: elevation, index }
    }

    #[gtest]
    fn test_vbap_panner_exact_alignment_on_speaker_0() {
        let speakers = vec![
            make_speaker(0.0, 0.0, 0),   // Speaker 0: Y-axis [0, 1, 0]
            make_speaker(-90.0, 0.0, 1), // Speaker 1: X-axis [1, 0, 0]
            make_speaker(0.0, 90.0, 2),  // Speaker 2: Z-axis [0, 0, 1]
        ];
        let active_triangles = vec![(0, 1, 2)];
        let panner = VbapPanner::new(speakers, active_triangles);
        let mut gains = vec![0.0f32; 3];

        assert_ok!(panner.calculate_gains(Degrees::ZERO, Degrees::ZERO, &mut gains));

        expect_near!(gains[0], 1.0, EPSILON);
        expect_near!(gains[1], 0.0, EPSILON);
        expect_near!(gains[2], 0.0, EPSILON);
    }

    #[gtest]
    fn test_vbap_panner_energy_conservation_inside_active_triangle() {
        let speakers = vec![
            make_speaker(0.0, 0.0, 0),
            make_speaker(-90.0, 0.0, 1),
            make_speaker(0.0, 90.0, 2),
        ];
        let active_triangles = vec![(0, 1, 2)];
        let panner = VbapPanner::new(speakers, active_triangles);
        let test_directions = vec![(-45.0, 30.0), (-20.0, 45.0), (-60.0, 10.0), (-10.0, 80.0)];

        for (az, el) in test_directions {
            let mut g = vec![0.0f32; 3];
            assert_ok!(panner.calculate_gains(
                Degrees::new(az).unwrap(),
                Degrees::new(el).unwrap(),
                &mut g
            ));

            expect_ge!(g[0], 0.0);
            expect_ge!(g[1], 0.0);
            expect_ge!(g[2], 0.0);

            let power_sum: f32 = g.iter().map(|&val| val * val).sum();
            expect_near!(power_sum, 1.0, POWER_EPSILON);
        }
    }

    #[gtest]
    fn test_vbap_panner_gains_are_zero_when_outside_active_triangles() {
        let speakers = vec![
            make_speaker(0.0, 0.0, 0),
            make_speaker(-90.0, 0.0, 1),
            make_speaker(0.0, 90.0, 2),
        ];
        let active_triangles = vec![(0, 1, 2)];
        let panner = VbapPanner::new(speakers, active_triangles);
        let mut gains = vec![0.0f32; 3];

        assert_ok!(panner.calculate_gains(
            Degrees::new(45.0).unwrap(),
            Degrees::new(10.0).unwrap(),
            &mut gains
        ));

        expect_true!(gains.iter().all(|&g| g == 0.0));
    }
}
