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
#include <tuple>
#include <utility>
#include <vector>

#include "absl/strings/str_format.h"
#include "absl/strings/str_join.h"
#include "equivalence_testing/test_helpers.h"
#include "gmock/gmock.h"
#include "gtest/gtest.h"
#include "include/roar.h"

namespace roar_equivalence_testing {
namespace {

using ::testing::Combine;
using ::testing::FloatNear;
using ::testing::NotNull;
using ::testing::Pointwise;
using ::testing::TestWithParam;
using ::testing::Values;
using ::testing::ValuesIn;

constexpr float kEpsilon = 1e-6f;
constexpr uint32_t kSamplesPerChannel = 1024;
constexpr uint32_t kSamplingRate = 48000;

// Executes an equivalence test comparing C OAR and Rust ROAR for a single
// channel-based element with downmixing.
void RunDownmixTest(
    roar_config_t config, const roar_audio_element_config_t& element_config,
    const std::vector<std::optional<roar_metadata_t>>& metadata_sequence) {
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

  const int c_group_id = oar_add_audio_group(oar.get());
  ASSERT_GE(c_group_id, 0);
  const int r_group_id = roar_add_audio_group(roar.get());
  ASSERT_GE(r_group_id, 0);

  constexpr uint32_t kElementId = 0;
  ASSERT_EQ(
      oar_add_audio_element(oar.get(), c_group_id, kElementId, &element_config),
      0);
  ASSERT_EQ(roar_add_audio_element(roar.get(), r_group_id, kElementId,
                                   &element_config),
            0);

  const uint32_t input_channels =
      roar_get_number_of_audio_element_channels(roar.get(), kElementId);
  ASSERT_EQ(oar_get_number_of_audio_element_channels(oar.get(), kElementId),
            input_channels);
  const size_t num_chunks = metadata_sequence.size();
  std::vector<std::vector<float>> input_chunks;
  input_chunks.reserve(num_chunks);
  for (size_t chunk = 0; chunk < num_chunks; ++chunk) {
    input_chunks.push_back(GetInputData(c_samples_per_channel, input_channels,
                                        config.sampling_rate));
  }

  const uint32_t c_output_channels =
      oar_get_number_of_output_channels(oar.get());
  const uint32_t r_output_channels =
      roar_get_number_of_output_channels(roar.get());
  ASSERT_EQ(c_output_channels, r_output_channels);
  ASSERT_GT(c_output_channels, 0);

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
    roar_audio_block_t input_block = {};
    input_block.channels = input_channels;
    input_block.samples_per_channel = c_samples_per_channel;
    input_block.data = input_chunks[chunk].data();

    ASSERT_EQ(
        oar_update_audio_element_data(oar.get(), kElementId, &input_block), 0)
        << "at chunk " << chunk;
    ASSERT_EQ(
        roar_update_audio_element_data(roar.get(), kElementId, &input_block), 0)
        << "at chunk " << chunk;

    const auto& metadata_opt = metadata_sequence[chunk];
    if (metadata_opt.has_value()) {
      const auto& metadata = metadata_opt.value();
      ASSERT_EQ(
          oar_update_audio_element_metadata(oar.get(), kElementId, &metadata),
          0)
          << "at chunk " << chunk;
      ASSERT_EQ(
          roar_update_audio_element_metadata(roar.get(), kElementId, &metadata),
          0)
          << "at chunk " << chunk;
    }

    ASSERT_EQ(oar_render(oar.get(), &c_output_block), 0)
        << "at chunk " << chunk;
    ASSERT_EQ(roar_render(roar.get(), &r_output_block), 0)
        << "at chunk " << chunk;

    EXPECT_THAT(c_output, Pointwise(FloatNear(kEpsilon), r_output))
        << "at chunk " << chunk;

    std::fill(c_output.begin(), c_output.end(), 0.0f);
    std::fill(r_output.begin(), r_output.end(), 0.0f);
  }
}

// Executes an equivalence test comparing C OAR and Rust roar when dynamic
// downmix mode metadata is updated on a target element within a multi-element
// group.
void RunMultiElementDownmixTest(
    roar_config_t config,
    const roar_audio_element_config_t& target_element_config,
    const roar_audio_element_config_t& secondary_element_config,
    const std::vector<std::optional<roar_metadata_t>>& metadata_sequence) {
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

  const int c_group_id = oar_add_audio_group(oar.get());
  ASSERT_GE(c_group_id, 0);
  const int r_group_id = roar_add_audio_group(roar.get());
  ASSERT_GE(r_group_id, 0);

  constexpr uint32_t kTargetElementId = 0;
  ASSERT_EQ(oar_add_audio_element(oar.get(), c_group_id, kTargetElementId,
                                  &target_element_config),
            0);
  ASSERT_EQ(roar_add_audio_element(roar.get(), r_group_id, kTargetElementId,
                                   &target_element_config),
            0);

  constexpr uint32_t kSecondaryElementId = 1;
  ASSERT_EQ(oar_add_audio_element(oar.get(), c_group_id, kSecondaryElementId,
                                  &secondary_element_config),
            0);
  ASSERT_EQ(roar_add_audio_element(roar.get(), r_group_id, kSecondaryElementId,
                                   &secondary_element_config),
            0);

  const uint32_t input_channels_0 =
      roar_get_number_of_audio_element_channels(roar.get(), kTargetElementId);
  const uint32_t input_channels_1 = roar_get_number_of_audio_element_channels(
      roar.get(), kSecondaryElementId);

  const size_t num_chunks = metadata_sequence.size();
  std::vector<std::vector<float>> input_0_chunks;
  std::vector<std::vector<float>> input_1_chunks;
  input_0_chunks.reserve(num_chunks);
  input_1_chunks.reserve(num_chunks);

  for (size_t chunk = 0; chunk < num_chunks; ++chunk) {
    input_0_chunks.push_back(GetInputData(
        c_samples_per_channel, input_channels_0, config.sampling_rate, 200.0f));
    input_1_chunks.push_back(GetInputData(
        c_samples_per_channel, input_channels_1, config.sampling_rate, 600.0f));
  }

  const uint32_t c_output_channels =
      oar_get_number_of_output_channels(oar.get());
  const uint32_t r_output_channels =
      roar_get_number_of_output_channels(roar.get());
  ASSERT_EQ(c_output_channels, r_output_channels);
  ASSERT_GT(c_output_channels, 0);

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
    roar_audio_block_t input_0_block = {};
    input_0_block.channels = input_channels_0;
    input_0_block.samples_per_channel = c_samples_per_channel;
    input_0_block.data = input_0_chunks[chunk].data();

    ASSERT_EQ(oar_update_audio_element_data(oar.get(), kTargetElementId,
                                            &input_0_block),
              0)
        << "at chunk " << chunk;
    ASSERT_EQ(roar_update_audio_element_data(roar.get(), kTargetElementId,
                                             &input_0_block),
              0)
        << "at chunk " << chunk;

    roar_audio_block_t input_1_block = {};
    input_1_block.channels = input_channels_1;
    input_1_block.samples_per_channel = c_samples_per_channel;
    input_1_block.data = input_1_chunks[chunk].data();

    ASSERT_EQ(oar_update_audio_element_data(oar.get(), kSecondaryElementId,
                                            &input_1_block),
              0)
        << "at chunk " << chunk;
    ASSERT_EQ(roar_update_audio_element_data(roar.get(), kSecondaryElementId,
                                             &input_1_block),
              0)
        << "at chunk " << chunk;

    const auto& metadata_opt = metadata_sequence[chunk];
    if (metadata_opt.has_value()) {
      const auto& metadata = metadata_opt.value();
      ASSERT_EQ(oar_update_audio_element_metadata(oar.get(), kTargetElementId,
                                                  &metadata),
                0)
          << "at chunk " << chunk;
      ASSERT_EQ(roar_update_audio_element_metadata(roar.get(), kTargetElementId,
                                                   &metadata),
                0)
          << "at chunk " << chunk;
    }

    ASSERT_EQ(oar_render(oar.get(), &c_output_block), 0)
        << "at chunk " << chunk;
    ASSERT_EQ(roar_render(roar.get(), &r_output_block), 0)
        << "at chunk " << chunk;

    EXPECT_THAT(c_output, Pointwise(FloatNear(kEpsilon), r_output))
        << "at chunk " << chunk;

    std::fill(c_output.begin(), c_output.end(), 0.0f);
    std::fill(r_output.begin(), r_output.end(), 0.0f);
  }
}

