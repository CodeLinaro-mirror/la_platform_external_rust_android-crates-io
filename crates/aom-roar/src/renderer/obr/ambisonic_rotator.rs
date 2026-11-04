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

//! Higher-Order Ambisonic (HOA) sound field rotator.
//!
//! Rotates a sound field block by a given target rotation. Used to align the spatial audio
//! scene with the listener's head movements.

use crate::common::audio_buffer::AudioBuffer;
use crate::common::definitions::Quaternion;
use crate::renderer::obr::common::ambisonic_utils::{
    get_num_nth_order_periphonic_components, get_num_periphonic_components,
};

const ROTATION_QUANTIZATION_RAD: f32 = 1.0f32.to_radians();
const SLERP_FRAME_INTERVAL: usize = 32;

/// Precomputed recurrence coefficients for spherical harmonic rotation.
#[derive(Debug, Clone, Copy, PartialEq)]
struct UvwEntry {
    /// Recurrence coefficient scaling the `U` coupling term.
    u: f32,
    /// Recurrence coefficient scaling the `V` coupling term.
    v: f32,
    /// Recurrence coefficient scaling the `W` coupling term.
    w: f32,
}

/// Sound field rotator for Higher-Order Ambisonics.
///
/// Employs recurrence relations to construct block-diagonal rotation matrices mapping input
/// Ambisonic channels to rotated output channels. Supports smooth spherical linear interpolation
/// (slerp) to prevent clicks when the orientation changes.
///
/// Note: "Band" is used in this module to refer to the channels added by a given ambisonic order.
/// For example band 2 refers to the 5 channels added for 2nd order ambisonics.
#[derive(Debug, Clone, PartialEq)]
pub struct AmbisonicRotator {
    /// The ambisonic order of the sound field.
    ambisonic_order: i32,
    /// Number of audio frames per buffer used to size scratch memory.
    frames_per_buffer: usize,
    /// The current rotation of the sound field to be applied.
    current_rotation: Quaternion,
    /// Whether audio has been processed at least once. Used to snap to the target rotation on the
    /// first processed block rather than interpolating from identity.
    has_processed: bool,
    /// Rotation matrices for each band of the sound field.
    /// `rotation_matrices[band]` is the (2 * band + 1) x (2 * band + 1) matrix for `band`.
    rotation_matrices: Vec<Vec<f32>>,
    /// Precomputed recurrence parameters for each band (bands 2..=ambisonic_order).
    uvw_tables: Vec<Vec<UvwEntry>>,
    /// Scratch memory used to decouple in-place channel reads and writes during band rotation.
    band_scratch: Vec<f32>,
}

impl AmbisonicRotator {
    /// Constructs a new `AmbisonicRotator` instance.
    ///
    /// # Parameters
    /// * `ambisonic_order` - Ambisonic order of the sound field (must be >= 1).
    /// * `frames_per_buffer` - Number of audio frames per buffer used to allocate scratch memory.
    pub fn new(ambisonic_order: i32, frames_per_buffer: usize) -> Self {
        assert!(ambisonic_order >= 1);
        assert!(frames_per_buffer > 0);
        // The maximum band size is the number of channels in the highest ambisonic band
        // (e.g. 25 - 16 = 9 for band 4).
        let max_band_size = get_num_nth_order_periphonic_components(ambisonic_order);
        let mut rotation_matrices = vec![Vec::new(); (ambisonic_order + 1) as usize];

        // Band 0 is a 1x1 identity matrix.
        rotation_matrices[0] = vec![1.0_f32];

        // Initialize each band's rotation matrix to identity.
        for band in 1..=ambisonic_order {
            let band_size = get_num_nth_order_periphonic_components(band);
            let mut r = vec![0.0_f32; band_size * band_size];
            for i in 0..band_size {
                r[i * band_size + i] = 1.0;
            }
            rotation_matrices[band as usize] = r;
        }

        // Precompute recurrence coefficients for bands 2..=ambisonic_order to eliminate
        // redundant divisions and square roots during dynamic head rotation.
        let mut uvw_tables = vec![Vec::new(); (ambisonic_order + 1) as usize];
        for band in 2..=ambisonic_order {
            let size = (2 * band + 1) as usize;
            let mut table = Vec::with_capacity(size * size);
            for m in -band..=band {
                for n in -band..=band {
                    let (u, v, w) = compute_uvw_coeff(m, n, band);
                    table.push(UvwEntry { u, v, w });
                }
            }
            uvw_tables[band as usize] = table;
        }

        Self {
            ambisonic_order,
            frames_per_buffer,
            current_rotation: Quaternion::identity(),
            has_processed: false,
            rotation_matrices,
            uvw_tables,
            band_scratch: vec![0.0_f32; max_band_size * frames_per_buffer],
        }
    }

