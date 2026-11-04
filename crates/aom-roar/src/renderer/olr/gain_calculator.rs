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

//! Common trait and implementations for gain calculators.
//!
//! This module defines the `GainCalculator` trait, which is implemented by various
//! panning backends (e.g., VBAP, DBAP, and custom layerwise calculators) to
//! compute loudspeaker gains from 3D source coordinates. It also provides `VogPanner`
//! for single-speaker scenarios.

use crate::common::definitions::{Degrees, Distance, OarError};

pub trait GainCalculator: std::fmt::Debug + Send + Sync {
    /// Calculates loudspeaker gains for a target source direction and distance.
    ///
    /// # Arguments
    ///
    /// * `azimuth` - The target source azimuth angle.
    /// * `elevation` - The target source elevation angle.
    /// * `distance` - The target source distance.
    /// * `gains` - A mutable slice to store the calculated gains.
    fn calculate_gains(
        &self,
        azimuth: Degrees,
        elevation: Degrees,
        distance: Distance,
        gains: &mut [f32],
    ) -> Result<(), OarError>;

    /// Returns the number of output gains this calculator produces.
    fn gains_count(&self) -> usize;

    /// Returns the indices of the speakers this calculator maps to.
    fn speaker_indices(&self) -> &[usize];

    /// Returns whether this panner is a convex hull.
    fn is_convex_hull(&self) -> bool;
}

/// A VOG gain calculator.
///
/// This panner handles single-speaker configurations and always returns a gain of 1.0.
#[derive(Debug, Clone, Copy)]
pub struct VogPanner;

impl GainCalculator for VogPanner {
    fn calculate_gains(
        &self,
        _azimuth: Degrees,
        _elevation: Degrees,
        _distance: Distance,
        gains: &mut [f32],
    ) -> Result<(), OarError> {
        if gains.is_empty() {
            return Err(OarError::InvalidParameter);
        }
        gains[0] = 1.0;
        gains[1..].fill(0.0);
        Ok(())
    }

    fn gains_count(&self) -> usize {
        1
    }

    fn speaker_indices(&self) -> &[usize] {
        static INDICES: [usize; 1] = [0];
        &INDICES
    }

    fn is_convex_hull(&self) -> bool {
        true // Vog is treated as convex hull to avoid separation in asymmetric layouts
    }
}
