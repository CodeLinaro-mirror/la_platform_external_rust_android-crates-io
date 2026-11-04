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
#![allow(clippy::excessive_precision)]

//! Multi-channel to multi-channel renderer and pre-computed gain matrices.
//!
//! Direct Rust port of `liboar/src/renderer/ear/m2m_rdr.c` and `ae_rdr.h`.

use crate::common::definitions::{
    InputLayout, OarError, OutputLayout, PlanarBufferMut, PlanarBufferRef,
};
use crate::renderer::ear::matrix_renderer::{
    matrix_render, s, MatrixData, MatrixView, SparseMatrixEntry,
};
use std::f32::consts::FRAC_1_SQRT_2;

/// Channel-to-channel matrix entry defining input/output systems and sparse non-zero gain
/// routing (`m2m_rdr_t`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelToChannelRenderer {
    pub in_system: InputLayout,
    pub out_system: OutputLayout,
    /// Non-zero `(in_ch, out_ch, lookup_idx)` entries indexing into [`CHAN_TO_CHAN_GAIN_LOOKUP`].
    pub mat: &'static [SparseMatrixEntry],
    /// Number of input channels.
    pub m: usize,
    /// Number of output channels.
    pub n: usize,
}

// The gain matrices below are generated to comply with EAR Direct Speakers
// renderer ([ITU2127-0]), except for 3.1.2ch and 7.1.2ch. When playback layout
// is 3.1.2ch and 7.1.2ch, the gain matrices follow 7.3.2.1 of IAMF. When input
// layout is 3.1.2ch and 7.1.2ch, the gain matrices are generated with the
// processing below. 3.1.2ch -> 5.1.2ch -> Playback layout 7.1.2ch -> 7.1.4ch ->
// Playback layout

/// Lookup table of the 8 unique non-zero `f32` gain coefficients used across all
/// channel-to-channel conversion matrices.
///
/// Zero weights (`0.0`, which account for 89.3% of dense matrix cells) are omitted by the
/// sparse representation. Combining 2-byte [`SparseMatrixEntry`] values with this 8-entry
/// table compresses the 18,839 matrix weights from 75,356 bytes to 4,062 bytes (18.6x
/// reduction) without any loss of precision.
pub static CHAN_TO_CHAN_GAIN_LOOKUP: [f32; 8] = [
    1.0,                       // 0: 1
    0.5,                       // 1: 1/2
    FRAC_1_SQRT_2,             // 2: 1/sqrt(2)
    0.57735026919,             // 3: 1/sqrt(3)
    0.81649658092,             // 4: sqrt(2/3)
    FRAC_1_SQRT_2 / 2.0,       // 5: 1/(2*sqrt(2))
    0.5 + FRAC_1_SQRT_2,       // 6: 1/2 + 1/sqrt(2)
    0.5 + FRAC_1_SQRT_2 / 2.0, // 7: 1/2 + 1/(2*sqrt(2))
];

#[rustfmt::skip]
pub static MONO_BS020: [SparseMatrixEntry; 2] = [
    s(0, 0, 2), s(0, 1, 2)
];
#[rustfmt::skip]
pub static MONO_BS050: [SparseMatrixEntry; 1] = [
    s(0, 2, 0)
];
#[rustfmt::skip]
pub static MONO_BS250: [SparseMatrixEntry; 1] = [
    s(0, 2, 0)
];
#[rustfmt::skip]
pub static MONO_BS450: [SparseMatrixEntry; 1] = [
    s(0, 2, 0)
];
#[rustfmt::skip]
pub static MONO_BS451: [SparseMatrixEntry; 1] = [
    s(0, 2, 0)
];
#[rustfmt::skip]
pub static MONO_BS370: [SparseMatrixEntry; 1] = [
    s(0, 0, 0)
];
#[rustfmt::skip]
pub static MONO_BS490: [SparseMatrixEntry; 1] = [
    s(0, 2, 0)
];
#[rustfmt::skip]
pub static MONO_BS9A3: [SparseMatrixEntry; 1] = [
    s(0, 2, 0)
];
#[rustfmt::skip]
pub static MONO_BS070: [SparseMatrixEntry; 1] = [
    s(0, 2, 0)
];
#[rustfmt::skip]
pub static MONO_BS470: [SparseMatrixEntry; 1] = [
    s(0, 2, 0)
];
#[rustfmt::skip]
pub static MONO_IAMF312: [SparseMatrixEntry; 1] = [
    s(0, 2, 0)
];
#[rustfmt::skip]
pub static MONO_IAMF712: [SparseMatrixEntry; 1] = [
    s(0, 2, 0)
];
#[rustfmt::skip]
pub static MONO_MONO: [SparseMatrixEntry; 1] = [
    s(0, 0, 0)
];
#[rustfmt::skip]
pub static MONO_IAMF916: [SparseMatrixEntry; 1] = [
    s(0, 2, 0)
];
///////////////////////////////

// 'M+030', 'M-030'
#[rustfmt::skip]
pub static STEREO_BS020: [SparseMatrixEntry; 2] = [
    s(0, 0, 0), s(1, 1, 0)
];
// 'M+030', 'M-030', 'M+000', 'LFE1', 'M+110', 'M-110'
#[rustfmt::skip]
pub static STEREO_BS050: [SparseMatrixEntry; 2] = [
    s(0, 0, 0), s(1, 1, 0)
];
// 'M+030', 'M-030', 'M+000', 'LFE1', 'M+110', 'M-110', 'U+030', 'U-030'
#[rustfmt::skip]
pub static STEREO_BS250: [SparseMatrixEntry; 2] = [
    s(0, 0, 0), s(1, 1, 0)
];
// 'M+030', 'M-030', 'M+000', 'LFE1', 'M+110', 'M-110', 'U+030', 'U-030',
// 'U+110', 'U-110'
#[rustfmt::skip]
pub static STEREO_BS450: [SparseMatrixEntry; 2] = [
    s(0, 0, 0), s(1, 1, 0)
];
// 'M+030', 'M-030', 'M+000', 'LFE1', 'M+110', 'M-110', 'U+030', 'U-030',
// 'U+110', 'U-110', 'B+000'
#[rustfmt::skip]
pub static STEREO_BS451: [SparseMatrixEntry; 2] = [
    s(0, 0, 0), s(1, 1, 0)
];
// 'M+000', 'M+030', 'M-030', 'U+045', 'U-045', 'M+090', 'M-090', 'M+135',
// 'M-135', 'UH+180', 'LFE1', 'LFE2'
#[rustfmt::skip]
pub static STEREO_BS370: [SparseMatrixEntry; 2] = [
    s(0, 1, 0), s(1, 2, 0)
];
// 'M+030', 'M-030', 'M+000', 'LFE1', 'M+090', 'M-090', 'M+135', 'M-135',
// 'U+045', 'U-045', 'U+135', 'U-135', 'M+SC', 'M-SC'
#[rustfmt::skip]
pub static STEREO_BS490: [SparseMatrixEntry; 2] = [
    s(0, 0, 0), s(1, 1, 0)
];
// 'M+060', 'M-060', 'M+000', 'LFE1', 'M+135', 'M-135', 'M+030', 'M-030',
// 'M+180', 'LFE2', 'M+090', 'M-090', 'U+045', 'U-045', 'U+000', 'T+000',
// 'U+135', 'U-135', 'U+090', 'U-090', 'U+180', 'B+000', 'B+045', 'B-045'
#[rustfmt::skip]
pub static STEREO_BS9A3: [SparseMatrixEntry; 2] = [
    s(0, 6, 0), s(1, 7, 0)
];
// 'M+030', 'M-030', 'M+000', 'LFE1', 'M+090', 'M-090', 'M+135', 'M-135'
#[rustfmt::skip]
pub static STEREO_BS070: [SparseMatrixEntry; 2] = [
    s(0, 0, 0), s(1, 1, 0)
];
// 'M+030', 'M-030', 'M+000', 'LFE1', 'M+090', 'M-090', 'M+135', 'M-135',
// 'U+045', 'U-045', 'U+135', 'U-135'
#[rustfmt::skip]
pub static STEREO_BS470: [SparseMatrixEntry; 2] = [
    s(0, 0, 0), s(1, 1, 0)
];
// 'M+030', 'M-030', 'M+000', 'LFE1', 'U+030', 'U-030'
#[rustfmt::skip]
pub static STEREO_IAMF312: [SparseMatrixEntry; 2] = [
    s(0, 0, 0), s(1, 1, 0)
];
// 'M+030', 'M-030', 'M+000', 'LFE1', 'M+090', 'M-090', 'M+135', 'M-135',
// 'U+045', 'U-045'
#[rustfmt::skip]
pub static STEREO_IAMF712: [SparseMatrixEntry; 2] = [
    s(0, 0, 0), s(1, 1, 0)
];
#[rustfmt::skip]
pub static STEREO_MONO: [SparseMatrixEntry; 2] = [
    s(0, 0, 1), s(1, 0, 1)
];
#[rustfmt::skip]
pub static STEREO_IAMF916: [SparseMatrixEntry; 2] = [
    s(0, 6, 0), s(1, 7, 0)
];
///////////////////////////

#[rustfmt::skip]
pub static IAMF51_BS020: [SparseMatrixEntry; 6] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 0, 2), s(2, 1, 2), s(4, 0, 2), s(5, 1, 2)
];
#[rustfmt::skip]
pub static IAMF51_BS050: [SparseMatrixEntry; 6] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0)
];
#[rustfmt::skip]
pub static IAMF51_BS250: [SparseMatrixEntry; 6] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0)
];
#[rustfmt::skip]
pub static IAMF51_BS450: [SparseMatrixEntry; 6] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0)
];
#[rustfmt::skip]
pub static IAMF51_BS451: [SparseMatrixEntry; 6] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0)
];
#[rustfmt::skip]
pub static IAMF51_BS370: [SparseMatrixEntry; 6] = [
    s(0, 1, 0), s(1, 2, 0), s(2, 0, 0), s(3, 10, 0), s(4, 7, 0), s(5, 8, 0)
];
#[rustfmt::skip]
pub static IAMF51_BS490: [SparseMatrixEntry; 6] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 6, 0), s(5, 7, 0)
];
#[rustfmt::skip]
pub static IAMF51_BS9A3: [SparseMatrixEntry; 6] = [
    s(0, 6, 0), s(1, 7, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0)
];
#[rustfmt::skip]
pub static IAMF51_BS070: [SparseMatrixEntry; 6] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 6, 0), s(5, 7, 0)
];
#[rustfmt::skip]
pub static IAMF51_BS470: [SparseMatrixEntry; 6] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 6, 0), s(5, 7, 0)
];
#[rustfmt::skip]
pub static IAMF51_IAMF312: [SparseMatrixEntry; 6] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 0, 2), s(5, 1, 2)
];
#[rustfmt::skip]
pub static IAMF51_IAMF712: [SparseMatrixEntry; 6] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 6, 0), s(5, 7, 0)
];
#[rustfmt::skip]
pub static IAMF51_MONO: [SparseMatrixEntry; 5] = [
    s(0, 0, 1), s(1, 0, 1), s(2, 0, 2), s(4, 0, 5), s(5, 0, 5)
];
#[rustfmt::skip]
pub static IAMF51_IAMF916: [SparseMatrixEntry; 6] = [
    s(0, 6, 0), s(1, 7, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0)
];
//////////////////////

