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
#include "equivalence_testing/test_helpers.h"

#include <cmath>
#include <cstddef>
#include <cstdint>
#include <numbers>
#include <string>
#include <vector>

#include "absl/strings/str_format.h"
#include "include/roar.h"

namespace roar_equivalence_testing {

std::vector<roar_hoa_t> GetAllAmbisonicOrders() {
  return {ck_roar_zoa, ck_roar_1oa, ck_roar_2oa, ck_roar_3oa, ck_roar_4oa};
}

std::vector<roar_layout_t> GetAllChannelBasedInputs() {
  return {
      ck_roar_layout_mono,     ck_roar_layout_stereo, ck_roar_layout_51,
      ck_roar_layout_512,      ck_roar_layout_514,    ck_roar_layout_71,
      ck_roar_layout_712,      ck_roar_layout_714,    ck_roar_layout_312,
      ck_roar_layout_916,      ck_roar_layout_a293,   ck_roar_layout_7154,
      ck_roar_layout_binaural,
  };
}

std::vector<roar_layout_t> GetAllOutputLayouts() {
  return {ck_roar_layout_mono,
          ck_roar_layout_stereo,
          ck_roar_layout_51,
          ck_roar_layout_512,
          ck_roar_layout_514,
          ck_roar_layout_71,
          ck_roar_layout_712,
          ck_roar_layout_714,
          ck_roar_layout_312,
          ck_roar_layout_916,
          ck_roar_layout_a293,
          ck_roar_layout_7154,
          ck_roar_layout_sound_system_e_451,
          ck_roar_layout_sound_system_f_370,
          ck_roar_layout_sound_system_g_490,
          ck_roar_layout_binaural};
}

std::vector<roar_config_t> GetAllOutputConfigs(uint32_t samples_per_channel,
                                               uint32_t sampling_rate) {
  auto output_layouts = GetAllOutputLayouts();
  std::vector<roar_config_t> output;
  output.reserve(output_layouts.size());
  for (auto layout : output_layouts) {
    roar_config_t config = {};
    config.target_layout = layout;
    config.samples_per_channel = samples_per_channel;
    config.sampling_rate = sampling_rate;
    output.push_back(config);
  }
  return output;
}

std::vector<float> GetInputData(size_t samples_per_channel, size_t num_channels,
                                int sample_rate, float base_frequency) {
  std::vector<float> buffer(num_channels * samples_per_channel);
  for (size_t channel = 0; channel < num_channels; ++channel) {
    const float scale = 1.0f / num_channels;
    const float factor = (base_frequency + 20.0f * channel) * 2.0 *
                         std::numbers::pi / sample_rate;
    for (size_t i = 0; i < samples_per_channel; ++i) {
      buffer[channel * samples_per_channel + i] = scale * std::sin(i * factor);
    }
  }
  return buffer;
}

std::vector<float> GetOutputBuffer(size_t samples_per_channel,
                                   size_t num_channels) {
  return std::vector<float>(num_channels * samples_per_channel);
}

// Creates an IAMF downmix mode metadata block.
roar_metadata_t CreateDownmixModeMetadata(int mode, uint32_t duration) {
  roar_metadata_t metadata = {};
  metadata.type = ck_roar_metadata_iamf_downmix_mode;
  metadata.duration = duration;
  metadata.iamf_downmix_mode.mode = mode;
  return metadata;
}

namespace {

roar_metadata_t CreateObjectMetadata(uint32_t duration) {
  roar_metadata_t metadata = {};
  metadata.type = ck_roar_metadata_object_positions;
  metadata.duration = static_cast<int>(duration);
  metadata.object_positions.param_type = ck_roar_param_constant;
  return metadata;
}

roar_metadata_t CreateAnimatedObjectMetadataInternal(uint32_t duration) {
  roar_metadata_t metadata = {};
  metadata.type = ck_roar_metadata_object_positions;
  metadata.duration = static_cast<int>(duration);
  metadata.object_positions.param_type = ck_roar_param_animated;
  return metadata;
}

}  // namespace

roar_metadata_t CreateObjectMetadata(const std::vector<roar_polar_t>& positions,
                                     uint32_t duration) {
  roar_metadata_t metadata = CreateObjectMetadata(duration);
  metadata.object_positions.position_type = ck_roar_polar;
  metadata.object_positions.num_objects = positions.size();
  for (uint32_t i = 0; i < positions.size(); ++i) {
    metadata.object_positions.polar_positions[i].azimuth = positions[i].azimuth;
    metadata.object_positions.polar_positions[i].elevation =
        positions[i].elevation;
    metadata.object_positions.polar_positions[i].distance =
        positions[i].distance;
  }
  return metadata;
}

roar_metadata_t CreateObjectMetadata(
    const std::vector<roar_cartesian_t>& positions, uint32_t duration) {
  roar_metadata_t metadata = CreateObjectMetadata(duration);
  metadata.object_positions.position_type = ck_roar_cartesian;
  metadata.object_positions.num_objects = positions.size();
  for (uint32_t i = 0; i < positions.size(); ++i) {
    metadata.object_positions.cartesian_positions[i].x = positions[i].x;
    metadata.object_positions.cartesian_positions[i].y = positions[i].y;
    metadata.object_positions.cartesian_positions[i].z = positions[i].z;
  }
  return metadata;
}

roar_metadata_t CreateAnimatedObjectMetadata(
    const std::vector<roar_animated_polar_t>& positions, uint32_t duration) {
  roar_metadata_t metadata = CreateAnimatedObjectMetadataInternal(duration);
  metadata.object_positions.position_type = ck_roar_polar;
  metadata.object_positions.num_objects = positions.size();
  for (uint32_t i = 0; i < positions.size(); ++i) {
    metadata.object_positions.animated_polar_positions[i] = positions[i];
  }
  return metadata;
}

roar_metadata_t CreateAnimatedObjectMetadata(
    const std::vector<roar_animated_cartesian_t>& positions,
    uint32_t duration) {
  roar_metadata_t metadata = CreateAnimatedObjectMetadataInternal(duration);
  metadata.object_positions.position_type = ck_roar_cartesian;
  metadata.object_positions.num_objects = positions.size();
  for (uint32_t i = 0; i < positions.size(); ++i) {
    metadata.object_positions.animated_cartesian_positions[i] = positions[i];
  }
  return metadata;
}

roar_metadata_t CreateConstantGainMetadata(uint32_t id, float gain_db,
                                           uint32_t duration) {
  roar_metadata_t metadata = {};
  metadata.type = ck_roar_metadata_gain;
  metadata.duration = duration;
  metadata.gain.id = id;
  metadata.gain.param_type = ck_roar_param_constant;
  metadata.gain.constant_gain = gain_db;
  return metadata;
}

roar_metadata_t CreateMultipleGainMetadata(uint32_t id,
                                           const std::vector<float>& gains_db,
                                           uint32_t duration) {
  roar_metadata_t metadata = {};
  metadata.type = ck_roar_metadata_gain;
  metadata.duration = duration;
  metadata.gain.id = id;
  metadata.gain.param_type = ck_roar_param_multiple;
  metadata.gain.gain_array = const_cast<float*>(gains_db.data());
  return metadata;
}

roar_metadata_t CreateAnimatedGainMetadata(
    uint32_t id, roar_animation_type_t anim_type, float start, float end,
    float control, float control_relative_time, uint32_t duration) {
  roar_metadata_t metadata = {};
  metadata.type = ck_roar_metadata_gain;
  metadata.duration = duration;
  metadata.gain.id = id;
  metadata.gain.param_type = ck_roar_param_animated;
  metadata.gain.animated_gains.animation_type = anim_type;
  metadata.gain.animated_gains.data.start = start;
  metadata.gain.animated_gains.data.end = end;
  metadata.gain.animated_gains.data.control = control;
  metadata.gain.animated_gains.data.control_relative_time =
      control_relative_time;
  return metadata;
}

namespace {
std::string DescribeChannelBased(
    const roar_audio_element_config_t& element_config) {
  return absl::StrFormat("ChannelBased(layout=%d)", element_config.cbc.layout);
}

std::string DescribeSceneBased(
    const roar_audio_element_config_t& element_config) {
  return absl::StrFormat("SceneBased(order=%d)", element_config.sbc.order);
}

std::string DescribeObjectBased(
    const roar_audio_element_config_t& element_config) {
  return absl::StrFormat("ObjectBased(num_objects=%d)",
                         element_config.obc.num_objects);
}

}  // namespace

std::string Describe(const roar_audio_element_config_t& element_config) {
  switch (element_config.type) {
    case ck_roar_channel_based:
      return DescribeChannelBased(element_config);
    case ck_roar_scene_based:
      return DescribeSceneBased(element_config);
    case ck_roar_object_based:
      return DescribeObjectBased(element_config);
    default:
      return "Unknown";
  }
}

}  // namespace roar_equivalence_testing
