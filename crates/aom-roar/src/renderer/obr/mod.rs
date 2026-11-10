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

//! Open Binaural RoarRenderer (OBR) sub-renderer module.
//!
//! This subsystem implements the OBR, which processes channel-based, scene-based (Ambisonic),
//! and object-based audio inputs to produce binaural headphone output.
//! It supports head tracking to dynamically rotate the audio scene based on the listener's
//! orientation.
//!
//! All DSP processing in this subsystem must be allocation-free and suitable for execution on
//! realtime threads.

pub mod ambisonic_binaural_decoder;
pub mod ambisonic_encoder;
pub mod ambisonic_rotator;
pub mod common;
pub mod renderer;