#[rustfmt::skip]
pub static IAMF512_BS020: [SparseMatrixEntry; 8] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 0, 2), s(2, 1, 2), s(4, 0, 2), s(5, 1, 2), s(6, 0, 0), s(7, 1, 0)
];
#[rustfmt::skip]
pub static IAMF512_BS050: [SparseMatrixEntry; 8] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0), s(6, 0, 0), s(7, 1, 0)
];
#[rustfmt::skip]
pub static IAMF512_BS250: [SparseMatrixEntry; 8] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0), s(6, 6, 0), s(7, 7, 0)
];
#[rustfmt::skip]
pub static IAMF512_BS450: [SparseMatrixEntry; 8] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0), s(6, 6, 0), s(7, 7, 0)
];
#[rustfmt::skip]
pub static IAMF512_BS451: [SparseMatrixEntry; 8] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0), s(6, 6, 0), s(7, 7, 0)
];
#[rustfmt::skip]
pub static IAMF512_BS370: [SparseMatrixEntry; 8] = [
    s(0, 1, 0), s(1, 2, 0), s(2, 0, 0), s(3, 10, 0), s(4, 7, 0), s(5, 8, 0), s(6, 3, 0), s(7, 4, 0)
];
#[rustfmt::skip]
pub static IAMF512_BS490: [SparseMatrixEntry; 8] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 6, 0), s(5, 7, 0), s(6, 8, 0), s(7, 9, 0)
];
#[rustfmt::skip]
pub static IAMF512_BS9A3: [SparseMatrixEntry; 8] = [
    s(0, 6, 0), s(1, 7, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0), s(6, 12, 0), s(7, 13, 0)
];
#[rustfmt::skip]
pub static IAMF512_BS070: [SparseMatrixEntry; 8] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 6, 0), s(5, 7, 0), s(6, 0, 0), s(7, 1, 0)
];
#[rustfmt::skip]
pub static IAMF512_BS470: [SparseMatrixEntry; 8] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 6, 0), s(5, 7, 0), s(6, 8, 0), s(7, 9, 0)
];
#[rustfmt::skip]
pub static IAMF512_IAMF312: [SparseMatrixEntry; 8] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 0, 2), s(5, 1, 2), s(6, 4, 0), s(7, 5, 0)
];
#[rustfmt::skip]
pub static IAMF512_IAMF712: [SparseMatrixEntry; 8] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 6, 0), s(5, 7, 0), s(6, 8, 0), s(7, 9, 0)
];
#[rustfmt::skip]
pub static IAMF512_MONO: [SparseMatrixEntry; 7] = [
    s(0, 0, 1), s(1, 0, 1), s(2, 0, 2), s(4, 0, 5), s(5, 0, 5), s(6, 0, 1), s(7, 0, 1)
];
#[rustfmt::skip]
pub static IAMF512_IAMF916: [SparseMatrixEntry; 8] = [
    s(0, 6, 0), s(1, 7, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0), s(6, 10, 0), s(7, 11, 0)
];
////////////////////////////////////////////////////

#[rustfmt::skip]
pub static IAMF514_BS020: [SparseMatrixEntry; 10] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 0, 2), s(2, 1, 2), s(4, 0, 2), s(5, 1, 2), s(6, 0, 0), s(7, 1, 0),
    s(8, 0, 2), s(9, 1, 2)
];
#[rustfmt::skip]
pub static IAMF514_BS050: [SparseMatrixEntry; 10] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0), s(6, 0, 0), s(7, 1, 0),
    s(8, 4, 0), s(9, 5, 0)
];
#[rustfmt::skip]
pub static IAMF514_BS250: [SparseMatrixEntry; 10] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0), s(6, 6, 0), s(7, 7, 0),
    s(8, 4, 0), s(9, 5, 0)
];
#[rustfmt::skip]
pub static IAMF514_BS450: [SparseMatrixEntry; 10] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0), s(6, 6, 0), s(7, 7, 0),
    s(8, 8, 0), s(9, 9, 0)
];
#[rustfmt::skip]
pub static IAMF514_BS451: [SparseMatrixEntry; 10] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0), s(6, 6, 0), s(7, 7, 0),
    s(8, 8, 0), s(9, 9, 0)
];
#[rustfmt::skip]
pub static IAMF514_BS370: [SparseMatrixEntry; 12] = [
    s(0, 1, 0), s(1, 2, 0), s(2, 0, 0), s(3, 10, 0), s(4, 7, 0), s(5, 8, 0), s(6, 3, 0), s(7, 4, 0),
    s(8, 3, 2), s(8, 9, 2), s(9, 4, 2), s(9, 9, 2)
];
#[rustfmt::skip]
pub static IAMF514_BS490: [SparseMatrixEntry; 10] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 6, 0), s(5, 7, 0), s(6, 8, 0), s(7, 9, 0),
    s(8, 10, 0), s(9, 11, 0)
];
#[rustfmt::skip]
pub static IAMF514_BS9A3: [SparseMatrixEntry; 10] = [
    s(0, 6, 0), s(1, 7, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0), s(6, 12, 0), s(7, 13, 0),
    s(8, 16, 0), s(9, 17, 0)
];
#[rustfmt::skip]
pub static IAMF514_BS070: [SparseMatrixEntry; 10] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 6, 0), s(5, 7, 0), s(6, 0, 0), s(7, 1, 0),
    s(8, 6, 0), s(9, 7, 0)
];
#[rustfmt::skip]
pub static IAMF514_BS470: [SparseMatrixEntry; 10] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 6, 0), s(5, 7, 0), s(6, 8, 0), s(7, 9, 0),
    s(8, 10, 0), s(9, 11, 0)
];
#[rustfmt::skip]
pub static IAMF514_IAMF312: [SparseMatrixEntry; 10] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 0, 2), s(5, 1, 2), s(6, 4, 0), s(7, 5, 0),
    s(8, 4, 2), s(9, 5, 2)
];
// 5.1.4->7.1.4->7.1.2
#[rustfmt::skip]
pub static IAMF514_IAMF712: [SparseMatrixEntry; 10] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 6, 0), s(5, 7, 0), s(6, 8, 0), s(7, 9, 0),
    s(8, 8, 2), s(9, 9, 2)
];
#[rustfmt::skip]
pub static IAMF514_MONO: [SparseMatrixEntry; 9] = [
    s(0, 0, 1), s(1, 0, 1), s(2, 0, 2), s(4, 0, 5), s(5, 0, 5), s(6, 0, 1), s(7, 0, 1), s(8, 0, 5),
    s(9, 0, 5)
];
#[rustfmt::skip]
pub static IAMF514_IAMF916: [SparseMatrixEntry; 10] = [
    s(0, 6, 0), s(1, 7, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0), s(6, 10, 0), s(7, 11, 0),
    s(8, 12, 0), s(9, 13, 0)
];
/////////////////////////////////////

#[rustfmt::skip]
pub static IAMF71_BS020: [SparseMatrixEntry; 8] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 0, 2), s(2, 1, 2), s(4, 0, 2), s(5, 1, 2), s(6, 0, 2), s(7, 1, 2)
];
#[rustfmt::skip]
pub static IAMF71_BS050: [SparseMatrixEntry; 10] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 0, 2), s(4, 4, 2), s(5, 1, 2), s(5, 5, 2),
    s(6, 4, 0), s(7, 5, 0)
];
#[rustfmt::skip]
pub static IAMF71_BS250: [SparseMatrixEntry; 10] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 0, 2), s(4, 4, 2), s(5, 1, 2), s(5, 5, 2),
    s(6, 4, 0), s(7, 5, 0)
];
#[rustfmt::skip]
pub static IAMF71_BS450: [SparseMatrixEntry; 10] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 0, 2), s(4, 4, 2), s(5, 1, 2), s(5, 5, 2),
    s(6, 4, 0), s(7, 5, 0)
];
#[rustfmt::skip]
pub static IAMF71_BS451: [SparseMatrixEntry; 10] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 0, 2), s(4, 4, 2), s(5, 1, 2), s(5, 5, 2),
    s(6, 4, 0), s(7, 5, 0)
];
#[rustfmt::skip]
pub static IAMF71_BS370: [SparseMatrixEntry; 8] = [
    s(0, 1, 0), s(1, 2, 0), s(2, 0, 0), s(3, 10, 0), s(4, 5, 0), s(5, 6, 0), s(6, 7, 0), s(7, 8, 0)
];
#[rustfmt::skip]
pub static IAMF71_BS490: [SparseMatrixEntry; 8] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0), s(6, 6, 0), s(7, 7, 0)
];
#[rustfmt::skip]
pub static IAMF71_BS9A3: [SparseMatrixEntry; 8] = [
    s(0, 6, 0), s(1, 7, 0), s(2, 2, 0), s(3, 3, 0), s(4, 10, 0), s(5, 11, 0), s(6, 4, 0), s(7, 5, 0)
];
#[rustfmt::skip]
pub static IAMF71_BS070: [SparseMatrixEntry; 8] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0), s(6, 6, 0), s(7, 7, 0)
];
#[rustfmt::skip]
pub static IAMF71_BS470: [SparseMatrixEntry; 8] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0), s(6, 6, 0), s(7, 7, 0)
];
#[rustfmt::skip]
pub static IAMF71_IAMF312: [SparseMatrixEntry; 8] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 0, 2), s(5, 1, 2), s(6, 0, 2), s(7, 1, 2)
];
#[rustfmt::skip]
pub static IAMF71_IAMF712: [SparseMatrixEntry; 8] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0), s(6, 6, 0), s(7, 7, 0)
];
#[rustfmt::skip]
pub static IAMF71_MONO: [SparseMatrixEntry; 7] = [
    s(0, 0, 1), s(1, 0, 1), s(2, 0, 2), s(4, 0, 5), s(5, 0, 5), s(6, 0, 5), s(7, 0, 5)
];
#[rustfmt::skip]
pub static IAMF71_IAMF916: [SparseMatrixEntry; 8] = [
    s(0, 6, 0), s(1, 7, 0), s(2, 2, 0), s(3, 3, 0), s(4, 8, 0), s(5, 9, 0), s(6, 4, 0), s(7, 5, 0)
];
///////////////////////////////////////

#[rustfmt::skip]
pub static IAMF712_BS020: [SparseMatrixEntry; 10] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 0, 2), s(2, 1, 2), s(4, 0, 2), s(5, 1, 2), s(6, 0, 2), s(7, 1, 2),
    s(8, 0, 0), s(9, 1, 0)
];
#[rustfmt::skip]
pub static IAMF712_BS050: [SparseMatrixEntry; 12] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 0, 2), s(4, 4, 2), s(5, 1, 2), s(5, 5, 2),
    s(6, 4, 0), s(7, 5, 0), s(8, 0, 0), s(9, 1, 0)
];
#[rustfmt::skip]
pub static IAMF712_BS250: [SparseMatrixEntry; 12] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 0, 2), s(4, 4, 2), s(5, 1, 2), s(5, 5, 2),
    s(6, 4, 0), s(7, 5, 0), s(8, 6, 0), s(9, 7, 0)
];
#[rustfmt::skip]
pub static IAMF712_BS450: [SparseMatrixEntry; 12] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 0, 2), s(4, 4, 2), s(5, 1, 2), s(5, 5, 2),
    s(6, 4, 0), s(7, 5, 0), s(8, 6, 0), s(9, 7, 0)
];
#[rustfmt::skip]
pub static IAMF712_BS451: [SparseMatrixEntry; 12] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 0, 2), s(4, 4, 2), s(5, 1, 2), s(5, 5, 2),
    s(6, 4, 0), s(7, 5, 0), s(8, 6, 0), s(9, 7, 0)
];
#[rustfmt::skip]
pub static IAMF712_BS370: [SparseMatrixEntry; 10] = [
    s(0, 1, 0), s(1, 2, 0), s(2, 0, 0), s(3, 10, 0), s(4, 5, 0), s(5, 6, 0), s(6, 7, 0), s(7, 8, 0),
    s(8, 3, 0), s(9, 4, 0)
];
#[rustfmt::skip]
pub static IAMF712_BS490: [SparseMatrixEntry; 10] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0), s(6, 6, 0), s(7, 7, 0),
    s(8, 8, 0), s(9, 9, 0)
];
#[rustfmt::skip]
pub static IAMF712_BS9A3: [SparseMatrixEntry; 10] = [
    s(0, 6, 0), s(1, 7, 0), s(2, 2, 0), s(3, 3, 0), s(4, 10, 0), s(5, 11, 0), s(6, 4, 0), s(7, 5, 0),
    s(8, 12, 0), s(9, 13, 0)
];
#[rustfmt::skip]
pub static IAMF712_BS070: [SparseMatrixEntry; 10] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0), s(6, 6, 0), s(7, 7, 0),
    s(8, 0, 0), s(9, 1, 0)
];
#[rustfmt::skip]
pub static IAMF712_BS470: [SparseMatrixEntry; 10] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0), s(6, 6, 0), s(7, 7, 0),
    s(8, 8, 0), s(9, 9, 0)
];
#[rustfmt::skip]
pub static IAMF712_IAMF312: [SparseMatrixEntry; 10] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 0, 2), s(5, 1, 2), s(6, 0, 2), s(7, 1, 2),
    s(8, 4, 0), s(9, 5, 0)
];
#[rustfmt::skip]
pub static IAMF712_IAMF712: [SparseMatrixEntry; 10] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0), s(6, 6, 0), s(7, 7, 0),
    s(8, 8, 0), s(9, 9, 0)
];
#[rustfmt::skip]
pub static IAMF712_MONO: [SparseMatrixEntry; 9] = [
    s(0, 0, 1), s(1, 0, 1), s(2, 0, 2), s(4, 0, 5), s(5, 0, 5), s(6, 0, 5), s(7, 0, 5), s(8, 0, 1),
    s(9, 0, 1)
];
#[rustfmt::skip]
pub static IAMF712_IAMF916: [SparseMatrixEntry; 10] = [
    s(0, 6, 0), s(1, 7, 0), s(2, 2, 0), s(3, 3, 0), s(4, 8, 0), s(5, 9, 0), s(6, 4, 0), s(7, 5, 0),
    s(8, 10, 0), s(9, 11, 0)
];
///////////////////////////////////////