    /// Marks that an audio block has been rendered while the rotator was bypassed (e.g., head
    /// tracking disabled), so that a subsequent rotation interpolates smoothly from identity rather
    /// than snapping on its first active block.
    pub fn mark_audio_rendered(&mut self) {
        self.has_processed = true;
    }

    /// Performs a smooth in-place rotation (`buffer = R * buffer`) of the sound field buffer.
    pub fn process_in_place(
        &mut self,
        target_rotation: &Quaternion,
        buffer: &mut AudioBuffer,
    ) -> bool {
        assert!(buffer.num_frames() <= self.frames_per_buffer);
        let num_channels = get_num_periphonic_components(self.ambisonic_order);
        assert_eq!(buffer.num_channels(), num_channels);

        let identity = Quaternion::identity();
        if self.current_rotation.angular_difference_rad(&identity) < ROTATION_QUANTIZATION_RAD
            && target_rotation.angular_difference_rad(&identity) < ROTATION_QUANTIZATION_RAD
        {
            self.has_processed = true;
            return false;
        }

        // On the first processed block, snap immediately to target_rotation instead of slerping
        // from identity so that a pre-configured head orientation takes full effect from sample 0.
        if !self.has_processed {
            self.has_processed = true;
            if self.current_rotation.angular_difference_rad(target_rotation)
                >= ROTATION_QUANTIZATION_RAD
            {
                self.current_rotation = *target_rotation;
                self.update_rotation_matrix(target_rotation);
            }
        }

        let num_frames = buffer.num_frames();
        if self.current_rotation.angular_difference_rad(target_rotation) < ROTATION_QUANTIZATION_RAD
        {
            self.multiply_channels_in_place(buffer, 0, num_frames);
            return true;
        }

        let mut i = 0;
        while i < num_frames {
            let duration = (num_frames - i).min(SLERP_FRAME_INTERVAL);
            let interpolation_factor = (i + duration) as f32 / num_frames as f32;
            let slerp_rot = self.current_rotation.slerp(interpolation_factor, target_rotation);
            self.update_rotation_matrix(&slerp_rot);
            self.multiply_channels_in_place(buffer, i, duration);
            i += duration;
        }
        self.current_rotation = *target_rotation;
        true
    }

    /// Multiplies the channels of the audio buffer in place using the current rotation matrices.
    fn multiply_channels_in_place(
        &mut self,
        buffer: &mut AudioBuffer,
        start_frame: usize,
        num_frames: usize,
    ) {
        // Ambisonic channel 0 is spherically symmetric (omnidirectional) and invariant to rotation.
        // Multiply each band's channels by its rotation matrix (flattened square matrix with side
        // length of 2 * band + 1).
        for band in 1..=self.ambisonic_order as usize {
            let band_matrix = &self.rotation_matrices[band];
            let band_size = 2 * band + 1; // Starting with band 1: 3, 5, 7, 9.
            let start_channel = band * band; // Starting with band 1: 1, 4, 9, 16.

            // Copy input channels for this band into contiguous rows of band_scratch.
            // This decouples reads from in-place writes and enables contiguous memory access.
            for chan in 0..band_size {
                let src = &buffer[start_channel + chan][start_frame..start_frame + num_frames];
                let scratch = &mut self.band_scratch[chan * num_frames..(chan + 1) * num_frames];
                scratch.copy_from_slice(src);
            }
            // Compute output channels as planar linear combinations of the scratch input channels.
            // Slices are contiguous and distinct from scratch memory, enabling auto-vectorization.
            for r in 0..band_size {
                let row_offset = r * band_size;
                let coeff_0 = band_matrix[row_offset];
                let src_0 = &self.band_scratch[0..num_frames];
                let mut ch = buffer.channel_mut(start_channel + r);
                let dst = &mut ch.as_mut_slice()[start_frame..start_frame + num_frames];
                for (d, &s) in dst.iter_mut().zip(src_0.iter()) {
                    *d = coeff_0 * s;
                }
                for c in 1..band_size {
                    let coeff = band_matrix[row_offset + c];
                    let src_c = &self.band_scratch[c * num_frames..(c + 1) * num_frames];
                    for (d, &s) in dst.iter_mut().zip(src_c.iter()) {
                        *d += coeff * s;
                    }
                }
            }
        }
    }

