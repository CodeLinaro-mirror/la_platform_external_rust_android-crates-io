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
#include <cstdint>

#include "absl/base/log_severity.h"
#include "absl/log/absl_check.h"
#include "absl/log/globals.h"
#include "absl/types/span.h"
#include "benchmark/benchmark.h"
#include "equivalence_testing/benchmarks/benchmark_helpers.h"
#include "equivalence_testing/test_helpers.h"
#include "include/roar.h"

namespace roar_equivalence_testing {
namespace {

constexpr uint32_t kElementId = 15;

// Cut down on INFO log spam (OAR is noisy).
const bool kLogsSuppressed = []() {
  absl::SetMinLogLevel(absl::LogSeverityAtLeast::kWarning);
  return true;
}();

// Computes and records standard audio throughput counters (`xRealtime` and
// items processed).
void SetAudioMetrics(benchmark::State& state, int64_t num_frames,
                     uint32_t frame_size = kDefaultFrameSize,
                     uint32_t sample_rate = kDefaultSampleRate) {
  const double total_audio_duration =
      (static_cast<double>(frame_size) / sample_rate) *
      static_cast<double>(num_frames) * static_cast<double>(state.iterations());
  state.counters["xRealtime"] =
      benchmark::Counter(total_audio_duration, benchmark::Counter::kIsRate);
}

// Updates planar input data for every element in `element_ids`.
void FeedAudioElements(RendererInstance& renderer,
                       absl::Span<const uint32_t> element_ids,
                       roar_audio_block_t& input_block) {
  for (uint32_t id : element_ids) {
    renderer.UpdateAudioElementData(id, &input_block);
  }
}

// Executes a benchmark render loop across `num_frames` consecutive frames,
// updating audio element data prior to each frame.
void RunRenderLoop(benchmark::State& state, RendererInstance& renderer,
                   absl::Span<const uint32_t> element_ids,
                   roar_audio_block_t& input_block,
                   roar_audio_block_t& output_block, int64_t num_frames,
                   uint32_t frame_size = kDefaultFrameSize,
                   uint32_t sample_rate = kDefaultSampleRate) {
  // Warm-up pass.
  FeedAudioElements(renderer, element_ids, input_block);
  ABSL_CHECK_EQ(renderer.Render(&output_block), 0);

  for (auto unused : state) {
    for (int64_t f = 0; f < num_frames; ++f) {
      FeedAudioElements(renderer, element_ids, input_block);
      benchmark::DoNotOptimize(renderer.Render(&output_block));
    }
  }

  SetAudioMetrics(state, num_frames, frame_size, sample_rate);
}

// Executes a benchmark render loop with audio data and metadata updates prior
// to each frame.
void RunDynamicRenderLoop(benchmark::State& state, RendererInstance& renderer,
                          roar_audio_block_t& input_block,
                          roar_audio_block_t& output_block,
                          absl::Span<const uint32_t> element_ids,
                          absl::Span<const roar_metadata_t> metadatas,
                          int64_t num_frames,
                          uint32_t frame_size = kDefaultFrameSize,
                          uint32_t sample_rate = kDefaultSampleRate) {
  ABSL_CHECK_EQ(element_ids.size(), metadatas.size());

  // Warm-up pass.
  FeedAudioElements(renderer, element_ids, input_block);
  for (size_t i = 0; i < element_ids.size(); ++i) {
    ABSL_CHECK_EQ(
        renderer.UpdateAudioElementMetadata(element_ids[i], metadatas[i]), 0);
  }
  ABSL_CHECK_EQ(renderer.Render(&output_block), 0);

  for (auto unused : state) {
    for (int64_t f = 0; f < num_frames; ++f) {
      for (size_t i = 0; i < element_ids.size(); ++i) {
        renderer.UpdateAudioElementData(element_ids[i], &input_block);
        renderer.UpdateAudioElementMetadata(element_ids[i], metadatas[i]);
      }
      benchmark::DoNotOptimize(renderer.Render(&output_block));
    }
  }

  SetAudioMetrics(state, num_frames, frame_size, sample_rate);
}

// Sets up a stereo channel-based renderer and runs per-frame element data and
// gain metadata updates to isolate gain calculation and application throughput.
void RunStereoElementGainBenchmark(benchmark::State& state,
                                   RendererEngine engine,
                                   const roar_metadata_t& gain_metadata) {
  const int64_t num_frames = state.range(0);
  constexpr uint32_t kStereoChannels = 2;

  const roar_config_t config = {
      .target_layout = ck_roar_layout_stereo,
      .samples_per_channel = kDefaultFrameSize,
      .sampling_rate = kDefaultSampleRate,
  };
  auto renderer = RendererInstance::Create(engine, config);
  ABSL_CHECK(renderer != nullptr);

  const int group_id = renderer->AddAudioGroup();
  ABSL_CHECK_GE(group_id, 0);

  const roar_audio_element_config_t element_cfg =
      CreateChannelConfig(ck_roar_layout_stereo);
  ABSL_CHECK_EQ(renderer->AddAudioElement(group_id, kElementId, element_cfg),
                0);

  AudioBlock input = AudioBlock::CreateInput(kStereoChannels);
  AudioBlock output = AudioBlock::CreateOutput(kStereoChannels);
  const uint32_t element_ids[] = {kElementId};
  const roar_metadata_t metadatas[] = {gain_metadata};
  RunDynamicRenderLoop(state, *renderer, input.block, output.block, element_ids,
                       metadatas, num_frames);
}

// Executes a benchmark render loop with audio element data and head rotation
// updates prior to each frame.
void RunHeadTrackingRenderLoop(benchmark::State& state,
                               RendererInstance& renderer, uint32_t group_id,
                               absl::Span<const uint32_t> element_ids,
                               roar_audio_block_t& input_block,
                               roar_audio_block_t& output_block,
                               absl::Span<const roar_metadata_t> head_rotations,
                               int64_t num_frames,
                               uint32_t frame_size = kDefaultFrameSize,
                               uint32_t sample_rate = kDefaultSampleRate) {
  ABSL_CHECK(!head_rotations.empty());

  // Warm-up pass.
  FeedAudioElements(renderer, element_ids, input_block);
  ABSL_CHECK_EQ(renderer.UpdateMetadata(group_id, head_rotations[0]), 0);
  ABSL_CHECK_EQ(renderer.Render(&output_block), 0);

  for (auto unused : state) {
    for (int64_t f = 0; f < num_frames; ++f) {
      FeedAudioElements(renderer, element_ids, input_block);
      const auto& metadata = head_rotations[f % head_rotations.size()];
      renderer.UpdateMetadata(group_id, metadata);
      benchmark::DoNotOptimize(renderer.Render(&output_block));
    }
  }

  SetAudioMetrics(state, num_frames, frame_size, sample_rate);
}

// -----------------------------------------------------------------------------
//   Audio Rendering Benchmarks (Setup executed outside the timed loop)
// -----------------------------------------------------------------------------

// World-locked 9.1.6 channel-based input to binaural rendering (OBR backend).
void Render9_1_6ToBinaural(benchmark::State& state, RendererEngine engine) {
  const int64_t num_frames = state.range(0);
  constexpr uint32_t kInputChannels = 16;  // 9.1.6
  constexpr uint32_t kOutputChannels = 2;  // Binaural

  const roar_config_t config = {
      .target_layout = ck_roar_layout_binaural,
      .samples_per_channel = kDefaultFrameSize,
      .sampling_rate = kDefaultSampleRate,
  };
  auto renderer = RendererInstance::Create(engine, config);
  ABSL_CHECK(renderer != nullptr);

  const int group_id = renderer->AddAudioGroup();
  ABSL_CHECK_GE(group_id, 0);

  auto element_cfg = CreateChannelConfig(ck_roar_layout_916);
  AddWorldLockedParameter(element_cfg);
  ABSL_CHECK_EQ(renderer->AddAudioElement(group_id, kElementId, element_cfg),
                0);

  auto input = AudioBlock::CreateInput(kInputChannels);
  auto output = AudioBlock::CreateOutput(kOutputChannels);
  RunRenderLoop(state, *renderer, {kElementId}, input.block, output.block,
                num_frames);
}
BENCHMARK_CAPTURE(Render9_1_6ToBinaural, oar, RendererEngine::kOar)->Arg(100);
BENCHMARK_CAPTURE(Render9_1_6ToBinaural, roar, RendererEngine::kRoar)->Arg(100);

// World-locked 9.1.6 channel-based input to binaural rendering at 44.1 kHz
// (exercising the resampled HRIR filter bank).
void Render9_1_6ToBinaural_Resampled_44100(benchmark::State& state,
                                           RendererEngine engine) {
  const int64_t num_frames = state.range(0);
  constexpr uint32_t kInputChannels = 16;  // 9.1.6
  constexpr uint32_t kOutputChannels = 2;  // Binaural
  constexpr uint32_t kSampleRate44100 = 44100;

  const roar_config_t config = {
      .target_layout = ck_roar_layout_binaural,
      .samples_per_channel = kDefaultFrameSize,
      .sampling_rate = kSampleRate44100,
  };
  auto renderer = RendererInstance::Create(engine, config);
  ABSL_CHECK(renderer != nullptr);

  const int group_id = renderer->AddAudioGroup();
  ABSL_CHECK_GE(group_id, 0);

  auto element_cfg = CreateChannelConfig(ck_roar_layout_916);
  AddWorldLockedParameter(element_cfg);
  ABSL_CHECK_EQ(renderer->AddAudioElement(group_id, kElementId, element_cfg),
                0);

  auto input = AudioBlock::CreateInput(kInputChannels, kDefaultFrameSize,
                                       kSampleRate44100);
  auto output = AudioBlock::CreateOutput(kOutputChannels);
  RunRenderLoop(state, *renderer, {kElementId}, input.block, output.block,
                num_frames, kDefaultFrameSize, kSampleRate44100);
}
BENCHMARK_CAPTURE(Render9_1_6ToBinaural_Resampled_44100, oar,
                  RendererEngine::kOar)
    ->Arg(100);
BENCHMARK_CAPTURE(Render9_1_6ToBinaural_Resampled_44100, roar,
                  RendererEngine::kRoar)
    ->Arg(100);

// World-locked 9.1.6 channel-based input to binaural rendering with dynamic
// head rotation updates per frame.
void Render9_1_6ToBinauralWithHeadRotation(benchmark::State& state,
                                           RendererEngine engine) {
  const int64_t num_frames = state.range(0);
  constexpr uint32_t kInputChannels = 16;  // 9.1.6
  constexpr uint32_t kOutputChannels = 2;  // Binaural

  const roar_config_t config = {
      .target_layout = ck_roar_layout_binaural,
      .samples_per_channel = kDefaultFrameSize,
      .sampling_rate = kDefaultSampleRate,
  };
  auto renderer = RendererInstance::Create(engine, config);
  ABSL_CHECK(renderer != nullptr);

  const int group_id = renderer->AddAudioGroup();
  ABSL_CHECK_GE(group_id, 0);

  auto element_cfg = CreateChannelConfig(ck_roar_layout_916);
  AddWorldLockedParameter(element_cfg);
  ABSL_CHECK_EQ(renderer->AddAudioElement(group_id, kElementId, element_cfg),
                0);

  ABSL_CHECK_EQ(renderer->EnableHeadTracking(true), 0);

  auto input = AudioBlock::CreateInput(kInputChannels);
  const auto head_rotations = CreateHeadRotationMetadatas();
  auto output = AudioBlock::CreateOutput(kOutputChannels);
  RunHeadTrackingRenderLoop(state, *renderer, group_id, {kElementId},
                            input.block, output.block, head_rotations,
                            num_frames);
}
BENCHMARK_CAPTURE(Render9_1_6ToBinauralWithHeadRotation, oar,
                  RendererEngine::kOar)
    ->Arg(100);
BENCHMARK_CAPTURE(Render9_1_6ToBinauralWithHeadRotation, roar,
                  RendererEngine::kRoar)
    ->Arg(100);

// Ambisonics input to binaural (OBR backend).
void Render4oaToBinaural(benchmark::State& state, RendererEngine engine) {
  const int64_t num_frames = state.range(0);
  constexpr uint32_t kInputChannels = 25;  // 4OA
  constexpr uint32_t kOutputChannels = 2;  // Binaural

  const roar_config_t config = {
      .target_layout = ck_roar_layout_binaural,
      .samples_per_channel = kDefaultFrameSize,
      .sampling_rate = kDefaultSampleRate,
  };
  auto renderer = RendererInstance::Create(engine, config);
  ABSL_CHECK(renderer != nullptr);

  const int group_id = renderer->AddAudioGroup();
  ABSL_CHECK_GE(group_id, 0);

  auto element_cfg = CreateAmbisonicConfig(ck_roar_4oa);
  AddWorldLockedParameter(element_cfg);
  ABSL_CHECK_EQ(renderer->AddAudioElement(group_id, kElementId, element_cfg),
                0);

  auto input = AudioBlock::CreateInput(kInputChannels);
  auto output = AudioBlock::CreateOutput(kOutputChannels);
  RunRenderLoop(state, *renderer, {kElementId}, input.block, output.block,
                num_frames);
}
BENCHMARK_CAPTURE(Render4oaToBinaural, oar, RendererEngine::kOar)->Arg(100);
BENCHMARK_CAPTURE(Render4oaToBinaural, roar, RendererEngine::kRoar)->Arg(100);

// World-locked ambisonics input to binaural with head rotation.
void Render4oaToBinauralWithHeadRotation(benchmark::State& state,
                                         RendererEngine engine) {
  const int64_t num_frames = state.range(0);
  constexpr uint32_t kInputChannels = 25;  // 4OA
  constexpr uint32_t kOutputChannels = 2;  // Binaural

  const roar_config_t config = {
      .target_layout = ck_roar_layout_binaural,
      .samples_per_channel = kDefaultFrameSize,
      .sampling_rate = kDefaultSampleRate,
  };
  auto renderer = RendererInstance::Create(engine, config);
  ABSL_CHECK(renderer != nullptr);

  const int group_id = renderer->AddAudioGroup();
  ABSL_CHECK_GE(group_id, 0);

  auto element_cfg = CreateAmbisonicConfig(ck_roar_4oa);
  AddWorldLockedParameter(element_cfg);
  ABSL_CHECK_EQ(renderer->AddAudioElement(group_id, kElementId, element_cfg),
                0);

  ABSL_CHECK_EQ(renderer->EnableHeadTracking(true), 0);

  auto input = AudioBlock::CreateInput(kInputChannels);
  const auto head_rotations = CreateHeadRotationMetadatas();
  auto output = AudioBlock::CreateOutput(kOutputChannels);
  RunHeadTrackingRenderLoop(state, *renderer, group_id, {kElementId},
                            input.block, output.block, head_rotations,
                            num_frames);
}
BENCHMARK_CAPTURE(Render4oaToBinauralWithHeadRotation, oar,
                  RendererEngine::kOar)
    ->Arg(100);
BENCHMARK_CAPTURE(Render4oaToBinauralWithHeadRotation, roar,
                  RendererEngine::kRoar)
    ->Arg(100);

// World-locked object-based input (8 pairs / 16 objects) to binaural rendering
// (OBR backend).
void RenderStaticObjectsToBinaural(benchmark::State& state,
                                   RendererEngine engine) {
  const int64_t num_frames = state.range(0);
  constexpr size_t kNumPairs = 8;
  constexpr uint32_t kOutputChannels = 2;  // Binaural

  const roar_config_t config = {
      .target_layout = ck_roar_layout_binaural,
      .samples_per_channel = kDefaultFrameSize,
      .sampling_rate = kDefaultSampleRate,
  };
  auto renderer = RendererInstance::Create(engine, config);
  ABSL_CHECK(renderer != nullptr);

  const int group_id = renderer->AddAudioGroup();
  ABSL_CHECK_GE(group_id, 0);

  const auto metadatas = CreateObjectPairMetadatas(kNumPairs);
  auto input = AudioBlock::CreateInput(/*channels=*/2);
  auto element_cfg = CreateObjectConfig();
  AddWorldLockedParameter(element_cfg);
  const auto element_ids = SetupObjectElements(
      *renderer, group_id, kNumPairs, element_cfg, &input.block, metadatas);

  auto output = AudioBlock::CreateOutput(kOutputChannels);
  RunRenderLoop(state, *renderer, element_ids, input.block, output.block,
                num_frames);
}
BENCHMARK_CAPTURE(RenderStaticObjectsToBinaural, oar, RendererEngine::kOar)
    ->Arg(100);
BENCHMARK_CAPTURE(RenderStaticObjectsToBinaural, roar, RendererEngine::kRoar)
    ->Arg(100);

// World-locked object-based input (8 pairs / 16 objects) with dynamic metadata
// updates per frame.
void RenderDynamicObjectsToBinaural(benchmark::State& state,
                                    RendererEngine engine) {
  const int64_t num_frames = state.range(0);
  constexpr size_t kNumPairs = 8;
  constexpr uint32_t kOutputChannels = 2;  // Binaural

  const roar_config_t config = {
      .target_layout = ck_roar_layout_binaural,
      .samples_per_channel = kDefaultFrameSize,
      .sampling_rate = kDefaultSampleRate,
  };
  auto renderer = RendererInstance::Create(engine, config);
  ABSL_CHECK(renderer != nullptr);

  const int group_id = renderer->AddAudioGroup();
  ABSL_CHECK_GE(group_id, 0);

  const auto metadatas = CreateObjectPairMetadatas(kNumPairs);
  auto input = AudioBlock::CreateInput(/*channels=*/2);
  auto element_cfg = CreateObjectConfig();
  AddWorldLockedParameter(element_cfg);
  const auto element_ids = SetupObjectElements(
      *renderer, group_id, kNumPairs, element_cfg, &input.block, metadatas);

  auto output = AudioBlock::CreateOutput(kOutputChannels);
  RunDynamicRenderLoop(state, *renderer, input.block, output.block, element_ids,
                       metadatas, num_frames);
}
BENCHMARK_CAPTURE(RenderDynamicObjectsToBinaural, oar, RendererEngine::kOar)
    ->Arg(100);
BENCHMARK_CAPTURE(RenderDynamicObjectsToBinaural, roar, RendererEngine::kRoar)
    ->Arg(100);

// 9.1.6 channel-based input downmixed to stereo loudspeakers (EAR backend).
void Render9_1_6ToStereo(benchmark::State& state, RendererEngine engine) {
  const int64_t num_frames = state.range(0);
  constexpr uint32_t kInputChannels = 16;  // 9.1.6
  constexpr uint32_t kOutputChannels = 2;  // Stereo

  const roar_config_t config = {
      .target_layout = ck_roar_layout_stereo,
      .samples_per_channel = kDefaultFrameSize,
      .sampling_rate = kDefaultSampleRate,
  };
  auto renderer = RendererInstance::Create(engine, config);
  ABSL_CHECK(renderer != nullptr);

  const int group_id = renderer->AddAudioGroup();
  ABSL_CHECK_GE(group_id, 0);

  const auto element_cfg = CreateChannelConfig(ck_roar_layout_916);
  ABSL_CHECK_EQ(renderer->AddAudioElement(group_id, kElementId, element_cfg),
                0);

  auto input = AudioBlock::CreateInput(kInputChannels);
  auto output = AudioBlock::CreateOutput(kOutputChannels);
  RunRenderLoop(state, *renderer, {kElementId}, input.block, output.block,
                num_frames);
}
BENCHMARK_CAPTURE(Render9_1_6ToStereo, oar, RendererEngine::kOar)->Arg(100);
BENCHMARK_CAPTURE(Render9_1_6ToStereo, roar, RendererEngine::kRoar)->Arg(100);

// Ambisonics input decoded to 9.1.6 loudspeakers (EAR backend).
void Render4oaTo9_1_6(benchmark::State& state, RendererEngine engine) {
  const int64_t num_frames = state.range(0);
  constexpr uint32_t kInputChannels = 25;   // 4OA
  constexpr uint32_t kOutputChannels = 16;  // 9.1.6

  const roar_config_t config = {
      .target_layout = ck_roar_layout_916,
      .samples_per_channel = kDefaultFrameSize,
      .sampling_rate = kDefaultSampleRate,
  };
  auto renderer = RendererInstance::Create(engine, config);
  ABSL_CHECK(renderer != nullptr);

  const int group_id = renderer->AddAudioGroup();
  ABSL_CHECK_GE(group_id, 0);

  const auto element_cfg = CreateAmbisonicConfig(ck_roar_4oa);
  ABSL_CHECK_EQ(renderer->AddAudioElement(group_id, kElementId, element_cfg),
                0);

  auto input = AudioBlock::CreateInput(kInputChannels);
  auto output = AudioBlock::CreateOutput(kOutputChannels);
  RunRenderLoop(state, *renderer, {kElementId}, input.block, output.block,
                num_frames);
}
BENCHMARK_CAPTURE(Render4oaTo9_1_6, oar, RendererEngine::kOar)->Arg(100);
BENCHMARK_CAPTURE(Render4oaTo9_1_6, roar, RendererEngine::kRoar)->Arg(100);

// Object-based input (8 pairs / 16 objects) rendered to 9.1.6 loudspeakers (OLR
// backend).
void RenderStaticObjectsTo9_1_6(benchmark::State& state,
                                RendererEngine engine) {
  const int64_t num_frames = state.range(0);
  constexpr size_t kNumPairs = 8;
  constexpr uint32_t kOutputChannels = 16;  // 9.1.6

  const roar_config_t config = {
      .target_layout = ck_roar_layout_916,
      .samples_per_channel = kDefaultFrameSize,
      .sampling_rate = kDefaultSampleRate,
  };
  auto renderer = RendererInstance::Create(engine, config);
  ABSL_CHECK(renderer != nullptr);

  const int group_id = renderer->AddAudioGroup();
  ABSL_CHECK_GE(group_id, 0);

  const auto metadatas = CreateObjectPairMetadatas(kNumPairs);
  auto input = AudioBlock::CreateInput(/*channels=*/2);
  const auto element_ids =
      SetupObjectElements(*renderer, group_id, kNumPairs, CreateObjectConfig(),
                          &input.block, metadatas);

  auto output = AudioBlock::CreateOutput(kOutputChannels);
  RunRenderLoop(state, *renderer, element_ids, input.block, output.block,
                num_frames);
}
BENCHMARK_CAPTURE(RenderStaticObjectsTo9_1_6, oar, RendererEngine::kOar)
    ->Arg(100);
BENCHMARK_CAPTURE(RenderStaticObjectsTo9_1_6, roar, RendererEngine::kRoar)
    ->Arg(100);

// Object-based input (8 pairs / 16 objects) rendered to 9.1.6 loudspeakers with
// dynamic metadata updates.
void RenderDynamicObjectsTo9_1_6(benchmark::State& state,
                                 RendererEngine engine) {
  const int64_t num_frames = state.range(0);
  constexpr size_t kNumPairs = 8;
  constexpr uint32_t kOutputChannels = 16;  // 9.1.6

  const roar_config_t config = {
      .target_layout = ck_roar_layout_916,
      .samples_per_channel = kDefaultFrameSize,
      .sampling_rate = kDefaultSampleRate,
  };
  auto renderer = RendererInstance::Create(engine, config);
  ABSL_CHECK(renderer != nullptr);

  const int group_id = renderer->AddAudioGroup();
  ABSL_CHECK_GE(group_id, 0);

  const auto metadatas = CreateObjectPairMetadatas(kNumPairs);
  auto input = AudioBlock::CreateInput(/*channels=*/2);
  const auto element_ids =
      SetupObjectElements(*renderer, group_id, kNumPairs, CreateObjectConfig(),
                          &input.block, metadatas);

  auto output = AudioBlock::CreateOutput(kOutputChannels);
  RunDynamicRenderLoop(state, *renderer, input.block, output.block, element_ids,
                       metadatas, num_frames);
}
BENCHMARK_CAPTURE(RenderDynamicObjectsTo9_1_6, oar, RendererEngine::kOar)
    ->Arg(100);
BENCHMARK_CAPTURE(RenderDynamicObjectsTo9_1_6, roar, RendererEngine::kRoar)
    ->Arg(100);

// Stereo element rendered to stereo loudspeakers with constant gain updates.
void RenderStereoWithConstantGain(benchmark::State& state,
                                  RendererEngine engine) {
  const roar_metadata_t gain_metadata = CreateConstantGainMetadata(
      /*id=*/1, /*gain_db=*/-6.0f, kDefaultFrameSize);
  RunStereoElementGainBenchmark(state, engine, gain_metadata);
}
BENCHMARK_CAPTURE(RenderStereoWithConstantGain, oar, RendererEngine::kOar)
    ->Arg(100);
BENCHMARK_CAPTURE(RenderStereoWithConstantGain, roar, RendererEngine::kRoar)
    ->Arg(100);

// Stereo element rendered to stereo loudspeakers with linear dB gain ramps.
void RenderStereoWithAnimatedGainLinear(benchmark::State& state,
                                        RendererEngine engine) {
  const roar_metadata_t gain_metadata = CreateAnimatedGainMetadata(
      /*id=*/1, ck_roar_animation_type_linear, /*start=*/0.0f, /*end=*/-12.0f,
      /*control=*/0.0f, /*control_relative_time=*/0.0f, kDefaultFrameSize);
  RunStereoElementGainBenchmark(state, engine, gain_metadata);
}
BENCHMARK_CAPTURE(RenderStereoWithAnimatedGainLinear, oar, RendererEngine::kOar)
    ->Arg(100);
BENCHMARK_CAPTURE(RenderStereoWithAnimatedGainLinear, roar,
                  RendererEngine::kRoar)
    ->Arg(100);

// Stereo element rendered to stereo loudspeakers with quadratic Bezier dB gain
// curves.
void RenderStereoWithAnimatedGainBezier(benchmark::State& state,
                                        RendererEngine engine) {
  const roar_metadata_t gain_metadata = CreateAnimatedGainMetadata(
      /*id=*/1, ck_roar_animation_type_bezier, /*start=*/0.0f, /*end=*/-12.0f,
      /*control=*/-6.0f, /*control_relative_time=*/0.5f, kDefaultFrameSize);
  RunStereoElementGainBenchmark(state, engine, gain_metadata);
}
BENCHMARK_CAPTURE(RenderStereoWithAnimatedGainBezier, oar, RendererEngine::kOar)
    ->Arg(100);
BENCHMARK_CAPTURE(RenderStereoWithAnimatedGainBezier, roar,
                  RendererEngine::kRoar)
    ->Arg(100);

// -----------------------------------------------------------------------------
//   Setup & Initialization Benchmarks (To explicitly isolate setup cost)
// -----------------------------------------------------------------------------

// Measures renderer creation, element registration, and DSP graph allocation
// time.
void Setup9_1_6ToBinaural(benchmark::State& state, RendererEngine engine) {
  constexpr uint32_t kInputChannels = 16;

  const roar_config_t config = {
      .target_layout = ck_roar_layout_binaural,
      .samples_per_channel = kDefaultFrameSize,
      .sampling_rate = kDefaultSampleRate,
  };
  auto element_cfg = CreateChannelConfig(ck_roar_layout_916);
  AddWorldLockedParameter(element_cfg);
  auto input = AudioBlock::CreateInput(kInputChannels);

  for (auto _ : state) {
    auto renderer = RendererInstance::Create(engine, config);
    const int group_id = renderer->AddAudioGroup();
    renderer->AddAudioElement(group_id, kElementId, element_cfg);
    renderer->UpdateAudioElementData(kElementId, &input.block);
    benchmark::DoNotOptimize(renderer);
  }
}
BENCHMARK_CAPTURE(Setup9_1_6ToBinaural, oar, RendererEngine::kOar);
BENCHMARK_CAPTURE(Setup9_1_6ToBinaural, roar, RendererEngine::kRoar);

// Measures binaural setup and initialization time when polyphase sinc
// resampling the internal HRIR filter bank from 48 kHz to 44.1 kHz.
void Setup9_1_6ToBinaural_Resampled_44100(benchmark::State& state,
                                          RendererEngine engine) {
  constexpr uint32_t kInputChannels = 16;
  constexpr uint32_t kSampleRate44100 = 44100;

  const roar_config_t config = {
      .target_layout = ck_roar_layout_binaural,
      .samples_per_channel = kDefaultFrameSize,
      .sampling_rate = kSampleRate44100,
  };
  auto element_cfg = CreateChannelConfig(ck_roar_layout_916);
  AddWorldLockedParameter(element_cfg);
  auto input = AudioBlock::CreateInput(kInputChannels, kDefaultFrameSize,
                                       kSampleRate44100);

  for (auto _ : state) {
    auto renderer = RendererInstance::Create(engine, config);
    const int group_id = renderer->AddAudioGroup();
    renderer->AddAudioElement(group_id, kElementId, element_cfg);
    renderer->UpdateAudioElementData(kElementId, &input.block);
    benchmark::DoNotOptimize(renderer);
  }
}
BENCHMARK_CAPTURE(Setup9_1_6ToBinaural_Resampled_44100, oar,
                  RendererEngine::kOar);
BENCHMARK_CAPTURE(Setup9_1_6ToBinaural_Resampled_44100, roar,
                  RendererEngine::kRoar);

// Measures EAR channel-to-channel downmixer setup and matrix initialization.
void Setup9_1_6ToStereo(benchmark::State& state, RendererEngine engine) {
  constexpr uint32_t kInputChannels = 16;

  const roar_config_t config = {
      .target_layout = ck_roar_layout_stereo,
      .samples_per_channel = kDefaultFrameSize,
      .sampling_rate = kDefaultSampleRate,
  };
  const auto element_cfg = CreateChannelConfig(ck_roar_layout_916);
  auto input = AudioBlock::CreateInput(kInputChannels);

  for (auto _ : state) {
    auto renderer = RendererInstance::Create(engine, config);
    const int group_id = renderer->AddAudioGroup();
    renderer->AddAudioElement(group_id, kElementId, element_cfg);
    renderer->UpdateAudioElementData(kElementId, &input.block);
    benchmark::DoNotOptimize(renderer);
  }
}
BENCHMARK_CAPTURE(Setup9_1_6ToStereo, oar, RendererEngine::kOar);
BENCHMARK_CAPTURE(Setup9_1_6ToStereo, roar, RendererEngine::kRoar);

// Measures EAR ambisonic decoding matrix setup and element registration.
void Setup4oaTo9_1_6(benchmark::State& state, RendererEngine engine) {
  constexpr uint32_t kInputChannels = 25;  // 4OA

  const roar_config_t config = {
      .target_layout = ck_roar_layout_916,
      .samples_per_channel = kDefaultFrameSize,
      .sampling_rate = kDefaultSampleRate,
  };
  const auto element_cfg = CreateAmbisonicConfig(ck_roar_4oa);
  auto input = AudioBlock::CreateInput(kInputChannels);

  for (auto _ : state) {
    auto renderer = RendererInstance::Create(engine, config);
    const int group_id = renderer->AddAudioGroup();
    renderer->AddAudioElement(group_id, kElementId, element_cfg);
    renderer->UpdateAudioElementData(kElementId, &input.block);
    benchmark::DoNotOptimize(renderer);
  }
}
BENCHMARK_CAPTURE(Setup4oaTo9_1_6, oar, RendererEngine::kOar);
BENCHMARK_CAPTURE(Setup4oaTo9_1_6, roar, RendererEngine::kRoar);

// Measures OLR 3D VBAP triangulation mesh and 16 object registrations with
// initial metadata.
void SetupObjectsTo9_1_6(benchmark::State& state, RendererEngine engine) {
  constexpr size_t kNumPairs = 8;

  const roar_config_t config = {
      .target_layout = ck_roar_layout_916,
      .samples_per_channel = kDefaultFrameSize,
      .sampling_rate = kDefaultSampleRate,
  };
  const auto metadatas = CreateObjectPairMetadatas(kNumPairs);
  auto input = AudioBlock::CreateInput(/*channels=*/2);

  for (auto _ : state) {
    auto renderer = RendererInstance::Create(engine, config);
    const int group_id = renderer->AddAudioGroup();
    SetupObjectElements(*renderer, group_id, kNumPairs, CreateObjectConfig(),
                        &input.block, metadatas);
    benchmark::DoNotOptimize(renderer);
  }
}
BENCHMARK_CAPTURE(SetupObjectsTo9_1_6, oar, RendererEngine::kOar);
BENCHMARK_CAPTURE(SetupObjectsTo9_1_6, roar, RendererEngine::kRoar);

}  // namespace
}  // namespace roar_equivalence_testing
