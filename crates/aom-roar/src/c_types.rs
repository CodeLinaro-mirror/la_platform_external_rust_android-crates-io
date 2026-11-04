// Copyright (c) 2026, Alliance for Open Media. All rights reserved
//
// This source code is subject to the terms of the BSD 3-Clause Clear License
// and the Alliance for Open Media Patent License 1.0. If the BSD 3-Clause Clear
// License was not distributed with this source code in the LICENSE file, you
// can obtain it at www.aomedia.org/license/software-license/bsd-3-c-c. If the
// Alliance for Open Media Patent License 1.0 was not distributed with this
// source code in the PATENTS file, you can obtain it at
// www.aomedia.org/license/patent.

//! C-compatible type layout structures and unions for FFI compatibility.
//!
//! This module contains the exact `#[repr(C)]` layouts of ROAR C API structures,
//! enums, and unions matching `include/roar.h`. All raw pointer conversions and
//! dynamic typestate bridging are handled here or delegated to other safe Layer 2
//! modules.

#![allow(non_camel_case_types, non_upper_case_globals, non_snake_case)]

use std::os::raw::c_int;

/// Maximum number of objects supported in object position metadata.
pub const ROAR_MAX_NUMBER_OF_OBJECTS: usize = 2;

/// Flag indicating IAMF downmix info is active in `roar_parameter_set_t::flags`.
pub const ROAR_PARAMETER_SET_FLAG_IAMF_DOWNMIX_INFO: u32 = 0x01;

/// Flag indicating IAMF element rendering configuration is active in `roar_parameter_set_t::flags`.
pub const ROAR_PARAMETER_SET_FLAG_IAMF_ELEMENT_RENDERING_CONFIG: u32 = 0x02;

/// Opaque wrapper structure for `Roar` C consumers.
#[repr(C)]
pub struct roar_t {
    /// Opaque private fields to prevent instantiation or inspection from C/FFI side.
    pub _private: [u8; 0],
}

/// Status codes returned by ROAR C API functions.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum roar_status_t {
    /// Operation succeeded.
    ck_roar_ok = 0,
    /// Out of memory error.
    ck_roar_error_nomem = -12,
    /// Resource busy or prerequisite not met.
    ck_roar_error_busy = -16,
    /// Invalid parameters error.
    ck_roar_error_inval = -22,
    /// Function not implemented error.
    ck_roar_error_nosys = -38,
    /// Operation not supported error.
    ck_roar_error_notsup = -95,
}

