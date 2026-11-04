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
#include <vector>

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
const float kEpsilon = 1e-6;

const uint32_t samples_per_channel = 1024;
const uint32_t sampling_rate = 48000;

void RunGroupGainsTest(
    roar_config_t config, const roar_audio_element_config_t& element_config,
    const std::vector<std::optional<roar_metadata_t>>& metadata_sequence) {
  // Wrap the C pointers in unique_ptrs with custom deleters for automatic
  // memory management.
  using OarPtr = std::unique_ptr<roar_t, decltype(&oar_destroy)>;
  OarPtr oar(oar_create(&config), oar_destroy);
  ASSERT_THAT(oar, NotNull());

  using RoarPtr = std::unique_ptr<roar_t, decltype(&roar_destroy)>;
  RoarPtr roar(roar_create(&config), roar_destroy);
  ASSERT_THAT(roar, NotNull());

  const uint32_t c_samples_per_channel = oar_get_samples_per_channel(oar.get());
  const uint32_t r_samples_per_channel =
      roar_get_samples_per_channel(roar.get());
  ASSERT_EQ(c_samples_per_channel, r_samples_per_channel);
  ASSERT_EQ(config.samples_per_channel, c_samples_per_channel);

  // Add groups
  const int c_group_id = oar_add_audio_group(oar.get());
  ASSERT_GE(c_group_id, 0);
  const int r_group_id = roar_add_audio_group(roar.get());
  ASSERT_GE(r_group_id, 0);

  // Add element
  uint32_t element_id = 0;
  const int c_add =
      oar_add_audio_element(oar.get(), c_group_id, element_id, &element_config);
  ASSERT_EQ(c_add, 0);
  const int r_add = roar_add_audio_element(roar.get(), r_group_id, element_id,
                                           &element_config);
  ASSERT_EQ(r_add, 0);

  // Feed input audio for the element. We will render multiple chunks.
  const uint32_t input_channels =
      roar_get_number_of_audio_element_channels(roar.get(), element_id);
  ASSERT_EQ(oar_get_number_of_audio_element_channels(oar.get(), element_id),
            input_channels);
  std::vector<std::vector<float>> input_chunks;
  const size_t num_chunks = metadata_sequence.size();
  input_chunks.reserve(num_chunks);
  for (size_t chunk = 0; chunk < num_chunks; ++chunk) {
    input_chunks.push_back(GetInputData(c_samples_per_channel, input_channels,
                                        config.sampling_rate));
  }

  // Get output buffers ready
  const uint32_t c_output_channels =
      oar_get_number_of_output_channels(oar.get());
  const uint32_t r_output_channels =
      roar_get_number_of_output_channels(roar.get());
  ASSERT_EQ(c_output_channels, r_output_channels);

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

  for (size_t chunk = 0; chunk < num_chunks; ++chunk) {
    // Update element data
    roar_audio_block_t input_block = {};
    input_block.channels = input_channels;
    input_block.samples_per_channel = c_samples_per_channel;
    input_block.data = input_chunks[chunk].data();

    ASSERT_EQ(
        oar_update_audio_element_data(oar.get(), element_id, &input_block), 0)
        << " at chunk " << chunk;
    ASSERT_EQ(
        roar_update_audio_element_data(roar.get(), element_id, &input_block), 0)
        << " at chunk " << chunk;

    // Update group gain metadata if provided (uses oar_update_metadata)
    const auto& metadata_opt = metadata_sequence[chunk];
    if (metadata_opt.has_value()) {
      const auto& metadata = metadata_opt.value();
      ASSERT_EQ(oar_update_metadata(oar.get(), c_group_id, &metadata), 0)
          << " at chunk " << chunk;
      ASSERT_EQ(roar_update_metadata(roar.get(), r_group_id, &metadata), 0)
          << " at chunk " << chunk;
    }

    // Render
    ASSERT_EQ(oar_render(oar.get(), &c_output_block), 0)
        << " at chunk " << chunk;
    ASSERT_EQ(roar_render(roar.get(), &r_output_block), 0)
        << " at chunk " << chunk;

    // Compare
    EXPECT_THAT(c_output, Pointwise(FloatNear(kEpsilon), r_output));

    // Zero output before next call.
    std::fill(c_output.begin(), c_output.end(), 0.0f);
    std::fill(r_output.begin(), r_output.end(), 0.0f);
  }
}

TEST(GroupGainTest, ConstantGain) {
  roar_config_t config = {};
  config.target_layout = ck_roar_layout_stereo;
  config.samples_per_channel = samples_per_channel;
  config.sampling_rate = sampling_rate;

  roar_audio_element_config_t element_config = {};
  element_config.type = ck_roar_channel_based;
  element_config.cbc.layout = ck_roar_layout_stereo;

  std::vector<std::optional<roar_metadata_t>> metadata_seq = {
      CreateConstantGainMetadata(1, -6.0f, samples_per_channel),
      CreateConstantGainMetadata(1, -6.0f, samples_per_channel),
      CreateConstantGainMetadata(1, -6.0f, samples_per_channel),
  };

  RunGroupGainsTest(config, element_config, metadata_seq);
}