// ===== Downmix information provided with the audio element and unchanged =====

using FixedDownmixParam = std::tuple<std::pair<roar_layout_t, roar_layout_t>,
                                     int /* mode */, int /* weight_index */>;
using FixedDownmixTest = TestWithParam<FixedDownmixParam>;

TEST_P(FixedDownmixTest, DownmixInfoWithElementConfig) {
  const auto& [layouts, mode, weight_index] = GetParam();
  const auto& [in_layout, out_layout] = layouts;

  roar_config_t config = {};
  config.target_layout = out_layout;
  config.samples_per_channel = kSamplesPerChannel;
  config.sampling_rate = kSamplingRate;

  roar_audio_element_config_t element_config = {};
  element_config.type = ck_roar_channel_based;
  element_config.cbc.layout = in_layout;
  element_config.parameters.flags |= ROAR_PARAMETER_SET_FLAG_IAMF_DOWNMIX_INFO;
  element_config.parameters.downmix_info.mode = mode;
  element_config.parameters.downmix_info.weight_index = weight_index;

  std::vector<std::optional<roar_metadata_t>> metadata_seq = {
      std::nullopt,
      std::nullopt,
  };

  RunDownmixTest(config, element_config, metadata_seq);
}

// Get a list of input/output layout pairs to test fixed downmix.
std::vector<std::pair<roar_layout_t, roar_layout_t>> GetValidDownmixLayouts() {
  return {
      {ck_roar_layout_71, ck_roar_layout_51},
      {ck_roar_layout_51, ck_roar_layout_stereo},
      {ck_roar_layout_71, ck_roar_layout_stereo},
      {ck_roar_layout_stereo, ck_roar_layout_mono},
      {ck_roar_layout_714, ck_roar_layout_512},
      {ck_roar_layout_514, ck_roar_layout_512},
      {ck_roar_layout_714, ck_roar_layout_312},
  };
}