#[rustfmt::skip]
pub static IAMF714_BS020: [SparseMatrixEntry; 12] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 0, 2), s(2, 1, 2), s(4, 0, 2), s(5, 1, 2), s(6, 0, 2), s(7, 1, 2),
    s(8, 0, 0), s(9, 1, 0), s(10, 0, 2), s(11, 1, 2)
];
#[rustfmt::skip]
pub static IAMF714_BS050: [SparseMatrixEntry; 14] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 0, 2), s(4, 4, 2), s(5, 1, 2), s(5, 5, 2),
    s(6, 4, 0), s(7, 5, 0), s(8, 0, 0), s(9, 1, 0), s(10, 4, 0), s(11, 5, 0)
];
#[rustfmt::skip]
pub static IAMF714_BS250: [SparseMatrixEntry; 14] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 0, 2), s(4, 4, 2), s(5, 1, 2), s(5, 5, 2),
    s(6, 4, 0), s(7, 5, 0), s(8, 6, 0), s(9, 7, 0), s(10, 4, 0), s(11, 5, 0)
];
#[rustfmt::skip]
pub static IAMF714_BS450: [SparseMatrixEntry; 14] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 0, 2), s(4, 4, 2), s(5, 1, 2), s(5, 5, 2),
    s(6, 4, 0), s(7, 5, 0), s(8, 6, 0), s(9, 7, 0), s(10, 8, 0), s(11, 9, 0)
];
#[rustfmt::skip]
pub static IAMF714_BS451: [SparseMatrixEntry; 14] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 0, 2), s(4, 4, 2), s(5, 1, 2), s(5, 5, 2),
    s(6, 4, 0), s(7, 5, 0), s(8, 6, 0), s(9, 7, 0), s(10, 8, 0), s(11, 9, 0)
];
#[rustfmt::skip]
pub static IAMF714_BS370: [SparseMatrixEntry; 14] = [
    s(0, 1, 0), s(1, 2, 0), s(2, 0, 0), s(3, 10, 0), s(4, 5, 0), s(5, 6, 0), s(6, 7, 0), s(7, 8, 0),
    s(8, 3, 0), s(9, 4, 0), s(10, 3, 2), s(10, 9, 2), s(11, 4, 2), s(11, 9, 2)
];
#[rustfmt::skip]
pub static IAMF714_BS490: [SparseMatrixEntry; 12] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0), s(6, 6, 0), s(7, 7, 0),
    s(8, 8, 0), s(9, 9, 0), s(10, 10, 0), s(11, 11, 0)
];
#[rustfmt::skip]
pub static IAMF714_BS9A3: [SparseMatrixEntry; 12] = [
    s(0, 6, 0), s(1, 7, 0), s(2, 2, 0), s(3, 3, 0), s(4, 10, 0), s(5, 11, 0), s(6, 4, 0), s(7, 5, 0),
    s(8, 12, 0), s(9, 13, 0), s(10, 16, 0), s(11, 17, 0)
];
#[rustfmt::skip]
pub static IAMF714_BS070: [SparseMatrixEntry; 12] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0), s(6, 6, 0), s(7, 7, 0),
    s(8, 0, 0), s(9, 1, 0), s(10, 6, 0), s(11, 7, 0)
];
#[rustfmt::skip]
pub static IAMF714_BS470: [SparseMatrixEntry; 12] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0), s(6, 6, 0), s(7, 7, 0),
    s(8, 8, 0), s(9, 9, 0), s(10, 10, 0), s(11, 11, 0)
];
#[rustfmt::skip]
pub static IAMF714_IAMF312: [SparseMatrixEntry; 12] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 0, 2), s(5, 1, 2), s(6, 0, 2), s(7, 1, 2),
    s(8, 4, 0), s(9, 5, 0), s(10, 4, 2), s(11, 5, 2)
];
#[rustfmt::skip]
pub static IAMF714_IAMF712: [SparseMatrixEntry; 12] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0), s(6, 6, 0), s(7, 7, 0),
    s(8, 8, 0), s(9, 9, 0), s(10, 8, 2), s(11, 9, 2)
];
#[rustfmt::skip]
pub static IAMF714_MONO: [SparseMatrixEntry; 11] = [
    s(0, 0, 1), s(1, 0, 1), s(2, 0, 2), s(4, 0, 5), s(5, 0, 5), s(6, 0, 5), s(7, 0, 5), s(8, 0, 1),
    s(9, 0, 1), s(10, 0, 5), s(11, 0, 5)
];
#[rustfmt::skip]
pub static IAMF714_IAMF916: [SparseMatrixEntry; 12] = [
    s(0, 6, 0), s(1, 7, 0), s(2, 2, 0), s(3, 3, 0), s(4, 8, 0), s(5, 9, 0), s(6, 4, 0), s(7, 5, 0),
    s(8, 10, 0), s(9, 11, 0), s(10, 12, 0), s(11, 13, 0)
];
///////////////////////////////////////

#[rustfmt::skip]
pub static IAMF312_BS020: [SparseMatrixEntry; 6] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 0, 2), s(2, 1, 2), s(4, 0, 0), s(5, 1, 0)
];
#[rustfmt::skip]
pub static IAMF312_BS050: [SparseMatrixEntry; 6] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 0, 0), s(5, 1, 0)
];
#[rustfmt::skip]
pub static IAMF312_BS250: [SparseMatrixEntry; 6] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 6, 0), s(5, 7, 0)
];
#[rustfmt::skip]
pub static IAMF312_BS450: [SparseMatrixEntry; 6] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 6, 0), s(5, 7, 0)
];
#[rustfmt::skip]
pub static IAMF312_BS451: [SparseMatrixEntry; 6] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 6, 0), s(5, 7, 0)
];
#[rustfmt::skip]
pub static IAMF312_BS370: [SparseMatrixEntry; 6] = [
    s(0, 1, 0), s(1, 2, 0), s(2, 0, 0), s(3, 10, 0), s(4, 3, 0), s(5, 4, 0)
];
#[rustfmt::skip]
pub static IAMF312_BS490: [SparseMatrixEntry; 6] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 8, 0), s(5, 9, 0)
];
#[rustfmt::skip]
pub static IAMF312_BS9A3: [SparseMatrixEntry; 6] = [
    s(0, 6, 0), s(1, 7, 0), s(2, 2, 0), s(3, 3, 0), s(4, 12, 0), s(5, 13, 0)
];
#[rustfmt::skip]
pub static IAMF312_BS070: [SparseMatrixEntry; 6] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 0, 0), s(5, 1, 0)
];
#[rustfmt::skip]
pub static IAMF312_BS470: [SparseMatrixEntry; 6] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 8, 0), s(5, 9, 0)
];
#[rustfmt::skip]
pub static IAMF312_IAMF312: [SparseMatrixEntry; 6] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0)
];
// 3.1.2->7.1.4->7.1.2
#[rustfmt::skip]
pub static IAMF312_IAMF712: [SparseMatrixEntry; 6] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 8, 0), s(5, 9, 0)
];
#[rustfmt::skip]
pub static IAMF312_MONO: [SparseMatrixEntry; 5] = [
    s(0, 0, 1), s(1, 0, 1), s(2, 0, 2), s(4, 0, 1), s(5, 0, 1)
];
#[rustfmt::skip]
pub static IAMF312_IAMF916: [SparseMatrixEntry; 6] = [
    s(0, 6, 0), s(1, 7, 0), s(2, 2, 0), s(3, 3, 0), s(4, 10, 0), s(5, 11, 0)
];
////////////////////////////////
#[rustfmt::skip]
pub static IAMF916_BS020: [SparseMatrixEntry; 16] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 0, 2), s(2, 1, 2), s(4, 0, 2), s(5, 1, 2), s(6, 0, 0), s(7, 1, 0),
    s(8, 0, 2), s(9, 1, 2), s(10, 0, 0), s(11, 1, 0), s(12, 0, 2), s(13, 1, 2), s(14, 0, 2), s(15, 1, 2)
];
#[rustfmt::skip]
pub static IAMF916_BS050: [SparseMatrixEntry; 22] = [
    s(0, 0, 4), s(0, 4, 3), s(1, 1, 4), s(1, 5, 3), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0),
    s(6, 0, 0), s(7, 1, 0), s(8, 0, 3), s(8, 4, 4), s(9, 1, 3), s(9, 5, 4), s(10, 0, 0), s(11, 1, 0),
    s(12, 4, 0), s(13, 5, 0), s(14, 0, 2), s(14, 4, 2), s(15, 1, 2), s(15, 5, 2)
];
#[rustfmt::skip]
pub static IAMF916_BS250: [SparseMatrixEntry; 22] = [
    s(0, 0, 4), s(0, 4, 3), s(1, 1, 4), s(1, 5, 3), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0),
    s(6, 0, 0), s(7, 1, 0), s(8, 0, 3), s(8, 4, 4), s(9, 1, 3), s(9, 5, 4), s(10, 6, 0), s(11, 7, 0),
    s(12, 4, 0), s(13, 5, 0), s(14, 4, 2), s(14, 6, 2), s(15, 5, 2), s(15, 7, 2)
];
#[rustfmt::skip]
pub static IAMF916_BS450: [SparseMatrixEntry; 22] = [
    s(0, 0, 4), s(0, 4, 3), s(1, 1, 4), s(1, 5, 3), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0),
    s(6, 0, 0), s(7, 1, 0), s(8, 0, 3), s(8, 4, 4), s(9, 1, 3), s(9, 5, 4), s(10, 6, 0), s(11, 7, 0),
    s(12, 8, 0), s(13, 9, 0), s(14, 6, 2), s(14, 8, 2), s(15, 7, 2), s(15, 9, 2)
];
#[rustfmt::skip]
pub static IAMF916_BS451: [SparseMatrixEntry; 22] = [
    s(0, 0, 4), s(0, 4, 3), s(1, 1, 4), s(1, 5, 3), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0),
    s(6, 0, 0), s(7, 1, 0), s(8, 0, 3), s(8, 4, 4), s(9, 1, 3), s(9, 5, 4), s(10, 6, 0), s(11, 7, 0),
    s(12, 8, 0), s(13, 9, 0), s(14, 6, 2), s(14, 8, 2), s(15, 7, 2), s(15, 9, 2)
];
#[rustfmt::skip]
pub static IAMF916_BS370: [SparseMatrixEntry; 22] = [
    s(0, 1, 2), s(0, 5, 2), s(1, 2, 2), s(1, 6, 2), s(2, 0, 0), s(3, 10, 0), s(4, 7, 0), s(5, 8, 0),
    s(6, 1, 0), s(7, 2, 0), s(8, 5, 0), s(9, 6, 0), s(10, 3, 0), s(11, 4, 0), s(12, 3, 3), s(12, 9, 4),
    s(13, 4, 3), s(13, 9, 4), s(14, 3, 4), s(14, 9, 3), s(15, 4, 4), s(15, 9, 3)
];
#[rustfmt::skip]
pub static IAMF916_BS490: [SparseMatrixEntry; 20] = [
    s(0, 0, 2), s(0, 4, 2), s(1, 1, 2), s(1, 5, 2), s(2, 2, 0), s(3, 3, 0), s(4, 6, 0), s(5, 7, 0),
    s(6, 0, 0), s(7, 1, 0), s(8, 4, 0), s(9, 5, 0), s(10, 8, 0), s(11, 9, 0), s(12, 10, 0), s(13, 11, 0),
    s(14, 8, 2), s(14, 10, 2), s(15, 9, 2), s(15, 11, 2)
];
#[rustfmt::skip]
pub static IAMF916_BS9A3: [SparseMatrixEntry; 16] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0), s(6, 6, 0), s(7, 7, 0),
    s(8, 10, 0), s(9, 11, 0), s(10, 12, 0), s(11, 13, 0), s(12, 16, 0), s(13, 17, 0), s(14, 18, 0), s(15, 19, 0)
];
#[rustfmt::skip]
pub static IAMF916_BS070: [SparseMatrixEntry; 18] = [
    s(0, 0, 2), s(0, 4, 2), s(1, 1, 2), s(1, 5, 2), s(2, 2, 0), s(3, 3, 0), s(4, 6, 0), s(5, 7, 0),
    s(6, 0, 0), s(7, 1, 0), s(8, 4, 0), s(9, 5, 0), s(10, 0, 0), s(11, 1, 0), s(12, 6, 0), s(13, 7, 0),
    s(14, 4, 0), s(15, 5, 0)
];
#[rustfmt::skip]
pub static IAMF916_BS470: [SparseMatrixEntry; 20] = [
    s(0, 0, 2), s(0, 4, 2), s(1, 1, 2), s(1, 5, 2), s(2, 2, 0), s(3, 3, 0), s(4, 6, 0), s(5, 7, 0),
    s(6, 0, 0), s(7, 1, 0), s(8, 4, 0), s(9, 5, 0), s(10, 8, 0), s(11, 9, 0), s(12, 10, 0), s(13, 11, 0),
    s(14, 8, 2), s(14, 10, 2), s(15, 9, 2), s(15, 11, 2)
];
#[rustfmt::skip]
pub static IAMF916_IAMF312: [SparseMatrixEntry; 16] = [
    s(0, 0, 6), s(1, 1, 6), s(2, 2, 0), s(3, 3, 0), s(4, 0, 2), s(5, 1, 2), s(6, 0, 0), s(7, 1, 0),
    s(8, 0, 2), s(9, 1, 2), s(10, 4, 0), s(11, 5, 0), s(12, 4, 2), s(13, 5, 2), s(14, 4, 6), s(15, 5, 6)
];
#[rustfmt::skip]
pub static IAMF916_IAMF712: [SparseMatrixEntry; 18] = [
    s(0, 0, 2), s(0, 4, 2), s(1, 1, 2), s(1, 5, 2), s(2, 2, 0), s(3, 3, 0), s(4, 6, 0), s(5, 7, 0),
    s(6, 0, 0), s(7, 1, 0), s(8, 4, 0), s(9, 5, 0), s(10, 8, 0), s(11, 9, 0), s(12, 8, 2), s(13, 9, 2),
    s(14, 8, 6), s(15, 9, 6)
];
#[rustfmt::skip]
pub static IAMF916_MONO: [SparseMatrixEntry; 15] = [
    s(0, 0, 1), s(1, 0, 1), s(2, 0, 2), s(4, 0, 5), s(5, 0, 5), s(6, 0, 1), s(7, 0, 1), s(8, 0, 5),
    s(9, 0, 5), s(10, 0, 1), s(11, 0, 1), s(12, 0, 5), s(13, 0, 5), s(14, 0, 5), s(15, 0, 5)
];
#[rustfmt::skip]
pub static IAMF916_IAMF916: [SparseMatrixEntry; 16] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0), s(6, 6, 0), s(7, 7, 0),
    s(8, 8, 0), s(9, 9, 0), s(10, 10, 0), s(11, 11, 0), s(12, 12, 0), s(13, 13, 0), s(14, 14, 0), s(15, 15, 0)
];

