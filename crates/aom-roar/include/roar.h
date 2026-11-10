/*
 * Copyright (c) 2026, Alliance for Open Media. All rights reserved
 *
 * This source code is subject to the terms of the BSD 3-Clause Clear License
 * and the Alliance for Open Media Patent License 1.0. If the BSD 3-Clause Clear
 * License was not distributed with this source code in the LICENSE file, you
 * can obtain it at www.aomedia.org/license/software-license/bsd-3-c-c. If the
 * Alliance for Open Media Patent License 1.0 was not distributed with this
 * source code in the PATENTS file, you can obtain it at
 * www.aomedia.org/license/patent.
 */

#ifndef INCLUDE_ROAR_H_
#define INCLUDE_ROAR_H_

#ifdef __cplusplus
#include <cstdint>
extern "C" {
#else
#include <stdint.h>
#endif

/** Maximum number of audio objects supported per object-based element. */
#define ROAR_MAX_NUMBER_OF_OBJECTS 2

/** Flag indicating `roar_parameter_set_t::downmix_info` is populated. */
#define ROAR_PARAMETER_SET_FLAG_IAMF_DOWNMIX_INFO 0x01

/**
 * Flag indicating `roar_parameter_set_t::element_rendering_config` is
 * populated.
 */
#define ROAR_PARAMETER_SET_FLAG_IAMF_ELEMENT_RENDERING_CONFIG 0x02

/**
 * Opaque handle to a ROAR renderer instance.
 *
 * Thread safety: Separate `roar_t` instances may be used concurrently across
 * threads, while a single `roar_t` instance requires exclusive (non-concurrent)
 * access during any mutating call (`roar_*` taking non-const `roar_t*`).
 */
typedef struct Roar roar_t;

/** Status codes returned by ROAR C API functions. */
typedef enum ERoarStatus {
  ck_roar_ok = 0,             /**< Operation succeeded. */
  ck_roar_error_nomem = -12,  /**< Out of memory error. */
  ck_roar_error_busy = -16,   /**< Resource busy or prerequisite not met. */
  ck_roar_error_inval = -22,  /**< Invalid parameter error. */
  ck_roar_error_nosys = -38,  /**< Function not implemented error. */
  ck_roar_error_notsup = -95, /**< Operation not supported error. */
} roar_status_t;

/**
 * Loudspeaker, binaural, and IAMF expanded subset channel layouts.
 *
 * Values up to `ck_roar_layout_binaural` are valid output target layouts
 * (except `ck_roar_layout_none`). Sound systems E, F, and G are target-only
 * layouts; subset layouts starting after `ck_roar_layout_subset_start` are
 * input-only layouts defined by IAMF
 * (https://aomediacodec.github.io/iamf/#expanded_loudspeaker_layout).
 */
typedef enum ERoarLayout {
  ck_roar_layout_none = 0, /**< No layout specified. */

  /* Audio element input layouts and target output layouts. */
  ck_roar_layout_mono = 1,   /**< Mono (1.0) layout. */
  ck_roar_layout_stereo = 2, /**< Stereo (2.0) layout. */
  ck_roar_layout_51 = 3,     /**< 5.1 surround layout. */
  ck_roar_layout_512 = 4,    /**< 5.1.2 surround layout. */
  ck_roar_layout_514 = 5,    /**< 5.1.4 surround layout. */
  ck_roar_layout_71 = 6,     /**< 7.1 surround layout. */
  ck_roar_layout_712 = 7,    /**< 7.1.2 surround layout. */
  ck_roar_layout_714 = 8,    /**< 7.1.4 surround layout. */
  ck_roar_layout_312 = 9,    /**< 3.1.2 surround layout. */
  ck_roar_layout_916 = 10,   /**< 9.1.6 surround layout. */
  ck_roar_layout_a293 = 11,  /**< 22.2 (10.2.9.3) surround layout. */
  ck_roar_layout_7154 = 12,  /**< 7.1.5.4 surround layout. */

  /* ITU-R BS.2051 sound system aliases. */
  /** Sound System A (0+2+0, stereo alias). */
  ck_roar_layout_sound_system_a_020 = ck_roar_layout_stereo,
  /** Sound System B (0+5+0, 5.1 alias). */
  ck_roar_layout_sound_system_b_050 = ck_roar_layout_51,
  /** Sound System C (2+5+0, 5.1.2 alias). */
  ck_roar_layout_sound_system_c_250 = ck_roar_layout_512,
  /** Sound System D (4+5+0, 5.1.4 alias). */
  ck_roar_layout_sound_system_d_450 = ck_roar_layout_514,
  /** Sound System I (0+7+0, 7.1 alias). */
  ck_roar_layout_sound_system_i_070 = ck_roar_layout_71,
  /** Sound System J (4+7+0, 7.1.4 alias). */
  ck_roar_layout_sound_system_j_470 = ck_roar_layout_714,
  /** Sound System H (9+10+3, 10.2.9.3 alias). */
  ck_roar_layout_sound_system_h_9a3 = ck_roar_layout_a293,

  /* Target-only loudspeaker layouts. */
  /** ITU-R BS.2051 Sound System E (4+5+1, 11 channels). */
  ck_roar_layout_sound_system_e_451 = 0x81,
  /** ITU-R BS.2051 Sound System F (3+7+0 with 2 LFE, 12 channels). */
  ck_roar_layout_sound_system_f_370 = 0x82,
  /** ITU-R BS.2051 Sound System G (4+9+0 with LFE, 14 channels). */
  ck_roar_layout_sound_system_g_490 = 0x83,

  /** Binaural 2-channel layout (valid as both input and target layout). */
  ck_roar_layout_binaural = 0xFF,

  /** Start sentinel for IAMF expanded input-only subset layouts. */
  ck_roar_layout_subset_start = 0x100,
  /** Low-frequency effects subset (LFE) of 7.1.4ch. */
  ck_roar_layout_lfe = 0x101,
  /** Surround subset (Ls/Rs) of 5.1.4ch. */
  ck_roar_layout_stereo_s = 0x102,
  /** Side surround subset (Lss/Rss) of 7.1.4ch. */
  ck_roar_layout_stereo_ss = 0x103,
  /** Rear surround subset (Lrs/Rrs) of 7.1.4ch. */
  ck_roar_layout_stereo_rs = 0x104,
  /** Top front subset (Ltf/Rtf) of 7.1.4ch. */
  ck_roar_layout_stereo_tf = 0x105,
  /** Top back subset (Ltb/Rtb) of 7.1.4ch. */
  ck_roar_layout_stereo_tb = 0x106,
  /** Top 4 channels (Ltf/Rtf/Ltb/Rtb) of 7.1.4ch. */
  ck_roar_layout_top_4ch = 0x107,
  /** Front 3 channels (L/C/R) of 7.1.4ch. */
  ck_roar_layout_3ch = 0x108,
  /** Front subset (FL/FR) of 9.1.6ch. */
  ck_roar_layout_stereo_f = 0x109,
  /** Side subset (SiL/SiR) of 9.1.6ch. */
  ck_roar_layout_stereo_si = 0x10a,
  /** Top side subset (TpSiL/TpSiR) of 9.1.6ch. */
  ck_roar_layout_stereo_tpsi = 0x10b,
  /** Top 6 channels (TpFL/TpFR/TpBL/TpBR/TpSiL/TpSiR) of 9.1.6ch. */
  ck_roar_layout_top_6ch = 0x10c,
  /** Low-frequency effects subset (LFE1/LFE2) of 10.2.9.3ch. */
  ck_roar_layout_lfe_pair = 0x10d,
  /** Bottom 3 channels (BtFC/BtFL/BtFR) of 10.2.9.3ch. */
  ck_roar_layout_bottom_3ch = 0x10e,
  /** Bottom 4 channels (BtFL/BtFR/BtBL/BtBR) of 7.1.5.4ch. */
  ck_roar_layout_bottom_4ch = 0x10f,
  /** Top subset (TpC) of 7.1.5.4ch. */
  ck_roar_layout_top_1ch = 0x110,
  /** Top 5 channels (Ltf/Rtf/TpC/Ltb/Rtb) of 7.1.5.4ch. */
  ck_roar_layout_top_5ch = 0x111,
} roar_layout_t;

/** Higher-Order Ambisonics (HOA) orders (`(order + 1)^2` channels). */
typedef enum ERoarHighOrderAmbisonics {
  ck_roar_hoa_none = -1, /**< No HOA order specified. */
  ck_roar_zoa = 0,       /**< Zeroth-order Ambisonics (1 channel). */
  ck_roar_1oa = 1,       /**< First-order Ambisonics (4 channels). */
  ck_roar_2oa = 2,       /**< Second-order Ambisonics (9 channels). */
  ck_roar_3oa = 3,       /**< Third-order Ambisonics (16 channels). */
  ck_roar_4oa = 4,       /**< Fourth-order Ambisonics (25 channels). */
} roar_hoa_t;

/** Audio element categories. */
typedef enum ERoarAudioElementType {
  ck_roar_element_type_none = -1, /**< No audio element type specified. */
  ck_roar_channel_based = 0,      /**< Channel-based speaker layout element. */
  ck_roar_scene_based = 1,        /**< Scene-based (Ambisonics) element. */
  ck_roar_object_based = 2,       /**< Object-based 3D point-source element. */
} roar_audio_element_type_t;

/**
 * Headphone rendering modes controlling how an element is spatialized when the
 * target output layout is `ck_roar_layout_binaural`.
 *
 * Refer to https://aomediacodec.github.io/iamf/#headphones_rendering_mode.
 */
typedef enum ERoarHeadphonesRenderingMode {
  /** Render to stereo loudspeakers instead of binaural spatialization. */
  ck_roar_world_locked_restricted = 0,
  /** Binaural spatialization compensated for listener head rotation. */
  ck_roar_world_locked = 1,
  /** Binaural spatialization fixed relative to the listener's head. */
  ck_roar_head_locked = 2,
  /** Reserved mode. */
  ck_roar_reserved = 3,
} roar_headphones_rendering_mode_t;

/** Binaural HRIR filter profiles for binaural rendering. */
typedef enum ERoarBinauralFilterProfile {
  ck_roar_ambient = 0,     /**< Ambient binaural filter profile. */
  ck_roar_direct = 1,      /**< Direct binaural filter profile. */
  ck_roar_reverberant = 2, /**< Reverberant binaural filter profile. */
  /** Default binaural filter profile (`ck_roar_ambient`). */
  ck_roar_binaural_filter_profile_default = ck_roar_ambient,
} roar_binaural_filter_profile_t;

/** 3D coordinate system used for object position metadata. */
typedef enum ERoarCoordinateType {
  /** Spherical polar coordinates (`azimuth`, `elevation`, `distance`). */
  ck_roar_polar = 0,
  /** ADM Cartesian coordinates (`x`, `y`, `z`). */
  ck_roar_cartesian = 1,
} roar_coordinate_type_t;

/** Spherical polar coordinates for 3D audio object positioning. */
typedef struct RoarPolarCoordinate {
  /**
   * Horizontal angle in degrees `[-180.0, 180.0]` (`0` = front, `+90` = left,
   * `-90` = right).
   */
  float azimuth;
  /** Vertical angle in degrees `[-90.0, 90.0]`. */
  float elevation;
  /** Normalized distance from listener `[0.0, 1.0]`. */
  float distance;
} roar_polar_t;

/** ADM Cartesian coordinates for 3D audio object positioning. */
typedef struct RoarCartesianCoordinate {
  float x; /**< Left (`-1.0`) to right (`+1.0`). */
  float y; /**< Back (`-1.0`) to front (`+1.0`). */
  float z; /**< Down (`-1.0`) to up (`+1.0`). */
} roar_cartesian_t;

/**
 * Unit quaternion `(w, x, y, z)` representing listener head orientation in the
 * ADM object coordinate system.
 */
typedef struct RoarQuaternion {
  float w; /**< Scalar component. */
  float x; /**< X imaginary component. */
  float y; /**< Y imaginary component. */
  float z; /**< Z imaginary component. */
} roar_quaternion_t;

/**
 * Contiguous planar audio buffer (`channels` planes of `samples_per_channel`
 * `float` samples each).
 */
typedef struct RoarAudioBlock {
  /** Pointer to `channels * samples_per_channel` contiguous planar `float`s. */
  float* data;
  /** Number of audio channels in the block. */
  uint32_t channels;
  /** Number of samples per channel. */
  uint32_t samples_per_channel;
} roar_audio_block_t;

/**
 * Initial IAMF downmix and demixing parameters supplied with a channel-based
 * element configuration.
 *
 * Activates the downmix renderer when
 * `ROAR_PARAMETER_SET_FLAG_IAMF_DOWNMIX_INFO` is set in
 * `roar_parameter_set_t::flags` and the element's input layout forms a
 * supported IAMF downmix pair with the effective loudspeaker target layout.
 *
 * Refer to https://aomediacodec.github.io/iamf/#demixing_info.
 */
typedef struct RoarDownmixInfo {
  /**
   * Initial IAMF `dmixp_mode` index (valid values: `0`, `1`, `2`, `4`, `5`, or
   * `6`; `3` is reserved).
   */
  int mode;
  /**
   * Initial IAMF `default_w` demixing weight parameter index (`[0, 10]`; values
   * outside `[0, 10]` leave `default_w` unset).
   */
  int weight_index;
} roar_downmix_info_t;

/** Headphone rendering and binaural filter configuration for an element. */
typedef struct RoarAudioElementRenderingConfig {
  /** Headphone spatialization mode when rendering to `ck_roar_layout_binaural`.
   */
  roar_headphones_rendering_mode_t headphones_rendering_mode;
  /** Selected binaural HRIR filter profile. */
  roar_binaural_filter_profile_t binaural_filter_profile;
} roar_audio_element_rendering_config_t;

/** Optional IAMF parameter set attached to an audio element configuration. */
typedef struct RoarParameterSet {
  /** Bitmask of `ROAR_PARAMETER_SET_FLAG_*` indicating which fields apply. */
  uint32_t flags;
  /** Demixing parameters (read when `ROAR_PARAMETER_SET_FLAG_IAMF_DOWNMIX_INFO`
   * is set). */
  roar_downmix_info_t downmix_info;
  /**
   * Headphone rendering configuration (read when
   * `ROAR_PARAMETER_SET_FLAG_IAMF_ELEMENT_RENDERING_CONFIG` is set).
   */
  roar_audio_element_rendering_config_t element_rendering_config;
} roar_parameter_set_t;

/** Output configuration for creating a ROAR renderer instance. */
typedef struct RoarConfig {
  /** Target output layout (must be a valid target `roar_layout_t`). */
  roar_layout_t target_layout;
  /** Number of samples processed per channel per render block (`[32, 16384]`).
   */
  uint32_t samples_per_channel;
  /** Sampling rate in Hz (`> 0`, e.g., `48000`). */
  uint32_t sampling_rate;
} roar_config_t;

/** Type-specific configuration for a channel-based audio element. */
typedef struct RoarChannelBasedConfig {
  /** Input channel layout (must be a valid input `roar_layout_t`). */
  roar_layout_t layout;
} roar_channel_based_config_t;

/** Type-specific configuration for a scene-based (Ambisonics) audio element. */
typedef struct RoarSceneBasedConfig {
  /** Ambisonics order of the scene input (`ck_roar_zoa`..`ck_roar_4oa`). */
  roar_hoa_t order;
} roar_scene_based_config_t;

/** Type-specific configuration for an object-based audio element. */
typedef struct RoarObjectBasedConfig {
  /** Number of objects in this element; valid range is `[1, 2]`. */
  int num_objects;
} roar_object_based_config_t;

/** Configuration describing an audio element to add to a group. */
typedef struct RoarAudioElementConfig {
  /** Audio element category selecting the active union member. */
  roar_audio_element_type_t type;
  union {
    /** Channel-based configuration (`type == ck_roar_channel_based`). */
    roar_channel_based_config_t cbc;
    /** Scene-based configuration (`type == ck_roar_scene_based`). */
    roar_scene_based_config_t sbc;
    /** Object-based configuration (`type == ck_roar_object_based`). */
    roar_object_based_config_t obc;
  };
  /** Optional IAMF demixing and headphone rendering parameters. */
  roar_parameter_set_t parameters;
} roar_audio_element_config_t;

/** Interpolation curve types for animated metadata parameters. */
typedef enum ERoarAnimationType {
  /** Constant `start` value across the entire duration. */
  ck_roar_animation_type_step = 0,
  /** Linear interpolation from `start` to `end` over the duration. */
  ck_roar_animation_type_linear = 1,
  /** Quadratic Bezier interpolation using `start`, `end`, and `control`. */
  ck_roar_animation_type_bezier = 2,
} roar_animation_type_t;

/** Keyframe endpoints and control point for a single animated `float` axis. */
typedef struct RoarAnimatedDataFloat32 {
  /** Starting parameter value. */
  float start;
  /** Ending parameter value (ignored in step animation). */
  float end;
  /** Control point value (ignored in step and linear animation). */
  float control;
  /**
   * Normalized time `[0.0, 1.0]` of `control` for Bezier animation (ignored in
   * step and linear animation).
   */
  float control_relative_time;
} roar_animated_data_float32_t;

/** Animated scalar `float` parameter (e.g., gain in dB). */
typedef struct RoarAnimatedFloat32 {
  /** Selected interpolation curve type. */
  roar_animation_type_t animation_type;
  /** Animation keyframe parameters. */
  roar_animated_data_float32_t data;
} roar_animated_float32_t;

/** Animated 3D spherical polar coordinates. */
typedef struct RoarAnimatedPolar {
  /** Interpolation curve applied across `azimuth`, `elevation`, and `distance`.
   */
  roar_animation_type_t animation_type;
  /** Horizontal angle animation parameters (degrees). */
  roar_animated_data_float32_t azimuth;
  /** Vertical angle animation parameters (degrees). */
  roar_animated_data_float32_t elevation;
  /** Normalized distance animation parameters (`[0.0, 1.0]`). */
  roar_animated_data_float32_t distance;
} roar_animated_polar_t;

/** Animated 3D ADM Cartesian coordinates. */
typedef struct RoarAnimatedCartesian {
  /** Interpolation curve applied across `x`, `y`, and `z`. */
  roar_animation_type_t animation_type;
  /** X-axis (left/right) animation parameters. */
  roar_animated_data_float32_t x;
  /** Y-axis (back/front) animation parameters. */
  roar_animated_data_float32_t y;
  /** Z-axis (down/up) animation parameters. */
  roar_animated_data_float32_t z;
} roar_animated_cartesian_t;

/** Metadata payload categories accepted by element and group update calls. */
typedef enum ERoarMetadataType {
  /** Unspecified metadata type. */
  ck_roar_metadata_none = 0,
  /** Gain adjustment in dB (supported on both audio elements and groups). */
  ck_roar_metadata_gain = 1,
  /**
   * Advisory dynamic IAMF downmix mode metadata for audio elements.
   *
   * Takes effect only when the element's active rendering path has a downmix
   * consumer; accepted and ignored (`ck_roar_ok`) on other rendering paths.
   */
  ck_roar_metadata_iamf_downmix_mode = 2,
  /** 3D spatial coordinates metadata for object-based audio elements. */
  ck_roar_metadata_object_positions = 3,
  /** Listener head orientation metadata on audio groups. */
  ck_roar_metadata_head_rotation = 4,
  /**
   * Sentinel count of metadata categories (rejected if passed as a metadata
   * type).
   */
  ck_roar_metadata_count = 5,
} roar_metadata_type_t;

/** Temporal variation representation for metadata parameters. */
typedef enum ERoarParamType {
  /** Constant value across `roar_metadata_t::duration`. */
  ck_roar_param_constant = 0,
  /** Array of distinct per-sample values. */
  ck_roar_param_multiple = 1,
  /** Keyframe-interpolated animated curve. */
  ck_roar_param_animated = 2,
} roar_param_type_t;

/**
 * Gain metadata payload for audio elements or groups.
 *
 * All gain values (`constant_gain`, `gain_array` entries, and `animated_gains`
 * endpoints) are specified in decibels (dB).
 */
typedef struct RoarMetadataGain {
  /** Identifier distinguishing independent gain parameter streams. */
  uint32_t id;
  /** Temporal variation representation selecting the active union member. */
  roar_param_type_t param_type;
  union {
    /** Constant gain in dB applied across `roar_metadata_t::duration`. */
    float constant_gain;
    /**
     * Pointer to `roar_metadata_t::duration` per-sample gain values in dB
     * (`ck_roar_param_multiple`).
     */
    float* gain_array;
    /** Keyframe-interpolated gain in dB (`ck_roar_param_animated`). */
    roar_animated_float32_t animated_gains;
  };
} roar_metadata_gain_t;

/**
 * Advisory dynamic IAMF downmix mode override metadata payload for audio
 * elements.
 *
 * Updates take effect only when the target element's active rendering path has
 * a downmix consumer; otherwise `roar_update_audio_element_metadata` accepts
 * and ignores the update with `ck_roar_ok` (`0`).
 */
typedef struct RoarMetadataIamfDownmixMode {
  /**
   * IAMF `dmixp_mode` identifier (valid values: `0`, `1`, `2`, `4`, `5`, or
   * `6`; `3` and values outside `[0, 6]` are invalid and return
   * `ck_roar_error_inval` (`-22`) when a downmix consumer is active on the
   * element's rendering path).
   */
  int mode;
} roar_metadata_iamf_downmix_mode_t;

/**
 * 3D object position metadata payload for object-based audio elements.
 *
 * Supports `ck_roar_param_constant` and `ck_roar_param_animated`.
 */
typedef struct RoarMetadataObjectPositions {
  /** Temporal variation (`ck_roar_param_constant` or `ck_roar_param_animated`).
   */
  roar_param_type_t param_type;
  /** Coordinate system (`ck_roar_polar` or `ck_roar_cartesian`). */
  roar_coordinate_type_t position_type;
  /** Number of object positions populated (`[1, ROAR_MAX_NUMBER_OF_OBJECTS]`).
   */
  uint32_t num_objects;
  union {
    /** Static polar coordinates (`constant` + `polar`). */
    roar_polar_t polar_positions[ROAR_MAX_NUMBER_OF_OBJECTS];
    /** Static Cartesian coordinates (`constant` + `cartesian`). */
    roar_cartesian_t cartesian_positions[ROAR_MAX_NUMBER_OF_OBJECTS];
    /** Animated polar coordinates (`animated` + `polar`). */
    roar_animated_polar_t animated_polar_positions[ROAR_MAX_NUMBER_OF_OBJECTS];
    /** Animated Cartesian coordinates (`animated` + `cartesian`). */
    roar_animated_cartesian_t
        animated_cartesian_positions[ROAR_MAX_NUMBER_OF_OBJECTS];
  };
} roar_metadata_object_positions_t;

/**
 * Tagged union carrying dynamic element or group metadata updates.
 *
 * Audio elements accept `ck_roar_metadata_gain`,
 * `ck_roar_metadata_iamf_downmix_mode`, and
 * `ck_roar_metadata_object_positions`. Audio groups accept
 * `ck_roar_metadata_gain` and `ck_roar_metadata_head_rotation`.
 */
typedef struct RoarMetadata {
  /** Active metadata category selecting the union payload. */
  roar_metadata_type_t type;
  union {
    /** Gain payload (`ck_roar_metadata_gain`). */
    roar_metadata_gain_t gain;
    /** Advisory dynamic IAMF downmix mode
     * (`ck_roar_metadata_iamf_downmix_mode`).
     */
    roar_metadata_iamf_downmix_mode_t iamf_downmix_mode;
    /** 3D object positions (`ck_roar_metadata_object_positions`). */
    roar_metadata_object_positions_t object_positions;
    /** Listener head orientation (`ck_roar_metadata_head_rotation`). */
    roar_quaternion_t head_rotation;
  };
  /**
   * Validity span in samples.
   *
   * Required (`> 0`) for `ck_roar_metadata_gain` and
   * `ck_roar_metadata_object_positions` (`<= 0` is silently ignored); optional
   * for `ck_roar_metadata_iamf_downmix_mode` (`<= 0` persists indefinitely
   * until the next downmix mode update); unused for
   * `ck_roar_metadata_head_rotation`.
   */
  int duration;
} roar_metadata_t;

/**
 * Creates a new ROAR renderer instance.
 *
 * @param config Non-null output configuration (`target_layout`,
 *     `samples_per_channel`, and `sampling_rate` must all be valid and
 *     non-zero).
 * @return Owned pointer to `roar_t` on success (must be freed with
 *     `roar_destroy`), or `NULL` if `config` is null or invalid.
 */
roar_t* roar_create(const roar_config_t* config);

/**
 * Destroys a ROAR renderer instance and releases all associated memory.
 *
 * @param roar Renderer handle returned by `roar_create`, or `NULL` (no-op).
 */
void roar_destroy(roar_t* roar);

/**
 * Adds a new audio group to the renderer (up to 2 groups: IDs `0` and `1`).
 *
 * Must be called before `roar_render` transitions the renderer out of its
 * configuration phase.
 *
 * @param roar Valid renderer handle.
 * @return Group ID (`0` or `1`) on success, or a negative `roar_status_t` code
 *     on failure.
 */
int roar_add_audio_group(roar_t* roar);

/**
 * Adds a configured audio element to an existing audio group.
 *
 * @param roar Valid renderer handle.
 * @param gid Existing group ID returned by `roar_add_audio_group`.
 * @param id Caller-assigned element ID; must be unique across all groups.
 * @param config Non-null element configuration matching `config->type`.
 * @return `ck_roar_ok` (`0`) on success, `ck_roar_error_busy` (`-16`) if `id`
 *     is already in use, or `ck_roar_error_inval` (`-22`) for invalid inputs.
 */
int roar_add_audio_element(roar_t* roar, uint32_t gid, uint32_t id,
                           const roar_audio_element_config_t* config);

/**
 * Removes an active audio element by ID.
 *
 * For binaural rendering, elements within a binaural sub-renderer must be
 * removed in LIFO (last-in, first-out) order; removing a non-last binaural
 * element returns `ck_roar_error_notsup` (`-95`) and leaves the element active.
 *
 * @param roar Valid renderer handle.
 * @param id Unique ID of an active audio element.
 * @return `ck_roar_ok` (`0`) on success, `ck_roar_error_inval` (`-22`) if
 *     `roar` is null or `id` does not exist, or `ck_roar_error_notsup` (`-95`)
 *     for unsupported removal order.
 */
int roar_remove_audio_element(roar_t* roar, uint32_t id);

/**
 * Queues or updates dynamic metadata parameters for an audio element.
 *
 * Supported metadata types:
 * - Object positions (`ck_roar_metadata_object_positions`,
 *   `roar_metadata_object_positions_t`): 3D spatial coordinates supported only
 *   on object-based audio elements; queued per element in FIFO order for
 *   `metadata->duration` samples (`duration <= 0` is silently dropped).
 * - Gain (`ck_roar_metadata_gain`, `roar_metadata_gain_t`): Dynamic gain
 *   adjustment supported on all audio element types; queued per element in FIFO
 *   order for `metadata->duration` samples (`duration <= 0` is silently
 *   dropped).
 * - IAMF downmix mode (`ck_roar_metadata_iamf_downmix_mode`,
 *   `roar_metadata_iamf_downmix_mode_t`): Advisory metadata hint that takes
 *   effect only when the element's active rendering path has a downmix
 *   consumer. When the element's active rendering path has no downmix consumer
 *   (such as EAR fallback, OLR, or binaural OBR), the update is accepted and
 *   ignored (`ck_roar_ok`) without validating `mode`. Duration is optional
 *   (`duration <= 0` persists indefinitely until a new update is provided).
 *
 * To update listener head rotation (which is always applied globally), use
 * `roar_update_metadata` instead.
 *
 * @param roar Valid renderer handle.
 * @param id Unique identifier of an active audio element (searched across all
 *     audio groups).
 * @param metadata Non-null metadata payload (`ck_roar_metadata_gain`,
 *     `ck_roar_metadata_object_positions`, or
 *     `ck_roar_metadata_iamf_downmix_mode`).
 * @return `ck_roar_ok` (`0`) on success (for advisory
 *     `ck_roar_metadata_iamf_downmix_mode`, `0` means accepted and also covers
 *     updates ignored because the element's active rendering path has no
 *     downmix consumer); `ck_roar_error_inval` (`-22`) for null pointers,
 *     invalid parameters, a non-existent element `id`, or an invalid `mode`
 *     value (outside `{0, 1, 2, 4, 5, 6}`) when the element's active rendering
 *     path has a downmix consumer; or `ck_roar_error_notsup` (`-95`) for
 *     unknown or group-only metadata types, or non-advisory metadata
 *     unsupported by the target element (e.g., object positions on a
 *     non-object element).
 */
int roar_update_audio_element_metadata(roar_t* roar, uint32_t id,
                                       const roar_metadata_t* metadata);

/**
 * Supplies one frame of planar input audio samples for an audio element.
 *
 * Input data is copied internally and consumed by the next `roar_render` call.
 *
 * @param roar Valid renderer handle.
 * @param id Unique ID of an active audio element.
 * @param data Non-null planar audio block whose `channels` equals
 *     `roar_get_number_of_audio_element_channels(roar, id)` and whose `data`
 *     points to `channels * samples_per_channel` contiguous `float` samples.
 * @return `ck_roar_ok` (`0`) on success, or `ck_roar_error_inval` (`-22`) on
 *     invalid arguments or channel count mismatch.
 */
int roar_update_audio_element_data(roar_t* roar, uint32_t id,
                                   roar_audio_block_t* data);

/**
 * Sets the sub-frame processing granularity in samples for animated metadata.
 *
 * Supported for both loudspeaker and binaural output layouts when `type` is
 * `ck_roar_metadata_object_positions`.
 *
 * @param roar Valid renderer handle.
 * @param type Metadata category to configure (must be
 *     `ck_roar_metadata_object_positions`).
 * @param samples Sub-frame step size in `(0, samples_per_channel]`.
 * @return `ck_roar_ok` (`0`) on success, `ck_roar_error_inval` (`-22`) if
 *     `roar` is null or `samples` is out of range, or `ck_roar_error_notsup`
 *     (`-95`) if `type` is not `ck_roar_metadata_object_positions`.
 */
int roar_set_metadata_unit_to_process(roar_t* roar, roar_metadata_type_t type,
                                      uint32_t samples);

/**
 * Updates group-level metadata (group gain or global listener head rotation).
 *
 * @param roar Valid renderer handle.
 * @param gid Existing group ID (`0` or `1`; must be valid even for global head
 *     rotation updates).
 * @param metadata Non-null metadata payload (`ck_roar_metadata_gain` or
 *     `ck_roar_metadata_head_rotation`). Head rotation applies globally, uses
 *     the ADM object coordinate system, requires head tracking to be enabled
 *     via `roar_enable_head_tracking`, and requires `head_rotation` to be a
 *     unit quaternion.
 * @return `ck_roar_ok` (`0`) on success, `ck_roar_error_busy` (`-16`) if head
 *     rotation is supplied while head tracking is disabled,
 *     `ck_roar_error_inval` (`-22`) for invalid parameters, or
 *     `ck_roar_error_notsup` (`-95`) for unsupported metadata types.
 */
int roar_update_metadata(roar_t* roar, uint32_t gid,
                         const roar_metadata_t* metadata);

/**
 * Renders one block of audio across all groups and elements into `output`.
 *
 * Consumes all element input buffers supplied via
 * `roar_update_audio_element_data`. When the peak limiter is enabled, the
 * first `roar_get_limiter_delay(roar)` samples are zero-padded for look-ahead
 * priming.
 *
 * @param roar Valid renderer handle with at least one audio group and input
 *     data provided for active elements.
 * @param output Non-null planar output block. Caller must set
 *     `output->channels` to `roar_get_number_of_output_channels(roar)`,
 *     `output->samples_per_channel` to at least
 *     `roar_get_samples_per_channel(roar)`, and `output->data` to an aligned
 *     buffer of at least `channels * roar_get_samples_per_channel(roar)`
 *     `float`s. On success, `output->samples_per_channel` is set to
 *     `roar_get_samples_per_channel(roar)`.
 * @return `ck_roar_ok` (`0`) on success, or a negative `roar_status_t` code on
 *     failure.
 */
int roar_render(roar_t* roar, roar_audio_block_t* output);

/**
 * Drains the peak limiter's look-ahead delay buffer and resets limiter state.
 *
 * @param roar Valid renderer handle.
 * @param output Non-null planar output block. Caller must set
 *     `output->channels` to `roar_get_number_of_output_channels(roar)` and
 *     `output->data` to an aligned buffer of at least
 *     `channels * roar_get_limiter_delay(roar)` `float`s when the limiter is
 *     enabled (`output->samples_per_channel` is ignored on entry). On return,
 *     `output->samples_per_channel` is set to the number of flushed samples
 *     (`0` if the limiter is disabled or has zero delay).
 * @return `ck_roar_ok` (`0`) on success, or `ck_roar_error_inval` (`-22`) for
 *     invalid parameters.
 */
int roar_flush(roar_t* roar, roar_audio_block_t* output);

/**
 * Enables or disables per-group loudness normalization during rendering.
 *
 * @param roar Valid renderer handle.
 * @param enable Non-zero to enable, `0` to disable.
 * @return `ck_roar_ok` (`0`) on success, or `ck_roar_error_inval` (`-22`) if
 *     `roar` is null.
 */
int roar_enable_loudness_processor(roar_t* roar, int enable);

/**
 * Configures source and target loudness levels for an audio group.
 *
 * Takes effect during `roar_render` when the loudness processor is enabled via
 * `roar_enable_loudness_processor`.
 *
 * @param roar Valid renderer handle.
 * @param gid Existing group ID (`0` or `1`).
 * @param loudness Measured group loudness in dB (must be finite).
 * @param target_loudness Desired output loudness in dB (must be finite).
 * @return `ck_roar_ok` (`0`) on success, or `ck_roar_error_inval` (`-22`) for
 *     invalid parameters or non-existent group ID.
 */
int roar_set_loudness(roar_t* roar, uint32_t gid, float loudness,
                      float target_loudness);

/**
 * Enables or disables the look-ahead output peak limiter.
 *
 * Re-enabling after disabling resets the limiter delay buffer.
 *
 * @param roar Valid renderer handle.
 * @param enable Non-zero to enable, `0` to disable.
 * @return `ck_roar_ok` (`0`) on success, or `ck_roar_error_inval` (`-22`) if
 *     `roar` is null.
 */
int roar_enable_limiter(roar_t* roar, int enable);

/**
 * Updates the look-ahead peak limiter threshold in-place without resetting
 * delay buffers.
 *
 * May be called before or after `roar_enable_limiter`, and between
 * `roar_render` calls.
 *
 * @param roar Valid renderer handle.
 * @param threshold_db Finite threshold in dBFS (values outside `[-60.0, 0.0]`
 *     are clamped to that range).
 * @return `ck_roar_ok` (`0`) on success, or `ck_roar_error_inval` (`-22`) if
 *     `roar` is null or `threshold_db` is non-finite.
 */
int roar_set_limiter_threshold(roar_t* roar, float threshold_db);

/**
 * Returns the peak limiter look-ahead delay in samples per channel.
 *
 * @param roar Valid renderer handle, or `NULL`.
 * @return Look-ahead delay in samples, or `0` if `roar` is null.
 */
int roar_get_limiter_delay(const roar_t* roar);

/**
 * Enables or disables listener head tracking for binaural output.
 *
 * Only supported when the renderer's `target_layout` is
 * `ck_roar_layout_binaural`.
 *
 * @param roar Valid renderer handle.
 * @param enable Non-zero to enable, `0` to disable.
 * @return `ck_roar_ok` (`0`) on success, `ck_roar_error_inval` (`-22`) if
 *     `roar` is null, or `ck_roar_error_notsup` (`-95`) if `target_layout` is
 *     not binaural.
 */
int roar_enable_head_tracking(roar_t* roar, int enable);

/**
 * Returns the configured block size in samples per channel.
 *
 * @param roar Valid renderer handle, or `NULL`.
 * @return Samples per channel, or `0` if `roar` is null.
 */
uint32_t roar_get_samples_per_channel(const roar_t* roar);

/**
 * Returns the configured sampling rate in Hz.
 *
 * @param roar Valid renderer handle, or `NULL`.
 * @return Sampling rate in Hz, or `0` if `roar` is null.
 */
uint32_t roar_get_sampling_rate(const roar_t* roar);

/**
 * Returns the expected input channel count for an active audio element.
 *
 * @param roar Valid renderer handle, or `NULL`.
 * @param id Unique ID of an active audio element.
 * @return Channel count, or `0` if `roar` is null or `id` is not found.
 */
uint32_t roar_get_number_of_audio_element_channels(const roar_t* roar,
                                                   uint32_t id);

/**
 * Returns the number of output channels for the configured target layout.
 *
 * @param roar Valid renderer handle, or `NULL`.
 * @return Output channel count, or `0` if `roar` is null.
 */
uint32_t roar_get_number_of_output_channels(const roar_t* roar);

/**
 * Returns the total number of active audio elements across all groups.
 *
 * @param roar Valid renderer handle, or `NULL`.
 * @return Total active audio element count, or `0` if `roar` is null.
 */
uint32_t roar_get_number_of_audio_elements(const roar_t* roar);

#ifdef __cplusplus
}  // extern "C"
#endif

#endif  // INCLUDE_ROAR_H_
