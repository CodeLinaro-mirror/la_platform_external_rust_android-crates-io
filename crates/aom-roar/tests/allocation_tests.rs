// Copyright (c) 2026, Alliance for Open Media. All rights reserved
//
// This source code is subject to the terms of the BSD 3-Clause Clear License
// and the Alliance for Open Media Patent License 1.0. If the BSD 3-Clause Clear
// License was not distributed with this source code in the LICENSE file, you
// can obtain it at www.aomedia.org/license/software-license/bsd-3-c-c. If the
// Alliance for Open Media Patent License 1.0 was not distributed with this
// source code in the PATENTS file, you can obtain it at
// www.aomedia.org/license/patent.

mod test_helpers;

/// Tests that rendering (and functions that would be called during rendering) do not allocate.
///
/// These tests use a custom allocator and a closure to ensure that calls are not allocating memory
/// during the rendering phase.
#[cfg(test)]
mod test {

    use googletest::prelude::*;

    use std::alloc::{GlobalAlloc, Layout as AllocLayout, System};
    use std::cell::Cell;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::test_helpers::{create_planar_buffer_mut, create_planar_buffer_ref};
    use aom_roar::{
        AudioElementConfig, ChannelBasedConfig, Config, Decibels, Gain, HighOrderAmbisonics,
        InputLayout, ObjectBasedConfig, ObjectPosition, OutputLayout, PolarCoordinate, Quaternion,
        RoarRenderer, SampleRate, Samples, SceneBasedConfig,
    };

    /// A tracking allocator to intercept heap allocations and assert that the
    /// render loops do not perform any heap allocation.
    struct TrackingAllocator;

    thread_local! {
        static TRACK_CURRENT_THREAD: Cell<bool> = const { Cell::new(false) };
    }

    static ALLOCATED_COUNT: AtomicUsize = AtomicUsize::new(0);

    // SAFETY: We are implementing `GlobalAlloc` by delegating directly to the
    // standard library's `System` allocator. We simply increment a thread-safe atomic
    // counter if tracking is currently active on the calling thread. This is sound
    // because the memory layout contract is preserved.
    unsafe impl GlobalAlloc for TrackingAllocator {
        unsafe fn alloc(&self, layout: AllocLayout) -> *mut u8 {
            let is_tracking = TRACK_CURRENT_THREAD.try_with(|cell| cell.get()).unwrap_or(false);
            if is_tracking {
                ALLOCATED_COUNT.fetch_add(1, Ordering::SeqCst);
            }
            // SAFETY: Delegating to standard allocator system with identical layout
            unsafe { System.alloc(layout) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: AllocLayout) {
            // SAFETY: Delegating to standard allocator system with identical ptr and layout
            unsafe { System.dealloc(ptr, layout) }
        }
    }

    // Just for the test binary.
    #[global_allocator] // allow_global_allocator
    static ALLOC: TrackingAllocator = TrackingAllocator;

    /// Runs a closure with allocation tracking active on the current thread,
    /// asserting that no allocations occur during its execution.
    fn assert_zero_allocations<F, R>(f: F) -> R
    where
        F: FnOnce() -> R,
    {
        ALLOCATED_COUNT.store(0, Ordering::SeqCst);
        let _ = TRACK_CURRENT_THREAD.try_with(|cell| cell.set(true));

        let result = f();

        let _ = TRACK_CURRENT_THREAD.try_with(|cell| cell.set(false));
        let allocs = ALLOCATED_COUNT.load(Ordering::SeqCst);

        expect_that!(
            allocs,
            eq(0),
            "Zero allocations expected inside render loop, but detected {} heap allocation(s)!",
            allocs
        );

        result
    }