// version 2.0

#[rustfmt::skip]
pub static MONO_IAMF7154: [SparseMatrixEntry; 1] = [
    s(0, 2, 0)
];

#[rustfmt::skip]
pub static STEREO_IAMF7154: [SparseMatrixEntry; 2] = [
    s(0, 0, 0), s(1, 1, 0)
];
#[rustfmt::skip]
pub static IAMF51_IAMF7154: [SparseMatrixEntry; 6] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 6, 0), s(5, 7, 0)
];
#[rustfmt::skip]
pub static IAMF512_IAMF7154: [SparseMatrixEntry; 8] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 6, 0), s(5, 7, 0), s(6, 8, 0), s(7, 9, 0)
];
#[rustfmt::skip]
pub static IAMF514_IAMF7154: [SparseMatrixEntry; 10] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 6, 0), s(5, 7, 0), s(6, 8, 0), s(7, 9, 0),
    s(8, 11, 0), s(9, 12, 0)
];
#[rustfmt::skip]
pub static IAMF71_IAMF7154: [SparseMatrixEntry; 8] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0), s(6, 6, 0), s(7, 7, 0)
];
#[rustfmt::skip]
pub static IAMF712_IAMF7154: [SparseMatrixEntry; 10] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0), s(6, 6, 0), s(7, 7, 0),
    s(8, 8, 0), s(9, 9, 0)
];
#[rustfmt::skip]
pub static IAMF714_IAMF7154: [SparseMatrixEntry; 12] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0), s(6, 6, 0), s(7, 7, 0),
    s(8, 8, 0), s(9, 9, 0), s(10, 11, 0), s(11, 12, 0)
];
#[rustfmt::skip]
pub static IAMF916_IAMF7154: [SparseMatrixEntry; 20] = [
    s(0, 0, 2), s(0, 4, 2), s(1, 1, 2), s(1, 5, 2), s(2, 2, 0), s(3, 3, 0), s(4, 6, 0), s(5, 7, 0),
    s(6, 0, 0), s(7, 1, 0), s(8, 4, 0), s(9, 5, 0), s(10, 8, 0), s(11, 9, 0), s(12, 11, 0), s(13, 12, 0),
    s(14, 8, 2), s(14, 11, 2), s(15, 9, 2), s(15, 12, 2)
];
#[rustfmt::skip]
pub static IAMF10293_IAMF7154: [SparseMatrixEntry; 32] = [
    s(0, 0, 2), s(0, 4, 2), s(1, 1, 2), s(1, 5, 2), s(2, 2, 0), s(3, 3, 2), s(4, 6, 0), s(5, 7, 0),
    s(6, 0, 0), s(7, 1, 0), s(8, 6, 2), s(8, 7, 2), s(9, 3, 2), s(10, 4, 0), s(11, 5, 0), s(12, 8, 0),
    s(13, 9, 0), s(14, 8, 2), s(14, 9, 2), s(15, 10, 0), s(16, 11, 0), s(17, 12, 0), s(18, 8, 2), s(18, 11, 2),
    s(19, 9, 2), s(19, 12, 2), s(20, 11, 2), s(20, 12, 2), s(21, 13, 2), s(21, 14, 2), s(22, 13, 0), s(23, 14, 0)
];
#[rustfmt::skip]
pub static IAMF312_IAMF7154: [SparseMatrixEntry; 6] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 8, 0), s(5, 9, 0)
];

#[rustfmt::skip]
pub static IAMF7154_BS020: [SparseMatrixEntry; 18] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 0, 2), s(2, 1, 2), s(4, 0, 2), s(5, 1, 2), s(6, 0, 2), s(7, 1, 2),
    s(8, 0, 0), s(9, 1, 0), s(10, 0, 1), s(10, 1, 1), s(11, 0, 2), s(12, 1, 2), s(13, 0, 0), s(14, 1, 0),
    s(15, 0, 2), s(16, 1, 2)
];
#[rustfmt::skip]
pub static IAMF7154_BS050: [SparseMatrixEntry; 22] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 0, 2), s(4, 4, 2), s(5, 1, 2), s(5, 5, 2),
    s(6, 4, 0), s(7, 5, 0), s(8, 0, 0), s(9, 1, 0), s(10, 0, 1), s(10, 1, 1), s(10, 4, 1), s(10, 5, 1),
    s(11, 4, 0), s(12, 5, 0), s(13, 0, 0), s(14, 1, 0), s(15, 4, 0), s(16, 5, 0)
];
#[rustfmt::skip]
pub static IAMF7154_BS250: [SparseMatrixEntry; 22] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 0, 2), s(4, 4, 2), s(5, 1, 2), s(5, 5, 2),
    s(6, 4, 0), s(7, 5, 0), s(8, 6, 0), s(9, 7, 0), s(10, 4, 1), s(10, 5, 1), s(10, 6, 1), s(10, 7, 1),
    s(11, 4, 0), s(12, 5, 0), s(13, 0, 0), s(14, 1, 0), s(15, 4, 0), s(16, 5, 0)
];
#[rustfmt::skip]
pub static IAMF7154_BS450: [SparseMatrixEntry; 22] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 0, 2), s(4, 4, 2), s(5, 1, 2), s(5, 5, 2),
    s(6, 4, 0), s(7, 5, 0), s(8, 6, 0), s(9, 7, 0), s(10, 6, 1), s(10, 7, 1), s(10, 8, 1), s(10, 9, 1),
    s(11, 8, 0), s(12, 9, 0), s(13, 0, 0), s(14, 1, 0), s(15, 4, 0), s(16, 5, 0)
];
#[rustfmt::skip]
pub static IAMF7154_BS451: [SparseMatrixEntry; 22] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 0, 2), s(4, 4, 2), s(5, 1, 2), s(5, 5, 2),
    s(6, 4, 0), s(7, 5, 0), s(8, 6, 0), s(9, 7, 0), s(10, 6, 1), s(10, 7, 1), s(10, 8, 1), s(10, 9, 1),
    s(11, 8, 0), s(12, 9, 0), s(13, 10, 2), s(14, 10, 2), s(15, 4, 0), s(16, 5, 0)
];
#[rustfmt::skip]
pub static IAMF7154_BS370: [SparseMatrixEntry; 19] = [
    s(0, 1, 0), s(1, 2, 0), s(2, 0, 0), s(3, 10, 0), s(4, 5, 0), s(5, 6, 0), s(6, 7, 0), s(7, 8, 0),
    s(8, 3, 0), s(9, 4, 0), s(10, 3, 3), s(10, 4, 3), s(10, 9, 3), s(11, 7, 0), s(12, 8, 0), s(13, 1, 0),
    s(14, 2, 0), s(15, 7, 0), s(16, 8, 0)
];
#[rustfmt::skip]
pub static IAMF7154_BS490: [SparseMatrixEntry; 20] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0), s(6, 6, 0), s(7, 7, 0),
    s(8, 8, 0), s(9, 9, 0), s(10, 8, 1), s(10, 9, 1), s(10, 10, 1), s(10, 11, 1), s(11, 10, 0), s(12, 11, 0),
    s(13, 0, 0), s(14, 1, 0), s(15, 6, 0), s(16, 7, 0)
];
#[rustfmt::skip]
pub static IAMF7154_BS9A3: [SparseMatrixEntry; 17] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 10, 0), s(5, 11, 0), s(6, 4, 0), s(7, 5, 0),
    s(8, 12, 0), s(9, 13, 0), s(10, 15, 0), s(11, 16, 0), s(12, 17, 0), s(13, 22, 0), s(14, 23, 0), s(15, 4, 0),
    s(16, 5, 0)
];
#[rustfmt::skip]
pub static IAMF7154_BS070: [SparseMatrixEntry; 20] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0), s(6, 6, 0), s(7, 7, 0),
    s(8, 0, 0), s(9, 1, 0), s(10, 0, 1), s(10, 1, 1), s(10, 6, 1), s(10, 7, 1), s(11, 6, 0), s(12, 7, 0),
    s(13, 0, 0), s(14, 1, 0), s(15, 6, 0), s(16, 7, 0)
];
#[rustfmt::skip]
pub static IAMF7154_BS470: [SparseMatrixEntry; 20] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0), s(6, 6, 0), s(7, 7, 0),
    s(8, 8, 0), s(9, 9, 0), s(10, 8, 1), s(10, 9, 1), s(10, 10, 1), s(10, 11, 1), s(11, 10, 0), s(12, 11, 0),
    s(13, 0, 0), s(14, 1, 0), s(15, 6, 0), s(16, 7, 0)
];
#[rustfmt::skip]
pub static IAMF7154_IAMF312: [SparseMatrixEntry; 18] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 0, 2), s(5, 1, 2), s(6, 0, 2), s(7, 1, 2),
    s(8, 4, 0), s(9, 5, 0), s(10, 4, 2), s(10, 5, 2), s(11, 4, 2), s(12, 5, 2), s(13, 0, 0), s(14, 1, 0),
    s(15, 0, 2), s(16, 1, 2)
];
#[rustfmt::skip]
pub static IAMF7154_IAMF712: [SparseMatrixEntry; 20] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0), s(6, 6, 0), s(7, 7, 0),
    s(8, 8, 0), s(9, 9, 0), s(10, 6, 1), s(10, 7, 1), s(10, 8, 1), s(10, 9, 1), s(11, 6, 0), s(12, 7, 0),
    s(13, 0, 0), s(14, 1, 0), s(15, 6, 0), s(16, 7, 0)
];
#[rustfmt::skip]
pub static IAMF7154_IAMF916: [SparseMatrixEntry; 20] = [
    s(0, 6, 0), s(1, 7, 0), s(2, 2, 0), s(3, 3, 0), s(4, 8, 0), s(5, 9, 0), s(6, 4, 0), s(7, 5, 0),
    s(8, 10, 0), s(9, 11, 0), s(10, 10, 1), s(10, 11, 1), s(10, 12, 1), s(10, 13, 1), s(11, 12, 0), s(12, 13, 0),
    s(13, 6, 0), s(14, 7, 0), s(15, 4, 0), s(16, 5, 0)
];
#[rustfmt::skip]
pub static IAMF7154_MONO: [SparseMatrixEntry; 16] = [
    s(0, 0, 1), s(1, 0, 1), s(2, 0, 2), s(4, 0, 5), s(5, 0, 5), s(6, 0, 5), s(7, 0, 5), s(8, 0, 1),
    s(9, 0, 1), s(10, 0, 1), s(11, 0, 5), s(12, 0, 5), s(13, 0, 1), s(14, 0, 1), s(15, 0, 5), s(16, 0, 5)
];

