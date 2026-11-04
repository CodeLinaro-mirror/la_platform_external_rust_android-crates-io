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

//! Matrix rendering module for EAR spatialization.
//!
//! Provides channel multiplication by transformation matrices without heap allocation.
//! This is Rust port initially inspired by OAR's matrix rendering implementations.  This produces
//! equivalent output but uses space- and computation-saving matrix representations.  Sparse
//! matrices are used for all channel-to-channel renders and dense matrices are used for all
//! ambisonic-to-channel renders.

use crate::common::audio_buffer::simd_utils;
use crate::common::definitions::{
    OarError, PlanarBufferMut, PlanarBufferRef, MAX_OUTPUT_CHANNEL_COUNT,
};

/// Bit-packed sparse matrix entry encoding `(in_ch, out_ch, lookup_idx)` in 16 bits (`u16`).
///
/// Bit layout:
/// * Bits `0..5` (5 bits): `in_ch` (`0..32`, logical input channel index).
/// * Bits `5..10` (5 bits): `out_ch` (`0..32`, output channel index).
/// * Bits `10..16` (6 bits): `lookup_idx` (`0..64`, index into the non-zero gain lookup table).
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SparseMatrixEntry(u16);

impl SparseMatrixEntry {
    /// Packs a sparse matrix coordinate and lookup-table index into a 16-bit entry at compile
    /// time.
    ///
    /// # Parameters
    /// * `in_ch`: Logical input channel index (`0..32`).
    /// * `out_ch`: Output channel index (`0..32`).
    /// * `lookup_idx`: Index into the non-zero gain lookup table (`0..64`).
    #[inline]
    pub const fn new(in_ch: u8, out_ch: u8, lookup_idx: u8) -> Self {
        debug_assert!(in_ch < 32);
        debug_assert!(out_ch < 32);
        debug_assert!(lookup_idx < 64);
        Self((in_ch as u16) | ((out_ch as u16) << 5) | ((lookup_idx as u16) << 10))
    }

    /// Extracts the logical input channel index (`0..32`).
    #[inline]
    pub const fn in_ch(self) -> usize {
        (self.0 & 0x1F) as usize
    }

    /// Extracts the output channel index (`0..32`).
    #[inline]
    pub const fn out_ch(self) -> usize {
        ((self.0 >> 5) & 0x1F) as usize
    }

    /// Extracts the lookup-table gain index (`0..64`).
    #[inline]
    pub const fn lookup_idx(self) -> usize {
        (self.0 >> 10) as usize
    }
}

/// Shorthand compile-time constructor for [`SparseMatrixEntry::new`].
///
/// # Parameters
/// * `in_ch`: Logical input channel index (`0..32`).
/// * `out_ch`: Output channel index (`0..32`).
/// * `lookup_idx`: Index into the non-zero gain lookup table (`0..64`).
#[inline]
pub(crate) const fn s(in_ch: u8, out_ch: u8, lookup_idx: u8) -> SparseMatrixEntry {
    SparseMatrixEntry::new(in_ch, out_ch, lookup_idx)
}

/// Storage representation for transformation matrix weights.
///
/// Supports both dense `f32` slices (used by HOA matrices) and bit-packed sparse lookup-table
/// entries (used by channel-to-channel matrices to compress static tables and skip zero-weight
/// passes).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MatrixData<'a> {
    /// Dense slice of `f32` weights scaled by a uniform scalar multiplier `scale`.
    Dense {
        /// Base dense `f32` matrix weights.
        weights: &'a [f32],
        /// Scalar normalization factor applied to each weight.
        scale: f32,
    },
    /// Sparse non-zero entries paired with a lookup table of `f32` gain weights.
    Sparse {
        /// Non-zero `(in_ch, out_ch, lookup_idx)` entries.
        entries: &'a [SparseMatrixEntry],
        /// Lookup table mapping `lookup_idx` to its non-zero `f32` gain weight.
        lookup: &'a [f32],
    },
}