/// Audio element layouts and target layouts.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum roar_layout_t {
    /// No layout specified.
    ck_roar_layout_none = 0,
    /// Mono channel layout.
    ck_roar_layout_mono = 1,
    /// Stereo channel layout.
    ck_roar_layout_stereo = 2,
    /// 5.1 channel layout.
    ck_roar_layout_51 = 3,
    /// 5.1.2 channel layout.
    ck_roar_layout_512 = 4,
    /// 5.1.4 channel layout.
    ck_roar_layout_514 = 5,
    /// 7.1 channel layout.
    ck_roar_layout_71 = 6,
    /// 7.1.2 channel layout.
    ck_roar_layout_712 = 7,
    /// 7.1.4 channel layout.
    ck_roar_layout_714 = 8,
    /// 3.1.2 channel layout.
    ck_roar_layout_312 = 9,
    /// 9.1.6 channel layout.
    ck_roar_layout_916 = 10,
    /// 10.2.9.3 (A293) channel layout.
    ck_roar_layout_a293 = 11,
    /// 7.1.5.4 channel layout.
    ck_roar_layout_7154 = 12,

    /// Sound system E (4.5.1) layout.
    ck_roar_layout_sound_system_e_451 = 0x81,
    /// Sound system F (3.7.0) layout.
    ck_roar_layout_sound_system_f_370 = 0x82,
    /// Sound system G (4.9.0) layout.
    ck_roar_layout_sound_system_g_490 = 0x83,

    /// Binaural output layout.
    ck_roar_layout_binaural = 0xFF,

    /// Beginning of the IAMF expanded/subset layouts range.
    ck_roar_layout_subset_start = 0x100,
    /// Low-frequency effects (LFE) subset of 7.1.4.
    ck_roar_layout_lfe = 0x101,
    /// Surround subset (Ls/Rs) of 5.1.4.
    ck_roar_layout_stereo_s = 0x102,
    /// Side surround subset (Lss/Rss) of 7.1.4.
    ck_roar_layout_stereo_ss = 0x103,
    /// Rear surround subset (Lrs/Rrs) of 7.1.4.
    ck_roar_layout_stereo_rs = 0x104,
    /// Top front subset (Ltf/Rtf) of 7.1.4.
    ck_roar_layout_stereo_tf = 0x105,
    /// Top back subset (Ltb/Rtb) of 7.1.4.
    ck_roar_layout_stereo_tb = 0x106,
    /// Top 4 channels (Ltf/Rtf/Ltb/Rtb) of 7.1.4.
    ck_roar_layout_top_4ch = 0x107,
    /// Front 3 channels (L/C/R) of 7.1.4.
    ck_roar_layout_3ch = 0x108,
    /// Front subset (FL/FR) of 9.1.6.
    ck_roar_layout_stereo_f = 0x109,
    /// Side subset (SiL/SiR) of 9.1.6.
    ck_roar_layout_stereo_si = 0x10a,
    /// Top side subset (TpSiL/TpSiR) of 9.1.6.
    ck_roar_layout_stereo_tpsi = 0x10b,
    /// Top 6 channels (TpFL/TpFR/TpBL/TpBR/TpSiL/TpSiR) of 9.1.6.
    ck_roar_layout_top_6ch = 0x10c,
    /// Low-frequency effects subset (LFE1/LFE2) of 10.2.9.3.
    ck_roar_layout_lfe_pair = 0x10d,
    /// Bottom 3 channels (BtFC/BtFL/BtFR) of 10.2.9.3.
    ck_roar_layout_bottom_3ch = 0x10e,
    /// Bottom 4 channels (BtFL/BtFR/BtBL/BtBR) of 7.1.5.4.
    ck_roar_layout_bottom_4ch = 0x10f,
    /// Top subset (TpC) of 7.1.5.4.
    ck_roar_layout_top_1ch = 0x110,
    /// Top 5 channels (Ltf/Rtf/TpC/Ltb/Rtb) of 7.1.5.4.
    ck_roar_layout_top_5ch = 0x111,
}

impl roar_layout_t {
    /// Sound system A (0.2.0) layout (Stereo alias).
    pub const ck_roar_layout_sound_system_a_020: Self = Self::ck_roar_layout_stereo;
    /// Sound system B (0.5.0) layout (5.1 alias).
    pub const ck_roar_layout_sound_system_b_050: Self = Self::ck_roar_layout_51;
    /// Sound system C (2.5.0) layout (5.1.2 alias).
    pub const ck_roar_layout_sound_system_c_250: Self = Self::ck_roar_layout_512;
    /// Sound system D (4.5.0) layout (5.1.4 alias).
    pub const ck_roar_layout_sound_system_d_450: Self = Self::ck_roar_layout_514;
    /// Sound system I (0.7.0) layout (7.1 alias).
    pub const ck_roar_layout_sound_system_i_070: Self = Self::ck_roar_layout_71;
    /// Sound system J (4.7.0) layout (7.1.4 alias).
    pub const ck_roar_layout_sound_system_j_470: Self = Self::ck_roar_layout_714;
    /// Sound system H (9.a.3) layout (10.2.9.3 / A293 alias).
    pub const ck_roar_layout_sound_system_h_9a3: Self = Self::ck_roar_layout_a293;
}

