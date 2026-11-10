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

#ifndef EQUIVALENCE_TESTING_BENCHMARKS_BENCHMARK_HELPERS_H_
#define EQUIVALENCE_TESTING_BENCHMARKS_BENCHMARK_HELPERS_H_

#include <cstddef>
#include <cstdint>
#include <memory>
#include <vector>

#include "absl/types/span.h"
#include "include/roar.h"

namespace roar_equivalence_testing {

// Default audio stream parameters across benchmarks.
inline constexpr uint32_t kDefaultFrameSize = 1024;
inline constexpr uint32_t kDefaultSampleRate = 48000;

// Identifies the underlying renderer.
enum class RendererEngine {
  kOar,   // C/C++ reference implementation (liboar)
  kRoar,  // Rust implementation (ROAR) via C FFI
};

// Wraps the renderer to manage the lifetime and delegate calls.
class RendererInstance {
 public:
  // Constructs the renderer of the given type with the given config.
  static std::unique_ptr<RendererInstance> Create(RendererEngine engine,
                                                  const roar_config_t& config);

  ~RendererInstance();

  RendererInstance(const RendererInstance&) = delete;
  RendererInstance& operator=(const RendererInstance&) = delete;

  // Adds an audio group to the renderer and returns its assigned group ID.
  int AddAudioGroup();

  // Adds an audio element to the specified audio group.
  int AddAudioElement(uint32_t gid, uint32_t id,
                      const roar_audio_element_config_t& config);

  // Updates metadata (e.g. object positions) for an audio element.
  int UpdateAudioElementMetadata(uint32_t id, const roar_metadata_t& metadata);

  // Updates group-level metadata (such as head rotation).
  int UpdateMetadata(uint32_t gid, const roar_metadata_t& metadata);

  // Enables or disables head tracking in the renderer.
  int EnableHeadTracking(bool enable);

  // Sets planar audio sample data for an audio element.
  int UpdateAudioElementData(uint32_t id, roar_audio_block_t* data);

  // Executes one frame of audio rendering into the supplied output block.
  int Render(roar_audio_block_t* output);

 private:
  RendererInstance(RendererEngine engine, roar_t* handle);

  RendererEngine engine_;
  roar_t* handle_;
};

// Bundles an allocated float vector with roar_audio_block_t referencing it.
struct AudioBlock {
  std::vector<float> data;
  roar_audio_block_t block;

  // Allocates synthetic test audio data for input channels.
  static AudioBlock CreateInput(
      uint32_t channels, uint32_t samples_per_channel = kDefaultFrameSize,
      uint32_t sample_rate = kDefaultSampleRate);

  // Allocates a zero-initialized buffer to receive output.
  static AudioBlock CreateOutput(
      uint32_t channels, uint32_t samples_per_channel = kDefaultFrameSize);
};

// Creates an element configuration for channel-based audio.
roar_audio_element_config_t CreateChannelConfig(roar_layout_t layout);

// Creates an element configuration for Higher-Order Ambisonics (HOA) audio.
roar_audio_element_config_t CreateAmbisonicConfig(roar_hoa_t order);

// Creates an element configuration for object-based audio.
roar_audio_element_config_t CreateObjectConfig(uint32_t num_objects = 2);

// Adds parameter indicating the element should be world-locked for binaural
// rendering.
void AddWorldLockedParameter(roar_audio_element_config_t& config);

// Generates polar position metadata for `num_pairs` (2 objects per pair)
// distributed across azimuths.
std::vector<roar_metadata_t> CreateObjectPairMetadatas(
    size_t num_pairs, uint32_t frame_size = kDefaultFrameSize);

// Generates a sequence of head rotation metadata quaternions.
std::vector<roar_metadata_t> CreateHeadRotationMetadatas(
    uint32_t frame_size = kDefaultFrameSize);

// Registers `num_pairs` object audio elements into `renderer`, associates their
// initial metadata and input blocks, and returns the registered element IDs.
std::vector<uint32_t> SetupObjectElements(
    RendererInstance& renderer, int group_id, size_t num_pairs,
    const roar_audio_element_config_t& element_cfg,
    roar_audio_block_t* input_block,
    absl::Span<const roar_metadata_t> metadatas);

}  // namespace roar_equivalence_testing

#endif  // EQUIVALENCE_TESTING_BENCHMARKS_BENCHMARK_HELPERS_H_