/// Describes the striding and mapping layout of a transformation matrix.
pub struct MatrixView<'a> {
    /// Matrix weights in dense or sparse form.
    pub data: MatrixData<'a>,
    /// Number of input channels to process (`inputs.len()`).
    pub in_dim: usize,
    /// Stride in `data` corresponding to moving to the next input channel column (`in_next`).
    pub in_stride: usize,
    /// Number of output channels to process (`outputs.len()`).
    pub out_dim: usize,
    /// Stride in `data` corresponding to moving to the next output channel row (`out_next`).
    pub out_stride: usize,
    /// Optional slice mapping physical input channel index to logical matrix column index.
    pub in_idx_map: Option<&'a [usize]>,
}

/// Accumulates dense `weights` scaled by `scale` across all `(in_idx, out_idx)` pairs into
/// zero-initialized `out_slices`.
fn multiply_dense_matrix(
    weights: &[f32],
    scale: f32,
    matrix: &MatrixView<'_>,
    inputs: PlanarBufferRef<'_, '_>,
    out_slices: &mut [&mut [f32]],
) -> Result<(), OarError> {
    if scale == 0.0 {
        return Ok(());
    }
    for in_idx in 0..matrix.in_dim {
        let in_slice = inputs.channel(in_idx);
        let in_mapped_idx = match matrix.in_idx_map {
            Some(map) => *map.get(in_idx).ok_or(OarError::InvalidParameter)?,
            None => in_idx,
        };
        let col_offset = in_mapped_idx * matrix.in_stride;

        for (out_idx, out_slice) in out_slices.iter_mut().enumerate().take(matrix.out_dim) {
            let mat_idx = out_idx * matrix.out_stride + col_offset;
            if let Some(&raw_c) = weights.get(mat_idx)
                && raw_c != 0.0
            {
                simd_utils::scalar_multiply_and_accumulate(raw_c * scale, in_slice, out_slice);
            }
        }
    }
    Ok(())
}

/// Accumulates only the non-zero `entries` from a sparse matrix into zero-initialized
/// `out_slices`.
fn multiply_sparse_matrix(
    entries: &[SparseMatrixEntry],
    lookup: &[f32],
    matrix: &MatrixView<'_>,
    inputs: PlanarBufferRef<'_, '_>,
    out_slices: &mut [&mut [f32]],
) {
    match matrix.in_idx_map {
        None => {
            for &entry in entries {
                let in_idx = entry.in_ch();
                let out_idx = entry.out_ch();
                if in_idx < matrix.in_dim
                    && out_idx < matrix.out_dim
                    && let Some(&c) = lookup.get(entry.lookup_idx())
                {
                    simd_utils::scalar_multiply_and_accumulate(
                        c,
                        inputs.channel(in_idx),
                        out_slices[out_idx],
                    );
                }
            }
        }
        Some(map) => {
            for (in_idx, &in_mapped_idx) in map.iter().enumerate().take(matrix.in_dim) {
                let in_slice = inputs.channel(in_idx);
                for &entry in entries {
                    if entry.in_ch() == in_mapped_idx {
                        let out_idx = entry.out_ch();
                        if out_idx < matrix.out_dim
                            && let Some(&c) = lookup.get(entry.lookup_idx())
                        {
                            simd_utils::scalar_multiply_and_accumulate(
                                c,
                                in_slice,
                                out_slices[out_idx],
                            );
                        }
                    }
                }
            }
        }
    }
}