std::string FixedDownmixParamsToString(
    const testing::TestParamInfo<FixedDownmixParam>& info) {
  const auto& [layouts, mode, weight] = info.param;
  const auto& [in_layout, out_layout] = layouts;
  return absl::StrFormat("In_%d_Out_%d_Mode_%d_Weight_%d",
                         static_cast<int>(in_layout),
                         static_cast<int>(out_layout), mode, weight);
}

INSTANTIATE_TEST_SUITE_P(
    AllLayoutsAndModes, FixedDownmixTest,
    Combine(ValuesIn(GetValidDownmixLayouts()),
            Values(0, 1, 2, 4, 5,
                   6),          // Modes 0,1,2 (negative) and 4,5,6 (positive)
            Values(0, 5, 10)),  // Boundary and midpoint weights
    FixedDownmixParamsToString);

// ===== Downmix info changes via metadata update during rendering =====

using DynamicDownmixParam =
    std::tuple<std::pair<roar_layout_t, roar_layout_t>, std::vector<int>>;
using DynamicDownmixModeTest = TestWithParam<DynamicDownmixParam>;

TEST_P(DynamicDownmixModeTest, DynamicModeTransitions) {
  const auto& [layouts, mode_seq] = GetParam();
  const auto& [in_layout, out_layout] = layouts;

  roar_config_t config = {};
  config.target_layout = out_layout;
  config.samples_per_channel = kSamplesPerChannel;
  config.sampling_rate = kSamplingRate;

  roar_audio_element_config_t element_config = {};
  element_config.type = ck_roar_channel_based;
  element_config.cbc.layout = in_layout;
  element_config.parameters.flags |= ROAR_PARAMETER_SET_FLAG_IAMF_DOWNMIX_INFO;
  element_config.parameters.downmix_info.mode = 0;
  element_config.parameters.downmix_info.weight_index = 5;

  std::vector<std::optional<roar_metadata_t>> metadata_seq;
  metadata_seq.reserve(mode_seq.size() + 1);
  metadata_seq.push_back(std::nullopt);
  for (int mode : mode_seq) {
    metadata_seq.push_back(CreateDownmixModeMetadata(mode, kSamplesPerChannel));
  }

  RunDownmixTest(config, element_config, metadata_seq);
}