    /// Verifies that rendering a channel-based element to a Stereo output with downmixing
    /// (5.1 -> Stereo) and limiting enabled does not allocate memory on the heap.
    #[gtest]
    fn downmix_rendering_does_not_allocate_memory() {
        let frame_size: usize = 128;
        let sample_rate: u32 = 48000;
        let input_channels: usize = 6;
        let output_channels: usize = 2;
        let element_id: u32 = 1;

        let config = Config::new(
            OutputLayout::Stereo,
            Samples::new(frame_size as u32).unwrap(),
            SampleRate::new(sample_rate).unwrap(),
        )
        .unwrap();
        let mut renderer = RoarRenderer::create(&config).unwrap();
        let group_id = renderer.add_audio_group().unwrap();

        let element_cfg = AudioElementConfig::ChannelBased(ChannelBasedConfig {
            layout: InputLayout::Layout51,
            downmix_info: None,
            rendering_config: None,
        });
        renderer.add_element(group_id, element_id, &element_cfg).unwrap();
        renderer.enable_limiter(true).unwrap();

        let inputs = vec![vec![0.5f32; frame_size]; input_channels];
        let mut output = vec![0.0f32; output_channels * frame_size];
        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let mut output_slices: Vec<&mut [f32]> =
            (0..output_channels).map(|_| &mut [] as &mut [f32]).collect();
        let mut output_buffer =
            create_planar_buffer_mut(&mut output, output_channels, frame_size, &mut output_slices);

        assert_zero_allocations(|| {
            let input_buffer_ref = create_planar_buffer_ref(&input_refs, frame_size);
            let inputs_arg = [(element_id, input_buffer_ref)];
            let res = renderer.render(&inputs_arg, &mut output_buffer);
            expect_ok!(res);
        });
    }

    /// Verifies that rendering a scene-based element to a Binaural output (routing through OBR)
    /// does not allocate memory on the heap.
    #[gtest]
    fn binaural_rendering_does_not_allocate_memory() {
        let frame_size: usize = 256;
        let sample_rate: u32 = 48000;
        let input_channels: usize = 4;
        let output_channels: usize = 2;
        let element_id: u32 = 1;

        let config = Config::new(
            OutputLayout::Binaural,
            Samples::new(frame_size as u32).unwrap(),
            SampleRate::new(sample_rate).unwrap(),
        )
        .unwrap();
        let mut renderer = RoarRenderer::create(&config).unwrap();
        let group_id = renderer.add_audio_group().unwrap();

        let element_cfg = AudioElementConfig::SceneBased(SceneBasedConfig {
            order: HighOrderAmbisonics::Order1,
            rendering_config: None,
        });
        renderer.add_element(group_id, element_id, &element_cfg).unwrap();

        let inputs = vec![vec![0.1f32; frame_size]; input_channels];
        let mut output = vec![0.0f32; output_channels * frame_size];
        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let mut output_slices: Vec<&mut [f32]> =
            (0..output_channels).map(|_| &mut [] as &mut [f32]).collect();
        let mut output_buffer =
            create_planar_buffer_mut(&mut output, output_channels, frame_size, &mut output_slices);

        assert_zero_allocations(|| {
            let input_buffer_ref = create_planar_buffer_ref(&input_refs, frame_size);
            let inputs_arg = [(element_id, input_buffer_ref)];
            let res = renderer.render(&inputs_arg, &mut output_buffer);
            expect_ok!(res);
        });
    }

    /// Verifies that rendering a scene-based element to a loudspeaker output (5.1, routing through
    /// EAR) does not allocate memory on the heap.
    #[gtest]
    fn ear_rendering_does_not_allocate_memory() {
        let frame_size: usize = 256;
        let sample_rate: u32 = 48000;
        let input_channels: usize = 4;
        let output_channels: usize = 6; // 5.1 is 6 channels
        let element_id: u32 = 1;

        let config = Config::new(
            OutputLayout::Layout51,
            Samples::new(frame_size as u32).unwrap(),
            SampleRate::new(sample_rate).unwrap(),
        )
        .unwrap();
        let mut renderer = RoarRenderer::create(&config).unwrap();
        let group_id = renderer.add_audio_group().unwrap();

        let element_cfg = AudioElementConfig::SceneBased(SceneBasedConfig {
            order: HighOrderAmbisonics::Order1,
            rendering_config: None,
        });
        renderer.add_element(group_id, element_id, &element_cfg).unwrap();

        let inputs = vec![vec![0.1f32; frame_size]; input_channels];
        let mut output = vec![0.0f32; output_channels * frame_size];
        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let mut output_slices: Vec<&mut [f32]> =
            (0..output_channels).map(|_| &mut [] as &mut [f32]).collect();
        let mut output_buffer =
            create_planar_buffer_mut(&mut output, output_channels, frame_size, &mut output_slices);

        assert_zero_allocations(|| {
            let input_buffer_ref = create_planar_buffer_ref(&input_refs, frame_size);
            let inputs_arg = [(element_id, input_buffer_ref)];
            let res = renderer.render(&inputs_arg, &mut output_buffer);
            expect_ok!(res);
        });
    }

