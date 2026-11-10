// Copyright (c) 2026, Alliance for Open Media. All rights reserved
//
// This source code is subject to the terms of the BSD 3-Clause Clear License
// and the Alliance for Open Media Patent License 1.0. If the BSD 3-Clause Clear
// License was not distributed with this source code in the LICENSE file, you
// can obtain it at www.aomedia.org/license/software-license/bsd-3-c-c. If the
// Alliance for Open Media Patent License 1.0 was not distributed with this
// source code in the PATENTS file, you can obtain it at
// www.aomedia.org/license/patent.

//! Rust Open Audio Renderer (ROAR) library.
//!
//! This library provides real-time spatialization, downmixing, and headphones/binaural rendering
//! for channel-based, scene-based (HOA), and object-based audio elements. It serves as a safe,
//! high-performance Rust port of the reference C implementation.

// Core internal modules (hidden from public API)
mod common;
mod ffi_ext;
mod limiter;
mod renderer;
mod utility;

// FFI modules (exposed for C integration and FFI testing)
pub mod c_types;
pub mod ffi;

// ==========================================
//               Public Rust API
// ==========================================

// Main Renderer and APIs
pub use renderer::audio_renderer_api::AudioRenderer;
pub use renderer::RoarRenderer;

// Coordinates
pub use common::coordinates::{CartesianCoordinate, PolarCoordinate};

// Animations
pub use common::animation::{
    Animatable, Animated, AnimatedCartesian, AnimatedPolar, CartesianControlRelativeTime,
    PolarControlRelativeTime,
};

// Audio Buffers
pub use common::audio_buffer::{AudioBuffer, PlanarBufferMut, PlanarBufferRef};

// Configurations
pub use common::config::{
    AudioElementConfig, BinauralFilterProfile, ChannelBasedConfig, ChannelCount, Config,
    ElementRenderingConfig, HeadphonesRenderingMode, ObjectBasedConfig, SceneBasedConfig,
};

// Common Definitions
pub use common::definitions::{GroupId, OarError, Samples};

// Downmix Info
pub use common::downmix::{DownmixInfo, DownmixMode};

// Gains
pub use common::gains::Gain;

// Higher Order Ambisonics
pub use common::hoa::HighOrderAmbisonics;

// Layouts
pub use common::layout::{InputLayout, OutputLayout};

// Object Positions
pub use common::object_positions::ObjectPosition;

// Quaternion for Head Tracking
pub use common::quaternion::Quaternion;

// Units and Gains
pub use common::units::{Decibels, LinearGain, Radians, SampleRate};
