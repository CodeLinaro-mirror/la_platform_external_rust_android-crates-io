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

//! Layerwise 2D/3D Gain Calculator.
//!
//! This module provides a gain calculator that handles 3D panning by dividing the
//! loudspeaker layout into horizontal layers at different elevations. It performs
//! vertical panning across layers using tangent-law interpolation, and horizontal
//! panning within each layer using a combination of VBAP and DBAP.

use crate::common::definitions::{Degrees, Distance, OarError, Radians, MAX_OUTPUT_CHANNEL_COUNT};
use crate::common::layout::SpeakerPosition;
use crate::renderer::olr::convex_hull::SpeakerNode;
use crate::renderer::olr::dbap::DbapPanner;
use crate::renderer::olr::gain_calculator::{GainCalculator, VogPanner};
use crate::renderer::olr::vbap_2d::VbapPanner2D;
use crate::renderer::olr::SENTINEL_ANGLE_DEGREES;

/// Represents the calculated gain for a specific elevation layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ElevationGain {
    /// The elevation angle.
    pub elevation: Degrees,
    /// The gain applied to this layer.
    pub gain: f32,
}

/// Performs 1D linear panning across layers of loudspeakers.
#[derive(Debug, Clone)]
pub struct LayerwisePanner {
    ref_layers: Vec<Degrees>,
}

/// Holds the results of a layerwise gain calculation.
///
/// Avoids dynamic memory allocation by using a fixed-size array of up to 4 elements.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerGainsResult {
    /// The computed gains per elevation layer.
    pub items: [ElevationGain; 4],
    /// The number of valid items in the array.
    pub len: usize,
}

impl LayerGainsResult {
    /// Returns the number of valid gains in the result.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Pushes a new elevation gain to the result.
    ///
    /// If the fixed-size buffer is full, the gain is ignored.
    pub fn push(&mut self, item: ElevationGain) {
        if self.len < self.items.len() {
            self.items[self.len] = item;
            self.len += 1;
        }
    }

    /// Returns the active elevation gains as a slice.
    pub fn as_slice(&self) -> &[ElevationGain] {
        &self.items[..self.len]
    }
}

impl std::ops::Index<usize> for LayerGainsResult {
    type Output = ElevationGain;
    fn index(&self, idx: usize) -> &Self::Output {
        &self.items[idx]
    }
}

impl LayerwisePanner {
    /// Creates a new `LayerwisePanner` with reference elevations.
    pub fn new(ref_layers: Vec<Degrees>) -> Self {
        LayerwisePanner { ref_layers }
    }

    /// Calculates layerwise gains for a given elevation.
    ///
    /// Determines which elevation layers are active for the target elevation
    /// and performs a tangent-law interpolation between them.
    pub fn calculate_gains(&self, elevation: Degrees) -> LayerGainsResult {
        let n = self.ref_layers.len();
        if n == 0 {
            return LayerGainsResult {
                items: [ElevationGain { elevation: Degrees::ZERO, gain: 0.0 }; 4],
                len: 0,
            };
        }

        let mut min_ele = f32::MAX;
        let mut max_ele = f32::MIN;
        for &el in &self.ref_layers {
            min_ele = min_ele.min(el.value());
            max_ele = max_ele.max(el.value());
        }

        // Check for exact match
        if let Some(&el) =
            self.ref_layers.iter().find(|&&el| (el.value() - elevation.value()).abs() < 1e-6)
        {
            return LayerGainsResult {
                items: [
                    ElevationGain { elevation: el, gain: 1.0 },
                    ElevationGain { elevation: Degrees::ZERO, gain: 0.0 },
                    ElevationGain { elevation: Degrees::ZERO, gain: 0.0 },
                    ElevationGain { elevation: Degrees::ZERO, gain: 0.0 },
                ],
                len: 1,
            };
        }

        if elevation.value() < min_ele {
            return LayerGainsResult {
                items: [
                    ElevationGain { elevation: Degrees::new(min_ele).unwrap(), gain: 1.0 },
                    ElevationGain { elevation: Degrees::ZERO, gain: 0.0 },
                    ElevationGain { elevation: Degrees::ZERO, gain: 0.0 },
                    ElevationGain { elevation: Degrees::ZERO, gain: 0.0 },
                ],
                len: 1,
            };
        }

        if elevation.value() > max_ele {
            return LayerGainsResult {
                items: [
                    ElevationGain { elevation: Degrees::new(max_ele).unwrap(), gain: 1.0 },
                    ElevationGain { elevation: Degrees::ZERO, gain: 0.0 },
                    ElevationGain { elevation: Degrees::ZERO, gain: 0.0 },
                    ElevationGain { elevation: Degrees::ZERO, gain: 0.0 },
                ],
                len: 1,
            };
        }

        // Allocation-free search for closest opposite-side layers ele0 and ele1 (`#alloc` safe)
        let ele0 = *self
            .ref_layers
            .iter()
            .min_by(|&&a, &&b| {
                (a.value() - elevation.value())
                    .abs()
                    .total_cmp(&(b.value() - elevation.value()).abs())
            })
            .unwrap();
        let sign0 = (ele0.value() - elevation.value()).signum();

        let mut ele1 = ele0;
        if let Some(&el) = self
            .ref_layers
            .iter()
            .filter(|&&el| (el.value() - elevation.value()).signum() != sign0)
            .min_by(|&&a, &&b| {
                (a.value() - elevation.value())
                    .abs()
                    .total_cmp(&(b.value() - elevation.value()).abs())
            })
        {
            ele1 = el;
        }

        let speaker_angle = (ele1.value() - ele0.value()).abs() / 2.0;
        let source_angle = speaker_angle - (ele0.value() - elevation.value()).abs();

        let mut ele_gains = [0.0; 2];
        if tanlaw(
            Degrees::new(source_angle).unwrap(),
            Degrees::new(speaker_angle).unwrap(),
            &mut ele_gains,
        )
        .is_ok()
        {
            LayerGainsResult {
                items: [
                    ElevationGain { elevation: ele0, gain: ele_gains[0] },
                    ElevationGain { elevation: ele1, gain: ele_gains[1] },
                    ElevationGain { elevation: Degrees::ZERO, gain: 0.0 },
                    ElevationGain { elevation: Degrees::ZERO, gain: 0.0 },
                ],
                len: 2,
            }
        } else {
            LayerGainsResult {
                items: [ElevationGain { elevation: Degrees::ZERO, gain: 0.0 }; 4],
                len: 0,
            }
        }
    }
}

