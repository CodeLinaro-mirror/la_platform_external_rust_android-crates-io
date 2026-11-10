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

//! Mathematical and architectural constants.
//!
//! Defines memory alignments, maximum channel counts, and mathematical variables
//! shared within the OBR subsystem.

/// Minimum Ambisonic order currently supported by OBR.
pub const MIN_SUPPORTED_AMBISONIC_ORDER: i32 = 1;

/// Maximum Ambisonic order currently supported by OBR (limited by the
/// available SH-HRIRs provided via binaural_filters).
pub const MAX_SUPPORTED_AMBISONIC_ORDER: i32 = 4;

// TODO(b/512062316): This may be a maximum supported by OBR but the OAR limit may be lower.  Also,
// can we avoid allocating slice references for full support when we know we have less?
/// Maximum number of input channels supported by OBR.
pub const MAX_SUPPORTED_NUM_INPUT_CHANNELS: usize = 128;

/// Maximum allowed size of internal buffers.
pub const MAX_SUPPORTED_NUM_FRAMES: usize = 16384;

/// Number of binaural channels.
pub const NUM_BINAURAL_CHANNELS: usize = 2;

/// Number of mono channels.
pub const NUM_MONO_CHANNELS: usize = 1;

/// Number of stereo channels.
pub const NUM_STEREO_CHANNELS: usize = 2;

/// Negative 120dB in amplitude.
pub const NEGATIVE_120DB_IN_AMPLITUDE: f32 = 0.000001;

/// Tolerated error margins for floating points.
pub const EPSILON_FLOAT: f32 = 1e-6;
