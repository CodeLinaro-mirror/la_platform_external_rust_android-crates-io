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
#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <memory>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

#include "absl/strings/str_format.h"
#include "equivalence_testing/test_helpers.h"
#include "gmock/gmock.h"
#include "gtest/gtest.h"
#include "include/roar.h"

namespace roar_equivalence_testing {
namespace {

using ::testing::FloatNear;
using ::testing::NotNull;
using ::testing::Pointwise;

// Tolerable error level.
constexpr float kEpsilon = 1e-6;
constexpr uint32_t kTargetSamplesToRender = 24000;

// Simple struct to pair element and metadata.
struct ElementWithMetadata {
  roar_audio_element_config_t element_config;
  std::optional<roar_metadata_t> metadata;
};

// ===== Runs a single test case =====
void RunTest(roar_config_t config,
             const std::vector<ElementWithMetadata>& elements,
             std::string_view error_message) {
  // Wrap the C pointers in unique_ptrs with custom deleters for automatic
  // memory management.
  using OarPtr = std::unique_ptr<roar_t, decltype(&oar_destroy)>;
  OarPtr oar(oar_create(&config), oar_destroy);
  ASSERT_THAT(oar, NotNull()) << error_message;

  using RoarPtr = std::unique_ptr<roar_t, decltype(&roar_destroy)>;
  RoarPtr roar(roar_create(&config), roar_destroy);
  ASSERT_THAT(roar, NotNull()) << error_message;

  const uint32_t c_samples_per_channel = oar_get_samples_per_channel(oar.get());
  const uint32_t r_samples_per_channel =
      roar_get_samples_per_channel(roar.get());
  ASSERT_EQ(c_samples_per_channel, r_samples_per_channel) << error_message;
  ASSERT_EQ(config.samples_per_channel, c_samples_per_channel) << error_message;

  // Add groups
  const int c_group_id = oar_add_audio_group(oar.get());
  ASSERT_GE(c_group_id, 0) << error_message;  // 0 or 1 are valid group IDs.
  const int r_group_id = roar_add_audio_group(roar.get());
  ASSERT_GE(r_group_id, 0) << error_message;

  // Add elements
  for (size_t i = 0; i < elements.size(); ++i) {
    uint32_t element_id = i;
    const auto& element = elements[i];
    const int c_add = oar_add_audio_element(oar.get(), c_group_id, element_id,
                                            &element.element_config);
    ASSERT_EQ(c_add, 0) << error_message;  // 0 signals success.
    const int r_add = roar_add_audio_element(roar.get(), r_group_id, element_id,
                                             &element.element_config);
    ASSERT_EQ(r_add, 0) << error_message;
    if (element.metadata.has_value()) {
      const int c_meta_update = oar_update_audio_element_metadata(
          oar.get(), element_id, &element.metadata.value());
      ASSERT_EQ(c_meta_update, 0) << error_message;

      const int r_meta_update = roar_update_audio_element_metadata(
          roar.get(), element_id, &element.metadata.value());
      ASSERT_EQ(r_meta_update, 0) << error_message;
    }
  }

  // Prepare input audio for each element.
  // Input audio lifetime must survive through to the render calls.
  std::vector<std::vector<float>> all_input_data(elements.size());
  std::vector<roar_audio_block_t> input_blocks(elements.size());
  for (size_t i = 0; i < elements.size(); ++i) {
    uint32_t element_id = i;
    const uint32_t input_channels =
        roar_get_number_of_audio_element_channels(roar.get(), element_id);
    ASSERT_EQ(oar_get_number_of_audio_element_channels(oar.get(), element_id),
              input_channels)
        << error_message;
    all_input_data[i] = GetInputData(c_samples_per_channel, input_channels,
                                     config.sampling_rate);
    input_blocks[i].channels = input_channels;
    input_blocks[i].samples_per_channel = c_samples_per_channel;
    input_blocks[i].data = all_input_data[i].data();
  }

  // Get output buffers ready
  const uint32_t c_output_channels =
      oar_get_number_of_output_channels(oar.get());
  const uint32_t r_output_channels =
      roar_get_number_of_output_channels(roar.get());
  ASSERT_EQ(c_output_channels, r_output_channels) << error_message;

  auto c_output = GetOutputBuffer(c_samples_per_channel, c_output_channels);
  roar_audio_block_t c_output_block = {};
  c_output_block.channels = c_output_channels;
  c_output_block.samples_per_channel = c_samples_per_channel;
  c_output_block.data = c_output.data();

  auto r_output = GetOutputBuffer(r_samples_per_channel, r_output_channels);
  roar_audio_block_t r_output_block = {};
  r_output_block.channels = r_output_channels;
  r_output_block.samples_per_channel = r_samples_per_channel;
  r_output_block.data = r_output.data();

  // Render multiple steps to reach at least kTargetSamplesToRender samples.
  for (uint32_t samples_rendered = 0, step = 0;
       samples_rendered < kTargetSamplesToRender;
       samples_rendered += c_samples_per_channel, ++step) {
    for (size_t i = 0; i < elements.size(); ++i) {
      uint32_t element_id = i;
      ASSERT_EQ(oar_update_audio_element_data(oar.get(), element_id,
                                              &input_blocks[i]),
                0)
          << error_message << " at step " << step;
      ASSERT_EQ(roar_update_audio_element_data(roar.get(), element_id,
                                               &input_blocks[i]),
                0)
          << error_message << " at step " << step;
    }

    ASSERT_EQ(oar_render(oar.get(), &c_output_block), 0)
        << error_message << " at step " << step;
    ASSERT_EQ(roar_render(roar.get(), &r_output_block), 0)
        << error_message << " at step " << step;

    EXPECT_THAT(c_output, Pointwise(FloatNear(kEpsilon), r_output))
        << error_message << " at step " << step;

    std::fill(c_output.begin(), c_output.end(), 0.0f);
    std::fill(r_output.begin(), r_output.end(), 0.0f);
  }
}

// ===== Tests =====

TEST(RoarEquivalence, ChannelBasedInput) {
  for (auto input : GetAllChannelBasedInputs()) {
    for (auto out_config : GetAllOutputConfigs()) {
      roar_audio_element_config_t element_config = {};
      element_config.type = ck_roar_channel_based;
      element_config.cbc.layout = input;
      std::string error_message =
          absl::StrFormat("Input layout: %d, Output layout: %d", input,
                          out_config.target_layout);

      RunTest(out_config, {{element_config, std::nullopt}}, error_message);
    }
  }
}

TEST(RoarEquivalence, ChannelBasedInputWithDownmixInfo) {
  for (auto input : GetAllChannelBasedInputs()) {
    for (auto out_config : GetAllOutputConfigs()) {
      roar_audio_element_config_t element_config = {};
      element_config.type = ck_roar_channel_based;
      element_config.cbc.layout = input;
      element_config.parameters.flags |=
          ROAR_PARAMETER_SET_FLAG_IAMF_DOWNMIX_INFO;
      element_config.parameters.downmix_info.mode = 0;
      element_config.parameters.downmix_info.weight_index = 5;
      std::string error_message =
          absl::StrFormat(" Input layout: %d, Output layout: %d", input,
                          out_config.target_layout);

      RunTest(out_config, {{element_config, std::nullopt}}, error_message);
    }
  }
}

TEST(RoarEquivalence, ObjectBasedInput) {
  for (auto output_config : GetAllOutputConfigs()) {
    roar_audio_element_config_t element_config = {};
    element_config.type = ck_roar_object_based;
    element_config.obc.num_objects = 2;

    std::vector<roar_polar_t> positions = {{50.0f, 0.5f, 1.0f},
                                           {-105.0f, -1.3f, 3.0f}};

    roar_metadata_t metadata =
        CreateObjectMetadata(positions, kTargetSamplesToRender);

    std::string error_message =
        absl::StrFormat("ObjectBasedInput (2 objects -> layout %d)",
                        output_config.target_layout);
    RunTest(output_config, {{element_config, metadata}}, error_message);
  }
}

TEST(RoarEquivalence, SceneBasedInput) {
  for (auto config : GetAllOutputConfigs()) {
    for (auto order : GetAllAmbisonicOrders()) {
      roar_audio_element_config_t element_config = {};
      element_config.type = ck_roar_scene_based;
      element_config.sbc.order = order;

      std::string error_message = absl::StrFormat(
          "scene_based (%d order -> layout %d)", order, config.target_layout);
      RunTest(config, {{element_config, std::nullopt}}, error_message);
    }
  }
}

TEST(RoarEquivalence, CombinationInput) {
  for (auto config : GetAllOutputConfigs()) {
    // One 5.1 channel-based element.
    roar_audio_element_config_t channel_element_config = {};
    channel_element_config.type = ck_roar_channel_based;
    channel_element_config.cbc.layout = ck_roar_layout_51;

    // 2 object-based elements.
    roar_audio_element_config_t object_element_config = {};
    object_element_config.type = ck_roar_object_based;
    object_element_config.obc.num_objects = 2;
    std::vector<roar_polar_t> positions = {{45.0f, 1.2f, 2.0f},
                                           {-180.0f, -0.1f, 1.0f}};
    roar_metadata_t metadata =
        CreateObjectMetadata(positions, kTargetSamplesToRender);

    // First order ambisonics element.
    roar_audio_element_config_t scene_element_config = {};
    scene_element_config.type = ck_roar_scene_based;
    scene_element_config.sbc.order = ck_roar_1oa;

    std::string error_message = absl::StrFormat(
        "(5.1, 2 objects, 1OA) -> layout %d", config.target_layout);
    RunTest(config,
            {{channel_element_config, std::nullopt},
             {object_element_config, metadata},
             {scene_element_config, std::nullopt}},
            error_message);
  }
}

}  // namespace
}  // namespace roar_equivalence_testing