/// Higher-Order Ambisonics (HOA) orders.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum roar_hoa_t {
    /// No HOA order specified.
    ck_roar_hoa_none = -1,
    /// Zeroth-Order Ambisonics (1 channel).
    ck_roar_zoa = 0,
    /// First-Order Ambisonics (4 channels).
    ck_roar_1oa = 1,
    /// Second-Order Ambisonics (9 channels).
    ck_roar_2oa = 2,
    /// Third-Order Ambisonics (16 channels).
    ck_roar_3oa = 3,
    /// Fourth-Order Ambisonics (25 channels).
    ck_roar_4oa = 4,
}

/// Types of audio elements.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum roar_audio_element_type_t {
    /// No audio element type specified.
    ck_roar_element_type_none = -1,
    /// Channel-based audio element (static speaker layout).
    ck_roar_channel_based = 0,
    /// Scene-based audio element (Ambisonics).
    ck_roar_scene_based = 1,
    /// Object-based audio element (spatialized 3D point source).
    ck_roar_object_based = 2,
}

/// Core configuration settings for initializing a ROAR renderer instance.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct roar_config_t {
    /// Target playback layout (e.g., Stereo, 5.1, Binaural).
    pub target_layout: i32,
    /// Number of samples to process per channel per block, [32, 16384].
    pub samples_per_channel: u32,
    /// Audio sampling rate in Hz (e.g., 48000).
    pub sampling_rate: u32,
}

/// Configuration details for a channel-based audio element.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct roar_channel_based_config_t {
    /// The input channel layout configuration.
    pub layout: i32,
}

/// Configuration details for a scene-based (Ambisonics) audio element.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct roar_scene_based_config_t {
    /// The Ambisonic order of the scene input.
    pub order: i32,
}

/// Configuration details for an object-based audio element.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct roar_object_based_config_t {
    /// Number of audio objects inside this element (must be between 1 and 2).
    pub num_objects: c_int,
}

/// Union of specific configurations for different audio element types.
#[repr(C)]
#[derive(Clone, Copy)]
pub union roar_audio_element_config_union_t {
    /// Configuration for channel-based rendering.
    pub cbc: roar_channel_based_config_t,
    /// Configuration for scene-based (Ambisonic) rendering.
    pub sbc: roar_scene_based_config_t,
    /// Configuration for object-based rendering.
    pub obc: roar_object_based_config_t,
}

/// Unified structure describing an audio element's configuration.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct roar_audio_element_config_t {
    /// The specific type of the audio element.
    pub r#type: i32,
    /// Type-specific inner configuration parameters.
    pub config: roar_audio_element_config_union_t,
    /// Additional parameters (such as demixing info and headphone rendering configurations).
    pub parameters: roar_parameter_set_t,
}

/// Headphone rendering modes for audio element playback over headphones.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum roar_headphones_rendering_mode_t {
    /// World-locked restricted rendering (renders to stereo loudspeakers).
    ck_roar_world_locked_restricted = 0,
    /// Fully world-locked rendering (compensates for head tracking).
    ck_roar_world_locked = 1,
    /// Head-locked rendering (fixed to listener head coordinates).
    ck_roar_head_locked = 2,
    /// Reserved mode.
    ck_roar_reserved = 3,
}

/// Binaural filter profile settings for binaural rendering.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum roar_binaural_filter_profile_t {
    /// Ambient binaural filter profile.
    ck_roar_ambient = 0,
    /// Direct binaural filter profile.
    ck_roar_direct = 1,
    /// Reverberant binaural filter profile.
    ck_roar_reverberant = 2,
}

impl roar_binaural_filter_profile_t {
    /// Default binaural filter profile.
    pub const ck_roar_binaural_filter_profile_default: Self = Self::ck_roar_ambient;
}

/// Coordinate systems used for positioning audio objects.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum roar_coordinate_type_t {
    /// Spherical polar coordinate system (azimuth, elevation, distance).
    ck_roar_polar = 0,
    /// Cartesian coordinate system (x, y, z).
    ck_roar_cartesian = 1,
}

