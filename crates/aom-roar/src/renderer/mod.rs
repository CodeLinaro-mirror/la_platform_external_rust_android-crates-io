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

//! RoarRenderer module for the Open Audio RoarRenderer (OAR).
//!
//! Coordinates the different sub-renderers:
//! - **OBR** (Open Binaural RoarRenderer): Renders any input for binaural.
//! - **EAR** (EBU ADM RoarRenderer): Renders channel-based and ambisonic inputs to loudspeaker
//!   layouts.
//! - **OLR** (Object Loudspeaker RoarRenderer): Renders object-based inputs to loudspeaker layouts.
//! - **Downmix**: Converts multi-channel loudspeaker layouts to lower loudspeaker configurations.

pub mod audio_elements_renderer;
pub mod audio_renderer_api;
pub mod downmix;
pub mod ear;
pub mod gain_parameter;
pub mod obr;
pub mod olr;
pub mod roar_renderer;
pub use roar_renderer::RoarRenderer;
