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

//! OLR sub-renderer for object to loudspeaker rendering.
//!
//! This module provides the core implementation of the OLR, which renders
//! object-based audio elements to target loudspeaker layouts. It coordinates
//! speaker layouts, gain calculations, metadata interpretation, and block-based
//! audio processing using VBAP and DBAP.

pub mod block_processing_channel;
pub mod convex_hull;
pub mod custom_gain_calculator;

// TODO(b/525080422): Get rid of the use of this magic number.
/// Sentinel angle value representing "no angle" or "unset", matching the C reference's
/// DEF_NONE_DEGREE.
pub const SENTINEL_ANGLE_DEGREES: f32 = 361.0;
pub mod dbap;
pub mod gain_calculator;
pub mod interpret_object_metadata;
pub mod object_renderer;
pub mod processing_block;
pub mod vbap;
pub mod vbap_2d;