#[rustfmt::skip]
pub static IAMF7154_IAMF7154: [SparseMatrixEntry; 17] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0), s(6, 6, 0), s(7, 7, 0),
    s(8, 8, 0), s(9, 9, 0), s(10, 10, 0), s(11, 11, 0), s(12, 12, 0), s(13, 13, 0), s(14, 14, 0), s(15, 15, 0),
    s(16, 16, 0)
];

#[rustfmt::skip]
pub static IAMF10293_BS020: [SparseMatrixEntry; 28] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 0, 2), s(2, 1, 2), s(4, 0, 2), s(5, 1, 2), s(6, 0, 0), s(7, 1, 0),
    s(8, 0, 1), s(8, 1, 1), s(10, 0, 2), s(11, 1, 2), s(12, 0, 0), s(13, 1, 0), s(14, 0, 2), s(14, 1, 2),
    s(15, 0, 1), s(15, 1, 1), s(16, 0, 2), s(17, 1, 2), s(18, 0, 2), s(19, 1, 2), s(20, 0, 1), s(20, 1, 1),
    s(21, 0, 2), s(21, 1, 2), s(22, 0, 0), s(23, 1, 0)
];

#[rustfmt::skip]
pub static IAMF10293_BS050: [SparseMatrixEntry; 35] = [
    s(0, 0, 4), s(0, 4, 3), s(1, 1, 4), s(1, 5, 3), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0),
    s(6, 0, 0), s(7, 1, 0), s(8, 4, 2), s(8, 5, 2), s(9, 3, 2), s(10, 0, 3), s(10, 4, 4), s(11, 1, 3),
    s(11, 5, 4), s(12, 0, 0), s(13, 1, 0), s(14, 2, 0), s(15, 0, 1), s(15, 1, 1), s(15, 4, 1), s(15, 5, 1),
    s(16, 4, 0), s(17, 5, 0), s(18, 0, 2), s(18, 4, 2), s(19, 1, 2), s(19, 5, 2), s(20, 4, 2), s(20, 5, 2),
    s(21, 2, 0), s(22, 0, 0), s(23, 1, 0)
];

#[rustfmt::skip]
pub static IAMF10293_BS250: [SparseMatrixEntry; 36] = [
    s(0, 0, 4), s(0, 4, 3), s(1, 1, 4), s(1, 5, 3), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0),
    s(6, 0, 0), s(7, 1, 0), s(8, 4, 2), s(8, 5, 2), s(9, 3, 2), s(10, 0, 3), s(10, 4, 4), s(11, 1, 3),
    s(11, 5, 4), s(12, 6, 0), s(13, 7, 0), s(14, 6, 2), s(14, 7, 2), s(15, 4, 1), s(15, 5, 1), s(15, 6, 1),
    s(15, 7, 1), s(16, 4, 0), s(17, 5, 0), s(18, 4, 2), s(18, 6, 2), s(19, 5, 2), s(19, 7, 2), s(20, 4, 2),
    s(20, 5, 2), s(21, 2, 0), s(22, 0, 0), s(23, 1, 0)
];

#[rustfmt::skip]
pub static IAMF10293_BS450: [SparseMatrixEntry; 36] = [
    s(0, 0, 4), s(0, 4, 3), s(1, 1, 4), s(1, 5, 3), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0),
    s(6, 0, 0), s(7, 1, 0), s(8, 4, 2), s(8, 5, 2), s(9, 3, 2), s(10, 0, 3), s(10, 4, 4), s(11, 1, 3),
    s(11, 5, 4), s(12, 6, 0), s(13, 7, 0), s(14, 6, 2), s(14, 7, 2), s(15, 6, 1), s(15, 7, 1), s(15, 8, 1),
    s(15, 9, 1), s(16, 8, 0), s(17, 9, 0), s(18, 6, 2), s(18, 8, 2), s(19, 7, 2), s(19, 9, 2), s(20, 8, 2),
    s(20, 9, 2), s(21, 2, 0), s(22, 0, 0), s(23, 1, 0)
];

#[rustfmt::skip]
pub static IAMF10293_BS451: [SparseMatrixEntry; 36] = [
    s(0, 0, 4), s(0, 4, 3), s(1, 1, 4), s(1, 5, 3), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0),
    s(6, 0, 0), s(7, 1, 0), s(8, 4, 2), s(8, 5, 2), s(9, 3, 2), s(10, 0, 3), s(10, 4, 4), s(11, 1, 3),
    s(11, 5, 4), s(12, 6, 0), s(13, 7, 0), s(14, 6, 2), s(14, 7, 2), s(15, 6, 1), s(15, 7, 1), s(15, 8, 1),
    s(15, 9, 1), s(16, 8, 0), s(17, 9, 0), s(18, 6, 2), s(18, 8, 2), s(19, 7, 2), s(19, 9, 2), s(20, 8, 2),
    s(20, 9, 2), s(21, 10, 0), s(22, 0, 0), s(23, 1, 0)
];

#[rustfmt::skip]
pub static IAMF10293_BS370: [SparseMatrixEntry; 34] = [
    s(0, 1, 2), s(0, 5, 2), s(1, 2, 2), s(1, 6, 2), s(2, 0, 0), s(3, 10, 0), s(4, 7, 0), s(5, 8, 0),
    s(6, 1, 0), s(7, 2, 0), s(8, 7, 2), s(8, 8, 2), s(9, 11, 0), s(10, 5, 0), s(11, 6, 0), s(12, 3, 0),
    s(13, 4, 0), s(14, 3, 2), s(14, 4, 2), s(15, 3, 3), s(15, 4, 3), s(15, 9, 3), s(16, 3, 3), s(16, 9, 4),
    s(17, 4, 3), s(17, 9, 4), s(18, 3, 4), s(18, 9, 3), s(19, 4, 4), s(19, 9, 3), s(20, 9, 0), s(21, 0, 0),
    s(22, 1, 0), s(23, 2, 0)
];

#[rustfmt::skip]
pub static IAMF10293_BS490: [SparseMatrixEntry; 34] = [
    s(0, 0, 2), s(0, 4, 2), s(1, 1, 2), s(1, 5, 2), s(2, 2, 0), s(3, 3, 0), s(4, 6, 0), s(5, 7, 0),
    s(6, 0, 0), s(7, 1, 0), s(8, 6, 2), s(8, 7, 2), s(9, 3, 2), s(10, 4, 0), s(11, 5, 0), s(12, 8, 0),
    s(13, 9, 0), s(14, 8, 2), s(14, 9, 2), s(15, 8, 1), s(15, 9, 1), s(15, 10, 1), s(15, 11, 1), s(16, 10, 0),
    s(17, 11, 0), s(18, 8, 2), s(18, 10, 2), s(19, 9, 2), s(19, 11, 2), s(20, 10, 2), s(20, 11, 2), s(21, 2, 0),
    s(22, 0, 0), s(23, 1, 0)
];

#[rustfmt::skip]
pub static IAMF10293_BS9A3: [SparseMatrixEntry; 24] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0), s(6, 6, 0), s(7, 7, 0),
    s(8, 8, 0), s(9, 9, 0), s(10, 10, 0), s(11, 11, 0), s(12, 12, 0), s(13, 13, 0), s(14, 14, 0), s(15, 15, 0),
    s(16, 16, 0), s(17, 17, 0), s(18, 18, 0), s(19, 19, 0), s(20, 20, 0), s(21, 21, 0), s(22, 22, 0), s(23, 23, 0)
];

#[rustfmt::skip]
pub static IAMF10293_BS070: [SparseMatrixEntry; 31] = [
    s(0, 0, 2), s(0, 4, 2), s(1, 1, 2), s(1, 5, 2), s(2, 2, 0), s(3, 3, 0), s(4, 6, 0), s(5, 7, 0),
    s(6, 0, 0), s(7, 1, 0), s(8, 6, 2), s(8, 7, 2), s(9, 3, 2), s(10, 4, 0), s(11, 5, 0), s(12, 0, 0),
    s(13, 1, 0), s(14, 2, 0), s(15, 0, 1), s(15, 1, 1), s(15, 6, 1), s(15, 7, 1), s(16, 6, 0), s(17, 7, 0),
    s(18, 4, 0), s(19, 5, 0), s(20, 6, 2), s(20, 7, 2), s(21, 2, 0), s(22, 0, 0), s(23, 1, 0)
];

#[rustfmt::skip]
pub static IAMF10293_BS470: [SparseMatrixEntry; 34] = [
    s(0, 0, 2), s(0, 4, 2), s(1, 1, 2), s(1, 5, 2), s(2, 2, 0), s(3, 3, 0), s(4, 6, 0), s(5, 7, 0),
    s(6, 0, 0), s(7, 1, 0), s(8, 6, 2), s(8, 7, 2), s(9, 3, 2), s(10, 4, 0), s(11, 5, 0), s(12, 8, 0),
    s(13, 9, 0), s(14, 8, 2), s(14, 9, 2), s(15, 8, 1), s(15, 9, 1), s(15, 10, 1), s(15, 11, 1), s(16, 10, 0),
    s(17, 11, 0), s(18, 8, 2), s(18, 10, 2), s(19, 9, 2), s(19, 11, 2), s(20, 10, 2), s(20, 11, 2), s(21, 2, 0),
    s(22, 0, 0), s(23, 1, 0)
];

#[rustfmt::skip]
pub static IAMF10293_IAMF312: [SparseMatrixEntry; 28] = [
    s(0, 0, 6), s(1, 1, 6), s(2, 2, 0), s(3, 3, 0), s(4, 0, 2), s(5, 1, 2), s(6, 0, 0), s(7, 1, 0),
    s(8, 0, 1), s(8, 1, 1), s(9, 3, 2), s(10, 0, 2), s(11, 1, 2), s(12, 4, 0), s(13, 5, 0), s(14, 4, 2),
    s(14, 5, 2), s(15, 4, 7), s(15, 5, 7), s(16, 4, 2), s(17, 5, 2), s(18, 4, 6), s(19, 5, 6), s(20, 4, 1),
    s(20, 5, 1), s(21, 2, 0), s(22, 0, 0), s(23, 1, 0)
];

#[rustfmt::skip]
pub static IAMF10293_IAMF712: [SparseMatrixEntry; 30] = [
    s(0, 0, 2), s(0, 4, 2), s(1, 1, 2), s(1, 5, 2), s(2, 2, 0), s(3, 3, 0), s(4, 6, 0), s(5, 7, 0),
    s(6, 0, 0), s(7, 1, 0), s(8, 6, 2), s(8, 7, 2), s(9, 3, 2), s(10, 4, 0), s(11, 5, 0), s(12, 8, 0),
    s(13, 9, 0), s(14, 8, 2), s(14, 9, 2), s(15, 8, 7), s(15, 9, 7), s(16, 8, 2), s(17, 9, 2), s(18, 8, 6),
    s(19, 9, 6), s(20, 8, 1), s(20, 9, 1), s(21, 2, 0), s(22, 0, 0), s(23, 1, 0)
];

#[rustfmt::skip]
pub static IAMF10293_MONO: [SparseMatrixEntry; 22] = [
    s(0, 0, 1), s(1, 0, 1), s(2, 0, 2), s(4, 0, 5), s(5, 0, 5), s(6, 0, 1), s(7, 0, 1), s(8, 0, 1),
    s(10, 0, 5), s(11, 0, 5), s(12, 0, 1), s(13, 0, 1), s(14, 0, 2), s(15, 0, 7), s(16, 0, 5), s(17, 0, 5),
    s(18, 0, 5), s(19, 0, 5), s(20, 0, 1), s(21, 0, 2), s(22, 0, 1), s(23, 0, 1)
];

#[rustfmt::skip]
pub static IAMF10293_IAMF916: [SparseMatrixEntry; 30] = [
    s(0, 0, 0), s(1, 1, 0), s(2, 2, 0), s(3, 3, 0), s(4, 4, 0), s(5, 5, 0), s(6, 6, 0), s(7, 7, 0),
    s(8, 4, 2), s(8, 5, 2), s(9, 3, 2), s(10, 8, 0), s(11, 9, 0), s(12, 10, 0), s(13, 11, 0), s(14, 10, 2),
    s(14, 11, 2), s(15, 10, 1), s(15, 11, 1), s(15, 12, 1), s(15, 13, 1), s(16, 12, 0), s(17, 13, 0), s(18, 14, 0),
    s(19, 15, 0), s(20, 12, 2), s(20, 13, 2), s(21, 2, 0), s(22, 6, 0), s(23, 7, 0)
];

////////////////////////////////