    /// Updates the rotation matrix using the supplied `Quaternion`.
    pub fn update_rotation_matrix(&mut self, rotation: &Quaternion) {
        // Convert from ADM object coordinates (X=right, Y=forward, Z=up) to AmbiX (X=front, Y=left,
        // Z=up).
        let ambix_rotation =
            Quaternion::new(rotation.w(), -rotation.x(), rotation.z(), rotation.y()).unwrap();
        let r_3x3 = ambix_rotation.to_rotation_matrix();

        // Band 1 rotation matrix.
        let r1 = &mut self.rotation_matrices[1];
        for r in 0..3 {
            for c in 0..3 {
                r1[r * 3 + c] = r_3x3[r][c];
            }
        }

        for band in 2..=self.ambisonic_order {
            self.compute_band_rotation(band);
        }
    }

    /// Computes the rotation matrix for `band` using Ivanic and Ruedenberg spherical harmonic
    /// recurrence relations.
    fn compute_band_rotation(&mut self, band: i32) {
        let (prev_matrices, target_slice) = self.rotation_matrices.split_at_mut(band as usize);
        let current_band_matrix = &mut target_slice[0];
        let band1_matrix = &prev_matrices[1];
        let prev_band_matrix = &prev_matrices[(band - 1) as usize];
        let uvw_table = &self.uvw_tables[band as usize];

        let mut matrix_and_uvw = current_band_matrix.iter_mut().zip(uvw_table);
        for m in -band..=band {
            for n in -band..=band {
                let (dst, entry) = matrix_and_uvw.next().unwrap();
                let mut term_u = 0.0_f32;
                let mut term_v = 0.0_f32;
                let mut term_w = 0.0_f32;
                if entry.u != 0.0 {
                    term_u = entry.u * u_func(m, n, band, band1_matrix, prev_band_matrix);
                }
                if entry.v != 0.0 {
                    term_v = entry.v * v_func(m, n, band, band1_matrix, prev_band_matrix);
                }
                if entry.w != 0.0 {
                    term_w = entry.w * w_func(m, n, band, band1_matrix, prev_band_matrix);
                }
                *dst = term_u + term_v + term_w;
            }
        }
    }
}

fn kronecker_delta(i: i32, j: i32) -> f32 {
    if i == j {
        1.0
    } else {
        0.0
    }
}

/// Computes the recurrence coupling term P(i, /*row*/ a, /*col*/ b, /*band*/ l) combining element
/// of the band 1 rotation matrix with the band (l - 1) rotation matrix.
#[inline(always)]
fn p_func(i: i32, a: i32, b: i32, l: i32, band1_matrix: &[f32], prev_band_matrix: &[f32]) -> f32 {
    let band1_row_offset = (i + 1) as usize * 3;
    let prev_band = (l - 1) as usize;
    let prev_band_stride = 2 * l as usize - 1;
    let prev_band_row_offset = (a + l - 1) as usize * prev_band_stride;

    if b == l {
        let band1_val_pos1 = band1_matrix[band1_row_offset + 2];
        let band1_val_neg1 = band1_matrix[band1_row_offset];
        let prev_val_pos = prev_band_matrix[prev_band_row_offset + 2 * prev_band];
        let prev_val_neg = prev_band_matrix[prev_band_row_offset];
        band1_val_pos1 * prev_val_pos - band1_val_neg1 * prev_val_neg
    } else if b == -l {
        let band1_val_pos1 = band1_matrix[band1_row_offset + 2];
        let band1_val_neg1 = band1_matrix[band1_row_offset];
        let prev_val_neg = prev_band_matrix[prev_band_row_offset];
        let prev_val_pos = prev_band_matrix[prev_band_row_offset + 2 * prev_band];
        band1_val_pos1 * prev_val_neg + band1_val_neg1 * prev_val_pos
    } else {
        let band1_val_zero = band1_matrix[band1_row_offset + 1];
        let prev_val_b = prev_band_matrix[prev_band_row_offset + (b + l - 1) as usize];
        band1_val_zero * prev_val_b
    }
}