// Get a list of input/output layout pairs to test dynamic downmix.
std::vector<std::pair<roar_layout_t, roar_layout_t>> GetDynamicLayouts() {
  return {
      {ck_roar_layout_51, ck_roar_layout_stereo},
      {ck_roar_layout_71, ck_roar_layout_51},
      {ck_roar_layout_714, ck_roar_layout_512},
  };
}

// Sequences of downmix modes to test.
std::vector<std::vector<int>> GetModeSequences() {
  return {
      {1, 2},        // Negative offset mode sequence
      {4, 5, 6},     // Positive offset mode sequence
      {4, 1, 5, 2},  // Alternating positive/negative mode sequence
  };
}

// Get a string name for the given dynamic downmix param.
std::string DynamicDownmixParamsToString(
    const testing::TestParamInfo<DynamicDownmixParam>& info) {
  const auto& [layouts, mode_seq] = info.param;
  const auto& [in_layout, out_layout] = layouts;
  return absl::StrFormat("In_%d_Out_%d_Modes_%s", static_cast<int>(in_layout),
                         static_cast<int>(out_layout),
                         absl::StrJoin(mode_seq, "_"));
}

INSTANTIATE_TEST_SUITE_P(DynamicTransitions, DynamicDownmixModeTest,
                         Combine(ValuesIn(GetDynamicLayouts()),
                                 ValuesIn(GetModeSequences())),
                         DynamicDownmixParamsToString);

// ===== Test downmix metadata duration and expiration logic =====

using DownmixDurationParam =
    std::tuple<int /* update_mode */, uint32_t /* duration */>;
using DownmixDurationTest = TestWithParam<DownmixDurationParam>;

TEST_P(DownmixDurationTest, DownmixDurationLogic) {
  const auto& [update_mode, duration] = GetParam();

  roar_config_t config = {};
  config.target_layout = ck_roar_layout_stereo;
  config.samples_per_channel = kSamplesPerChannel;
  config.sampling_rate = kSamplingRate;

  roar_audio_element_config_t element_config = {};
  element_config.type = ck_roar_channel_based;
  element_config.cbc.layout = ck_roar_layout_51;
  element_config.parameters.flags |= ROAR_PARAMETER_SET_FLAG_IAMF_DOWNMIX_INFO;
  element_config.parameters.downmix_info.mode = 0;
  element_config.parameters.downmix_info.weight_index = 5;

  std::vector<std::optional<roar_metadata_t>> metadata_seq = {
      std::nullopt,
      CreateDownmixModeMetadata(update_mode, duration),
      std::nullopt,
      std::nullopt,
  };

  RunDownmixTest(config, element_config, metadata_seq);
}

std::string DownmixDurationParamsToString(
    const testing::TestParamInfo<DownmixDurationParam>& info) {
  const auto& [mode, duration] = info.param;
  return absl::StrFormat("Mode_%d_Duration_%u", mode, duration);
}

INSTANTIATE_TEST_SUITE_P(DurationVariations, DownmixDurationTest,
                         Combine(Values(1, 2, 4, 5),  // mode
                                 Values(0, kSamplesPerChannel,
                                        2 * kSamplesPerChannel)),  // duration
                         DownmixDurationParamsToString);

// ===== Test multiple elements rendered with and without downmix info =====

using MultiElementParam = std::tuple<int /* mode */, uint32_t /* duration */>;
using MultiElementDownmixTest = TestWithParam<MultiElementParam>;