/// Multiplies planar input channels by a transformation matrix and accumulates into output
/// channels.
///
/// Port of `multiply_channels_by_matrix_c` from `matrix_render_x86.c`.
///
/// # Parameters
/// * `matrix`: Configuration descriptor describing matrix weights, mapping, and striding
///   dimensions.
/// * `inputs`: Validated read-only view of planar input channels.
/// * `outputs`: Validated mutable view of planar output channels to be overwritten with accumulated
///   products.
pub fn multiply_channels_by_matrix_c(
    matrix: &MatrixView<'_>,
    inputs: PlanarBufferRef<'_, '_>,
    outputs: &mut PlanarBufferMut<'_, '_>,
) -> Result<(), OarError> {
    if matrix.in_dim == 0 || matrix.out_dim == 0 {
        return Ok(());
    }
    if inputs.num_channels() < matrix.in_dim || outputs.num_channels() < matrix.out_dim {
        return Err(OarError::InvalidParameter);
    }
    if let Some(map) = matrix.in_idx_map
        && map.len() < matrix.in_dim
    {
        return Err(OarError::InvalidParameter);
    }
    if outputs.num_samples() != inputs.num_samples() {
        return Err(OarError::InvalidParameter);
    }

    let num_out_channels = outputs.num_channels();
    let mut out_slices: [&mut [f32]; MAX_OUTPUT_CHANNEL_COUNT] =
        std::array::from_fn(|_| &mut [] as &mut [f32]);
    if num_out_channels > MAX_OUTPUT_CHANNEL_COUNT {
        return Err(OarError::InvalidParameter);
    }
    outputs.as_slices_mut(&mut out_slices[..num_out_channels]);

    // Initialize outputs to 0.0
    for out_slice in out_slices.iter_mut().take(matrix.out_dim) {
        out_slice.fill(0.0);
    }

    match matrix.data {
        MatrixData::Dense { weights, scale } => {
            multiply_dense_matrix(
                weights,
                scale,
                matrix,
                inputs,
                &mut out_slices[..matrix.out_dim],
            )?;
        }
        MatrixData::Sparse { entries, lookup } => {
            multiply_sparse_matrix(
                entries,
                lookup,
                matrix,
                inputs,
                &mut out_slices[..matrix.out_dim],
            );
        }
    }
    Ok(())
}

/// Dispatches multi-channel matrix rendering to the underlying implementation.
///
/// Port of `matrix_render` from `liboar/src/renderer/ear/matrix_render.c`.
/// Currently delegates directly to [`multiply_channels_by_matrix_c`] as SIMD
/// optimizations (`NEON`/`x86`) are bypassed for initial porting.
///
/// # Parameters
///
/// * `matrix`: Configuration descriptor describing matrix weights, mapping, and striding
///   dimensions.
/// * `inputs`: Validated read-only view of planar input channels.
/// * `outputs`: Validated mutable view of planar output channels to be accumulated or overwritten.
pub fn matrix_render(
    matrix: &MatrixView<'_>,
    inputs: PlanarBufferRef<'_, '_>,
    outputs: &mut PlanarBufferMut<'_, '_>,
) -> Result<(), OarError> {
    multiply_channels_by_matrix_c(matrix, inputs, outputs)
}

