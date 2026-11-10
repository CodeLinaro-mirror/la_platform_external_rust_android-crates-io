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

// Tolerable error level between C liboar and Rust ROAR.
constexpr float kEpsilon = 1e-6f;

constexpr uint32_t kSamplesPerChannel = 512;
constexpr uint32_t kSampleRate = 48000;
constexpr size_t kDefaultNumRenders = 3;
constexpr uint32_t kSubframeSize = 32;

// Holds a named animated motion test case.
struct ObjectMotionTestCase {
  std::string name;
  roar_metadata_t metadata;
};

// Generates the 7 valid motion variations:
// - Polar: Static, Step, Linear (Bezier is only supported for Cartesian).
// - Cartesian: Static, Step, Linear, Bezier.
std::vector<ObjectMotionTestCase> GetAllObjectMotionVariations(
    uint32_t total_duration) {
  return {
      // --- Polar ---
      {"PolarStatic",
       CreateObjectMetadata(
           std::vector<roar_polar_t>{{/*azimuth=*/45.0f, /*elevation=*/15.0f,
                                      /*distance=*/0.8f}},
           total_duration)},
      {"PolarStep", CreateAnimatedObjectMetadata(
                        std::vector<roar_animated_polar_t>{{
                            .animation_type = ck_roar_animation_type_step,
                            .azimuth = {/*start=*/-90.0f},
                            .elevation = {/*start=*/15.0f},
                            .distance = {/*start=*/0.8f},
                        }},
                        total_duration)},
      {"PolarLinear", CreateAnimatedObjectMetadata(
                          std::vector<roar_animated_polar_t>{{
                              .animation_type = ck_roar_animation_type_linear,
                              .azimuth = {/*start=*/-90.0f, /*end=*/90.0f},
                              .elevation = {/*start=*/0.0f, /*end=*/30.0f},
                              .distance = {/*start=*/0.5f, /*end=*/1.0f},
                          }},
                          total_duration)},

      // --- Cartesian ---
      {"CartesianStatic",
       CreateObjectMetadata(
           std::vector<roar_cartesian_t>{{/*x=*/0.0f, /*y=*/0.8f, /*z=*/0.2f}},
           total_duration)},
      {"CartesianStep", CreateAnimatedObjectMetadata(
                            std::vector<roar_animated_cartesian_t>{{
                                .animation_type = ck_roar_animation_type_step,
                                .x = {/*start=*/-1.0f},
                                .y = {/*start=*/0.5f},
                                .z = {/*start=*/-0.5f},
                            }},
                            total_duration)},
      {"CartesianLinear",
       CreateAnimatedObjectMetadata(
           std::vector<roar_animated_cartesian_t>{{
               .animation_type = ck_roar_animation_type_linear,
               .x = {/*start=*/-1.0f, /*end=*/1.0f},
               .y = {/*start=*/0.5f, /*end=*/-0.5f},
               .z = {/*start=*/-0.2f, /*end=*/0.4f},
           }},
           total_duration)},
      {"CartesianBezier",
       CreateAnimatedObjectMetadata(
           std::vector<roar_animated_cartesian_t>{{
               .animation_type = ck_roar_animation_type_bezier,
               .x = {/*start=*/-1.0f,
                     /*end=*/1.0f,
                     /*control=*/-0.5f,
                     /*control_relative_time=*/0.25f},
               .y = {/*start=*/0.8f,
                     /*end=*/-0.8f,
                     /*control=*/0.9f,
                     /*control_relative_time=*/0.75f},
               .z = {/*start=*/-0.2f,
                     /*end=*/0.2f,
                     /*control=*/0.5f,
                     /*control_relative_time=*/0.5f},
           }},
           total_duration)},
  };
}