/// 3D spherical polar coordinates.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct roar_polar_t {
    /// Horizontal angle in degrees, range: -180.0 to 180.0.
    pub azimuth: f32,
    /// Vertical angle in degrees, range: -90.0 to 90.0.
    pub elevation: f32,
    /// Distance from listener, range: 0.0 to 1.0 (normalized).
    pub distance: f32,
}

/// 3D Cartesian coordinates.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct roar_cartesian_t {
    /// X coordinate (left/right), range: -1.0 to 1.0.
    pub x: f32,
    /// Y coordinate (front/back), range: -1.0 to 1.0.
    pub y: f32,
    /// Z coordinate (up/down), range: -1.0 to 1.0.
    pub z: f32,
}

/// Quaternion representation of 3D rotations (typically head orientations).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct roar_quaternion_t {
    /// Scalar component.
    pub w: f32,
    /// X imaginary component.
    pub x: f32,
    /// Y imaginary component.
    pub y: f32,
    /// Z imaginary component.
    pub z: f32,
}

/// Block of planar audio data passed across the FFI boundary.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct roar_audio_block_t {
    /// Raw mutable pointer to contiguous planar `f32` audio samples.
    pub data: *mut f32,
    /// Number of audio channels in the block.
    pub channels: u32,
    /// Number of samples per channel.
    pub samples_per_channel: u32,
}

/// Initial IAMF downmix and demixing parameters supplied with a channel-based element config.
///
/// Activates the Downmix renderer when `ROAR_PARAMETER_SET_FLAG_IAMF_DOWNMIX_INFO` is set in
/// `roar_parameter_set_t::flags` and the element's input layout forms a supported IAMF downmix pair
/// with the effective loudspeaker target layout.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct roar_downmix_info_t {
    /// Initial IAMF downmix mode index (valid values: `0, 1, 2, 4, 5, 6`; `3` is reserved).
    pub mode: c_int,
    /// Initial demixing weight parameter index (`[0, 10]`; values outside `[0, 10]` leave the
    /// initial weight index unset).
    pub weight_index: c_int,
}

/// Headphone and binaural rendering instructions for audio elements.
///
/// Enum fields are stored as raw `u32` values rather than Rust `enum`s so out-of-range
/// discriminants from C callers can be validated without triggering Rust undefined behavior.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct roar_audio_element_rendering_config_t {
    /// Headphone rendering spatialization mode (`roar_headphones_rendering_mode_t`).
    pub headphones_rendering_mode: u32,
    /// Selected binaural filter profile (`roar_binaural_filter_profile_t`).
    pub binaural_filter_profile: u32,
}

/// Element parameter set flags and configuration values.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct roar_parameter_set_t {
    /// Flags indicating which configuration components are active.
    pub flags: u32,
    /// Demixing parameter block.
    pub downmix_info: roar_downmix_info_t,
    /// Audio element rendering configuration block.
    pub element_rendering_config: roar_audio_element_rendering_config_t,
}

impl roar_parameter_set_t {
    /// Flag indicating IAMF downmix info is active.
    pub const ROAR_PARAMETER_SET_FLAG_IAMF_DOWNMIX_INFO: u32 =
        ROAR_PARAMETER_SET_FLAG_IAMF_DOWNMIX_INFO;
    /// Flag indicating IAMF element rendering configuration is active.
    pub const ROAR_PARAMETER_SET_FLAG_IAMF_ELEMENT_RENDERING_CONFIG: u32 =
        ROAR_PARAMETER_SET_FLAG_IAMF_ELEMENT_RENDERING_CONFIG;
}

/// ROAR metadata category definitions.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum roar_metadata_type_t {
    /// Unspecified metadata type.
    ck_roar_metadata_none = 0,
    /// Gain/Volume adjustment metadata (supported on both audio elements and audio groups).
    ck_roar_metadata_gain = 1,
    /// Advisory IAMF downmix mode metadata for audio elements.
    ///
    /// Takes effect only when the element's active rendering path has a downmix consumer
    /// (`DownmixRenderer`); accepted and ignored (`ROAR_STATUS_OK`) on other rendering paths.
    ck_roar_metadata_iamf_downmix_mode = 2,
    /// 3D coordinates metadata for object-based spatial audio elements.
    ck_roar_metadata_object_positions = 3,
    /// Rotational tracking offsets for listener head orientation on audio groups.
    ck_roar_metadata_head_rotation = 4,
    /// Count of supported metadata categories (sentinel; rejected if passed as a metadata type).
    ck_roar_metadata_count = 5,
}