    /// Verifies that rendering an object-based element to a loudspeaker output (5.1, routing
    /// through OLR) does not allocate memory on the heap.
    #[gtest]
    fn olr_rendering_does_not_allocate_memory() {
        let frame_size: usize = 128;
        let sample_rate: u32 = 48000;
        let input_channels: usize = 1; // 1 object
        let output_channels: usize = 6; // 5.1 is 6 channels
        let element_id: u32 = 1;

        let config = Config::new(
            OutputLayout::Layout51,
            Samples::new(frame_size as u32).unwrap(),
            SampleRate::new(sample_rate).unwrap(),
        )
        .unwrap();
        let mut renderer = RoarRenderer::create(&config).unwrap();
        let group_id = renderer.add_audio_group().unwrap();

        let element_cfg =
            AudioElementConfig::ObjectBased(ObjectBasedConfig::new(input_channels, None).unwrap());
        renderer.add_element(group_id, element_id, &element_cfg).unwrap();

        // Object needs position
        let pos =
            ObjectPosition::Polar(vec![PolarCoordinate::new_from_floats(0.0, 0.0, 1.0).unwrap()]);
        renderer
            .update_element_positions(element_id, &pos, Samples::new(frame_size as u32).unwrap())
            .unwrap();

        let inputs = vec![vec![0.5f32; frame_size]; input_channels];
        let mut output = vec![0.0f32; output_channels * frame_size];
        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let mut output_slices: Vec<&mut [f32]> =
            (0..output_channels).map(|_| &mut [] as &mut [f32]).collect();
        let mut output_buffer =
            create_planar_buffer_mut(&mut output, output_channels, frame_size, &mut output_slices);

        assert_zero_allocations(|| {
            let input_buffer_ref = create_planar_buffer_ref(&input_refs, frame_size);
            let inputs_arg = [(element_id, input_buffer_ref)];
            let res = renderer.render(&inputs_arg, &mut output_buffer);
            expect_ok!(res);
        });
    }

    /// Verifies that updating object positions during rendering under OBR (Binaural output)
    /// does not allocate memory on the heap (triggers interpolation).
    #[gtest]
    fn obr_object_position_update_does_not_allocate_memory() {
        let frame_size: usize = 128;
        let sample_rate: u32 = 48000;
        let input_channels: usize = 1;
        let output_channels: usize = 2; // Binaural is Stereo (2 channels)
        let element_id: u32 = 1;

        let config = Config::new(
            OutputLayout::Binaural,
            Samples::new(frame_size as u32).unwrap(),
            SampleRate::new(sample_rate).unwrap(),
        )
        .unwrap();
        let mut renderer = RoarRenderer::create(&config).unwrap();
        let group_id = renderer.add_audio_group().unwrap();

        let element_cfg =
            AudioElementConfig::ObjectBased(ObjectBasedConfig::new(input_channels, None).unwrap());
        renderer.add_element(group_id, element_id, &element_cfg).unwrap();

        // Queue initial position
        let pos1 =
            ObjectPosition::Polar(vec![PolarCoordinate::new_from_floats(0.0, 0.0, 1.0).unwrap()]);
        renderer
            .update_element_positions(element_id, &pos1, Samples::new(frame_size as u32).unwrap())
            .unwrap();

        let inputs = vec![vec![0.5f32; frame_size]; input_channels];
        let mut output = vec![0.0f32; output_channels * frame_size];
        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let mut output_slices: Vec<&mut [f32]> =
            (0..output_channels).map(|_| &mut [] as &mut [f32]).collect();
        let mut output_buffer =
            create_planar_buffer_mut(&mut output, output_channels, frame_size, &mut output_slices);

        // Render once to establish initial position
        let input_buffer_ref = create_planar_buffer_ref(&input_refs, frame_size);
        renderer.render(&[(element_id, input_buffer_ref)], &mut output_buffer).unwrap();

        // Queue new position (triggers interpolation in next render)
        let pos2 =
            ObjectPosition::Polar(vec![PolarCoordinate::new_from_floats(90.0, 0.0, 1.0).unwrap()]);
        renderer
            .update_element_positions(element_id, &pos2, Samples::new(frame_size as u32).unwrap())
            .unwrap();

        assert_zero_allocations(|| {
            let input_buffer_ref = create_planar_buffer_ref(&input_refs, frame_size);
            let res = renderer.render(&[(element_id, input_buffer_ref)], &mut output_buffer);
            expect_ok!(res);
        });
    }