/// Computes the U(/*row*/ m, /*col*/ n, /*band*/ l) recurrence term using the central coupling
/// P(0, m, n, l).
#[inline(always)]
fn u_func(m: i32, n: i32, l: i32, band1_matrix: &[f32], prev_band_matrix: &[f32]) -> f32 {
    p_func(0, m, n, l, band1_matrix, prev_band_matrix)
}

/// Computes the V(/*row*/ m, /*col*/ n, /*band*/ l) recurrence term combining off-diagonal P terms
/// based on the sign of `m`.
#[inline(always)]
fn v_func(m: i32, n: i32, l: i32, band1_matrix: &[f32], prev_band_matrix: &[f32]) -> f32 {
    if m == 0 {
        p_func(1, 1, n, l, band1_matrix, prev_band_matrix)
            + p_func(-1, -1, n, l, band1_matrix, prev_band_matrix)
    } else if m > 0 {
        if m == 1 {
            p_func(1, 0, n, l, band1_matrix, prev_band_matrix) * std::f32::consts::SQRT_2
        } else {
            p_func(1, m - 1, n, l, band1_matrix, prev_band_matrix)
                - p_func(-1, -m + 1, n, l, band1_matrix, prev_band_matrix)
        }
    } else if m == -1 {
        p_func(-1, 0, n, l, band1_matrix, prev_band_matrix) * std::f32::consts::SQRT_2
    } else {
        p_func(1, m + 1, n, l, band1_matrix, prev_band_matrix)
            + p_func(-1, -m - 1, n, l, band1_matrix, prev_band_matrix)
    }
}

/// Computes the W(/*row*/ m, /*col*/ n, /*band*/ l) recurrence term based on the sign of `m`.
#[inline(always)]
fn w_func(m: i32, n: i32, l: i32, band1_matrix: &[f32], prev_band_matrix: &[f32]) -> f32 {
    // `compute_band_rotation` avoids calling `w_func` when m == 0 but assert in debug for safety.
    debug_assert!(m != 0);
    if m > 0 {
        p_func(1, m + 1, n, l, band1_matrix, prev_band_matrix)
            + p_func(-1, -m - 1, n, l, band1_matrix, prev_band_matrix)
    } else {
        p_func(1, m - 1, n, l, band1_matrix, prev_band_matrix)
            - p_func(-1, -m + 1, n, l, band1_matrix, prev_band_matrix)
    }
}

/// Computes the recurrence coefficients (u, v, w) for /*row*/ `m`, /*col*/ `n`, and /*band*/ `l`.
fn compute_uvw_coeff(m: i32, n: i32, l: i32) -> (f32, f32, f32) {
    let d = kronecker_delta(m, 0);
    let denom =
        if n.abs() == l { (2 * l * (2 * l - 1)) as f32 } else { ((l + n) * (l - n)) as f32 };
    let one_over_denom = 1.0 / denom;

    let u_coeff = (((l + m) * (l - m)) as f32 * one_over_denom).sqrt();
    let v_coeff = 0.5
        * ((1.0 + d) * (l + m.abs() - 1) as f32 * (l + m.abs()) as f32 * one_over_denom).sqrt()
        * (1.0 - 2.0 * d);
    let w_coeff =
        -0.5 * (((l - m.abs() - 1) * (l - m.abs())) as f32 * one_over_denom).sqrt() * (1.0 - d);
    (u_coeff, v_coeff, w_coeff)
}