TEST_P(MultiElementDownmixTest, DownmixIsolation) {
  const auto& [mode, duration] = GetParam();

  roar_config_t config = {};
  config.target_layout = ck_roar_layout_stereo;
  config.samples_per_channel = kSamplesPerChannel;
  config.sampling_rate = kSamplingRate;

  // Target element: receives dynamic downmix mode updates.
  roar_audio_element_config_t target_element_config = {};
  target_element_config.type = ck_roar_channel_based;
  target_element_config.cbc.layout = ck_roar_layout_51;
  target_element_config.parameters.flags |=
      ROAR_PARAMETER_SET_FLAG_IAMF_DOWNMIX_INFO;
  target_element_config.parameters.downmix_info.mode = 0;
  target_element_config.parameters.downmix_info.weight_index = 5;

  // Secondary element: static stereo baseline without downmix metadata.
  roar_audio_element_config_t secondary_element_config = {};
  secondary_element_config.type = ck_roar_channel_based;
  secondary_element_config.cbc.layout = ck_roar_layout_stereo;

  std::vector<std::optional<roar_metadata_t>> metadata_seq = {
      std::nullopt,
      CreateDownmixModeMetadata(mode, duration),
      std::nullopt,
  };

  RunMultiElementDownmixTest(config, target_element_config,
                             secondary_element_config, metadata_seq);
}

std::string MultiElementParamName(
    const testing::TestParamInfo<MultiElementParam>& info) {
  const auto& [mode, duration] = info.param;
  return absl::StrFormat("TargetMode_%d_Duration_%u", mode, duration);
}

INSTANTIATE_TEST_SUITE_P(IsolationModes, MultiElementDownmixTest,
                         Combine(Values(1, 2, 4, 5, 6),           // Mode
                                 Values(0, kSamplesPerChannel)),  // Duration
                         MultiElementParamName);

// ===== Advisory downmix mode metadata equivalence tests =====

TEST(AdvisoryDownmixModeEquivalenceTest,
     EarFallback512ToStereoAcceptsAdvisoryDownmixUpdatesAndMatchesLiboar) {
  roar_config_t config = {};
  config.target_layout = ck_roar_layout_stereo;
  config.samples_per_channel = kSamplesPerChannel;
  config.sampling_rate = kSamplingRate;

  roar_audio_element_config_t element_config = {};
  element_config.type = ck_roar_channel_based;
  element_config.cbc.layout = ck_roar_layout_512;
  element_config.parameters.flags |= ROAR_PARAMETER_SET_FLAG_IAMF_DOWNMIX_INFO;
  element_config.parameters.downmix_info.mode = 0;
  element_config.parameters.downmix_info.weight_index = 5;

  std::vector<std::optional<roar_metadata_t>> metadata_seq = {
      std::nullopt,
      CreateDownmixModeMetadata(1, kSamplesPerChannel),
      CreateDownmixModeMetadata(2, kSamplesPerChannel),
      CreateDownmixModeMetadata(3, kSamplesPerChannel),
      CreateDownmixModeMetadata(99, kSamplesPerChannel),
  };

  RunDownmixTest(config, element_config, metadata_seq);
}

TEST(AdvisoryDownmixModeEquivalenceTest,
     Matching512To512LayoutAcceptsAdvisoryDownmixUpdatesMatchingOar) {
  roar_config_t config = {};
  config.target_layout = ck_roar_layout_512;
  config.samples_per_channel = kSamplesPerChannel;
  config.sampling_rate = kSamplingRate;

  roar_audio_element_config_t element_config = {};
  element_config.type = ck_roar_channel_based;
  element_config.cbc.layout = ck_roar_layout_512;
  element_config.parameters.flags |= ROAR_PARAMETER_SET_FLAG_IAMF_DOWNMIX_INFO;
  element_config.parameters.downmix_info.mode = 0;
  element_config.parameters.downmix_info.weight_index = 5;

  std::vector<std::optional<roar_metadata_t>> metadata_seq = {
      std::nullopt,
      CreateDownmixModeMetadata(1, kSamplesPerChannel),
      CreateDownmixModeMetadata(3, kSamplesPerChannel),
  };

  RunDownmixTest(config, element_config, metadata_seq);
}