#[rustfmt::skip]
pub static CHAN_TO_CHAN_RDR_TAB: [ChannelToChannelRenderer; 208] = [
    ChannelToChannelRenderer { in_system: InputLayout::Mono, out_system: OutputLayout::Stereo, mat: &MONO_BS020, m: 1, n: 2 },
    ChannelToChannelRenderer { in_system: InputLayout::Mono, out_system: OutputLayout::Layout51, mat: &MONO_BS050, m: 1, n: 6 },
    ChannelToChannelRenderer { in_system: InputLayout::Mono, out_system: OutputLayout::Layout512, mat: &MONO_BS250, m: 1, n: 8 },
    ChannelToChannelRenderer { in_system: InputLayout::Mono, out_system: OutputLayout::Layout514, mat: &MONO_BS450, m: 1, n: 10 },
    ChannelToChannelRenderer { in_system: InputLayout::Mono, out_system: OutputLayout::SoundSystemE451, mat: &MONO_BS451, m: 1, n: 11 },
    ChannelToChannelRenderer { in_system: InputLayout::Mono, out_system: OutputLayout::SoundSystemF370, mat: &MONO_BS370, m: 1, n: 12 },
    ChannelToChannelRenderer { in_system: InputLayout::Mono, out_system: OutputLayout::SoundSystemG490, mat: &MONO_BS490, m: 1, n: 14 },
    ChannelToChannelRenderer { in_system: InputLayout::Mono, out_system: OutputLayout::LayoutA293, mat: &MONO_BS9A3, m: 1, n: 24 },
    ChannelToChannelRenderer { in_system: InputLayout::Mono, out_system: OutputLayout::Layout71, mat: &MONO_BS070, m: 1, n: 8 },
    ChannelToChannelRenderer { in_system: InputLayout::Mono, out_system: OutputLayout::Layout714, mat: &MONO_BS470, m: 1, n: 12 },
    ChannelToChannelRenderer { in_system: InputLayout::Mono, out_system: OutputLayout::Layout312, mat: &MONO_IAMF312, m: 1, n: 6 },
    ChannelToChannelRenderer { in_system: InputLayout::Mono, out_system: OutputLayout::Layout712, mat: &MONO_IAMF712, m: 1, n: 10 },
    ChannelToChannelRenderer { in_system: InputLayout::Mono, out_system: OutputLayout::Binaural, mat: &MONO_BS020, m: 1, n: 2 },
    ChannelToChannelRenderer { in_system: InputLayout::Mono, out_system: OutputLayout::Mono, mat: &MONO_MONO, m: 1, n: 1 },
    ChannelToChannelRenderer { in_system: InputLayout::Mono, out_system: OutputLayout::Layout916, mat: &MONO_IAMF916, m: 1, n: 16 },
    ChannelToChannelRenderer { in_system: InputLayout::Stereo, out_system: OutputLayout::Stereo, mat: &STEREO_BS020, m: 2, n: 2 },
    ChannelToChannelRenderer { in_system: InputLayout::Stereo, out_system: OutputLayout::Layout51, mat: &STEREO_BS050, m: 2, n: 6 },
    ChannelToChannelRenderer { in_system: InputLayout::Stereo, out_system: OutputLayout::Layout512, mat: &STEREO_BS250, m: 2, n: 8 },
    ChannelToChannelRenderer { in_system: InputLayout::Stereo, out_system: OutputLayout::Layout514, mat: &STEREO_BS450, m: 2, n: 10 },
    ChannelToChannelRenderer { in_system: InputLayout::Stereo, out_system: OutputLayout::SoundSystemE451, mat: &STEREO_BS451, m: 2, n: 11 },
    ChannelToChannelRenderer { in_system: InputLayout::Stereo, out_system: OutputLayout::SoundSystemF370, mat: &STEREO_BS370, m: 2, n: 12 },
    ChannelToChannelRenderer { in_system: InputLayout::Stereo, out_system: OutputLayout::SoundSystemG490, mat: &STEREO_BS490, m: 2, n: 14 },
    ChannelToChannelRenderer { in_system: InputLayout::Stereo, out_system: OutputLayout::LayoutA293, mat: &STEREO_BS9A3, m: 2, n: 24 },
    ChannelToChannelRenderer { in_system: InputLayout::Stereo, out_system: OutputLayout::Layout71, mat: &STEREO_BS070, m: 2, n: 8 },
    ChannelToChannelRenderer { in_system: InputLayout::Stereo, out_system: OutputLayout::Layout714, mat: &STEREO_BS470, m: 2, n: 12 },
    ChannelToChannelRenderer { in_system: InputLayout::Stereo, out_system: OutputLayout::Layout312, mat: &STEREO_IAMF312, m: 2, n: 6 },
    ChannelToChannelRenderer { in_system: InputLayout::Stereo, out_system: OutputLayout::Layout712, mat: &STEREO_IAMF712, m: 2, n: 10 },
    ChannelToChannelRenderer { in_system: InputLayout::Stereo, out_system: OutputLayout::Binaural, mat: &STEREO_BS020, m: 2, n: 2 },
    ChannelToChannelRenderer { in_system: InputLayout::Stereo, out_system: OutputLayout::Mono, mat: &STEREO_MONO, m: 2, n: 1 },
    ChannelToChannelRenderer { in_system: InputLayout::Stereo, out_system: OutputLayout::Layout916, mat: &STEREO_IAMF916, m: 2, n: 16 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout312, out_system: OutputLayout::Stereo, mat: &IAMF312_BS020, m: 6, n: 2 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout312, out_system: OutputLayout::Layout51, mat: &IAMF312_BS050, m: 6, n: 6 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout312, out_system: OutputLayout::Layout512, mat: &IAMF312_BS250, m: 6, n: 8 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout312, out_system: OutputLayout::Layout514, mat: &IAMF312_BS450, m: 6, n: 10 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout312, out_system: OutputLayout::SoundSystemE451, mat: &IAMF312_BS451, m: 6, n: 11 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout312, out_system: OutputLayout::SoundSystemF370, mat: &IAMF312_BS370, m: 6, n: 12 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout312, out_system: OutputLayout::SoundSystemG490, mat: &IAMF312_BS490, m: 6, n: 14 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout312, out_system: OutputLayout::LayoutA293, mat: &IAMF312_BS9A3, m: 6, n: 24 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout312, out_system: OutputLayout::Layout71, mat: &IAMF312_BS070, m: 6, n: 8 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout312, out_system: OutputLayout::Layout714, mat: &IAMF312_BS470, m: 6, n: 12 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout312, out_system: OutputLayout::Layout312, mat: &IAMF312_IAMF312, m: 6, n: 6 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout312, out_system: OutputLayout::Layout712, mat: &IAMF312_IAMF712, m: 6, n: 10 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout312, out_system: OutputLayout::Binaural, mat: &IAMF312_BS020, m: 6, n: 2 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout312, out_system: OutputLayout::Mono, mat: &IAMF312_MONO, m: 6, n: 1 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout312, out_system: OutputLayout::Layout916, mat: &IAMF312_IAMF916, m: 6, n: 16 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout51, out_system: OutputLayout::Stereo, mat: &IAMF51_BS020, m: 6, n: 2 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout51, out_system: OutputLayout::Layout51, mat: &IAMF51_BS050, m: 6, n: 6 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout51, out_system: OutputLayout::Layout512, mat: &IAMF51_BS250, m: 6, n: 8 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout51, out_system: OutputLayout::Layout514, mat: &IAMF51_BS450, m: 6, n: 10 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout51, out_system: OutputLayout::SoundSystemE451, mat: &IAMF51_BS451, m: 6, n: 11 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout51, out_system: OutputLayout::SoundSystemF370, mat: &IAMF51_BS370, m: 6, n: 12 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout51, out_system: OutputLayout::SoundSystemG490, mat: &IAMF51_BS490, m: 6, n: 14 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout51, out_system: OutputLayout::LayoutA293, mat: &IAMF51_BS9A3, m: 6, n: 24 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout51, out_system: OutputLayout::Layout71, mat: &IAMF51_BS070, m: 6, n: 8 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout51, out_system: OutputLayout::Layout714, mat: &IAMF51_BS470, m: 6, n: 12 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout51, out_system: OutputLayout::Layout312, mat: &IAMF51_IAMF312, m: 6, n: 6 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout51, out_system: OutputLayout::Layout712, mat: &IAMF51_IAMF712, m: 6, n: 10 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout51, out_system: OutputLayout::Binaural, mat: &IAMF51_BS020, m: 6, n: 2 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout51, out_system: OutputLayout::Mono, mat: &IAMF51_MONO, m: 6, n: 1 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout51, out_system: OutputLayout::Layout916, mat: &IAMF51_IAMF916, m: 6, n: 16 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout512, out_system: OutputLayout::Stereo, mat: &IAMF512_BS020, m: 8, n: 2 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout512, out_system: OutputLayout::Layout51, mat: &IAMF512_BS050, m: 8, n: 6 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout512, out_system: OutputLayout::Layout512, mat: &IAMF512_BS250, m: 8, n: 8 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout512, out_system: OutputLayout::Layout514, mat: &IAMF512_BS450, m: 8, n: 10 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout512, out_system: OutputLayout::SoundSystemE451, mat: &IAMF512_BS451, m: 8, n: 11 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout512, out_system: OutputLayout::SoundSystemF370, mat: &IAMF512_BS370, m: 8, n: 12 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout512, out_system: OutputLayout::SoundSystemG490, mat: &IAMF512_BS490, m: 8, n: 14 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout512, out_system: OutputLayout::LayoutA293, mat: &IAMF512_BS9A3, m: 8, n: 24 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout512, out_system: OutputLayout::Layout71, mat: &IAMF512_BS070, m: 8, n: 8 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout512, out_system: OutputLayout::Layout714, mat: &IAMF512_BS470, m: 8, n: 12 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout512, out_system: OutputLayout::Layout312, mat: &IAMF512_IAMF312, m: 8, n: 6 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout512, out_system: OutputLayout::Layout712, mat: &IAMF512_IAMF712, m: 8, n: 10 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout512, out_system: OutputLayout::Binaural, mat: &IAMF512_BS020, m: 8, n: 2 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout512, out_system: OutputLayout::Mono, mat: &IAMF512_MONO, m: 8, n: 1 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout512, out_system: OutputLayout::Layout916, mat: &IAMF512_IAMF916, m: 8, n: 16 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout514, out_system: OutputLayout::Stereo, mat: &IAMF514_BS020, m: 10, n: 2 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout514, out_system: OutputLayout::Layout51, mat: &IAMF514_BS050, m: 10, n: 6 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout514, out_system: OutputLayout::Layout512, mat: &IAMF514_BS250, m: 10, n: 8 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout514, out_system: OutputLayout::Layout514, mat: &IAMF514_BS450, m: 10, n: 10 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout514, out_system: OutputLayout::SoundSystemE451, mat: &IAMF514_BS451, m: 10, n: 11 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout514, out_system: OutputLayout::SoundSystemF370, mat: &IAMF514_BS370, m: 10, n: 12 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout514, out_system: OutputLayout::SoundSystemG490, mat: &IAMF514_BS490, m: 10, n: 14 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout514, out_system: OutputLayout::LayoutA293, mat: &IAMF514_BS9A3, m: 10, n: 24 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout514, out_system: OutputLayout::Layout71, mat: &IAMF514_BS070, m: 10, n: 8 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout514, out_system: OutputLayout::Layout714, mat: &IAMF514_BS470, m: 10, n: 12 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout514, out_system: OutputLayout::Layout312, mat: &IAMF514_IAMF312, m: 10, n: 6 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout514, out_system: OutputLayout::Layout712, mat: &IAMF514_IAMF712, m: 10, n: 10 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout514, out_system: OutputLayout::Binaural, mat: &IAMF514_BS020, m: 10, n: 2 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout514, out_system: OutputLayout::Mono, mat: &IAMF514_MONO, m: 10, n: 1 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout514, out_system: OutputLayout::Layout916, mat: &IAMF514_IAMF916, m: 10, n: 16 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout71, out_system: OutputLayout::Stereo, mat: &IAMF71_BS020, m: 8, n: 2 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout71, out_system: OutputLayout::Layout51, mat: &IAMF71_BS050, m: 8, n: 6 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout71, out_system: OutputLayout::Layout512, mat: &IAMF71_BS250, m: 8, n: 8 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout71, out_system: OutputLayout::Layout514, mat: &IAMF71_BS450, m: 8, n: 10 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout71, out_system: OutputLayout::SoundSystemE451, mat: &IAMF71_BS451, m: 8, n: 11 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout71, out_system: OutputLayout::SoundSystemF370, mat: &IAMF71_BS370, m: 8, n: 12 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout71, out_system: OutputLayout::SoundSystemG490, mat: &IAMF71_BS490, m: 8, n: 14 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout71, out_system: OutputLayout::LayoutA293, mat: &IAMF71_BS9A3, m: 8, n: 24 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout71, out_system: OutputLayout::Layout71, mat: &IAMF71_BS070, m: 8, n: 8 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout71, out_system: OutputLayout::Layout714, mat: &IAMF71_BS470, m: 8, n: 12 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout71, out_system: OutputLayout::Layout312, mat: &IAMF71_IAMF312, m: 8, n: 6 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout71, out_system: OutputLayout::Layout712, mat: &IAMF71_IAMF712, m: 8, n: 10 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout71, out_system: OutputLayout::Binaural, mat: &IAMF71_BS020, m: 8, n: 2 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout71, out_system: OutputLayout::Mono, mat: &IAMF71_MONO, m: 8, n: 1 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout71, out_system: OutputLayout::Layout916, mat: &IAMF71_IAMF916, m: 8, n: 16 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout712, out_system: OutputLayout::Stereo, mat: &IAMF712_BS020, m: 10, n: 2 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout712, out_system: OutputLayout::Layout51, mat: &IAMF712_BS050, m: 10, n: 6 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout712, out_system: OutputLayout::Layout512, mat: &IAMF712_BS250, m: 10, n: 8 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout712, out_system: OutputLayout::Layout514, mat: &IAMF712_BS450, m: 10, n: 10 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout712, out_system: OutputLayout::SoundSystemE451, mat: &IAMF712_BS451, m: 10, n: 11 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout712, out_system: OutputLayout::SoundSystemF370, mat: &IAMF712_BS370, m: 10, n: 12 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout712, out_system: OutputLayout::SoundSystemG490, mat: &IAMF712_BS490, m: 10, n: 14 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout712, out_system: OutputLayout::LayoutA293, mat: &IAMF712_BS9A3, m: 10, n: 24 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout712, out_system: OutputLayout::Layout71, mat: &IAMF712_BS070, m: 10, n: 8 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout712, out_system: OutputLayout::Layout714, mat: &IAMF712_BS470, m: 10, n: 12 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout712, out_system: OutputLayout::Layout312, mat: &IAMF712_IAMF312, m: 10, n: 6 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout712, out_system: OutputLayout::Layout712, mat: &IAMF712_IAMF712, m: 10, n: 10 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout712, out_system: OutputLayout::Binaural, mat: &IAMF712_BS020, m: 10, n: 2 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout712, out_system: OutputLayout::Mono, mat: &IAMF712_MONO, m: 10, n: 1 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout712, out_system: OutputLayout::Layout916, mat: &IAMF712_IAMF916, m: 10, n: 16 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout714, out_system: OutputLayout::Stereo, mat: &IAMF714_BS020, m: 12, n: 2 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout714, out_system: OutputLayout::Layout51, mat: &IAMF714_BS050, m: 12, n: 6 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout714, out_system: OutputLayout::Layout512, mat: &IAMF714_BS250, m: 12, n: 8 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout714, out_system: OutputLayout::Layout514, mat: &IAMF714_BS450, m: 12, n: 10 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout714, out_system: OutputLayout::SoundSystemE451, mat: &IAMF714_BS451, m: 12, n: 11 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout714, out_system: OutputLayout::SoundSystemF370, mat: &IAMF714_BS370, m: 12, n: 12 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout714, out_system: OutputLayout::SoundSystemG490, mat: &IAMF714_BS490, m: 12, n: 14 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout714, out_system: OutputLayout::LayoutA293, mat: &IAMF714_BS9A3, m: 12, n: 24 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout714, out_system: OutputLayout::Layout71, mat: &IAMF714_BS070, m: 12, n: 8 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout714, out_system: OutputLayout::Layout714, mat: &IAMF714_BS470, m: 12, n: 12 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout714, out_system: OutputLayout::Layout312, mat: &IAMF714_IAMF312, m: 12, n: 6 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout714, out_system: OutputLayout::Layout712, mat: &IAMF714_IAMF712, m: 12, n: 10 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout714, out_system: OutputLayout::Binaural, mat: &IAMF714_BS020, m: 12, n: 2 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout714, out_system: OutputLayout::Mono, mat: &IAMF714_MONO, m: 12, n: 1 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout714, out_system: OutputLayout::Layout916, mat: &IAMF714_IAMF916, m: 12, n: 16 },
    ChannelToChannelRenderer { in_system: InputLayout::Binaural, out_system: OutputLayout::Stereo, mat: &STEREO_BS020, m: 2, n: 2 },
    ChannelToChannelRenderer { in_system: InputLayout::Binaural, out_system: OutputLayout::Layout51, mat: &STEREO_BS050, m: 2, n: 6 },
    ChannelToChannelRenderer { in_system: InputLayout::Binaural, out_system: OutputLayout::Layout512, mat: &STEREO_BS250, m: 2, n: 8 },
    ChannelToChannelRenderer { in_system: InputLayout::Binaural, out_system: OutputLayout::Layout514, mat: &STEREO_BS450, m: 2, n: 10 },
    ChannelToChannelRenderer { in_system: InputLayout::Binaural, out_system: OutputLayout::SoundSystemE451, mat: &STEREO_BS451, m: 2, n: 11 },
    ChannelToChannelRenderer { in_system: InputLayout::Binaural, out_system: OutputLayout::SoundSystemF370, mat: &STEREO_BS370, m: 2, n: 12 },
    ChannelToChannelRenderer { in_system: InputLayout::Binaural, out_system: OutputLayout::SoundSystemG490, mat: &STEREO_BS490, m: 2, n: 14 },
    ChannelToChannelRenderer { in_system: InputLayout::Binaural, out_system: OutputLayout::LayoutA293, mat: &STEREO_BS9A3, m: 2, n: 24 },
    ChannelToChannelRenderer { in_system: InputLayout::Binaural, out_system: OutputLayout::Layout71, mat: &STEREO_BS070, m: 2, n: 8 },
    ChannelToChannelRenderer { in_system: InputLayout::Binaural, out_system: OutputLayout::Layout714, mat: &STEREO_BS470, m: 2, n: 12 },
    ChannelToChannelRenderer { in_system: InputLayout::Binaural, out_system: OutputLayout::Layout312, mat: &STEREO_IAMF312, m: 2, n: 6 },
    ChannelToChannelRenderer { in_system: InputLayout::Binaural, out_system: OutputLayout::Layout712, mat: &STEREO_IAMF712, m: 2, n: 10 },
    ChannelToChannelRenderer { in_system: InputLayout::Binaural, out_system: OutputLayout::Binaural, mat: &STEREO_BS020, m: 2, n: 2 },
    ChannelToChannelRenderer { in_system: InputLayout::Binaural, out_system: OutputLayout::Mono, mat: &STEREO_MONO, m: 2, n: 1 },
    ChannelToChannelRenderer { in_system: InputLayout::Binaural, out_system: OutputLayout::Layout916, mat: &STEREO_IAMF916, m: 2, n: 16 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout916, out_system: OutputLayout::Stereo, mat: &IAMF916_BS020, m: 16, n: 2 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout916, out_system: OutputLayout::Layout51, mat: &IAMF916_BS050, m: 16, n: 6 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout916, out_system: OutputLayout::Layout512, mat: &IAMF916_BS250, m: 16, n: 8 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout916, out_system: OutputLayout::Layout514, mat: &IAMF916_BS450, m: 16, n: 10 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout916, out_system: OutputLayout::SoundSystemE451, mat: &IAMF916_BS451, m: 16, n: 11 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout916, out_system: OutputLayout::SoundSystemF370, mat: &IAMF916_BS370, m: 16, n: 12 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout916, out_system: OutputLayout::SoundSystemG490, mat: &IAMF916_BS490, m: 16, n: 14 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout916, out_system: OutputLayout::LayoutA293, mat: &IAMF916_BS9A3, m: 16, n: 24 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout916, out_system: OutputLayout::Layout71, mat: &IAMF916_BS070, m: 16, n: 8 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout916, out_system: OutputLayout::Layout714, mat: &IAMF916_BS470, m: 16, n: 12 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout916, out_system: OutputLayout::Layout312, mat: &IAMF916_IAMF312, m: 16, n: 6 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout916, out_system: OutputLayout::Layout712, mat: &IAMF916_IAMF712, m: 16, n: 10 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout916, out_system: OutputLayout::Binaural, mat: &IAMF916_BS020, m: 16, n: 2 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout916, out_system: OutputLayout::Mono, mat: &IAMF916_MONO, m: 16, n: 1 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout916, out_system: OutputLayout::Layout916, mat: &IAMF916_IAMF916, m: 16, n: 16 },
    ChannelToChannelRenderer { in_system: InputLayout::Mono, out_system: OutputLayout::Layout7154, mat: &MONO_IAMF7154, m: 1, n: 17 },
    ChannelToChannelRenderer { in_system: InputLayout::Stereo, out_system: OutputLayout::Layout7154, mat: &STEREO_IAMF7154, m: 2, n: 17 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout312, out_system: OutputLayout::Layout7154, mat: &IAMF312_IAMF7154, m: 6, n: 17 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout51, out_system: OutputLayout::Layout7154, mat: &IAMF51_IAMF7154, m: 6, n: 17 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout512, out_system: OutputLayout::Layout7154, mat: &IAMF512_IAMF7154, m: 8, n: 17 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout514, out_system: OutputLayout::Layout7154, mat: &IAMF514_IAMF7154, m: 10, n: 17 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout71, out_system: OutputLayout::Layout7154, mat: &IAMF71_IAMF7154, m: 8, n: 17 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout712, out_system: OutputLayout::Layout7154, mat: &IAMF712_IAMF7154, m: 10, n: 17 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout714, out_system: OutputLayout::Layout7154, mat: &IAMF714_IAMF7154, m: 12, n: 17 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout916, out_system: OutputLayout::Layout7154, mat: &IAMF916_IAMF7154, m: 16, n: 17 },
    ChannelToChannelRenderer { in_system: InputLayout::Binaural, out_system: OutputLayout::Layout7154, mat: &STEREO_IAMF7154, m: 2, n: 17 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout7154, out_system: OutputLayout::Stereo, mat: &IAMF7154_BS020, m: 17, n: 2 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout7154, out_system: OutputLayout::Layout51, mat: &IAMF7154_BS050, m: 17, n: 6 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout7154, out_system: OutputLayout::Layout512, mat: &IAMF7154_BS250, m: 17, n: 8 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout7154, out_system: OutputLayout::Layout514, mat: &IAMF7154_BS450, m: 17, n: 10 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout7154, out_system: OutputLayout::SoundSystemE451, mat: &IAMF7154_BS451, m: 17, n: 11 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout7154, out_system: OutputLayout::SoundSystemF370, mat: &IAMF7154_BS370, m: 17, n: 12 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout7154, out_system: OutputLayout::SoundSystemG490, mat: &IAMF7154_BS490, m: 17, n: 14 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout7154, out_system: OutputLayout::LayoutA293, mat: &IAMF7154_BS9A3, m: 17, n: 24 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout7154, out_system: OutputLayout::Layout71, mat: &IAMF7154_BS070, m: 17, n: 8 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout7154, out_system: OutputLayout::Layout714, mat: &IAMF7154_BS470, m: 17, n: 12 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout7154, out_system: OutputLayout::Layout312, mat: &IAMF7154_IAMF312, m: 17, n: 6 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout7154, out_system: OutputLayout::Layout712, mat: &IAMF7154_IAMF712, m: 17, n: 10 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout7154, out_system: OutputLayout::Layout916, mat: &IAMF7154_IAMF916, m: 17, n: 16 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout7154, out_system: OutputLayout::Binaural, mat: &IAMF7154_BS020, m: 17, n: 2 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout7154, out_system: OutputLayout::Mono, mat: &IAMF7154_MONO, m: 17, n: 1 },
    ChannelToChannelRenderer { in_system: InputLayout::Layout7154, out_system: OutputLayout::Layout7154, mat: &IAMF7154_IAMF7154, m: 17, n: 17 },
    ChannelToChannelRenderer { in_system: InputLayout::LayoutA293, out_system: OutputLayout::Stereo, mat: &IAMF10293_BS020, m: 24, n: 2 },
    ChannelToChannelRenderer { in_system: InputLayout::LayoutA293, out_system: OutputLayout::Layout51, mat: &IAMF10293_BS050, m: 24, n: 6 },
    ChannelToChannelRenderer { in_system: InputLayout::LayoutA293, out_system: OutputLayout::Layout512, mat: &IAMF10293_BS250, m: 24, n: 8 },
    ChannelToChannelRenderer { in_system: InputLayout::LayoutA293, out_system: OutputLayout::Layout514, mat: &IAMF10293_BS450, m: 24, n: 10 },
    ChannelToChannelRenderer { in_system: InputLayout::LayoutA293, out_system: OutputLayout::SoundSystemE451, mat: &IAMF10293_BS451, m: 24, n: 11 },
    ChannelToChannelRenderer { in_system: InputLayout::LayoutA293, out_system: OutputLayout::SoundSystemF370, mat: &IAMF10293_BS370, m: 24, n: 12 },
    ChannelToChannelRenderer { in_system: InputLayout::LayoutA293, out_system: OutputLayout::SoundSystemG490, mat: &IAMF10293_BS490, m: 24, n: 14 },
    ChannelToChannelRenderer { in_system: InputLayout::LayoutA293, out_system: OutputLayout::LayoutA293, mat: &IAMF10293_BS9A3, m: 24, n: 24 },
    ChannelToChannelRenderer { in_system: InputLayout::LayoutA293, out_system: OutputLayout::Layout71, mat: &IAMF10293_BS070, m: 24, n: 8 },
    ChannelToChannelRenderer { in_system: InputLayout::LayoutA293, out_system: OutputLayout::Layout714, mat: &IAMF10293_BS470, m: 24, n: 12 },
    ChannelToChannelRenderer { in_system: InputLayout::LayoutA293, out_system: OutputLayout::Layout312, mat: &IAMF10293_IAMF312, m: 24, n: 6 },
    ChannelToChannelRenderer { in_system: InputLayout::LayoutA293, out_system: OutputLayout::Layout712, mat: &IAMF10293_IAMF712, m: 24, n: 10 },
    ChannelToChannelRenderer { in_system: InputLayout::LayoutA293, out_system: OutputLayout::Layout916, mat: &IAMF10293_IAMF916, m: 24, n: 16 },
    ChannelToChannelRenderer { in_system: InputLayout::LayoutA293, out_system: OutputLayout::Binaural, mat: &IAMF10293_BS020, m: 24, n: 2 },
    ChannelToChannelRenderer { in_system: InputLayout::LayoutA293, out_system: OutputLayout::Mono, mat: &IAMF10293_MONO, m: 24, n: 1 },
    ChannelToChannelRenderer { in_system: InputLayout::LayoutA293, out_system: OutputLayout::Layout7154, mat: &IAMF10293_IAMF7154, m: 24, n: 17 },
];