fn tanlaw(
    source_angle: Degrees,
    speaker_angle: Degrees,
    gains: &mut [f32; 2],
) -> Result<(), OarError> {
    if source_angle.value() > speaker_angle.value() {
        return Err(OarError::InvalidParameter);
    }
    if speaker_angle.value() < 0.0 || speaker_angle.value() >= 90.0 {
        return Err(OarError::InvalidParameter);
    }

    let source_rad = Radians::from(source_angle).value();
    let speaker_rad = Radians::from(speaker_angle).value();
    let c = source_rad.cos() / speaker_rad.cos();
    let s = source_rad.sin() / speaker_rad.sin();
    gains[0] = c + s;
    gains[1] = c - s;
    let gains_norm = (gains[0] * gains[0] + gains[1] * gains[1]).sqrt();

    if gains_norm == 0.0 {
        return Err(OarError::InvalidParameter);
    }

    gains[0] /= gains_norm;
    gains[1] /= gains_norm;

    Ok(())
}

#[derive(Debug)]
struct LayerInfo {
    elevation: Degrees,
    vbap_panner: Box<dyn GainCalculator>,
    dbap_panner: Box<dyn GainCalculator>,
    speaker_indices: Vec<usize>,
    max_az: f32,
}

/// A custom gain calculator that combines layerwise, VBAP, and DBAP panning.
///
/// This calculator partitions a 3D speaker layout into separate horizontal elevation
/// layers. When computing gains, it performs layerwise panning in the vertical direction
/// and a combination of VBAP and DBAP panning in the horizontal plane of each active layer.
#[derive(Debug)]
pub struct CustomGainCalculator {
    speakers: &'static [SpeakerPosition],
    num_speakers_without_lfe: usize,
    layers: Vec<LayerInfo>,
    layerwise_panner: LayerwisePanner,
    speaker_indices: Vec<usize>,
}