/// Dynamic parameter temporal characteristics.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum roar_param_type_t {
    /// Constant parameter value across the block.
    ck_roar_param_constant = 0,
    /// Multiple distinct values (e.g., per-sample array).
    ck_roar_param_multiple = 1,
    /// Parameter values generated via dynamic animation keyframes.
    ck_roar_param_animated = 2,
}

/// Union of underlying value types for audio gain control metadata.
#[repr(C)]
#[derive(Clone, Copy)]
pub union roar_metadata_gain_union_t {
    /// Fixed/Static gain value (specified in dB).
    pub constant_gain: f32,
    /// Raw pointer to an array containing one gain value per sample.
    pub gain_array: *mut f32,
    /// Dynamically interpolated animated gains.
    pub animated_gains: roar_animated_float32_t,
}

/// Structure managing volume/gain adjustments with flexible temporal controls.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct roar_metadata_gain_t {
    /// Unique identifier for this gain parameter instance.
    pub id: u32,
    /// Selection representing how the gain value varies across samples (`roar_param_type_t`),
    /// stored as a raw `u32` to avoid undefined behavior from invalid C enum discriminants.
    pub param_type: u32,
    /// The underlying static, sample-array, or animated gain data.
    pub value: roar_metadata_gain_union_t,
}

/// Advisory dynamic IAMF downmix mode metadata payload for audio elements.
///
/// Updates take effect only when the target element's active rendering path has a downmix consumer
/// (`DownmixRenderer`); otherwise `roar_update_audio_element_metadata` accepts and ignores the
/// update with `ROAR_STATUS_OK` (`0`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct roar_metadata_iamf_downmix_mode_t {
    /// IAMF downmix mode identifier (valid values: `0, 1, 2, 4, 5, 6`; `3` and values outside
    /// `[0, 6]` are invalid and return `-22` when a downmix consumer is active on the element's
    /// rendering path).
    pub mode: c_int,
}

/// Union containing polar, Cartesian, or animated coordinate blocks for up to 2 audio objects.
#[repr(C)]
#[derive(Clone, Copy)]
pub union roar_metadata_object_positions_union_t {
    /// Static polar coordinates array.
    pub polar_positions: [roar_polar_t; ROAR_MAX_NUMBER_OF_OBJECTS],
    /// Static Cartesian coordinates array.
    pub cartesian_positions: [roar_cartesian_t; ROAR_MAX_NUMBER_OF_OBJECTS],
    /// Animated polar coordinates array.
    pub animated_polar_positions: [roar_animated_polar_t; ROAR_MAX_NUMBER_OF_OBJECTS],
    /// Animated Cartesian coordinates array.
    pub animated_cartesian_positions: [roar_animated_cartesian_t; ROAR_MAX_NUMBER_OF_OBJECTS],
}

/// Complete 3D object positioning parameter metadata block.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct roar_metadata_object_positions_t {
    /// Selection representing how the positions vary across samples (`roar_param_type_t`),
    /// stored as a raw `u32` to avoid undefined behavior from invalid C enum discriminants.
    pub param_type: u32,
    /// Selected coordinate layout scheme (`roar_coordinate_type_t`), stored as a raw `u32` to
    /// avoid undefined behavior from invalid C enum discriminants.
    pub position_type: u32,
    /// Count of valid objects mapped (valid range: 1 to 2).
    pub num_objects: u32,
    /// Raw polar or Cartesian layout collections representing object points.
    pub positions: roar_metadata_object_positions_union_t,
}