// Runs the OAR-ROAR equivalence test.
void RunAnimatedObjectTest(
    roar_config_t config, const roar_audio_element_config_t& element_config,
    const std::vector<roar_metadata_t>& metadata_sequence, size_t num_renders,
    std::optional<uint32_t> metadata_unit_to_process,
    std::string_view error_message) {
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

  const int c_group_id = oar_add_audio_group(oar.get());
  ASSERT_GE(c_group_id, 0) << error_message;
  const int r_group_id = roar_add_audio_group(roar.get());
  ASSERT_GE(r_group_id, 0) << error_message;

  uint32_t element_id = 0;
  const int c_add =
      oar_add_audio_element(oar.get(), c_group_id, element_id, &element_config);
  ASSERT_EQ(c_add, 0) << error_message;
  const int r_add = roar_add_audio_element(roar.get(), r_group_id, element_id,
                                           &element_config);
  ASSERT_EQ(r_add, 0) << error_message;

  // `metadata_unit_to_process` is an optional way to increase the resolution of
  // animations.
  if (metadata_unit_to_process.has_value()) {
    ASSERT_EQ(oar_set_metadata_unit_to_process(
                  oar.get(), ck_roar_metadata_object_positions,
                  metadata_unit_to_process.value()),
              0)
        << error_message;
    ASSERT_EQ(roar_set_metadata_unit_to_process(
                  roar.get(), ck_roar_metadata_object_positions,
                  metadata_unit_to_process.value()),
              0)
        << error_message;
  }

  for (const auto& metadata : metadata_sequence) {
    ASSERT_EQ(
        oar_update_audio_element_metadata(oar.get(), element_id, &metadata), 0)
        << error_message;
    ASSERT_EQ(
        roar_update_audio_element_metadata(roar.get(), element_id, &metadata),
        0)
        << error_message;
  }

  const uint32_t input_channels =
      roar_get_number_of_audio_element_channels(roar.get(), element_id);
  ASSERT_EQ(oar_get_number_of_audio_element_channels(oar.get(), element_id),
            input_channels)
      << error_message;
  std::vector<std::vector<float>> input_data;
  input_data.reserve(num_renders);
  for (size_t render_i = 0; render_i < num_renders; ++render_i) {
    input_data.push_back(GetInputData(c_samples_per_channel, input_channels,
                                      config.sampling_rate));
  }

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

  for (size_t render_i = 0; render_i < num_renders; ++render_i) {
    roar_audio_block_t input_block = {};
    input_block.channels = input_channels;
    input_block.samples_per_channel = c_samples_per_channel;
    input_block.data = input_data[render_i].data();

    ASSERT_EQ(
        oar_update_audio_element_data(oar.get(), element_id, &input_block), 0)
        << error_message << " at render " << render_i;
    ASSERT_EQ(
        roar_update_audio_element_data(roar.get(), element_id, &input_block), 0)
        << error_message << " at render " << render_i;

    ASSERT_EQ(oar_render(oar.get(), &c_output_block), 0)
        << error_message << " at render " << render_i;
    ASSERT_EQ(roar_render(roar.get(), &r_output_block), 0)
        << error_message << " at render " << render_i;

    EXPECT_THAT(c_output, Pointwise(FloatNear(kEpsilon), r_output))
        << error_message << " at render " << render_i;

    std::fill(c_output.begin(), c_output.end(), 0.0f);
    std::fill(r_output.begin(), r_output.end(), 0.0f);
  }
}

// Tests rendering equivalence across all 7 motion variations and all output
// layouts.
TEST(AnimatedObjectTest, MotionVariations_RenderEquivalentlyAcrossAllLayouts) {
  const size_t num_renders = kDefaultNumRenders;
  const uint32_t total_duration = num_renders * kSamplesPerChannel;

  for (const auto& config :
       GetAllOutputConfigs(kSamplesPerChannel, kSampleRate)) {
    for (const auto& test_case : GetAllObjectMotionVariations(total_duration)) {
      roar_audio_element_config_t element_config = {};
      element_config.type = ck_roar_object_based;
      element_config.obc.num_objects = 1;

      std::string error_message = absl::StrFormat(
          "Layout: %d, Variation: %s", config.target_layout, test_case.name);

      RunAnimatedObjectTest(config, element_config, {test_case.metadata},
                            num_renders, std::nullopt, error_message);
    }
  }
}

