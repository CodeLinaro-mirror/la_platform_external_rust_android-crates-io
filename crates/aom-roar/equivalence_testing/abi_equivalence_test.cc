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

#include <cstddef>
#include <memory>

#include "gtest/gtest.h"
#include "include/roar.h"

extern "C" {
#include "include/animation.h"
#include "include/oar.h"
#include "include/oar_base.h"
#include "include/oar_metadata.h"
}

namespace roar_equivalence_testing {
namespace {

// Compile-time verification that `roar.h` maintains exact C ABI layout, size,
// alignment, field offset, and constant value parity with `liboar`. This
// guarantees that `test_helpers.h` can safely forward-declare `oar_*` functions
// using `roar_*` types, while also verifying that `roar.h` and `liboar` headers
// can be included together in a single translation unit without collisions.

static_assert(ROAR_MAX_NUMBER_OF_OBJECTS == def_max_number_of_objects);
static_assert(ROAR_PARAMETER_SET_FLAG_IAMF_DOWNMIX_INFO ==
              def_parameter_set_flag_iamf_downmix_info);
static_assert(ROAR_PARAMETER_SET_FLAG_IAMF_ELEMENT_RENDERING_CONFIG ==
              def_parameter_set_flag_iamf_element_rendering_config);

static_assert(static_cast<int>(ck_roar_ok) == static_cast<int>(ck_oar_ok));
static_assert(static_cast<int>(ck_roar_error_nomem) ==
              static_cast<int>(ck_oar_error_nomem));
static_assert(static_cast<int>(ck_roar_error_busy) ==
              static_cast<int>(ck_oar_error_busy));
static_assert(static_cast<int>(ck_roar_error_inval) ==
              static_cast<int>(ck_oar_error_inval));
static_assert(static_cast<int>(ck_roar_error_nosys) ==
              static_cast<int>(ck_oar_error_nosys));
static_assert(static_cast<int>(ck_roar_error_notsup) ==
              static_cast<int>(ck_oar_error_notsup));

static_assert(sizeof(roar_layout_t) == sizeof(oar_layout_t));
static_assert(static_cast<int>(ck_roar_layout_none) ==
              static_cast<int>(ck_oar_layout_none));
static_assert(static_cast<int>(ck_roar_layout_mono) ==
              static_cast<int>(ck_oar_layout_mono));
static_assert(static_cast<int>(ck_roar_layout_stereo) ==
              static_cast<int>(ck_oar_layout_stereo));
static_assert(static_cast<int>(ck_roar_layout_51) ==
              static_cast<int>(ck_oar_layout_51));
static_assert(static_cast<int>(ck_roar_layout_512) ==
              static_cast<int>(ck_oar_layout_512));
static_assert(static_cast<int>(ck_roar_layout_514) ==
              static_cast<int>(ck_oar_layout_514));
static_assert(static_cast<int>(ck_roar_layout_71) ==
              static_cast<int>(ck_oar_layout_71));
static_assert(static_cast<int>(ck_roar_layout_712) ==
              static_cast<int>(ck_oar_layout_712));
static_assert(static_cast<int>(ck_roar_layout_714) ==
              static_cast<int>(ck_oar_layout_714));
static_assert(static_cast<int>(ck_roar_layout_312) ==
              static_cast<int>(ck_oar_layout_312));
static_assert(static_cast<int>(ck_roar_layout_916) ==
              static_cast<int>(ck_oar_layout_916));
static_assert(static_cast<int>(ck_roar_layout_a293) ==
              static_cast<int>(ck_oar_layout_a293));
static_assert(static_cast<int>(ck_roar_layout_7154) ==
              static_cast<int>(ck_oar_layout_7154));
static_assert(static_cast<int>(ck_roar_layout_sound_system_a_020) ==
              static_cast<int>(ck_oar_layout_sound_system_a_020));
static_assert(static_cast<int>(ck_roar_layout_sound_system_b_050) ==
              static_cast<int>(ck_oar_layout_sound_system_b_050));
static_assert(static_cast<int>(ck_roar_layout_sound_system_c_250) ==
              static_cast<int>(ck_oar_layout_sound_system_c_250));
static_assert(static_cast<int>(ck_roar_layout_sound_system_d_450) ==
              static_cast<int>(ck_oar_layout_sound_system_d_450));
static_assert(static_cast<int>(ck_roar_layout_sound_system_i_070) ==
              static_cast<int>(ck_oar_layout_sound_system_i_070));
static_assert(static_cast<int>(ck_roar_layout_sound_system_j_470) ==
              static_cast<int>(ck_oar_layout_sound_system_j_470));
static_assert(static_cast<int>(ck_roar_layout_sound_system_h_9a3) ==
              static_cast<int>(ck_oar_layout_sound_system_h_9a3));
static_assert(static_cast<int>(ck_roar_layout_sound_system_e_451) ==
              static_cast<int>(ck_oar_layout_sound_system_e_451));
static_assert(static_cast<int>(ck_roar_layout_sound_system_f_370) ==
              static_cast<int>(ck_oar_layout_sound_system_f_370));
static_assert(static_cast<int>(ck_roar_layout_sound_system_g_490) ==
              static_cast<int>(ck_oar_layout_sound_system_g_490));
static_assert(static_cast<int>(ck_roar_layout_binaural) ==
              static_cast<int>(ck_oar_layout_binaural));
static_assert(static_cast<int>(ck_roar_layout_subset_start) ==
              static_cast<int>(ck_oar_layout_subset_start));
static_assert(static_cast<int>(ck_roar_layout_lfe) ==
              static_cast<int>(ck_oar_layout_lfe));
static_assert(static_cast<int>(ck_roar_layout_stereo_s) ==
              static_cast<int>(ck_oar_layout_stereo_s));
static_assert(static_cast<int>(ck_roar_layout_stereo_ss) ==
              static_cast<int>(ck_oar_layout_stereo_ss));
static_assert(static_cast<int>(ck_roar_layout_stereo_rs) ==
              static_cast<int>(ck_oar_layout_stereo_rs));
static_assert(static_cast<int>(ck_roar_layout_stereo_tf) ==
              static_cast<int>(ck_oar_layout_stereo_tf));
static_assert(static_cast<int>(ck_roar_layout_stereo_tb) ==
              static_cast<int>(ck_oar_layout_stereo_tb));
static_assert(static_cast<int>(ck_roar_layout_top_4ch) ==
              static_cast<int>(ck_oar_layout_top_4ch));
static_assert(static_cast<int>(ck_roar_layout_3ch) ==
              static_cast<int>(ck_oar_layout_3ch));
static_assert(static_cast<int>(ck_roar_layout_stereo_f) ==
              static_cast<int>(ck_oar_layout_stereo_f));
static_assert(static_cast<int>(ck_roar_layout_stereo_si) ==
              static_cast<int>(ck_oar_layout_stereo_si));
static_assert(static_cast<int>(ck_roar_layout_stereo_tpsi) ==
              static_cast<int>(ck_oar_layout_stereo_tpsi));
static_assert(static_cast<int>(ck_roar_layout_top_6ch) ==
              static_cast<int>(ck_oar_layout_top_6ch));
static_assert(static_cast<int>(ck_roar_layout_lfe_pair) ==
              static_cast<int>(ck_oar_layout_lfe_pair));
static_assert(static_cast<int>(ck_roar_layout_bottom_3ch) ==
              static_cast<int>(ck_oar_layout_bottom_3ch));
static_assert(static_cast<int>(ck_roar_layout_bottom_4ch) ==
              static_cast<int>(ck_oar_layout_bottom_4ch));
static_assert(static_cast<int>(ck_roar_layout_top_1ch) ==
              static_cast<int>(ck_oar_layout_top_1ch));
static_assert(static_cast<int>(ck_roar_layout_top_5ch) ==
              static_cast<int>(ck_oar_layout_top_5ch));

static_assert(sizeof(roar_hoa_t) == sizeof(oar_hoa_t));
static_assert(static_cast<int>(ck_roar_hoa_none) ==
              static_cast<int>(ck_oar_hoa_none));
static_assert(static_cast<int>(ck_roar_zoa) == static_cast<int>(ck_oar_zoa));
static_assert(static_cast<int>(ck_roar_1oa) == static_cast<int>(ck_oar_1oa));
static_assert(static_cast<int>(ck_roar_2oa) == static_cast<int>(ck_oar_2oa));
static_assert(static_cast<int>(ck_roar_3oa) == static_cast<int>(ck_oar_3oa));
static_assert(static_cast<int>(ck_roar_4oa) == static_cast<int>(ck_oar_4oa));

static_assert(sizeof(roar_audio_element_type_t) ==
              sizeof(audio_element_type_t));
static_assert(static_cast<int>(ck_roar_element_type_none) ==
              static_cast<int>(ck_oar_element_type_none));
static_assert(static_cast<int>(ck_roar_channel_based) ==
              static_cast<int>(ck_channel_based));
static_assert(static_cast<int>(ck_roar_scene_based) ==
              static_cast<int>(ck_scene_based));
static_assert(static_cast<int>(ck_roar_object_based) ==
              static_cast<int>(ck_object_based));

static_assert(sizeof(roar_headphones_rendering_mode_t) ==
              sizeof(oar_headphones_rendering_mode_t));
static_assert(static_cast<int>(ck_roar_world_locked_restricted) ==
              static_cast<int>(ck_world_locked_restricted));
static_assert(static_cast<int>(ck_roar_world_locked) ==
              static_cast<int>(ck_world_locked));
static_assert(static_cast<int>(ck_roar_head_locked) ==
              static_cast<int>(ck_head_locked));
static_assert(static_cast<int>(ck_roar_reserved) ==
              static_cast<int>(ck_reserved));

static_assert(sizeof(roar_binaural_filter_profile_t) ==
              sizeof(oar_binaural_filter_profile_t));
static_assert(static_cast<int>(ck_roar_ambient) ==
              static_cast<int>(ck_ambient));
static_assert(static_cast<int>(ck_roar_direct) == static_cast<int>(ck_direct));
static_assert(static_cast<int>(ck_roar_reverberant) ==
              static_cast<int>(ck_reverberant));
static_assert(static_cast<int>(ck_roar_binaural_filter_profile_default) ==
              static_cast<int>(ck_binaural_filter_profile_default));

static_assert(sizeof(roar_coordinate_type_t) == sizeof(coordinate_type_t));
static_assert(static_cast<int>(ck_roar_polar) == static_cast<int>(ck_polar));
static_assert(static_cast<int>(ck_roar_cartesian) ==
              static_cast<int>(ck_cartesian));

static_assert(sizeof(roar_animation_type_t) == sizeof(animation_type_t));
static_assert(static_cast<int>(ck_roar_animation_type_step) ==
              static_cast<int>(ck_animation_type_step));
static_assert(static_cast<int>(ck_roar_animation_type_linear) ==
              static_cast<int>(ck_animation_type_linear));
static_assert(static_cast<int>(ck_roar_animation_type_bezier) ==
              static_cast<int>(ck_animation_type_bezier));

static_assert(sizeof(roar_metadata_type_t) == sizeof(oar_metadata_type_t));
static_assert(static_cast<int>(ck_roar_metadata_none) ==
              static_cast<int>(ck_metadata_none));
static_assert(static_cast<int>(ck_roar_metadata_gain) ==
              static_cast<int>(ck_metadata_gain));
static_assert(static_cast<int>(ck_roar_metadata_iamf_downmix_mode) ==
              static_cast<int>(ck_metadata_iamf_downmix_mode));
static_assert(static_cast<int>(ck_roar_metadata_object_positions) ==
              static_cast<int>(ck_metadata_object_positions));
static_assert(static_cast<int>(ck_roar_metadata_head_rotation) ==
              static_cast<int>(ck_metadata_head_rotation));
static_assert(static_cast<int>(ck_roar_metadata_count) ==
              static_cast<int>(ck_metadata_count));

static_assert(sizeof(roar_param_type_t) == sizeof(oar_param_type_t));
static_assert(static_cast<int>(ck_roar_param_constant) ==
              static_cast<int>(ck_param_constant));
static_assert(static_cast<int>(ck_roar_param_multiple) ==
              static_cast<int>(ck_param_multiple));
static_assert(static_cast<int>(ck_roar_param_animated) ==
              static_cast<int>(ck_param_animated));

static_assert(sizeof(roar_polar_t) == sizeof(polar_t));
static_assert(alignof(roar_polar_t) == alignof(polar_t));
static_assert(offsetof(roar_polar_t, azimuth) == offsetof(polar_t, azimuth));
static_assert(offsetof(roar_polar_t, elevation) ==
              offsetof(polar_t, elevation));
static_assert(offsetof(roar_polar_t, distance) == offsetof(polar_t, distance));

static_assert(sizeof(roar_cartesian_t) == sizeof(cartesian_t));
static_assert(alignof(roar_cartesian_t) == alignof(cartesian_t));
static_assert(offsetof(roar_cartesian_t, x) == offsetof(cartesian_t, x));
static_assert(offsetof(roar_cartesian_t, y) == offsetof(cartesian_t, y));
static_assert(offsetof(roar_cartesian_t, z) == offsetof(cartesian_t, z));

static_assert(sizeof(roar_quaternion_t) == sizeof(quaternion_t));
static_assert(alignof(roar_quaternion_t) == alignof(quaternion_t));
static_assert(offsetof(roar_quaternion_t, w) == offsetof(quaternion_t, w));
static_assert(offsetof(roar_quaternion_t, x) == offsetof(quaternion_t, x));
static_assert(offsetof(roar_quaternion_t, y) == offsetof(quaternion_t, y));
static_assert(offsetof(roar_quaternion_t, z) == offsetof(quaternion_t, z));

static_assert(sizeof(roar_audio_block_t) == sizeof(oar_audio_block_t));
static_assert(alignof(roar_audio_block_t) == alignof(oar_audio_block_t));
static_assert(offsetof(roar_audio_block_t, data) ==
              offsetof(oar_audio_block_t, data));
static_assert(offsetof(roar_audio_block_t, channels) ==
              offsetof(oar_audio_block_t, channels));
static_assert(offsetof(roar_audio_block_t, samples_per_channel) ==
              offsetof(oar_audio_block_t, samples_per_channel));

static_assert(sizeof(roar_downmix_info_t) == sizeof(downmix_info_t));
static_assert(alignof(roar_downmix_info_t) == alignof(downmix_info_t));
static_assert(offsetof(roar_downmix_info_t, mode) ==
              offsetof(downmix_info_t, mode));
static_assert(offsetof(roar_downmix_info_t, weight_index) ==
              offsetof(downmix_info_t, weight_index));

static_assert(sizeof(roar_audio_element_rendering_config_t) ==
              sizeof(audio_element_rendering_config_t));
static_assert(alignof(roar_audio_element_rendering_config_t) ==
              alignof(audio_element_rendering_config_t));
static_assert(offsetof(roar_audio_element_rendering_config_t,
                       headphones_rendering_mode) ==
              offsetof(audio_element_rendering_config_t,
                       headphones_rendering_mode));
static_assert(offsetof(roar_audio_element_rendering_config_t,
                       binaural_filter_profile) ==
              offsetof(audio_element_rendering_config_t,
                       binaural_filter_profile));

static_assert(sizeof(roar_parameter_set_t) == sizeof(parameter_set_t));
static_assert(alignof(roar_parameter_set_t) == alignof(parameter_set_t));
static_assert(offsetof(roar_parameter_set_t, flags) ==
              offsetof(parameter_set_t, flags));
static_assert(offsetof(roar_parameter_set_t, downmix_info) ==
              offsetof(parameter_set_t, downmix_info));
static_assert(offsetof(roar_parameter_set_t, element_rendering_config) ==
              offsetof(parameter_set_t, element_rendering_config));

static_assert(sizeof(roar_config_t) == sizeof(oar_config_t));
static_assert(alignof(roar_config_t) == alignof(oar_config_t));
static_assert(offsetof(roar_config_t, target_layout) ==
              offsetof(oar_config_t, target_layout));
static_assert(offsetof(roar_config_t, samples_per_channel) ==
              offsetof(oar_config_t, samples_per_channel));
static_assert(offsetof(roar_config_t, sampling_rate) ==
              offsetof(oar_config_t, sampling_rate));

static_assert(sizeof(roar_channel_based_config_t) ==
              sizeof(oar_channel_based_config_t));
static_assert(alignof(roar_channel_based_config_t) ==
              alignof(oar_channel_based_config_t));
static_assert(offsetof(roar_channel_based_config_t, layout) ==
              offsetof(oar_channel_based_config_t, layout));

static_assert(sizeof(roar_scene_based_config_t) ==
              sizeof(oar_scene_based_config_t));
static_assert(alignof(roar_scene_based_config_t) ==
              alignof(oar_scene_based_config_t));
static_assert(offsetof(roar_scene_based_config_t, order) ==
              offsetof(oar_scene_based_config_t, order));

static_assert(sizeof(roar_object_based_config_t) ==
              sizeof(oar_object_based_config_t));
static_assert(alignof(roar_object_based_config_t) ==
              alignof(oar_object_based_config_t));
static_assert(offsetof(roar_object_based_config_t, num_objects) ==
              offsetof(oar_object_based_config_t, num_objects));

static_assert(sizeof(roar_audio_element_config_t) ==
              sizeof(oar_audio_element_config_t));
static_assert(alignof(roar_audio_element_config_t) ==
              alignof(oar_audio_element_config_t));
static_assert(offsetof(roar_audio_element_config_t, type) ==
              offsetof(oar_audio_element_config_t, type));
static_assert(offsetof(roar_audio_element_config_t, cbc) ==
              offsetof(oar_audio_element_config_t, cbc));
static_assert(offsetof(roar_audio_element_config_t, sbc) ==
              offsetof(oar_audio_element_config_t, sbc));
static_assert(offsetof(roar_audio_element_config_t, obc) ==
              offsetof(oar_audio_element_config_t, obc));
static_assert(offsetof(roar_audio_element_config_t, parameters) ==
              offsetof(oar_audio_element_config_t, parameters));

static_assert(sizeof(roar_animated_data_float32_t) ==
              sizeof(animated_data_float32_t));
static_assert(alignof(roar_animated_data_float32_t) ==
              alignof(animated_data_float32_t));
static_assert(offsetof(roar_animated_data_float32_t, start) ==
              offsetof(animated_data_float32_t, start));
static_assert(offsetof(roar_animated_data_float32_t, end) ==
              offsetof(animated_data_float32_t, end));
static_assert(offsetof(roar_animated_data_float32_t, control) ==
              offsetof(animated_data_float32_t, control));
static_assert(offsetof(roar_animated_data_float32_t, control_relative_time) ==
              offsetof(animated_data_float32_t, control_relative_time));

static_assert(sizeof(roar_animated_float32_t) == sizeof(animated_float32_t));
static_assert(alignof(roar_animated_float32_t) == alignof(animated_float32_t));
static_assert(offsetof(roar_animated_float32_t, animation_type) ==
              offsetof(animated_float32_t, animation_type));
static_assert(offsetof(roar_animated_float32_t, data) ==
              offsetof(animated_float32_t, data));

static_assert(sizeof(roar_animated_polar_t) == sizeof(animated_polar_t));
static_assert(alignof(roar_animated_polar_t) == alignof(animated_polar_t));
static_assert(offsetof(roar_animated_polar_t, animation_type) ==
              offsetof(animated_polar_t, animation_type));
static_assert(offsetof(roar_animated_polar_t, azimuth) ==
              offsetof(animated_polar_t, azimuth));
static_assert(offsetof(roar_animated_polar_t, elevation) ==
              offsetof(animated_polar_t, elevation));
static_assert(offsetof(roar_animated_polar_t, distance) ==
              offsetof(animated_polar_t, distance));

static_assert(sizeof(roar_animated_cartesian_t) ==
              sizeof(animated_cartesian_t));
static_assert(alignof(roar_animated_cartesian_t) ==
              alignof(animated_cartesian_t));
static_assert(offsetof(roar_animated_cartesian_t, animation_type) ==
              offsetof(animated_cartesian_t, animation_type));
static_assert(offsetof(roar_animated_cartesian_t, x) ==
              offsetof(animated_cartesian_t, x));
static_assert(offsetof(roar_animated_cartesian_t, y) ==
              offsetof(animated_cartesian_t, y));
static_assert(offsetof(roar_animated_cartesian_t, z) ==
              offsetof(animated_cartesian_t, z));

static_assert(sizeof(roar_metadata_gain_t) == sizeof(oar_metadata_gain_t));
static_assert(alignof(roar_metadata_gain_t) == alignof(oar_metadata_gain_t));
static_assert(offsetof(roar_metadata_gain_t, id) ==
              offsetof(oar_metadata_gain_t, id));
static_assert(offsetof(roar_metadata_gain_t, param_type) ==
              offsetof(oar_metadata_gain_t, param_type));
static_assert(offsetof(roar_metadata_gain_t, constant_gain) ==
              offsetof(oar_metadata_gain_t, constant_gain));
static_assert(offsetof(roar_metadata_gain_t, gain_array) ==
              offsetof(oar_metadata_gain_t, gain_array));
static_assert(offsetof(roar_metadata_gain_t, animated_gains) ==
              offsetof(oar_metadata_gain_t, animated_gains));

static_assert(sizeof(roar_metadata_iamf_downmix_mode_t) ==
              sizeof(oar_metadata_iamf_downmix_mode_t));
static_assert(alignof(roar_metadata_iamf_downmix_mode_t) ==
              alignof(oar_metadata_iamf_downmix_mode_t));
static_assert(offsetof(roar_metadata_iamf_downmix_mode_t, mode) ==
              offsetof(oar_metadata_iamf_downmix_mode_t, mode));

static_assert(sizeof(roar_metadata_object_positions_t) ==
              sizeof(oar_metadata_object_positions_t));
static_assert(alignof(roar_metadata_object_positions_t) ==
              alignof(oar_metadata_object_positions_t));
static_assert(offsetof(roar_metadata_object_positions_t, param_type) ==
              offsetof(oar_metadata_object_positions_t, param_type));
static_assert(offsetof(roar_metadata_object_positions_t, position_type) ==
              offsetof(oar_metadata_object_positions_t, position_type));
static_assert(offsetof(roar_metadata_object_positions_t, num_objects) ==
              offsetof(oar_metadata_object_positions_t, num_objects));
static_assert(offsetof(roar_metadata_object_positions_t, polar_positions) ==
              offsetof(oar_metadata_object_positions_t, polar_positions));
static_assert(offsetof(roar_metadata_object_positions_t, cartesian_positions) ==
              offsetof(oar_metadata_object_positions_t, cartesian_positions));
static_assert(offsetof(roar_metadata_object_positions_t,
                       animated_polar_positions) ==
              offsetof(oar_metadata_object_positions_t,
                       animated_polar_positions));
static_assert(offsetof(roar_metadata_object_positions_t,
                       animated_cartesian_positions) ==
              offsetof(oar_metadata_object_positions_t,
                       animated_cartesian_positions));

static_assert(sizeof(roar_metadata_t) == sizeof(oar_metadata_t));
static_assert(alignof(roar_metadata_t) == alignof(oar_metadata_t));
static_assert(offsetof(roar_metadata_t, type) ==
              offsetof(oar_metadata_t, type));
static_assert(offsetof(roar_metadata_t, gain) ==
              offsetof(oar_metadata_t, gain));
static_assert(offsetof(roar_metadata_t, iamf_downmix_mode) ==
              offsetof(oar_metadata_t, iamf_downmix_mode));
static_assert(offsetof(roar_metadata_t, object_positions) ==
              offsetof(oar_metadata_t, object_positions));
static_assert(offsetof(roar_metadata_t, head_rotation) ==
              offsetof(oar_metadata_t, head_rotation));
static_assert(offsetof(roar_metadata_t, duration) ==
              offsetof(oar_metadata_t, duration));

TEST(AbiEquivalenceTest, CoexistsWithLiboarHeadersAndCreatesBothRenderers) {
  oar_config_t oar_config = {
      .target_layout = ck_oar_layout_stereo,
      .samples_per_channel = 256,
      .sampling_rate = 48000,
  };
  roar_config_t roar_config = {
      .target_layout = ck_roar_layout_stereo,
      .samples_per_channel = 256,
      .sampling_rate = 48000,
  };

  std::unique_ptr<oar_t, decltype(&oar_destroy)> oar(oar_create(&oar_config),
                                                     oar_destroy);
  std::unique_ptr<roar_t, decltype(&roar_destroy)> roar(
      roar_create(&roar_config), roar_destroy);

  ASSERT_NE(oar, nullptr);
  ASSERT_NE(roar, nullptr);
  EXPECT_EQ(oar_get_samples_per_channel(oar.get()), 256u);
  EXPECT_EQ(roar_get_samples_per_channel(roar.get()), 256u);
  EXPECT_EQ(oar_get_sampling_rate(oar.get()), 48000u);
  EXPECT_EQ(roar_get_sampling_rate(roar.get()), 48000u);
  EXPECT_EQ(oar_get_number_of_output_channels(oar.get()), 2u);
  EXPECT_EQ(roar_get_number_of_output_channels(roar.get()), 2u);
}

}  // namespace
}  // namespace roar_equivalence_testing