// ===== Tests =====

#[cfg(test)]
mod test {

    use super::*;
    use crate::common::definitions::{Degrees, Distance, LinearGain};
    use crate::renderer::obr::ambisonic_encoder::AmbisonicEncoder;
    use googletest::prelude::*;

    const AMBISONIC_ORDER: i32 = 3;
    const ANGLE_DEGREES: f32 = 90.0;
    const SLERP_FRAME_INTERVAL: usize = 32;
    const EPSILON: f32 = 1e-4;

    fn from_angle_axis(angle_rad: f32, axis: [f32; 3]) -> Quaternion {
        let half_angle = angle_rad * 0.5;
        let s = half_angle.sin();
        let c = half_angle.cos();
        let norm = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
        if norm > 0.0 {
            let x = axis[0] / norm * s;
            let y = axis[1] / norm * s;
            let z = axis[2] / norm * s;
            Quaternion::new(c, x, y, z).unwrap()
        } else {
            Quaternion::identity()
        }
    }

    const INITIAL_SOURCE_ANGLE: (f32, f32) = (22.0, 33.0);
    const X_ROTATED_SOURCE_ANGLE: (f32, f32) = (150.02178, 51.04152);
    const Y_ROTATED_SOURCE_ANGLE: (f32, f32) = (-35.00773, 18.310807);
    const Z_ROTATED_SOURCE_ANGLE: (f32, f32) = (112.0, 33.0);

    fn compare_rotated_and_reference_soundfields(
        input_data: &[f32],
        rotation_angle_deg: f32,
        rotation_axis: [f32; 3],
        expected_source_angle: (f32, f32),
    ) {
        let buffer_size = input_data.len();
        let mut input_buffer = AudioBuffer::new(1, buffer_size);
        input_buffer.channel_mut(0).as_mut_slice().copy_from_slice(input_data);

        let mut rotated_source_mono_codec = AmbisonicEncoder::new(1, AMBISONIC_ORDER as usize);
        rotated_source_mono_codec.set_source(
            0,
            LinearGain::ONE,
            Degrees::new(INITIAL_SOURCE_ANGLE.0).unwrap(),
            Degrees::new(INITIAL_SOURCE_ANGLE.1).unwrap(),
            Distance::ONE,
        );

        let mut reference_source_mono_codec = AmbisonicEncoder::new(1, AMBISONIC_ORDER as usize);
        reference_source_mono_codec.set_source(
            0,
            LinearGain::ONE,
            Degrees::new(expected_source_angle.0).unwrap(),
            Degrees::new(expected_source_angle.1).unwrap(),
            Distance::ONE,
        );

        let num_channels = get_num_periphonic_components(AMBISONIC_ORDER);
        let mut encoded_rotated_buffer = AudioBuffer::new(num_channels, buffer_size);
        let mut encoded_reference_buffer = AudioBuffer::new(num_channels, buffer_size);

        rotated_source_mono_codec
            .process_planar_audio_data(&input_buffer, &mut encoded_rotated_buffer);
        reference_source_mono_codec
            .process_planar_audio_data(&input_buffer, &mut encoded_reference_buffer);

        let rotation = from_angle_axis(rotation_angle_deg.to_radians(), rotation_axis);

        let mut rotator = AmbisonicRotator::new(AMBISONIC_ORDER, buffer_size);
        let result = rotator.process_in_place(&rotation, &mut encoded_rotated_buffer);
        expect_true!(result);

        for channel in 0..num_channels {
            let num_frames = encoded_rotated_buffer.num_frames();
            let interval = SLERP_FRAME_INTERVAL;
            let frames_to_compare =
                if !num_frames.is_multiple_of(interval) { num_frames % interval } else { interval };
            let start_frame = num_frames.saturating_sub(frames_to_compare);

            for frame in start_frame..num_frames {
                expect_that!(
                    encoded_rotated_buffer[channel][frame],
                    near(encoded_reference_buffer[channel][frame], EPSILON)
                );
            }
        }
    }