TEST(AdvisoryDownmixModeEquivalenceTest,
     ChannelElementWithoutDownmixInfoAcceptsAdvisoryDownmixUpdatesMatchingOar) {
  roar_config_t config = {};
  config.target_layout = ck_roar_layout_stereo;
  config.samples_per_channel = kSamplesPerChannel;
  config.sampling_rate = kSamplingRate;

  roar_audio_element_config_t element_config = {};
  element_config.type = ck_roar_channel_based;
  element_config.cbc.layout = ck_roar_layout_51;

  std::vector<std::optional<roar_metadata_t>> metadata_seq = {
      std::nullopt,
      CreateDownmixModeMetadata(2, kSamplesPerChannel),
      CreateDownmixModeMetadata(99, kSamplesPerChannel),
  };

  RunDownmixTest(config, element_config, metadata_seq);
}

TEST(AdvisoryDownmixModeEquivalenceTest,
     BinauralWorldLocked51AcceptsAdvisoryDownmixUpdatesAndMatchesLiboar) {
  roar_config_t config = {};
  config.target_layout = ck_roar_layout_binaural;
  config.samples_per_channel = kSamplesPerChannel;
  config.sampling_rate = kSamplingRate;

  roar_audio_element_config_t element_config = {};
  element_config.type = ck_roar_channel_based;
  element_config.cbc.layout = ck_roar_layout_51;
  element_config.parameters.flags =
      ROAR_PARAMETER_SET_FLAG_IAMF_DOWNMIX_INFO |
      ROAR_PARAMETER_SET_FLAG_IAMF_ELEMENT_RENDERING_CONFIG;
  element_config.parameters.downmix_info.mode = 0;
  element_config.parameters.downmix_info.weight_index = 5;
  element_config.parameters.element_rendering_config.headphones_rendering_mode =
      ck_roar_world_locked;
  element_config.parameters.element_rendering_config.binaural_filter_profile =
      ck_roar_ambient;

  std::vector<std::optional<roar_metadata_t>> metadata_seq = {
      std::nullopt,
      CreateDownmixModeMetadata(2, kSamplesPerChannel),
      CreateDownmixModeMetadata(3, kSamplesPerChannel),
  };

  RunDownmixTest(config, element_config, metadata_seq);
}

TEST(AdvisoryDownmixModeEquivalenceTest,
     BinauralWorldLockedRestricted51AppliesDownmixUpdatesAndMatchesLiboar) {
  roar_config_t config = {};
  config.target_layout = ck_roar_layout_binaural;
  config.samples_per_channel = kSamplesPerChannel;
  config.sampling_rate = kSamplingRate;

  roar_audio_element_config_t element_config = {};
  element_config.type = ck_roar_channel_based;
  element_config.cbc.layout = ck_roar_layout_51;
  element_config.parameters.flags =
      ROAR_PARAMETER_SET_FLAG_IAMF_DOWNMIX_INFO |
      ROAR_PARAMETER_SET_FLAG_IAMF_ELEMENT_RENDERING_CONFIG;
  element_config.parameters.downmix_info.mode = 0;
  element_config.parameters.downmix_info.weight_index = 5;
  element_config.parameters.element_rendering_config.headphones_rendering_mode =
      ck_roar_world_locked_restricted;
  element_config.parameters.element_rendering_config.binaural_filter_profile =
      ck_roar_ambient;

  std::vector<std::optional<roar_metadata_t>> metadata_seq = {
      std::nullopt,
      CreateDownmixModeMetadata(1, kSamplesPerChannel),
      CreateDownmixModeMetadata(2, kSamplesPerChannel),
      CreateDownmixModeMetadata(4, kSamplesPerChannel),
  };

  RunDownmixTest(config, element_config, metadata_seq);
}

}  // namespace
}  // namespace roar_equivalence_testing
