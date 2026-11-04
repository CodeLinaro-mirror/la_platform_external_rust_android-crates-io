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

#ifndef EQUIVALENCE_TESTING_TEST_HELPERS_H_
#define EQUIVALENCE_TESTING_TEST_HELPERS_H_

#include <cstddef>
#include <cstdint>
#include <string>
#include <vector>

#include "include/roar.h"

extern "C" {
// C reference declarations (`oar_*`) exported by `liboar`, declared using
// ROAR's ABI-compatible C types so equivalence tests and benchmarks can pass
// `roar_*` structs directly to both libraries without including `liboar`
// headers. Exact C ABI layout parity between `roar.h` and `liboar` is verified
// at compile time in `abi_equivalence_test.cc`.
roar_t* oar_create(const roar_config_t* config);
void oar_destroy(roar_t* oar);
int oar_add_audio_group(roar_t* oar);
int oar_add_audio_element(roar_t* oar, uint32_t gid, uint32_t id,
                          const roar_audio_element_config_t* config);
int oar_remove_audio_element(roar_t* oar, uint32_t id);
int oar_update_audio_element_metadata(roar_t* oar, uint32_t id,
                                      const roar_metadata_t* metadata);
int oar_update_audio_element_data(roar_t* oar, uint32_t id,
                                  roar_audio_block_t* data);
int oar_set_metadata_unit_to_process(roar_t* oar, roar_metadata_type_t type,
                                     uint32_t samples);
int oar_update_metadata(roar_t* oar, uint32_t gid,
                        const roar_metadata_t* metadata);
int oar_render(roar_t* oar, roar_audio_block_t* output);
int oar_flush(roar_t* oar, roar_audio_block_t* output);
int oar_enable_loudness_processor(roar_t* oar, int enable);
int oar_set_loudness(roar_t* oar, uint32_t gid, float loudness,
                     float target_loudness);
int oar_enable_limiter(roar_t* oar, int enable);
int oar_set_limiter_threshold(roar_t* oar, float threshold_db);
int oar_get_limiter_delay(const roar_t* oar);
int oar_enable_head_tracking(roar_t* oar, int enable);
uint32_t oar_get_samples_per_channel(const roar_t* oar);
uint32_t oar_get_sampling_rate(const roar_t* oar);
uint32_t oar_get_number_of_audio_element_channels(const roar_t* oar,
                                                  uint32_t id);
uint32_t oar_get_number_of_output_channels(const roar_t* oar);
uint32_t oar_get_number_of_audio_elements(const roar_t* oar);
}  // extern "C"

namespace roar_equivalence_testing {

// Returns all valid ambisonic orders.
std::vector<roar_hoa_t> GetAllAmbisonicOrders();

// Returns all valid channel-based input layouts.
std::vector<roar_layout_t> GetAllChannelBasedInputs();

// Returns all valid output layouts (including binaural).
std::vector<roar_layout_t> GetAllOutputLayouts();

// Returns all valid output layouts, wrapped into the config.
std::vector<roar_config_t> GetAllOutputConfigs(
    uint32_t samples_per_channel = 1024, uint32_t sampling_rate = 48000);

// Gets a vector of input data with sine waves on each channel.
std::vector<float> GetInputData(size_t samples_per_channel, size_t num_channels,
                                int sample_rate, float base_frequency = 200.0f);

// Gets a buffer to receive output from the rendering.
std::vector<float> GetOutputBuffer(size_t samples_per_channel,
                                   size_t num_channels);

// Creates an IAMF downmix mode metadata block.
roar_metadata_t CreateDownmixModeMetadata(int mode, uint32_t duration);

// Creates a metadata instance to configure polar object-based element.
roar_metadata_t CreateObjectMetadata(const std::vector<roar_polar_t>& positions,
                                     uint32_t duration);

// Creates a metadata instance to configure cartesian object-based element.
roar_metadata_t CreateObjectMetadata(
    const std::vector<roar_cartesian_t>& positions, uint32_t duration);

// Creates a metadata instance to configure animated polar object(s).
roar_metadata_t CreateAnimatedObjectMetadata(
    const std::vector<roar_animated_polar_t>& positions, uint32_t duration);

// Creates a metadata instance to configure animated cartesian object(s).
roar_metadata_t CreateAnimatedObjectMetadata(
    const std::vector<roar_animated_cartesian_t>& positions, uint32_t duration);

// Creates a metadata instance to configure constant gain.
roar_metadata_t CreateConstantGainMetadata(uint32_t id, float gain_db,
                                           uint32_t duration);

// Creates a metadata instance to configure multiple gains.
roar_metadata_t CreateMultipleGainMetadata(uint32_t id,
                                           const std::vector<float>& gains_db,
                                           uint32_t duration);

// Creates a metadata instance to configure animated gain.
roar_metadata_t CreateAnimatedGainMetadata(
    uint32_t id, roar_animation_type_t anim_type, float start, float end,
    float control, float control_relative_time, uint32_t duration);

// Creates a debug string for the audio element config.
std::string Describe(const roar_audio_element_config_t& element_config);

}  // namespace roar_equivalence_testing
#endif  // EQUIVALENCE_TESTING_TEST_HELPERS_H_