impl CustomGainCalculator {
    /// Creates a new `CustomGainCalculator` for the specified target output layout.
    ///
    /// # Errors
    /// Returns `OarError::InvalidParameter` if the speaker layout is unsupported or missing.
    pub fn new(layout_enum: crate::common::definitions::OutputLayout) -> Result<Self, OarError> {
        let speakers = layout_enum.olr_speakers().ok_or(OarError::InvalidParameter)?;
        if speakers.len() > MAX_OUTPUT_CHANNEL_COUNT {
            return Err(OarError::NotSupported);
        }
        let num_speakers_without_lfe = speakers.iter().filter(|sp| !sp.is_lfe).count();

        let mut unique_elevations: Vec<f32> = Vec::new();
        for sp in speakers.iter().filter(|sp| !sp.is_lfe) {
            if !unique_elevations.iter().any(|&el| (el - sp.elevation.value()).abs() < 1e-6) {
                unique_elevations.push(sp.elevation.value());
            }
        }

        let mut layers = Vec::new();
        for &el in &unique_elevations {
            let mut layer_speakers = Vec::new();
            let mut speaker_indices = Vec::new();
            for (j, sp) in speakers.iter().filter(|sp| !sp.is_lfe).enumerate() {
                if (sp.elevation.value() - el).abs() < 1e-6 {
                    layer_speakers.push(SpeakerNode {
                        x: sp.azimuth.value(),
                        y: sp.elevation.value(),
                        index: 0, // Will be set below
                    });
                    speaker_indices.push(j);
                }
            }

            let m = layer_speakers.len();
            for (local_idx, sp) in layer_speakers.iter_mut().enumerate() {
                sp.index = local_idx as u32;
            }

            let mut max_az = -SENTINEL_ANGLE_DEGREES;
            for sp in &layer_speakers {
                max_az = max_az.max(sp.x);
            }

            let vbap_panner: Box<dyn GainCalculator> = if m != 1 {
                Box::new(VbapPanner2D::new(layer_speakers.clone()))
            } else {
                Box::new(VogPanner)
            };

            let dbap_panner: Box<dyn GainCalculator> = if m != 1 {
                Box::new(DbapPanner::new(layer_speakers))
            } else {
                Box::new(VogPanner)
            };

            layers.push(LayerInfo {
                elevation: Degrees::new(el).unwrap(),
                vbap_panner,
                dbap_panner,
                speaker_indices,
                max_az,
            });
        }

        let ref_layers =
            unique_elevations.iter().map(|&el| Degrees::new(el).unwrap()).collect::<Vec<_>>();
        let layerwise_panner = LayerwisePanner::new(ref_layers);
        let speaker_indices = (0..speakers.len()).collect();

        Ok(CustomGainCalculator {
            speakers,
            num_speakers_without_lfe,
            layers,
            layerwise_panner,
            speaker_indices,
        })
    }

    fn get_layer_max_min_elevation(&self, max: &mut f32, min: &mut f32) {
        let mut mn = SENTINEL_ANGLE_DEGREES;
        let mut mx = -SENTINEL_ANGLE_DEGREES;
        for ly in &self.layers {
            mn = mn.min(ly.elevation.value());
            mx = mx.max(ly.elevation.value());
        }
        *min = mn;
        *max = mx;
    }
}

fn linalg_norm(v: &[f32]) -> f32 {
    v.iter().map(|&x| x * x).sum::<f32>().sqrt()
}

impl GainCalculator for CustomGainCalculator {
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

        let n_wo_lfe = self.num_speakers_without_lfe;
        let mut gains_wo_lfe = [0.0f32; MAX_OUTPUT_CHANNEL_COUNT];

        let mut layer_gains = self.layerwise_panner.calculate_gains(elevation);

        let mut new_elevation = SENTINEL_ANGLE_DEGREES;
        let mut l = layer_gains.len();
        let mut layer_cnt = l as i32;