    #[gtest]
    fn test_rotation_threshold() {
        let num_channels = 16;
        let frames_per_buffer = 16;
        let mut buffer = AudioBuffer::new(num_channels, frames_per_buffer);
        for ch in 0..num_channels {
            buffer.channel_mut(ch).as_mut_slice().fill(1.0);
        }

        let small_rotation = Quaternion::new(1.0, 0.001, 0.001, 0.001).unwrap();
        let large_rotation = Quaternion::new(1.0, 0.1, 0.1, 0.1).unwrap();

        let mut rotator = AmbisonicRotator::new(AMBISONIC_ORDER, frames_per_buffer);

        expect_false!(rotator.process_in_place(&small_rotation, &mut buffer));
        expect_true!(rotator.process_in_place(&large_rotation, &mut buffer));
    }

    #[gtest]
    fn test_axes_rotation_long_buffer() {
        let input_data = vec![1.0; 512];
        compare_rotated_and_reference_soundfields(
            &input_data,
            ANGLE_DEGREES,
            [1.0, 0.0, 0.0],
            X_ROTATED_SOURCE_ANGLE,
        );
        compare_rotated_and_reference_soundfields(
            &input_data,
            ANGLE_DEGREES,
            [0.0, 1.0, 0.0],
            Y_ROTATED_SOURCE_ANGLE,
        );
        compare_rotated_and_reference_soundfields(
            &input_data,
            ANGLE_DEGREES,
            [0.0, 0.0, 1.0],
            Z_ROTATED_SOURCE_ANGLE,
        );
    }

    #[gtest]
    fn test_axes_rotation_short_buffer() {
        let input_data = vec![1.0; SLERP_FRAME_INTERVAL / 2];
        compare_rotated_and_reference_soundfields(
            &input_data,
            ANGLE_DEGREES,
            [1.0, 0.0, 0.0],
            X_ROTATED_SOURCE_ANGLE,
        );
        compare_rotated_and_reference_soundfields(
            &input_data,
            ANGLE_DEGREES,
            [0.0, 1.0, 0.0],
            Y_ROTATED_SOURCE_ANGLE,
        );
        compare_rotated_and_reference_soundfields(
            &input_data,
            ANGLE_DEGREES,
            [0.0, 0.0, 1.0],
            Z_ROTATED_SOURCE_ANGLE,
        );
    }

    #[gtest]
    fn test_axes_rotation_odd_buffer_size() {
        let input_data = vec![1.0; SLERP_FRAME_INTERVAL + 3];
        compare_rotated_and_reference_soundfields(
            &input_data,
            ANGLE_DEGREES,
            [1.0, 0.0, 0.0],
            X_ROTATED_SOURCE_ANGLE,
        );
        compare_rotated_and_reference_soundfields(
            &input_data,
            ANGLE_DEGREES,
            [0.0, 1.0, 0.0],
            Y_ROTATED_SOURCE_ANGLE,
        );
        compare_rotated_and_reference_soundfields(
            &input_data,
            ANGLE_DEGREES,
            [0.0, 0.0, 1.0],
            Z_ROTATED_SOURCE_ANGLE,
        );
    }

    #[gtest]
    fn test_channel_0_remains_invariant_under_rotation() {
        let order = 3;
        let num_channels = get_num_periphonic_components(order);
        let num_frames = 64;
        let rotation = from_angle_axis(45.0f32.to_radians(), [0.0, 1.0, 0.0]);
        let mut buffer = AudioBuffer::new(num_channels, num_frames);
        buffer.channel_mut(0).as_mut_slice().fill(1.5);
        let mut rotator = AmbisonicRotator::new(order, num_frames);

        rotator.process_in_place(&rotation, &mut buffer);

        expect_that!(buffer[0], each(eq(&1.5)));
        for ch in 1..num_channels {
            expect_that!(buffer[ch], each(eq(&0.0)));
        }
    }

