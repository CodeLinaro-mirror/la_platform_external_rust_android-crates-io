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
#include <cstdint>
#include <memory>
#include <vector>

#include "equivalence_testing/test_helpers.h"
#include "gmock/gmock.h"
#include "gtest/gtest.h"
#include "include/roar.h"

namespace roar_equivalence_testing {
namespace {

using ::testing::Ge;
using ::testing::NotNull;

const uint32_t samples_per_channel = 256;
const uint32_t sampling_rate = 48000;

using OarPtr = std::unique_ptr<roar_t, decltype(&oar_destroy)>;
using RoarPtr = std::unique_ptr<roar_t, decltype(&roar_destroy)>;

TEST(ErrorConditionsTest, UpdateHeadPositionWithoutHeadTrackingEnabled) {
  roar_config_t config = {};
  config.target_layout = ck_roar_layout_stereo;
  config.samples_per_channel = samples_per_channel;
  config.sampling_rate = sampling_rate;

  OarPtr oar(oar_create(&config), oar_destroy);
  ASSERT_THAT(oar, NotNull());
  RoarPtr roar(roar_create(&config), roar_destroy);
  ASSERT_THAT(roar, NotNull());

  // Create head rotation metadata
  roar_metadata_t metadata = {};
  metadata.type = ck_roar_metadata_head_rotation;
  metadata.duration = 256;
  metadata.head_rotation.w = 1.0f;
  metadata.head_rotation.x = 0.0f;
  metadata.head_rotation.y = 0.0f;
  metadata.head_rotation.z = 0.0f;

  auto c_group_id = oar_add_audio_group(oar.get());
  ASSERT_THAT(c_group_id, Ge(0));
  auto r_group_id = roar_add_audio_group(roar.get());
  ASSERT_THAT(r_group_id, Ge(0));

  int c_res = oar_update_metadata(oar.get(), c_group_id, &metadata);
  int r_res = roar_update_metadata(roar.get(), r_group_id, &metadata);

  EXPECT_EQ(c_res, r_res);
  EXPECT_EQ(r_res, ck_roar_error_busy);
}

TEST(ErrorConditionsTest, EnableHeadTrackingOnNonBinauralLayout) {
  roar_config_t config = {};
  config.target_layout = ck_roar_layout_stereo;
  config.samples_per_channel = samples_per_channel;
  config.sampling_rate = sampling_rate;

  OarPtr oar(oar_create(&config), oar_destroy);
  ASSERT_THAT(oar, NotNull());
  RoarPtr roar(roar_create(&config), roar_destroy);
  ASSERT_THAT(roar, NotNull());

  int c_res = oar_enable_head_tracking(oar.get(), 1);
  int r_res = roar_enable_head_tracking(roar.get(), 1);

  EXPECT_EQ(c_res, r_res);
  EXPECT_EQ(r_res, ck_roar_error_notsup);
}

TEST(ErrorConditionsTest, AddElementWithInvalidGroupId) {
  roar_config_t config = {};
  config.target_layout = ck_roar_layout_stereo;
  config.samples_per_channel = samples_per_channel;
  config.sampling_rate = sampling_rate;

  OarPtr oar(oar_create(&config), oar_destroy);
  ASSERT_THAT(oar, NotNull());
  RoarPtr roar(roar_create(&config), roar_destroy);
  ASSERT_THAT(roar, NotNull());

  roar_audio_element_config_t element_config = {};
  element_config.type = ck_roar_channel_based;
  element_config.cbc.layout = ck_roar_layout_stereo;

  // Group 0 does not exist because no group has been created.
  uint32_t group_id = 0;
  uint32_t element_id = 10;
  int c_res =
      oar_add_audio_element(oar.get(), group_id, element_id, &element_config);
  int r_res =
      roar_add_audio_element(roar.get(), group_id, element_id, &element_config);

  EXPECT_EQ(c_res, r_res);
  EXPECT_EQ(r_res, ck_roar_error_inval);
}

TEST(ErrorConditionsTest, RemoveNonExistentElement) {
  roar_config_t config = {};
  config.target_layout = ck_roar_layout_stereo;
  config.samples_per_channel = samples_per_channel;
  config.sampling_rate = sampling_rate;

  OarPtr oar(oar_create(&config), oar_destroy);
  ASSERT_THAT(oar, NotNull());
  RoarPtr roar(roar_create(&config), roar_destroy);
  ASSERT_THAT(roar, NotNull());

  // Do successfully add a group.
  int c_gid = oar_add_audio_group(oar.get());
  ASSERT_THAT(c_gid, Ge(0));
  int r_gid = roar_add_audio_group(roar.get());
  ASSERT_THAT(r_gid, Ge(0));

  // Element ID does not exist because no element has been created.
  uint32_t element_id = 10;
  int c_res = oar_remove_audio_element(oar.get(), element_id);
  int r_res = roar_remove_audio_element(roar.get(), element_id);

  EXPECT_EQ(c_res, r_res);
  EXPECT_EQ(r_res, ck_roar_error_inval);
}

TEST(ErrorConditionsTest, AddDuplicateElementId) {
  roar_config_t config = {};
  config.target_layout = ck_roar_layout_stereo;
  config.samples_per_channel = samples_per_channel;
  config.sampling_rate = sampling_rate;

  OarPtr oar(oar_create(&config), oar_destroy);
  ASSERT_THAT(oar, NotNull());
  RoarPtr roar(roar_create(&config), roar_destroy);
  ASSERT_THAT(roar, NotNull());

  int c_gid = oar_add_audio_group(oar.get());
  int r_gid = roar_add_audio_group(roar.get());

  roar_audio_element_config_t element_config = {};
  element_config.type = ck_roar_channel_based;
  element_config.cbc.layout = ck_roar_layout_stereo;

  // Add first element successfully
  uint32_t element_id = 10;
  ASSERT_EQ(
      oar_add_audio_element(oar.get(), c_gid, element_id, &element_config), 0);
  ASSERT_EQ(
      roar_add_audio_element(roar.get(), r_gid, element_id, &element_config),
      0);

  // Add duplicate element ID
  int c_res =
      oar_add_audio_element(oar.get(), c_gid, element_id, &element_config);
  int r_res =
      roar_add_audio_element(roar.get(), r_gid, element_id, &element_config);

  EXPECT_EQ(c_res, r_res);
  EXPECT_EQ(r_res, ck_roar_error_busy);
}

TEST(ErrorConditionsTest, UpdateMetadataForNonExistentElement) {
  roar_config_t config = {};
  config.target_layout = ck_roar_layout_stereo;
  config.samples_per_channel = samples_per_channel;
  config.sampling_rate = sampling_rate;

  OarPtr oar(oar_create(&config), oar_destroy);
  ASSERT_THAT(oar, NotNull());
  RoarPtr roar(roar_create(&config), roar_destroy);
  ASSERT_THAT(roar, NotNull());

  roar_metadata_t metadata = {};
  metadata.type = ck_roar_metadata_gain;
  metadata.duration = 256;
  metadata.gain.param_type = ck_roar_param_constant;
  metadata.gain.constant_gain = 0.5f;

  uint32_t element_id = 10;
  int c_res =
      oar_update_audio_element_metadata(oar.get(), element_id, &metadata);
  int r_res =
      roar_update_audio_element_metadata(roar.get(), element_id, &metadata);

  EXPECT_EQ(c_res, r_res);
  EXPECT_EQ(r_res, ck_roar_error_inval);
}

TEST(ErrorConditionsTest, UpdateMetadataForNonExistentGroup) {
  roar_config_t config = {};
  config.target_layout = ck_roar_layout_stereo;
  config.samples_per_channel = samples_per_channel;
  config.sampling_rate = sampling_rate;

  OarPtr oar(oar_create(&config), oar_destroy);
  ASSERT_THAT(oar, NotNull());
  RoarPtr roar(roar_create(&config), roar_destroy);
  ASSERT_THAT(roar, NotNull());

  roar_metadata_t metadata = {};
  metadata.type = ck_roar_metadata_gain;
  metadata.duration = 256;
  metadata.gain.param_type = ck_roar_param_constant;
  metadata.gain.constant_gain = 0.5f;

  uint32_t group_id = 0;
  int c_res = oar_update_metadata(oar.get(), group_id, &metadata);
  int r_res = roar_update_metadata(roar.get(), group_id, &metadata);

  EXPECT_EQ(c_res, r_res);
  EXPECT_LT(c_res, 0);  // Should be an error.
}

TEST(ErrorConditionsTest, UpdateMetadataWithInvalidType) {
  roar_config_t config = {};
  config.target_layout = ck_roar_layout_stereo;
  config.samples_per_channel = samples_per_channel;
  config.sampling_rate = sampling_rate;

  OarPtr oar(oar_create(&config), oar_destroy);
  ASSERT_THAT(oar, NotNull());
  RoarPtr roar(roar_create(&config), roar_destroy);
  ASSERT_THAT(roar, NotNull());

  int c_gid = oar_add_audio_group(oar.get());
  int r_gid = roar_add_audio_group(roar.get());

  roar_audio_element_config_t element_config = {};
  element_config.type = ck_roar_channel_based;
  element_config.cbc.layout = ck_roar_layout_stereo;

  uint32_t element_id = 10;
  ASSERT_EQ(
      oar_add_audio_element(oar.get(), c_gid, element_id, &element_config), 0);
  ASSERT_EQ(
      roar_add_audio_element(roar.get(), r_gid, element_id, &element_config),
      0);

  // Invalid metadata type (e.g. 999)
  roar_metadata_t metadata = {};
  metadata.type = static_cast<roar_metadata_type_t>(999);
  metadata.duration = 256;

  int c_res =
      oar_update_audio_element_metadata(oar.get(), element_id, &metadata);
  int r_res =
      roar_update_audio_element_metadata(roar.get(), element_id, &metadata);

  EXPECT_EQ(c_res, r_res);
  EXPECT_EQ(r_res, ck_roar_error_notsup);
}

TEST(ErrorConditionsTest,
     RenderWithoutUpdatingAudioElementDataReturnsInvalidParameter) {
  roar_config_t config = {};
  config.target_layout = ck_roar_layout_stereo;
  config.samples_per_channel = samples_per_channel;
  config.sampling_rate = sampling_rate;

  OarPtr oar(oar_create(&config), oar_destroy);
  ASSERT_THAT(oar, NotNull());
  RoarPtr roar(roar_create(&config), roar_destroy);
  ASSERT_THAT(roar, NotNull());

  int c_gid = oar_add_audio_group(oar.get());
  ASSERT_THAT(c_gid, Ge(0));
  int r_gid = roar_add_audio_group(roar.get());
  ASSERT_THAT(r_gid, Ge(0));

  roar_audio_element_config_t element_config = {};
  element_config.type = ck_roar_channel_based;
  element_config.cbc.layout = ck_roar_layout_stereo;

  uint32_t element_id = 10;
  ASSERT_EQ(
      oar_add_audio_element(oar.get(), c_gid, element_id, &element_config), 0);
  ASSERT_EQ(
      roar_add_audio_element(roar.get(), r_gid, element_id, &element_config),
      0);

  auto input_data = GetInputData(samples_per_channel, 2, sampling_rate);
  roar_audio_block_t input_block = {};
  input_block.channels = 2;
  input_block.samples_per_channel = samples_per_channel;
  input_block.data = input_data.data();

  ASSERT_EQ(oar_update_audio_element_data(oar.get(), element_id, &input_block),
            0);
  ASSERT_EQ(
      roar_update_audio_element_data(roar.get(), element_id, &input_block), 0);

  auto c_output = GetOutputBuffer(samples_per_channel, 2);
  roar_audio_block_t c_output_block = {};
  c_output_block.channels = 2;
  c_output_block.samples_per_channel = samples_per_channel;
  c_output_block.data = c_output.data();

  auto r_output = GetOutputBuffer(samples_per_channel, 2);
  roar_audio_block_t r_output_block = {};
  r_output_block.channels = 2;
  r_output_block.samples_per_channel = samples_per_channel;
  r_output_block.data = r_output.data();

  ASSERT_EQ(oar_render(oar.get(), &c_output_block), 0);
  ASSERT_EQ(roar_render(roar.get(), &r_output_block), 0);

  int c_second_res = oar_render(oar.get(), &c_output_block);
  int r_second_res = roar_render(roar.get(), &r_output_block);

  EXPECT_EQ(c_second_res, r_second_res);
  EXPECT_EQ(r_second_res, ck_roar_error_inval);
}

TEST(ErrorConditionsTest,
     InvalidDownmixModeWithActiveConsumerReturnsInvalidParameter) {
  roar_config_t config = {};
  config.target_layout = ck_roar_layout_stereo;
  config.samples_per_channel = samples_per_channel;
  config.sampling_rate = sampling_rate;

  OarPtr oar(oar_create(&config), oar_destroy);
  ASSERT_THAT(oar, NotNull());
  RoarPtr roar(roar_create(&config), roar_destroy);
  ASSERT_THAT(roar, NotNull());

  int c_gid = oar_add_audio_group(oar.get());
  ASSERT_THAT(c_gid, Ge(0));
  int r_gid = roar_add_audio_group(roar.get());
  ASSERT_THAT(r_gid, Ge(0));

  roar_audio_element_config_t element_config = {};
  element_config.type = ck_roar_channel_based;
  element_config.cbc.layout = ck_roar_layout_51;
  element_config.parameters.flags = ROAR_PARAMETER_SET_FLAG_IAMF_DOWNMIX_INFO;
  element_config.parameters.downmix_info.mode = 0;
  element_config.parameters.downmix_info.weight_index = -1;

  uint32_t element_id = 1;
  ASSERT_EQ(
      oar_add_audio_element(oar.get(), c_gid, element_id, &element_config), 0);
  ASSERT_EQ(
      roar_add_audio_element(roar.get(), r_gid, element_id, &element_config),
      0);

  roar_metadata_t neg_meta = CreateDownmixModeMetadata(-1, samples_per_channel);
  roar_metadata_t mode3_meta =
      CreateDownmixModeMetadata(3, samples_per_channel);
  roar_metadata_t mode7_meta =
      CreateDownmixModeMetadata(7, samples_per_channel);
  roar_metadata_t mode99_meta =
      CreateDownmixModeMetadata(99, samples_per_channel);
  roar_metadata_t valid_meta =
      CreateDownmixModeMetadata(1, samples_per_channel);

  int c_neg =
      oar_update_audio_element_metadata(oar.get(), element_id, &neg_meta);
  int r_neg =
      roar_update_audio_element_metadata(roar.get(), element_id, &neg_meta);
  int c_mode3 =
      oar_update_audio_element_metadata(oar.get(), element_id, &mode3_meta);
  int r_mode3 =
      roar_update_audio_element_metadata(roar.get(), element_id, &mode3_meta);
  int c_mode7 =
      oar_update_audio_element_metadata(oar.get(), element_id, &mode7_meta);
  int r_mode7 =
      roar_update_audio_element_metadata(roar.get(), element_id, &mode7_meta);
  int c_mode99 =
      oar_update_audio_element_metadata(oar.get(), element_id, &mode99_meta);
  int r_mode99 =
      roar_update_audio_element_metadata(roar.get(), element_id, &mode99_meta);
  int c_valid =
      oar_update_audio_element_metadata(oar.get(), element_id, &valid_meta);
  int r_valid =
      roar_update_audio_element_metadata(roar.get(), element_id, &valid_meta);

  EXPECT_EQ(c_neg, -22);
  EXPECT_EQ(r_neg, -22);
  EXPECT_EQ(c_mode3, -22);
  EXPECT_EQ(r_mode3, -22);
  EXPECT_EQ(c_mode7, -22);
  EXPECT_EQ(r_mode7, -22);
  EXPECT_EQ(c_mode99, -22);
  EXPECT_EQ(r_mode99, -22);
  EXPECT_EQ(c_valid, 0);
  EXPECT_EQ(r_valid, 0);
}

TEST(ErrorConditionsTest, InvalidDownmixModeWithoutConsumerReturnsOk) {
  roar_config_t config = {};
  config.target_layout = ck_roar_layout_stereo;
  config.samples_per_channel = samples_per_channel;
  config.sampling_rate = sampling_rate;

  OarPtr oar(oar_create(&config), oar_destroy);
  ASSERT_THAT(oar, NotNull());
  RoarPtr roar(roar_create(&config), roar_destroy);
  ASSERT_THAT(roar, NotNull());

  int c_gid = oar_add_audio_group(oar.get());
  ASSERT_THAT(c_gid, Ge(0));
  int r_gid = roar_add_audio_group(roar.get());
  ASSERT_THAT(r_gid, Ge(0));

  roar_audio_element_config_t element_config = {};
  element_config.type = ck_roar_channel_based;
  element_config.cbc.layout = ck_roar_layout_512;
  element_config.parameters.flags = ROAR_PARAMETER_SET_FLAG_IAMF_DOWNMIX_INFO;
  element_config.parameters.downmix_info.mode = 0;
  element_config.parameters.downmix_info.weight_index = -1;

  uint32_t element_id = 1;
  ASSERT_EQ(
      oar_add_audio_element(oar.get(), c_gid, element_id, &element_config), 0);
  ASSERT_EQ(
      roar_add_audio_element(roar.get(), r_gid, element_id, &element_config),
      0);

  roar_metadata_t neg_meta = CreateDownmixModeMetadata(-1, samples_per_channel);
  roar_metadata_t mode3_meta =
      CreateDownmixModeMetadata(3, samples_per_channel);
  roar_metadata_t mode7_meta =
      CreateDownmixModeMetadata(7, samples_per_channel);
  roar_metadata_t mode99_meta =
      CreateDownmixModeMetadata(99, samples_per_channel);

  int c_neg =
      oar_update_audio_element_metadata(oar.get(), element_id, &neg_meta);
  int r_neg =
      roar_update_audio_element_metadata(roar.get(), element_id, &neg_meta);
  int c_mode3 =
      oar_update_audio_element_metadata(oar.get(), element_id, &mode3_meta);
  int r_mode3 =
      roar_update_audio_element_metadata(roar.get(), element_id, &mode3_meta);
  int c_mode7 =
      oar_update_audio_element_metadata(oar.get(), element_id, &mode7_meta);
  int r_mode7 =
      roar_update_audio_element_metadata(roar.get(), element_id, &mode7_meta);
  int c_mode99 =
      oar_update_audio_element_metadata(oar.get(), element_id, &mode99_meta);
  int r_mode99 =
      roar_update_audio_element_metadata(roar.get(), element_id, &mode99_meta);

  EXPECT_EQ(c_neg, 0);
  EXPECT_EQ(r_neg, 0);
  EXPECT_EQ(c_mode3, 0);
  EXPECT_EQ(r_mode3, 0);
  EXPECT_EQ(c_mode7, 0);
  EXPECT_EQ(r_mode7, 0);
  EXPECT_EQ(c_mode99, 0);
  EXPECT_EQ(r_mode99, 0);
}

TEST(ErrorConditionsTest, InvalidDownmixModeOnMatching512LayoutReturnsOk) {
  roar_config_t config = {};
  config.target_layout = ck_roar_layout_512;
  config.samples_per_channel = samples_per_channel;
  config.sampling_rate = sampling_rate;

  OarPtr oar(oar_create(&config), oar_destroy);
  ASSERT_THAT(oar, NotNull());
  RoarPtr roar(roar_create(&config), roar_destroy);
  ASSERT_THAT(roar, NotNull());

  int c_gid = oar_add_audio_group(oar.get());
  ASSERT_THAT(c_gid, Ge(0));
  int r_gid = roar_add_audio_group(roar.get());
  ASSERT_THAT(r_gid, Ge(0));

  roar_audio_element_config_t element_config = {};
  element_config.type = ck_roar_channel_based;
  element_config.cbc.layout = ck_roar_layout_512;
  element_config.parameters.flags = ROAR_PARAMETER_SET_FLAG_IAMF_DOWNMIX_INFO;
  element_config.parameters.downmix_info.mode = 0;
  element_config.parameters.downmix_info.weight_index = -1;

  uint32_t element_id = 1;
  ASSERT_EQ(
      oar_add_audio_element(oar.get(), c_gid, element_id, &element_config), 0);
  ASSERT_EQ(
      roar_add_audio_element(roar.get(), r_gid, element_id, &element_config),
      0);

  roar_metadata_t mode3_meta =
      CreateDownmixModeMetadata(3, samples_per_channel);

  int c_res =
      oar_update_audio_element_metadata(oar.get(), element_id, &mode3_meta);
  int r_res =
      roar_update_audio_element_metadata(roar.get(), element_id, &mode3_meta);

  EXPECT_EQ(c_res, 0);
  EXPECT_EQ(r_res, 0);
}

TEST(ErrorConditionsTest,
     DownmixModeUpdateForNonExistentElementReturnsInvalidParameter) {
  roar_config_t config = {};
  config.target_layout = ck_roar_layout_stereo;
  config.samples_per_channel = samples_per_channel;
  config.sampling_rate = sampling_rate;

  OarPtr oar(oar_create(&config), oar_destroy);
  ASSERT_THAT(oar, NotNull());
  RoarPtr roar(roar_create(&config), roar_destroy);
  ASSERT_THAT(roar, NotNull());

  int c_gid = oar_add_audio_group(oar.get());
  ASSERT_THAT(c_gid, Ge(0));
  int r_gid = roar_add_audio_group(roar.get());
  ASSERT_THAT(r_gid, Ge(0));

  roar_audio_element_config_t element_config = {};
  element_config.type = ck_roar_channel_based;
  element_config.cbc.layout = ck_roar_layout_512;
  element_config.parameters.flags = ROAR_PARAMETER_SET_FLAG_IAMF_DOWNMIX_INFO;
  element_config.parameters.downmix_info.mode = 0;
  element_config.parameters.downmix_info.weight_index = -1;

  ASSERT_EQ(oar_add_audio_element(oar.get(), c_gid, 1, &element_config), 0);
  ASSERT_EQ(roar_add_audio_element(roar.get(), r_gid, 1, &element_config), 0);

  roar_metadata_t metadata = CreateDownmixModeMetadata(1, samples_per_channel);

  int c_res = oar_update_audio_element_metadata(oar.get(), 999, &metadata);
  int r_res = roar_update_audio_element_metadata(roar.get(), 999, &metadata);

  EXPECT_EQ(c_res, -22);
  EXPECT_EQ(r_res, -22);
}

TEST(ErrorConditionsTest,
     ObjectPositionsUpdateOnChannelElementReturnsNotSupported) {
  roar_config_t config = {};
  config.target_layout = ck_roar_layout_stereo;
  config.samples_per_channel = samples_per_channel;
  config.sampling_rate = sampling_rate;

  OarPtr oar(oar_create(&config), oar_destroy);
  ASSERT_THAT(oar, NotNull());
  RoarPtr roar(roar_create(&config), roar_destroy);
  ASSERT_THAT(roar, NotNull());

  int c_gid = oar_add_audio_group(oar.get());
  ASSERT_THAT(c_gid, Ge(0));
  int r_gid = roar_add_audio_group(roar.get());
  ASSERT_THAT(r_gid, Ge(0));

  roar_audio_element_config_t element_config = {};
  element_config.type = ck_roar_channel_based;
  element_config.cbc.layout = ck_roar_layout_512;
  element_config.parameters.flags = ROAR_PARAMETER_SET_FLAG_IAMF_DOWNMIX_INFO;
  element_config.parameters.downmix_info.mode = 0;
  element_config.parameters.downmix_info.weight_index = -1;

  uint32_t element_id = 1;
  ASSERT_EQ(
      oar_add_audio_element(oar.get(), c_gid, element_id, &element_config), 0);
  ASSERT_EQ(
      roar_add_audio_element(roar.get(), r_gid, element_id, &element_config),
      0);

  roar_polar_t pos = {30.0f, 0.0f, 1.0f};
  roar_metadata_t pos_meta = CreateObjectMetadata({pos}, samples_per_channel);

  int c_res =
      oar_update_audio_element_metadata(oar.get(), element_id, &pos_meta);
  int r_res =
      roar_update_audio_element_metadata(roar.get(), element_id, &pos_meta);

  EXPECT_EQ(c_res, -95);
  EXPECT_EQ(r_res, -95);
}

TEST(ErrorConditionsTest, ReservedElementMetadataTypeCountReturnsNotSupported) {
  roar_config_t config = {};
  config.target_layout = ck_roar_layout_stereo;
  config.samples_per_channel = samples_per_channel;
  config.sampling_rate = sampling_rate;

  OarPtr oar(oar_create(&config), oar_destroy);
  ASSERT_THAT(oar, NotNull());
  RoarPtr roar(roar_create(&config), roar_destroy);
  ASSERT_THAT(roar, NotNull());

  int c_gid = oar_add_audio_group(oar.get());
  ASSERT_THAT(c_gid, Ge(0));
  int r_gid = roar_add_audio_group(roar.get());
  ASSERT_THAT(r_gid, Ge(0));

  roar_audio_element_config_t element_config = {};
  element_config.type = ck_roar_channel_based;
  element_config.cbc.layout = ck_roar_layout_512;

  uint32_t element_id = 1;
  ASSERT_EQ(
      oar_add_audio_element(oar.get(), c_gid, element_id, &element_config), 0);
  ASSERT_EQ(
      roar_add_audio_element(roar.get(), r_gid, element_id, &element_config),
      0);

  roar_metadata_t metadata = {};
  metadata.type = ck_roar_metadata_count;
  metadata.duration = samples_per_channel;

  int c_res =
      oar_update_audio_element_metadata(oar.get(), element_id, &metadata);
  int r_res =
      roar_update_audio_element_metadata(roar.get(), element_id, &metadata);

  EXPECT_EQ(c_res, -95);
  EXPECT_EQ(r_res, -95);
}

TEST(ErrorConditionsTest,
     SetMetadataUnitToProcessWithOutOfRangeTypeReturnsNotSupported) {
  roar_config_t config = {};
  config.target_layout = ck_roar_layout_stereo;
  config.samples_per_channel = samples_per_channel;
  config.sampling_rate = sampling_rate;
  OarPtr oar(oar_create(&config), oar_destroy);
  ASSERT_THAT(oar, NotNull());
  RoarPtr roar(roar_create(&config), roar_destroy);
  ASSERT_THAT(roar, NotNull());
  const auto out_of_range_type = static_cast<roar_metadata_type_t>(999);

  int c_res =
      oar_set_metadata_unit_to_process(oar.get(), out_of_range_type, 64);
  int r_res =
      roar_set_metadata_unit_to_process(roar.get(), out_of_range_type, 64);

  EXPECT_EQ(c_res, -95);
  EXPECT_EQ(r_res, -95);
}

}  // namespace
}  // namespace roar_equivalence_testing