// Tests subframe metadata processing (every 32 samples) across motion
// variations.
// Note: Subframe processing is tested on loudspeaker rendering only because
// liboar's binaural backend (OBR) crashes when passed sub-frame blocks
// (ABSL_CHECK_EQ in ObrImpl::Process).
TEST(AnimatedObjectTest,
     SubframeProcessing_RendersEquivalentlyAcrossLoudspeakerLayouts) {
  const size_t num_renders = kDefaultNumRenders;
  const uint32_t total_duration = num_renders * kSamplesPerChannel;
  const std::vector<roar_layout_t> layouts = {ck_roar_layout_stereo,
                                              ck_roar_layout_51};

  for (auto layout : layouts) {
    for (const auto& test_case : GetAllObjectMotionVariations(total_duration)) {
      roar_config_t config = {};
      config.target_layout = layout;
      config.samples_per_channel = kSamplesPerChannel;
      config.sampling_rate = kSampleRate;

      roar_audio_element_config_t element_config = {};
      element_config.type = ck_roar_object_based;
      element_config.obc.num_objects = 1;

      std::string error_message = absl::StrFormat(
          "Subframe32 (Layout: %d, Variation: %s)", layout, test_case.name);

      RunAnimatedObjectTest(config, element_config, {test_case.metadata},
                            num_renders, kSubframeSize, error_message);
    }
  }
}

// Attempt to move azimuth from -90 to +90 but with 0 duration.
// For OLR, this means the metadata is discarded and there is no output.
// For OBR, the metadata is applied immediately and maintained.
// Related bug: https://github.com/AOMediaCodec/oar/issues/20
TEST(AnimatedObjectTest,
     ZeroDurationMetadata_IsIgnoredAndMaintainsDefaultPosition) {
  roar_config_t config = {};
  config.target_layout = ck_roar_layout_binaural;
  config.samples_per_channel = kSamplesPerChannel;
  config.sampling_rate = kSampleRate;

  roar_audio_element_config_t element_config = {};
  element_config.type = ck_roar_object_based;
  element_config.obc.num_objects = 1;

  const size_t num_renders = kDefaultNumRenders;
  std::vector<roar_metadata_t> metadata_seq = {CreateAnimatedObjectMetadata(
      std::vector<roar_animated_polar_t>{{
          .animation_type = ck_roar_animation_type_linear,
          .azimuth = {/*start=*/-90.0f, /*end=*/90.0f},
          .elevation = {/*start=*/0.0f, /*end=*/0.0f},
          .distance = {/*start=*/1.0f, /*end=*/1.0f},
      }},
      /*duration=*/0)};

  RunAnimatedObjectTest(
      config, element_config, metadata_seq, num_renders,
      /*metadata_unit_to_process=*/std::nullopt,
      "ZeroDurationMetadata_IsIgnoredAndMaintainsDefaultPosition");
}

// Two consecutive animated polar moves: -90 -> 0 for 2 renders, then 0 -> 90
// for 2 renders.
TEST(AnimatedObjectTest,
     ConsecutiveLinearMoves_RenderSequentiallyAcrossBlocks) {
  roar_config_t config = {};
  config.target_layout = ck_roar_layout_binaural;
  config.samples_per_channel = kSamplesPerChannel;
  config.sampling_rate = kSampleRate;

  roar_audio_element_config_t element_config = {};
  element_config.type = ck_roar_object_based;
  element_config.obc.num_objects = 1;

  const size_t num_renders_per_move = 2;
  const uint32_t move_duration = num_renders_per_move * kSamplesPerChannel;
  const size_t num_renders = 2 * num_renders_per_move;
  std::vector<roar_metadata_t> metadata_seq = {
      CreateAnimatedObjectMetadata(
          std::vector<roar_animated_polar_t>{{
              .animation_type = ck_roar_animation_type_linear,
              .azimuth = {/*start=*/-90.0f, /*end=*/0.0f},
              .elevation = {/*start=*/0.0f, /*end=*/0.0f},
              .distance = {/*start=*/1.0f, /*end=*/1.0f},
          }},
          move_duration),
      CreateAnimatedObjectMetadata(
          std::vector<roar_animated_polar_t>{{
              .animation_type = ck_roar_animation_type_linear,
              .azimuth = {/*start=*/0.0f, /*end=*/90.0f},
              .elevation = {/*start=*/0.0f, /*end=*/0.0f},
              .distance = {/*start=*/1.0f, /*end=*/1.0f},
          }},
          move_duration),
  };

  RunAnimatedObjectTest(
      config, element_config, metadata_seq, num_renders,
      /*metadata_unit_to_process=*/std::nullopt,
      "ConsecutiveLinearMoves_RenderSequentiallyAcrossBlocks");
}