    /// Verifies that updating group gains during rendering does not allocate memory on the heap
    /// (triggers interpolation).
    #[gtest]
    fn group_gain_update_does_not_allocate_memory() {
        let frame_size: usize = 128;
        let sample_rate: u32 = 48000;
        let input_channels: usize = 1;
        let output_channels: usize = 2; // Stereo output
        let element_id: u32 = 1;

        let config = Config::new(
            OutputLayout::Stereo,
            Samples::new(frame_size as u32).unwrap(),
            SampleRate::new(sample_rate).unwrap(),
        )
        .unwrap();
        let mut renderer = RoarRenderer::create(&config).unwrap();
        let group_id = renderer.add_audio_group().unwrap();

        let element_cfg =
            AudioElementConfig::ObjectBased(ObjectBasedConfig::new(input_channels, None).unwrap());
        renderer.add_element(group_id, element_id, &element_cfg).unwrap();

        let inputs = vec![vec![0.5f32; frame_size]; input_channels];
        let mut output = vec![0.0f32; output_channels * frame_size];
        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let mut output_slices: Vec<&mut [f32]> =
            (0..output_channels).map(|_| &mut [] as &mut [f32]).collect();
        let mut output_buffer =
            create_planar_buffer_mut(&mut output, output_channels, frame_size, &mut output_slices);

        // Queue group gain update (triggers interpolation in next render)
        let gain = Gain::new_constant(Decibels::new(-3.0).unwrap()).unwrap();
        renderer
            .update_group_gain(group_id, &gain, Samples::new(frame_size as u32).unwrap())
            .unwrap();

        assert_zero_allocations(|| {
            let input_buffer_ref = create_planar_buffer_ref(&input_refs, frame_size);
            let res = renderer.render(&[(element_id, input_buffer_ref)], &mut output_buffer);
            expect_ok!(res);
        });
    }

    /// Verifies that updating head rotation during rendering (under OBR) does not allocate memory
    /// on the heap (triggers interpolation).
    #[gtest]
    fn head_rotation_update_does_not_allocate_memory() {
        let frame_size: usize = 128;
        let sample_rate: u32 = 48000;
        let input_channels: usize = 4;
        let output_channels: usize = 2; // Binaural output
        let element_id: u32 = 1;

        let config = Config::new(
            OutputLayout::Binaural,
            Samples::new(frame_size as u32).unwrap(),
            SampleRate::new(sample_rate).unwrap(),
        )
        .unwrap();
        let mut renderer = RoarRenderer::create(&config).unwrap();
        let group_id = renderer.add_audio_group().unwrap();

        let element_cfg = AudioElementConfig::SceneBased(SceneBasedConfig {
            order: HighOrderAmbisonics::Order1,
            rendering_config: None,
        });
        renderer.add_element(group_id, element_id, &element_cfg).unwrap();
        renderer.enable_head_tracking(true).unwrap();

        let inputs = vec![vec![0.1f32; frame_size]; input_channels];
        let mut output = vec![0.0f32; output_channels * frame_size];
        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let mut output_slices: Vec<&mut [f32]> =
            (0..output_channels).map(|_| &mut [] as &mut [f32]).collect();
        let mut output_buffer =
            create_planar_buffer_mut(&mut output, output_channels, frame_size, &mut output_slices);

        // Set initial rotation
        renderer.set_head_rotation(Quaternion::identity()).unwrap();
        let input_buffer_ref = create_planar_buffer_ref(&input_refs, frame_size);
        renderer.render(&[(element_id, input_buffer_ref)], &mut output_buffer).unwrap();

        // Change rotation (triggers interpolation in next render)
        let rot = Quaternion::new(0.707, 0.0, 0.707, 0.0).unwrap(); // 90 deg rotation

        assert_zero_allocations(|| {
            renderer.set_head_rotation(rot).unwrap();
            let input_buffer_ref = create_planar_buffer_ref(&input_refs, frame_size);
            let res = renderer.render(&[(element_id, input_buffer_ref)], &mut output_buffer);
            expect_ok!(res);
        });
    }
}