/// Union representing specific metadata values based on metadata type selection.
#[repr(C)]
#[derive(Clone, Copy)]
pub union roar_metadata_union_t {
    /// Active gain adjustment properties.
    pub gain: roar_metadata_gain_t,
    /// Active IAMF downmix settings.
    pub iamf_downmix_mode: roar_metadata_iamf_downmix_mode_t,
    /// Active 3D object positions coordinates block.
    pub object_positions: roar_metadata_object_positions_t,
    /// Active rotational adjustment properties (for head tracking).
    pub head_rotation: roar_quaternion_t,
}

/// Unified API metadata structure passed for dynamic rendering adjustments.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct roar_metadata_t {
    /// Metadata category discriminant (`roar_metadata_type_t`) selecting the active `value` union
    /// field.
    pub r#type: u32,
    /// Target metadata properties block.
    pub value: roar_metadata_union_t,
    /// Duration in samples for which this metadata payload is valid.
    ///
    /// Required (`> 0`) for gain and object positions (`<= 0` is ignored); optional for
    /// `ck_roar_metadata_iamf_downmix_mode` (`<= 0` persists indefinitely until the next downmix
    /// mode update); unused for `ck_roar_metadata_head_rotation`.
    pub duration: c_int,
}

/// Supported interpolation/animation schemes.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum roar_animation_type_t {
    /// Step transition: value remains constant at `start` until next keyframe.
    ck_roar_animation_type_step = 0,
    /// Linear transition: value is interpolated smoothly from `start` to `end`.
    ck_roar_animation_type_linear = 1,
    /// Bezier transition: value curve is generated using control points.
    ck_roar_animation_type_bezier = 2,
}

/// Interpolation parameters defining a single dimensional float animation segment.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct roar_animated_data_float32_t {
    /// Starting parameter value.
    pub start: f32,
    /// Ending target parameter value (ignored in step animation).
    pub end: f32,
    /// Curved control point value (ignored in step/linear animation).
    pub control: f32,
    /// Normalized timing offset of the control point (0.0 to 1.0).
    pub control_relative_time: f32,
}

/// Animated 1D floating-point value.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct roar_animated_float32_t {
    /// Selected interpolation curve type (`roar_animation_type_t`), stored as a raw `u32` to
    /// avoid undefined behavior from invalid C enum discriminants.
    pub animation_type: u32,
    /// Underlying animation segment parameters.
    pub data: roar_animated_data_float32_t,
}

/// Animated polar coordinates.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct roar_animated_polar_t {
    /// Global interpolation curve applied across all dimensional components
    /// (`roar_animation_type_t`), stored as a raw `u32` to avoid undefined behavior from invalid
    /// C enum discriminants.
    pub animation_type: u32,
    /// Horizontal angle animation parameters.
    pub azimuth: roar_animated_data_float32_t,
    /// Vertical angle animation parameters.
    pub elevation: roar_animated_data_float32_t,
    /// Distance animation parameters.
    pub distance: roar_animated_data_float32_t,
}

/// Animated Cartesian coordinates.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct roar_animated_cartesian_t {
    /// Global interpolation curve applied across all dimensional components
    /// (`roar_animation_type_t`), stored as a raw `u32` to avoid undefined behavior from invalid
    /// C enum discriminants.
    pub animation_type: u32,
    /// X axis position animation parameters.
    pub x: roar_animated_data_float32_t,
    /// Y axis position animation parameters.
    pub y: roar_animated_data_float32_t,
    /// Z axis position animation parameters.
    pub z: roar_animated_data_float32_t,
}

/// A wrapper that manages the audio renderer and input buffers for FFI integration.
///
/// This struct bridges the C API and the safe Rust `RoarRenderer` implementation. It encapsulates
/// the renderer's state and caches input planar buffers for subsequent render calls.
pub struct FfiWrapper {
    /// The underlying active RoarRenderer instance.
    pub renderer: crate::renderer::RoarRenderer,
    /// Cache of input planar buffers for each active element ID.
    pub inputs: std::collections::HashMap<u32, Vec<Vec<f32>>>,
}