    #[gtest]
    fn test_channels_1_to_3_conserve_energy_and_rotate_without_leakage() {
        let order = 3;
        let num_channels = get_num_periphonic_components(order);
        let num_frames = 64;
        let rotation = from_angle_axis(45.0f32.to_radians(), [0.0, 1.0, 0.0]);
        let mut buffer = AudioBuffer::new(num_channels, num_frames);
        buffer.channel_mut(1).as_mut_slice().fill(1.0);
        buffer.channel_mut(2).as_mut_slice().fill(2.0);
        buffer.channel_mut(3).as_mut_slice().fill(3.0);
        let mut rotator = AmbisonicRotator::new(order, num_frames);

        rotator.process_in_place(&rotation, &mut buffer);

        expect_that!(buffer[0], each(eq(&0.0)));
        for ch in 4..num_channels {
            expect_that!(buffer[ch], each(eq(&0.0)));
        }
        // Energy is conserved within band 1 under 3D rotation (1.0^2 + 2.0^2 + 3.0^2 = 14.0).
        for ((&c1, &c2), &c3) in buffer[1].iter().zip(&buffer[2]).zip(&buffer[3]) {
            let energy = c1 * c1 + c2 * c2 + c3 * c3;
            expect_that!(energy, near(14.0, EPSILON));
        }
        // The rotation axis has an orthogonal component to channels 1 and 2, causing them
        // to rotate away from their initial values.
        expect_that!(buffer[1], each(not(near(1.0, EPSILON))));
        expect_that!(buffer[2], each(not(near(2.0, EPSILON))));
    }

    #[gtest]
    fn first_processed_block_applies_target_rotation_in_full() {
        let frames_per_buffer = 2 * SLERP_FRAME_INTERVAL;
        let num_channels = get_num_periphonic_components(AMBISONIC_ORDER);
        let mut input_buffer = AudioBuffer::new(1, frames_per_buffer);
        input_buffer.channel_mut(0).as_mut_slice().fill(1.0);
        let mut source_mono_codec = AmbisonicEncoder::new(1, AMBISONIC_ORDER as usize);
        source_mono_codec.set_source(
            0,
            LinearGain::ONE,
            Degrees::from_const(INITIAL_SOURCE_ANGLE.0),
            Degrees::from_const(INITIAL_SOURCE_ANGLE.1),
            Distance::ONE,
        );
        let mut encoded_buffer = AudioBuffer::new(num_channels, frames_per_buffer);
        source_mono_codec.process_planar_audio_data(&input_buffer, &mut encoded_buffer);
        let mut reference_mono_codec = AmbisonicEncoder::new(1, AMBISONIC_ORDER as usize);
        reference_mono_codec.set_source(
            0,
            LinearGain::ONE,
            Degrees::from_const(Z_ROTATED_SOURCE_ANGLE.0),
            Degrees::from_const(Z_ROTATED_SOURCE_ANGLE.1),
            Distance::ONE,
        );
        let mut reference_buffer = AudioBuffer::new(num_channels, frames_per_buffer);
        reference_mono_codec.process_planar_audio_data(&input_buffer, &mut reference_buffer);
        let rotation = from_angle_axis(ANGLE_DEGREES.to_radians(), [0.0, 0.0, 1.0]);
        let mut rotator = AmbisonicRotator::new(AMBISONIC_ORDER, frames_per_buffer);

        let result = rotator.process_in_place(&rotation, &mut encoded_buffer);

        expect_true!(result);
        for channel in 0..num_channels {
            for frame in 0..frames_per_buffer {
                expect_that!(
                    encoded_buffer[channel][frame],
                    near(reference_buffer[channel][frame], EPSILON)
                );
            }
        }
    }