// ===== Tests =====

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::units::Samples;
    use googletest::prelude::*;

    #[gtest]
    fn multiply_channels_by_matrix_multiple_inputs_no_map_clears_first_and_accumulates_second() {
        let mat = [1.0f32, 2.0f32, 3.0f32, 4.0f32];
        let in_left = [10.0f32, 20.0f32];
        let in_right = [5.0f32, 10.0f32];
        let mut out_left = [999.0f32, 999.0f32];
        let mut out_right = [888.0f32, 888.0f32];
        let inputs = [&in_left[..], &in_right[..]];
        let mut out_slices = [&mut out_left[..], &mut out_right[..]];
        let samples_2 = Samples::new(2).unwrap();
        let inputs_buf = PlanarBufferRef::new(&inputs, 2, samples_2).unwrap();
        let mut outputs_buf = PlanarBufferMut::new(&mut out_slices, 2, samples_2).unwrap();

        let mv = MatrixView {
            data: MatrixData::Dense { weights: &mat, scale: 1.0 },
            in_dim: 2,
            in_stride: 1,
            out_dim: 2,
            out_stride: 2,
            in_idx_map: None,
        };

        let result = multiply_channels_by_matrix_c(&mv, inputs_buf, &mut outputs_buf);

        expect_ok!(result);
        expect_that!(out_left[0], eq(20.0f32));
        expect_that!(out_left[1], eq(40.0f32));
        expect_that!(out_right[0], eq(50.0f32));
        expect_that!(out_right[1], eq(100.0f32));
    }

    #[gtest]
    fn multiply_channels_by_matrix_with_channel_map_maps_physical_input_to_logical_column() {
        let mat = [10.0f32, 20.0f32];
        let map = [1usize];
        let in_channel = [2.0f32];
        let mut out_channel = [100.0f32];
        let inputs = [&in_channel[..]];
        let mut out_slices = [&mut out_channel[..]];
        let samples_1 = Samples::new(1).unwrap();
        let inputs_buf = PlanarBufferRef::new(&inputs, 1, samples_1).unwrap();
        let mut outputs_buf = PlanarBufferMut::new(&mut out_slices, 1, samples_1).unwrap();
        let mv = MatrixView {
            data: MatrixData::Dense { weights: &mat, scale: 1.0 },
            in_dim: 1,
            in_stride: 1,
            out_dim: 1,
            out_stride: 2,
            in_idx_map: Some(&map),
        };

        let result = multiply_channels_by_matrix_c(&mv, inputs_buf, &mut outputs_buf);

        expect_ok!(result);
        expect_that!(out_channel[0], eq(40.0f32));
    }

    #[gtest]
    fn multiply_channels_by_matrix_zero_dimensions_does_not_modify_output() {
        let mat = [1.0f32];
        let in_channel = [10.0f32];
        let mut out_channel = [42.0f32];
        let inputs = [&in_channel[..]];
        let mut out_slices = [&mut out_channel[..]];
        let samples_1 = Samples::new(1).unwrap();
        let inputs_buf = PlanarBufferRef::new(&inputs, 1, samples_1).unwrap();
        let mut outputs_buf = PlanarBufferMut::new(&mut out_slices, 1, samples_1).unwrap();
        let mv = MatrixView {
            data: MatrixData::Dense { weights: &mat, scale: 1.0 },
            in_dim: 0,
            in_stride: 1,
            out_dim: 1,
            out_stride: 1,
            in_idx_map: None,
        };

        let result = multiply_channels_by_matrix_c(&mv, inputs_buf, &mut outputs_buf);

        expect_ok!(result);
        expect_that!(out_channel[0], eq(42.0f32));
    }

    #[gtest]
    fn multiply_channels_by_matrix_processes_expected_samples() {
        let mat = [2.0f32];
        let in_channel = [3.0f32, 4.0f32];
        let mut out_channel = [0.0f32; 2];
        let inputs = [&in_channel[..]];
        let mut out_slices = [&mut out_channel[..]];
        let samples_2 = Samples::new(2).unwrap();
        let inputs_buf = PlanarBufferRef::new(&inputs, 1, samples_2).unwrap();
        let mut outputs_buf = PlanarBufferMut::new(&mut out_slices, 1, samples_2).unwrap();
        let mv = MatrixView {
            data: MatrixData::Dense { weights: &mat, scale: 1.0 },
            in_dim: 1,
            in_stride: 1,
            out_dim: 1,
            out_stride: 1,
            in_idx_map: None,
        };

        let result = multiply_channels_by_matrix_c(&mv, inputs_buf, &mut outputs_buf);

        expect_ok!(result);
        expect_that!(out_channel[0], eq(6.0f32));
        expect_that!(out_channel[1], eq(8.0f32));
    }

    #[gtest]
    fn matrix_render_dispatches_to_accumulation_produces_expected_scaled_outputs() {
        let mat = [3.0f32, -1.0f32];
        let in_first = [4.0f32];
        let in_second = [2.0f32];
        let mut out_channel = [0.0f32];
        let inputs = [&in_first[..], &in_second[..]];
        let mut out_slices = [&mut out_channel[..]];
        let samples_1 = Samples::new(1).unwrap();
        let inputs_buf = PlanarBufferRef::new(&inputs, 2, samples_1).unwrap();
        let mut outputs_buf = PlanarBufferMut::new(&mut out_slices, 1, samples_1).unwrap();
        let mv = MatrixView {
            data: MatrixData::Dense { weights: &mat, scale: 1.0 },
            in_dim: 2,
            in_stride: 1,
            out_dim: 1,
            out_stride: 2,
            in_idx_map: None,
        };

        let result = matrix_render(&mv, inputs_buf, &mut outputs_buf);

        expect_ok!(result);
        expect_that!(out_channel[0], eq(10.0f32));
    }

    #[gtest]
    fn sparse_matrix_entry_new_packs_and_extracts_coordinates_and_lookup_index() {
        let entry = SparseMatrixEntry::new(23, 17, 7);

        let in_ch = entry.in_ch();
        let out_ch = entry.out_ch();
        let lookup_idx = entry.lookup_idx();

        expect_that!(in_ch, eq(23));
        expect_that!(out_ch, eq(17));
        expect_that!(lookup_idx, eq(7));
    }

    #[gtest]
    fn multiply_channels_by_matrix_sparse_resolves_lookup_weights_and_accumulates() {
        let lookup = [2.0f32, 3.0f32];
        let entries = [
            SparseMatrixEntry::new(0, 0, 0),
            SparseMatrixEntry::new(0, 1, 1),
            SparseMatrixEntry::new(1, 0, 1),
        ];
        let in_left = [10.0f32];
        let in_right = [4.0f32];
        let mut out_left = [0.0f32];
        let mut out_right = [0.0f32];
        let inputs = [&in_left[..], &in_right[..]];
        let mut out_slices = [&mut out_left[..], &mut out_right[..]];
        let samples_1 = Samples::new(1).unwrap();
        let inputs_buf = PlanarBufferRef::new(&inputs, 2, samples_1).unwrap();
        let mut outputs_buf = PlanarBufferMut::new(&mut out_slices, 2, samples_1).unwrap();
        let mv = MatrixView {
            data: MatrixData::Sparse { entries: &entries, lookup: &lookup },
            in_dim: 2,
            in_stride: 1,
            out_dim: 2,
            out_stride: 2,
            in_idx_map: None,
        };

        let result = multiply_channels_by_matrix_c(&mv, inputs_buf, &mut outputs_buf);

        expect_ok!(result);
        expect_that!(out_left[0], eq(32.0f32));
        expect_that!(out_right[0], eq(30.0f32));
    }

    #[gtest]
    fn multiply_channels_by_matrix_sparse_with_channel_map_routes_mapped_input() {
        let lookup = [5.0f32, 7.0f32];
        let entries = [SparseMatrixEntry::new(0, 0, 0), SparseMatrixEntry::new(2, 1, 1)];
        let map = [2usize];
        let in_channel = [3.0f32];
        let mut out_left = [99.0f32];
        let mut out_right = [99.0f32];
        let inputs = [&in_channel[..]];
        let mut out_slices = [&mut out_left[..], &mut out_right[..]];
        let samples_1 = Samples::new(1).unwrap();
        let inputs_buf = PlanarBufferRef::new(&inputs, 1, samples_1).unwrap();
        let mut outputs_buf = PlanarBufferMut::new(&mut out_slices, 2, samples_1).unwrap();
        let mv = MatrixView {
            data: MatrixData::Sparse { entries: &entries, lookup: &lookup },
            in_dim: 1,
            in_stride: 1,
            out_dim: 2,
            out_stride: 1,
            in_idx_map: Some(&map),
        };

        let result = multiply_channels_by_matrix_c(&mv, inputs_buf, &mut outputs_buf);

        expect_ok!(result);
        expect_that!(out_left[0], eq(0.0f32));
        expect_that!(out_right[0], eq(21.0f32));
    }

    #[gtest]
    fn multiply_channels_by_matrix_dense_with_scale_scales_weights_and_accumulates() {
        let mat = [2.0f32, 3.0f32, 99.0f32, 4.0f32, 5.0f32, 99.0f32];
        let in_ch0 = [10.0f32];
        let in_ch1 = [2.0f32];
        let mut out_ch0 = [0.0f32];
        let mut out_ch1 = [0.0f32];
        let inputs = [&in_ch0[..], &in_ch1[..]];
        let mut out_slices = [&mut out_ch0[..], &mut out_ch1[..]];
        let samples_1 = Samples::new(1).unwrap();
        let inputs_buf = PlanarBufferRef::new(&inputs, 2, samples_1).unwrap();
        let mut outputs_buf = PlanarBufferMut::new(&mut out_slices, 2, samples_1).unwrap();
        let mv = MatrixView {
            data: MatrixData::Dense { weights: &mat, scale: 1.5 },
            in_dim: 2,
            in_stride: 1,
            out_dim: 2,
            out_stride: 3,
            in_idx_map: None,
        };

        let result = multiply_channels_by_matrix_c(&mv, inputs_buf, &mut outputs_buf);

        expect_ok!(result);
        expect_that!(out_ch0[0], eq(39.0f32));
        expect_that!(out_ch1[0], eq(75.0f32));
    }
}