TEST(GroupGainTest, MultipleGain) {
  roar_config_t config = {};
  config.target_layout = ck_roar_layout_stereo;
  config.samples_per_channel = samples_per_channel;
  config.sampling_rate = sampling_rate;

  roar_audio_element_config_t element_config = {};
  element_config.type = ck_roar_channel_based;
  element_config.cbc.layout = ck_roar_layout_stereo;

  // We will run 2 chunks.
  // In each chunk we have a different per-sample gain vector.
  std::vector<float> gains_chunk0(samples_per_channel);
  std::vector<float> gains_chunk1(samples_per_channel);
  for (size_t i = 0; i < samples_per_channel; ++i) {
    gains_chunk0[i] = -10.0f * (static_cast<float>(i) / samples_per_channel);
    gains_chunk1[i] =
        -10.0f * (1.0f - static_cast<float>(i) / samples_per_channel);
  }

  std::vector<std::optional<roar_metadata_t>> metadata_seq = {
      CreateMultipleGainMetadata(1, gains_chunk0, samples_per_channel),
      CreateMultipleGainMetadata(1, gains_chunk1, samples_per_channel),
  };

  RunGroupGainsTest(config, element_config, metadata_seq);
}

TEST(GroupGainTest, AnimatedGainLinear) {
  roar_config_t config = {};
  config.target_layout = ck_roar_layout_stereo;
  config.samples_per_channel = samples_per_channel;
  config.sampling_rate = sampling_rate;

  roar_audio_element_config_t element_config = {};
  element_config.type = ck_roar_channel_based;
  element_config.cbc.layout = ck_roar_layout_stereo;

  const uint32_t total_duration = 3 * samples_per_channel;

  std::vector<std::optional<roar_metadata_t>> metadata_seq = {
      CreateAnimatedGainMetadata(1, ck_roar_animation_type_linear, 0.0f, -12.0f,
                                 0.0f, 0.0f, total_duration),
      std::nullopt,  // No follow-up metadata for chunks 2 and 3.
      std::nullopt,
  };

  RunGroupGainsTest(config, element_config, metadata_seq);
}

TEST(GroupGainTest, AnimatedGainStep) {
  roar_config_t config = {};
  config.target_layout = ck_roar_layout_stereo;
  config.samples_per_channel = samples_per_channel;
  config.sampling_rate = sampling_rate;

  roar_audio_element_config_t element_config = {};
  element_config.type = ck_roar_channel_based;
  element_config.cbc.layout = ck_roar_layout_stereo;

  const uint32_t total_duration = 3 * samples_per_channel;

  std::vector<std::optional<roar_metadata_t>> metadata_seq = {
      CreateAnimatedGainMetadata(1, ck_roar_animation_type_step, -6.0f, 0.0f,
                                 0.0f, 0.0f, total_duration),
      std::nullopt,  // No follow-up metadata for chunks 2 and 3.
      std::nullopt,
  };

  RunGroupGainsTest(config, element_config, metadata_seq);
}

TEST(GroupGainTest, AnimatedGainBezier) {
  roar_config_t config = {};
  config.target_layout = ck_roar_layout_stereo;
  config.samples_per_channel = samples_per_channel;
  config.sampling_rate = sampling_rate;

  roar_audio_element_config_t element_config = {};
  element_config.type = ck_roar_channel_based;
  element_config.cbc.layout = ck_roar_layout_stereo;

  // Bezier animation from 0dB to -12dB, with control -6dB at 50% time, over 3
  // chunks.
  const uint32_t total_duration = 3 * samples_per_channel;

  std::vector<std::optional<roar_metadata_t>> metadata_seq = {
      CreateAnimatedGainMetadata(1, ck_roar_animation_type_bezier, 0.0f, -12.0f,
                                 -6.0f, 0.5f, total_duration),
      std::nullopt,  // No follow-up metadata for chunks 2 and 3.
      std::nullopt,
  };

  RunGroupGainsTest(config, element_config, metadata_seq);
}

TEST(GroupGainTest, ConstantGainDurationZeroIgnored) {
  roar_config_t config = {};
  config.target_layout = ck_roar_layout_stereo;
  config.samples_per_channel = samples_per_channel;
  config.sampling_rate = sampling_rate;

  roar_audio_element_config_t element_config = {};
  element_config.type = ck_roar_channel_based;
  element_config.cbc.layout = ck_roar_layout_stereo;

  // Chunk 0: Default gain (0dB)
  // Chunk 1: Update to -6dB with duration 0. This should be IGNORED.
  //          Output should still be 0dB.
  // Chunk 2: No update. Output should still be 0dB.
  std::vector<std::optional<roar_metadata_t>> metadata_seq = {
      std::nullopt,
      CreateConstantGainMetadata(1, -6.0f, 0),
      std::nullopt,
  };

  RunGroupGainsTest(config, element_config, metadata_seq);
}

}  // namespace
}  // namespace roar_equivalence_testing