/// Retrieves the multichannel-to-multichannel conversion matrix for a full (non-subset) input
/// layout and target output layout.
///
/// # Parameters
/// * `in_system`: Full (non-subset) input layout.
/// * `out_system`: Target output layout.
pub fn element_renderer_get_chan_to_chan_matrix(
    in_system: InputLayout,
    out_system: OutputLayout,
) -> Result<ChannelToChannelRenderer, OarError> {
    CHAN_TO_CHAN_RDR_TAB
        .iter()
        .copied()
        .find(|entry| entry.in_system == in_system && entry.out_system == out_system)
        .ok_or(OarError::NotSupported)
}

/// Retrieves the conversion matrix and superset-layout channel index map for an IAMF
/// channel-subset input layout and target output layout.
///
/// Resolves `subset_layout` to its superset [`InputLayout`] and channel indices via
/// [`InputLayout::superset_and_subset_channels`], and sets `matrix.m` to the active subset channel
/// count.
///
/// # Parameters
/// * `subset_layout`: IAMF channel-subset input layout (returns `Err(OarError::NotSupported)` for
///   full layouts).
/// * `out_system`: Target output layout.
pub fn element_renderer_get_chan_to_chan_custom_matrix(
    subset_layout: InputLayout,
    out_system: OutputLayout,
) -> Result<(ChannelToChannelRenderer, Vec<usize>), OarError> {
    let (superset_system, channel_indices) =
        subset_layout.superset_and_subset_channels().ok_or(OarError::NotSupported)?;
    let mut matrix = element_renderer_get_chan_to_chan_matrix(superset_system, out_system)?;
    let channels_map: Vec<usize> = channel_indices.collect();
    matrix.m = channels_map.len();
    Ok((matrix, channels_map))
}