// Tests equivalence for multiple animated objects moving concurrently along
// distinct trajectories.
TEST(AnimatedObjectTest,
     MultiObjectAnimation_RendersDistinctTrajectoriesIndependently) {
  const size_t num_renders = kDefaultNumRenders;
  const uint32_t total_duration = num_renders * kSamplesPerChannel;

  for (const auto& config :
       GetAllOutputConfigs(kSamplesPerChannel, kSampleRate)) {
    roar_audio_element_config_t element_config = {};
    element_config.type = ck_roar_object_based;
    element_config.obc.num_objects = 2;

    roar_animated_polar_t obj0 = {
        .animation_type = ck_roar_animation_type_linear,
        .azimuth = {/*start=*/-90.0f, /*end=*/0.0f},
        .elevation = {/*start=*/0.0f, /*end=*/30.0f},
        .distance = {/*start=*/1.0f, /*end=*/0.8f},
    };
    roar_animated_polar_t obj1 = {
        .animation_type = ck_roar_animation_type_linear,
        .azimuth = {/*start=*/90.0f, /*end=*/0.0f},
        .elevation = {/*start=*/0.0f, /*end=*/-20.0f},
        .distance = {/*start=*/0.5f, /*end=*/1.0f},
    };

    roar_metadata_t metadata =
        CreateAnimatedObjectMetadata({obj0, obj1}, total_duration);

    std::string error_message = absl::StrFormat(
        "MultiObjectAnimation (Layout: %d)", config.target_layout);

    RunAnimatedObjectTest(config, element_config, {metadata}, num_renders,
                          /*metadata_unit_to_process=*/std::nullopt,
                          error_message);
  }
}

// Tests equivalence when metadata duration is shorter than total render
// duration.
// - Binaural (OBR) maintains the last position.
// - Loudspeaker (OLR) outputs silence (since its metadata queue is depleted).
// Related bug: https://github.com/AOMediaCodec/oar/issues/20.
TEST(AnimatedObjectTest,
     TrajectoryExpiration_HoldsTerminalPositionAfterAnimationCompletes) {
  const size_t num_renders = 3;
  // Animation duration lasts for only 1 render block (512 samples);
  // remaining renders (renders 1 and 2) execute with expired trajectory.
  const uint32_t move_duration = 1 * kSamplesPerChannel;

  for (const auto& config :
       GetAllOutputConfigs(kSamplesPerChannel, kSampleRate)) {
    roar_audio_element_config_t element_config = {};
    element_config.type = ck_roar_object_based;
    element_config.obc.num_objects = 1;

    roar_metadata_t metadata = CreateAnimatedObjectMetadata(
        std::vector<roar_animated_polar_t>{{
            .animation_type = ck_roar_animation_type_linear,
            .azimuth = {/*start=*/-90.0f, /*end=*/45.0f},
            .elevation = {/*start=*/0.0f, /*end=*/20.0f},
            .distance = {/*start=*/0.6f, /*end=*/1.0f},
        }},
        move_duration);

    std::string error_message = absl::StrFormat(
        "TrajectoryExpiration (Layout: %d)", config.target_layout);

    RunAnimatedObjectTest(config, element_config, {metadata}, num_renders,
                          /*metadata_unit_to_process=*/std::nullopt,
                          error_message);
  }
}

}  // namespace
}  // namespace roar_equivalence_testing