    #[gtest]
    fn rotation_after_bypassed_blocks_still_slerps() {
        let frames_per_buffer = 2 * SLERP_FRAME_INTERVAL;
        let num_channels = get_num_periphonic_components(AMBISONIC_ORDER);
        let mut input_buffer = AudioBuffer::new(1, frames_per_buffer);
        input_buffer.channel_mut(0).as_mut_slice().fill(1.0);
        let mut source_mono_codec = AmbisonicEncoder::new(1, AMBISONIC_ORDER as usize);
        source_mono_codec.set_source(
            0,
            LinearGain::ONE,
            Degrees::from_const(INITIAL_SOURCE_ANGLE.0),
            Degrees::from_const(INITIAL_SOURCE_ANGLE.1),
            Distance::ONE,
        );
        let mut rotated_buffer = AudioBuffer::new(num_channels, frames_per_buffer);
        source_mono_codec.process_planar_audio_data(&input_buffer, &mut rotated_buffer);
        let mut reference_mono_codec = AmbisonicEncoder::new(1, AMBISONIC_ORDER as usize);
        reference_mono_codec.set_source(
            0,
            LinearGain::ONE,
            Degrees::from_const(Z_ROTATED_SOURCE_ANGLE.0),
            Degrees::from_const(Z_ROTATED_SOURCE_ANGLE.1),
            Distance::ONE,
        );
        let mut reference_buffer = AudioBuffer::new(num_channels, frames_per_buffer);
        reference_mono_codec.process_planar_audio_data(&input_buffer, &mut reference_buffer);
        let rotation = from_angle_axis(ANGLE_DEGREES.to_radians(), [0.0, 0.0, 1.0]);
        let mut rotator = AmbisonicRotator::new(AMBISONIC_ORDER, frames_per_buffer);
        rotator.mark_audio_rendered();

        let result = rotator.process_in_place(&rotation, &mut rotated_buffer);

        expect_true!(result);
        let mut max_first_interval_difference = 0.0_f32;
        for channel in 0..num_channels {
            for frame in (frames_per_buffer - SLERP_FRAME_INTERVAL)..frames_per_buffer {
                expect_that!(
                    rotated_buffer[channel][frame],
                    near(reference_buffer[channel][frame], EPSILON)
                );
            }
            for frame in 0..SLERP_FRAME_INTERVAL {
                max_first_interval_difference = max_first_interval_difference
                    .max((rotated_buffer[channel][frame] - reference_buffer[channel][frame]).abs());
            }
        }
        expect_that!(max_first_interval_difference, gt(0.01));
    }

    #[gtest]
    fn rotation_change_after_first_block_still_slerps() {
        let frames_per_buffer = 2 * SLERP_FRAME_INTERVAL;
        let num_channels = get_num_periphonic_components(AMBISONIC_ORDER);
        let mut input_buffer = AudioBuffer::new(1, frames_per_buffer);
        input_buffer.channel_mut(0).as_mut_slice().fill(1.0);
        let mut source_mono_codec = AmbisonicEncoder::new(1, AMBISONIC_ORDER as usize);
        source_mono_codec.set_source(
            0,
            LinearGain::ONE,
            Degrees::from_const(INITIAL_SOURCE_ANGLE.0),
            Degrees::from_const(INITIAL_SOURCE_ANGLE.1),
            Distance::ONE,
        );
        let mut encoded_buffer = AudioBuffer::new(num_channels, frames_per_buffer);
        source_mono_codec.process_planar_audio_data(&input_buffer, &mut encoded_buffer);
        let mut rotated_buffer = encoded_buffer.clone();
        let mut transition_buffer = encoded_buffer.clone();
        let rotation = from_angle_axis(ANGLE_DEGREES.to_radians(), [0.0, 0.0, 1.0]);
        let mut rotator = AmbisonicRotator::new(AMBISONIC_ORDER, frames_per_buffer);

        expect_true!(rotator.process_in_place(&rotation, &mut rotated_buffer));
        expect_true!(rotator.process_in_place(&Quaternion::default(), &mut transition_buffer));

        let mut max_first_interval_difference = 0.0_f32;
        for channel in 0..num_channels {
            for frame in (frames_per_buffer - SLERP_FRAME_INTERVAL)..frames_per_buffer {
                expect_that!(
                    transition_buffer[channel][frame],
                    near(encoded_buffer[channel][frame], EPSILON)
                );
            }
            for frame in 0..SLERP_FRAME_INTERVAL {
                max_first_interval_difference = max_first_interval_difference.max(
                    (transition_buffer[channel][frame] - encoded_buffer[channel][frame]).abs(),
                );
            }
        }
        expect_that!(max_first_interval_difference, gt(0.01));
    }
}