/// Renders predefined multichannel input signals into multichannel output signals.
///
/// Port of `IAMF_element_renderer_render_M2M`.
///
/// # Parameters
/// * `chan_to_chan_matrix`: Conversion matrix configuration obtained via
///   [`element_renderer_get_chan_to_chan_matrix`].
/// * `inputs`: Validated read-only view of planar input channels.
/// * `outputs`: Validated mutable view of planar output channels to be overwritten with accumulated
///   outputs.
pub fn element_renderer_render_chan_to_chan(
    chan_to_chan_matrix: &ChannelToChannelRenderer,
    inputs: PlanarBufferRef<'_, '_>,
    outputs: &mut PlanarBufferMut<'_, '_>,
) -> Result<(), OarError> {
    if inputs.num_channels() != chan_to_chan_matrix.m
        || outputs.num_channels() != chan_to_chan_matrix.n
    {
        return Err(OarError::InvalidParameter);
    }
    if outputs.num_samples() != inputs.num_samples() {
        return Err(OarError::InvalidParameter);
    }
    let mv = MatrixView {
        data: MatrixData::Sparse {
            entries: chan_to_chan_matrix.mat,
            lookup: &CHAN_TO_CHAN_GAIN_LOOKUP,
        },
        in_dim: chan_to_chan_matrix.m,
        in_stride: chan_to_chan_matrix.n,
        out_dim: chan_to_chan_matrix.n,
        out_stride: 1,
        in_idx_map: None,
    };
    matrix_render(&mv, inputs, outputs)
}

/// Renders custom multichannel input signals into multichannel output signals using channel
/// mapping.
///
/// Port of `IAMF_element_renderer_render_M2M_custom`.
///
/// # Parameters
/// * `chan_to_chan_matrix`: Conversion matrix configuration obtained via
///   [`element_renderer_get_chan_to_chan_custom_matrix`].
/// * `inputs`: Validated read-only view of planar input channels.
/// * `outputs`: Validated mutable view of planar output channels to be accumulated or overwritten.
/// * `chmap`: Channel mapping slice obtained during
///   [`element_renderer_get_chan_to_chan_custom_matrix`].
pub fn element_renderer_render_chan_to_chan_custom(
    chan_to_chan_matrix: &ChannelToChannelRenderer,
    inputs: PlanarBufferRef<'_, '_>,
    outputs: &mut PlanarBufferMut<'_, '_>,
    chmap: &[usize],
) -> Result<(), OarError> {
    if inputs.num_channels() != chan_to_chan_matrix.m
        || outputs.num_channels() != chan_to_chan_matrix.n
    {
        return Err(OarError::InvalidParameter);
    }
    if outputs.num_samples() != inputs.num_samples() {
        return Err(OarError::InvalidParameter);
    }
    let mv = MatrixView {
        data: MatrixData::Sparse {
            entries: chan_to_chan_matrix.mat,
            lookup: &CHAN_TO_CHAN_GAIN_LOOKUP,
        },
        in_dim: chan_to_chan_matrix.m,
        in_stride: chan_to_chan_matrix.n,
        out_dim: chan_to_chan_matrix.n,
        out_stride: 1,
        in_idx_map: Some(chmap),
    };
    matrix_render(&mv, inputs, outputs)
}

// ===== Tests =====

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::units::Samples;
    use googletest::prelude::*;

    const EPSILON: f32 = 1e-6;

    #[gtest]
    fn get_chan_to_chan_matrix_predefined_mono_to_stereo_returns_matching_matrix() {
        let result =
            element_renderer_get_chan_to_chan_matrix(InputLayout::Mono, OutputLayout::Stereo);

        let out_matrix = result.unwrap();
        expect_that!(out_matrix.in_system, eq(InputLayout::Mono));
        expect_that!(out_matrix.out_system, eq(OutputLayout::Stereo));
        expect_that!(out_matrix.m, eq(1));
        expect_that!(out_matrix.n, eq(2));
    }

    #[gtest]
    fn get_chan_to_chan_custom_matrix_subset_layout_returns_superset_matrix_and_channel_map() {
        let result = element_renderer_get_chan_to_chan_custom_matrix(
            InputLayout::ThreeCh,
            OutputLayout::Stereo,
        );

        let (out_matrix, chmap) = result.unwrap();
        expect_that!(out_matrix.in_system, eq(InputLayout::Layout714));
        expect_that!(out_matrix.out_system, eq(OutputLayout::Stereo));
        expect_that!(out_matrix.m, eq(3));
        expect_eq!(chmap.as_slice(), &[0, 1, 2]);
    }

    #[gtest]
    fn render_chan_to_chan_mono_to_stereo_planar_inputs_produces_expected_scaled_outputs() {
        let chan_to_chan_matrix = ChannelToChannelRenderer {
            in_system: InputLayout::Mono,
            out_system: OutputLayout::Stereo,
            mat: &MONO_BS020,
            m: 1,
            n: 2,
        };
        let in_channel = [1.0f32, -1.0f32];
        let mut out_left = [0.0f32; 2];
        let mut out_right = [0.0f32; 2];

        let result = {
            let inputs = [&in_channel[..]];
            let mut out_slices = [&mut out_left[..], &mut out_right[..]];
            let inputs_buf = PlanarBufferRef::new(&inputs, 1, Samples::new(2).unwrap()).unwrap();
            let mut outputs_buf =
                PlanarBufferMut::new(&mut out_slices, 2, Samples::new(2).unwrap()).unwrap();
            element_renderer_render_chan_to_chan(&chan_to_chan_matrix, inputs_buf, &mut outputs_buf)
        };

        assert_ok!(result);
        expect_that!(out_left[0], near(FRAC_1_SQRT_2, EPSILON));
        expect_that!(out_left[1], near(-FRAC_1_SQRT_2, EPSILON));
        expect_that!(out_right[0], near(FRAC_1_SQRT_2, EPSILON));
        expect_that!(out_right[1], near(-FRAC_1_SQRT_2, EPSILON));
    }

    #[gtest]
    fn render_chan_to_chan_custom_subset_channels_with_mapping_accumulates_outputs_using_mapped_indices(
    ) {
        let chan_to_chan_matrix = ChannelToChannelRenderer {
            in_system: InputLayout::Stereo,
            out_system: OutputLayout::Stereo,
            mat: &STEREO_BS020,
            m: 1,
            n: 2,
        };
        let chmap = [1usize];
        let in_channel = [3.0f32, 5.0f32];
        let mut out_left = [0.0f32; 2];
        let mut out_right = [0.0f32; 2];

        let result = {
            let inputs = [&in_channel[..]];
            let mut out_slices = [&mut out_left[..], &mut out_right[..]];
            let inputs_buf = PlanarBufferRef::new(&inputs, 1, Samples::new(2).unwrap()).unwrap();
            let mut outputs_buf =
                PlanarBufferMut::new(&mut out_slices, 2, Samples::new(2).unwrap()).unwrap();
            element_renderer_render_chan_to_chan_custom(
                &chan_to_chan_matrix,
                inputs_buf,
                &mut outputs_buf,
                &chmap,
            )
        };

        assert_ok!(result);
        expect_that!(out_left[0], near(0.0f32, EPSILON));
        expect_that!(out_left[1], near(0.0f32, EPSILON));
        expect_that!(out_right[0], near(3.0f32, EPSILON));
        expect_that!(out_right[1], near(5.0f32, EPSILON));
    }

    #[gtest]
    fn get_chan_to_chan_matrix_subset_layout_returns_not_supported() {
        let result =
            element_renderer_get_chan_to_chan_matrix(InputLayout::StereoS, OutputLayout::Stereo);

        expect_that!(result, err(eq(OarError::NotSupported)));
    }

    #[gtest]
    fn get_chan_to_chan_custom_matrix_full_layout_returns_not_supported() {
        let result = element_renderer_get_chan_to_chan_custom_matrix(
            InputLayout::Layout51,
            OutputLayout::Stereo,
        );

        expect_that!(result, err(eq(&OarError::NotSupported)));
    }

    #[gtest]
    fn render_chan_to_chan_stereo_to_stereo_accumulates_inputs_correctly() {
        let chan_to_chan_matrix = ChannelToChannelRenderer {
            in_system: InputLayout::Stereo,
            out_system: OutputLayout::Stereo,
            mat: &STEREO_BS020,
            m: 2,
            n: 2,
        };
        let in_left = [3.0f32];
        let in_right = [7.0f32];
        let mut out_left = [0.0f32];
        let mut out_right = [0.0f32];

        let result = {
            let inputs = [&in_left[..], &in_right[..]];
            let mut out_slices = [&mut out_left[..], &mut out_right[..]];
            let inputs_buf = PlanarBufferRef::new(&inputs, 2, Samples::new(1).unwrap()).unwrap();
            let mut outputs_buf =
                PlanarBufferMut::new(&mut out_slices, 2, Samples::new(1).unwrap()).unwrap();
            element_renderer_render_chan_to_chan(&chan_to_chan_matrix, inputs_buf, &mut outputs_buf)
        };

        assert_ok!(result);
        expect_that!(out_left[0], eq(3.0f32));
        expect_that!(out_right[0], eq(7.0f32));
    }
}
