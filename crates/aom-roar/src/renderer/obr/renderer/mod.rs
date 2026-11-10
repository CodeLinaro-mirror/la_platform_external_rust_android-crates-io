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

//! High-level OBR orchestrator and processing groups.
//!
//! This module contains the main entry points and high-level orchestrations for the OBR subsystem,
//! including the top-level `ObrImpl` struct and `ProcessingGroup` for grouping audio elements that
//! share binaural filter profiles and Ambisonic orders.
//!
//! These components coordinate the rendering lifecycle, mapping inputs (loudspeaker channels,
//! object channels, or Ambisonic scenes) to the appropriate internal representations and invoking
//! the DSP components.

pub mod audio_element_config;
pub mod obr_impl;
pub mod processing_group;

pub use obr_impl::ObrAudioRendererAdapter;