        let mut i = 0;
        while i < l {
            let ele_gain = layer_gains[i];
            let dbap_dist;
            let mut alpha;
            let mut max = -SENTINEL_ANGLE_DEGREES;
            let mut min = SENTINEL_ANGLE_DEGREES;

            layer_cnt -= 1;

            let ly = match self
                .layers
                .iter()
                .find(|ly| (ly.elevation.value() - ele_gain.elevation.value()).abs() < 1e-6)
            {
                Some(ly) => ly,
                None => {
                    i += 1;
                    continue;
                }
            };

            let m = ly.speaker_indices.len();

            // Support back speaker of asymmetric layout
            if layer_cnt >= 0
                && ele_gain.elevation.value() != 0.0
                && !ly.vbap_panner.is_convex_hull()
                && azimuth.value().abs() > ly.max_az
            {
                new_elevation = ele_gain.elevation.value()
                    * (1.0 - (azimuth.value().abs() - ly.max_az) / (180.0 - ly.max_az));
                let layerwise_gains =
                    self.layerwise_panner.calculate_gains(Degrees::new(new_elevation)?);
                for lg in layerwise_gains.as_slice().iter().copied() {
                    let mut item_lg = lg;
                    item_lg.gain *= ele_gain.gain;
                    layer_gains.push(item_lg);
                }
                l = layer_gains.len();
                i += 1;
                continue;
            }

            let mut vbap_gains = [0.0f32; MAX_OUTPUT_CHANNEL_COUNT];
            let mut dbap_gains = [0.0f32; MAX_OUTPUT_CHANNEL_COUNT];
            let mut gains_2d = [0.0f32; MAX_OUTPUT_CHANNEL_COUNT];

            let current_elevation = if layer_cnt < 0 { new_elevation } else { elevation.value() };
            let current_elevation_deg = Degrees::new(current_elevation)?;

            self.get_layer_max_min_elevation(&mut max, &mut min);
            let mut current_distance = distance.value();
            if current_elevation > max || current_elevation < min {
                current_distance *= Radians::from(current_elevation_deg).value().cos();
            }

            // Calculate VBAP
            let mut final_azimuth = azimuth.value();
            if !ly.vbap_panner.is_convex_hull() {
                if final_azimuth > 90.0 && final_azimuth <= 180.0 {
                    final_azimuth = 180.0 - final_azimuth;
                } else if (-180.0..-90.0).contains(&final_azimuth) {
                    final_azimuth = -180.0 - final_azimuth;
                }
            }
            let _ = ly.vbap_panner.calculate_gains(
                Degrees::new(final_azimuth)?,
                current_elevation_deg,
                Distance::ONE,
                &mut vbap_gains[..m],
            );

            // Calculate DBAP distance and alpha
            if current_distance < 0.5 {
                dbap_dist = 0.0;
                alpha = current_distance;
            } else if current_distance < 0.9 {
                dbap_dist = 2.0 * current_distance - 1.0;
                alpha = 0.5;
            } else {
                dbap_dist = 0.8;
                alpha = 0.5 + 0.5 * (current_distance - 0.9) / 0.1;
                if alpha > 1.0 {
                    alpha = 1.0;
                }
            }

            // Calculate DBAP
            let _ = ly.dbap_panner.calculate_gains(
                azimuth,
                current_elevation_deg,
                Distance::new(dbap_dist).unwrap(),
                &mut dbap_gains[..m],
            );

            // Combine VBAP + DBAP
            for j in 0..m {
                gains_2d[j] = alpha * vbap_gains[j] + (1.0 - alpha) * dbap_gains[j];
            }
            let gain_2d_norm = linalg_norm(&gains_2d[..m]);
            if gain_2d_norm > 1e-10 {
                for gain_2d in gains_2d[..m].iter_mut() {
                    *gain_2d /= gain_2d_norm;
                }
            }

            // Map to global gains_wo_lfe
            for (j, &gain_2d) in gains_2d[..m].iter().enumerate() {
                let global_idx = ly.speaker_indices[j];
                gains_wo_lfe[global_idx] += gain_2d * ele_gain.gain;
            }

            i += 1;
        }

        let gains_wo_lfe_norm = linalg_norm(&gains_wo_lfe[..n_wo_lfe]);

        let mut j = 0;
        for (i, gain) in gains.iter_mut().enumerate().take(self.speakers.len()) {
            let sp = &self.speakers[i];
            if !sp.is_lfe {
                if gains_wo_lfe_norm != 0.0 {
                    *gain = gains_wo_lfe[j] / gains_wo_lfe_norm;
                } else {
                    *gain = 0.0;
                }
                j += 1;
            } else {
                *gain = 0.0;
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
        false
    }
}

// ===== Tests =====

#[cfg(test)]
mod test {

    use super::*;
    use crate::common::definitions::OutputLayout;
    use googletest::prelude::*;

    const EPSILON: f32 = 1e-4;

    #[gtest]
    fn test_custom_gain_calculator_stereo_exact_alignment() {
        let calculator = CustomGainCalculator::new(OutputLayout::Stereo).unwrap();
        let mut gains = [0.0; 2];

        let result = calculator.calculate_gains(
            Degrees::new(30.0).unwrap(),
            Degrees::ZERO,
            Distance::ONE,
            &mut gains,
        );

        assert_ok!(result);
        expect_that!(gains[0], near(1.0, EPSILON));
        expect_that!(gains[1], near(0.0, EPSILON));
    }

    #[gtest]
    fn test_custom_gain_calculator_stereo_midpoint() {
        let calculator = CustomGainCalculator::new(OutputLayout::Stereo).unwrap();
        let mut gains = [0.0; 2];

        let result =
            calculator.calculate_gains(Degrees::ZERO, Degrees::ZERO, Distance::ONE, &mut gains);

        assert_ok!(result);
        expect_that!(gains[0], near(std::f32::consts::FRAC_1_SQRT_2, EPSILON));
        expect_that!(gains[1], near(std::f32::consts::FRAC_1_SQRT_2, EPSILON));
    }
}
