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

/// Integration tests for rendering a combination of channel-based, scene-based, and
/// object-based elements simultaneously.
#[cfg(test)]
mod test {

    use crate::test_helpers::{
        create_channel_based_config, create_object_based_config, create_planar_buffer_mut,
        create_planar_buffer_ref, create_scene_based_config, rms, TestRoar,
    };
    use aom_roar::c_types::{
        roar_audio_block_t, roar_coordinate_type_t, roar_hoa_t, roar_layout_t,
        roar_metadata_object_positions_t, roar_metadata_object_positions_union_t, roar_metadata_t,
        roar_metadata_type_t, roar_metadata_union_t, roar_param_type_t, roar_polar_t,
        ROAR_MAX_NUMBER_OF_OBJECTS,
    };
    use aom_roar::ffi::*;
    use aom_roar::{
        AudioElementConfig, BinauralFilterProfile, ChannelBasedConfig, Config,
        ElementRenderingConfig, HeadphonesRenderingMode, HighOrderAmbisonics, InputLayout,
        ObjectBasedConfig, ObjectPosition, OutputLayout, PolarCoordinate, Quaternion, RoarRenderer,
        SampleRate, Samples, SceneBasedConfig,
    };
    use googletest::prelude::*;

    /// Expect at least this value for non-silent output.
    const NON_SILENT_RMS_THRESHOLD: f32 = 0.01;

    // ===== C FFI API Tests =====

    /// Verifies that rendering a combination of a ChannelBased, a SceneBased, and an ObjectBased
    /// element simultaneously via the C FFI API succeeds and produces output.
    #[gtest]
    fn ffi_rendering_combination_of_all_element_types_succeeds() {
        let frame_size: usize = 128;
        let sample_rate: u32 = 48000;
        let output_channels: usize = 2;
        let chan_based_element_id: u32 = 10;
        let scene_based_element_id: u32 = 20;
        let obj_based_element_id: u32 = 30;

        // Create test ROAR instance.
        let oar =
            TestRoar::new(roar_layout_t::ck_roar_layout_stereo, frame_size as u32, sample_rate);

        // SAFETY: oar is valid.
        let group_id = unsafe { roar_add_audio_group(oar.ptr()) };
        assert_eq!(group_id, 0);

        // 1. Channel-Based Element (Stereo) -> ID 10
        let chan_based_cfg = create_channel_based_config(roar_layout_t::ck_roar_layout_stereo);
        // SAFETY: config and oar are valid.
        let ret = unsafe {
            roar_add_audio_element(
                oar.ptr(),
                group_id as u32,
                chan_based_element_id,
                &chan_based_cfg,
            )
        };
        assert_eq!(ret, 0);

        // 2. Scene-Based Element (1OA) -> ID 20
        let scene_based_cfg = create_scene_based_config(roar_hoa_t::ck_roar_1oa);
        // SAFETY: config and oar are valid.
        let ret = unsafe {
            roar_add_audio_element(
                oar.ptr(),
                group_id as u32,
                scene_based_element_id,
                &scene_based_cfg,
            )
        };
        assert_eq!(ret, 0);

        // 3. Object-Based Element (1 object) -> ID 30
        let obj_based_cfg = create_object_based_config(1);
        // SAFETY: config and oar are valid.
        let ret = unsafe {
            roar_add_audio_element(oar.ptr(), group_id as u32, obj_based_element_id, &obj_based_cfg)
        };
        assert_eq!(ret, 0);

        // Set position for Object (ID 30)
        let mut polar_pos = [roar_polar_t { azimuth: 0.0, elevation: 0.0, distance: 0.0 };
            ROAR_MAX_NUMBER_OF_OBJECTS];
        polar_pos[0] = roar_polar_t { azimuth: 0.0, elevation: 0.0, distance: 1.0 };
        let pos_meta = roar_metadata_t {
            r#type: roar_metadata_type_t::ck_roar_metadata_object_positions as u32,
            value: roar_metadata_union_t {
                object_positions: roar_metadata_object_positions_t {
                    param_type: roar_param_type_t::ck_roar_param_constant as u32,
                    position_type: roar_coordinate_type_t::ck_roar_polar as u32,
                    num_objects: 1,
                    positions: roar_metadata_object_positions_union_t {
                        polar_positions: polar_pos,
                    },
                },
            },
            duration: frame_size as i32,
        };
        // SAFETY: metadata and oar are valid.
        let ret = unsafe {
            roar_update_audio_element_metadata(oar.ptr(), obj_based_element_id, &pos_meta)
        };
        assert_eq!(ret, 0);

        let mut chan_based_input = vec![0.2f32; 2 * frame_size];
        let mut scene_based_input = vec![0.0f32; 4 * frame_size];
        scene_based_input[..frame_size].fill(0.1); // Fill first channel
        let mut obj_based_input = vec![0.3f32; frame_size];

        let mut chan_based_block = roar_audio_block_t {
            data: chan_based_input.as_mut_ptr(),
            channels: 2,
            samples_per_channel: frame_size as u32,
        };
        let mut scene_based_block = roar_audio_block_t {
            data: scene_based_input.as_mut_ptr(),
            channels: 4,
            samples_per_channel: frame_size as u32,
        };
        let mut obj_based_block = roar_audio_block_t {
            data: obj_based_input.as_mut_ptr(),
            channels: 1,
            samples_per_channel: frame_size as u32,
        };

        let mut output = vec![0.0f32; output_channels * frame_size];
        let mut out_block = roar_audio_block_t {
            data: output.as_mut_ptr(),
            channels: output_channels as u32,
            samples_per_channel: frame_size as u32,
        };

        // SAFETY: data and oar are valid.
        let ret = unsafe {
            roar_update_audio_element_data(oar.ptr(), chan_based_element_id, &mut chan_based_block)
        };
        assert_eq!(ret, 0);
        // SAFETY: data and oar are valid.
        let ret = unsafe {
            roar_update_audio_element_data(
                oar.ptr(),
                scene_based_element_id,
                &mut scene_based_block,
            )
        };
        assert_eq!(ret, 0);
        // SAFETY: data and oar are valid.
        let ret = unsafe {
            roar_update_audio_element_data(oar.ptr(), obj_based_element_id, &mut obj_based_block)
        };
        assert_eq!(ret, 0);

        // SAFETY: output block and oar are valid.
        let ret = unsafe { roar_render(oar.ptr(), &mut out_block) };
        assert_eq!(ret, 0);

        // Verify output is non-silent in planar layout.
        expect_that!(rms(&output[..frame_size]), gt(NON_SILENT_RMS_THRESHOLD));
        expect_that!(rms(&output[frame_size..]), gt(NON_SILENT_RMS_THRESHOLD));
    }

    // ===== Rust API Tests =====

    /// Verifies that rendering a combination of a ChannelBased, a SceneBased, and an ObjectBased
    /// element simultaneously using the Rust API succeeds and produces a non-silent stereo output.
    #[gtest]
    fn rust_rendering_combination_of_all_element_types_succeeds() {
        let frame_size: usize = 128;
        let sample_rate: u32 = 48000;
        let output_channels: usize = 2;

        let config = Config::new(
            OutputLayout::Stereo,
            Samples::new(frame_size as u32).unwrap(),
            SampleRate::new(sample_rate).unwrap(),
        )
        .unwrap();
        let mut renderer = RoarRenderer::create(&config).unwrap();
        let group_id = renderer.add_audio_group().unwrap();

        // 1. Channel-Based Element (Stereo)
        let chan_based_element_id: u32 = 10;
        let chan_based_cfg = AudioElementConfig::ChannelBased(ChannelBasedConfig {
            layout: InputLayout::Stereo,
            downmix_info: None,
            rendering_config: None,
        });
        renderer.add_element(group_id, chan_based_element_id, &chan_based_cfg).unwrap();

        // 2. Scene-Based Element (1OA - 4 channels)
        let scene_based_element_id: u32 = 20;
        let scene_based_cfg = AudioElementConfig::SceneBased(SceneBasedConfig {
            order: HighOrderAmbisonics::Order1,
            rendering_config: None,
        });
        renderer.add_element(group_id, scene_based_element_id, &scene_based_cfg).unwrap();

        // 3. Object-Based Element (1 object)
        let obj_based_element_id: u32 = 30;
        let obj_based_cfg =
            AudioElementConfig::ObjectBased(ObjectBasedConfig::new(1, None).unwrap());
        renderer.add_element(group_id, obj_based_element_id, &obj_based_cfg).unwrap();

        // Set position for Object
        let pos =
            ObjectPosition::Polar(vec![PolarCoordinate::new_from_floats(0.0, 0.0, 1.0).unwrap()]);
        renderer
            .update_element_positions(
                obj_based_element_id,
                &pos,
                Samples::new(frame_size as u32).unwrap(),
            )
            .unwrap();

        let chan_based_inputs = vec![vec![0.2; frame_size]; 2];
        let mut scene_based_inputs = vec![vec![0.0; frame_size]; 4];
        scene_based_inputs[0].fill(0.1);
        let obj_based_inputs = vec![vec![0.3; frame_size]; 1];

        let chan_based_refs: Vec<&[f32]> = chan_based_inputs.iter().map(|v| v.as_slice()).collect();
        let scene_based_refs: Vec<&[f32]> =
            scene_based_inputs.iter().map(|v| v.as_slice()).collect();
        let obj_based_refs: Vec<&[f32]> = obj_based_inputs.iter().map(|v| v.as_slice()).collect();

        let chan_based_in_view = create_planar_buffer_ref(&chan_based_refs[..], frame_size);
        let scene_based_in_view = create_planar_buffer_ref(&scene_based_refs[..], frame_size);
        let obj_based_in_view = create_planar_buffer_ref(&obj_based_refs[..], frame_size);

        let mut output = vec![0.0; output_channels * frame_size];
        let mut output_slices: Vec<&mut [f32]> =
            (0..output_channels).map(|_| &mut [] as &mut [f32]).collect();
        let mut output_buffer =
            create_planar_buffer_mut(&mut output, output_channels, frame_size, &mut output_slices);

        let result = renderer.render(
            &[
                (chan_based_element_id, chan_based_in_view),
                (scene_based_element_id, scene_based_in_view),
                (obj_based_element_id, obj_based_in_view),
            ],
            &mut output_buffer,
        );

        expect_ok!(result);
        // Verify output is non-silent (has energy from all elements)
        expect_that!(rms(output_buffer.channel_mut(0)), gt(NON_SILENT_RMS_THRESHOLD));
        expect_that!(rms(output_buffer.channel_mut(1)), gt(NON_SILENT_RMS_THRESHOLD));
    }

    /// Verifies that removing the first of two binaural elements updates element counts and channel
    /// lookups and leaves the remaining element able to render non-silent output.
    #[gtest]
    fn binaural_removing_first_element_preserves_remaining_element_rendering() {
        let frame_size: usize = 128;
        let sample_rate: u32 = 48000;
        let output_channels: usize = 2;
        let config = Config::new(
            OutputLayout::Binaural,
            Samples::new(frame_size as u32).unwrap(),
            SampleRate::new(sample_rate).unwrap(),
        )
        .unwrap();
        let mut renderer = RoarRenderer::create(&config).unwrap();
        let group_id = renderer.add_audio_group().unwrap();
        let stereo_cfg = AudioElementConfig::ChannelBased(ChannelBasedConfig {
            layout: InputLayout::Stereo,
            downmix_info: None,
            rendering_config: None,
        });
        let mono_cfg = AudioElementConfig::ChannelBased(ChannelBasedConfig {
            layout: InputLayout::Mono,
            downmix_info: None,
            rendering_config: None,
        });
        renderer.add_element(group_id, 1, &stereo_cfg).unwrap();
        renderer.add_element(group_id, 2, &mono_cfg).unwrap();
        assert_eq!(renderer.number_of_audio_elements(), 2);

        let remove_result = renderer.remove_element(1);
        let mono_input = [vec![0.4f32; frame_size]];
        let mono_refs: Vec<&[f32]> = mono_input.iter().map(|v| v.as_slice()).collect();
        let mono_view = create_planar_buffer_ref(&mono_refs, frame_size);
        let mut output = vec![0.0f32; output_channels * frame_size];
        let mut output_slices: Vec<&mut [f32]> =
            (0..output_channels).map(|_| &mut [] as &mut [f32]).collect();
        let mut output_buffer =
            create_planar_buffer_mut(&mut output, output_channels, frame_size, &mut output_slices);
        let render_result = renderer.render(&[(2, mono_view)], &mut output_buffer);

        expect_ok!(remove_result);
        expect_eq!(renderer.number_of_audio_elements(), 1);
        expect_eq!(renderer.element_channels(1), None);
        expect_eq!(renderer.element_channels(2).map(|c| c.get()), Some(1));
        expect_ok!(render_result);
        expect_that!(rms(output_buffer.channel_mut(0)), gt(NON_SILENT_RMS_THRESHOLD));
        expect_that!(rms(output_buffer.channel_mut(1)), gt(NON_SILENT_RMS_THRESHOLD));
    }

    /// Verifies that removing all binaural elements from a group and re-adding an element preserves
    /// the previously configured head rotation state.
    #[gtest]
    fn binaural_removing_all_elements_and_re_adding_preserves_head_rotation() {
        let frame_size: usize = 256;
        let sample_rate: u32 = 48000;
        let output_channels: usize = 2;
        let config = Config::new(
            OutputLayout::Binaural,
            Samples::new(frame_size as u32).unwrap(),
            SampleRate::new(sample_rate).unwrap(),
        )
        .unwrap();
        let mut renderer = RoarRenderer::create(&config).unwrap();
        let group_id = renderer.add_audio_group().unwrap();
        renderer.enable_head_tracking(true).unwrap();
        let obj_cfg = AudioElementConfig::ObjectBased(ObjectBasedConfig::new(1, None).unwrap());
        let pos =
            ObjectPosition::Polar(vec![PolarCoordinate::new_from_floats(45.0, 0.0, 1.0).unwrap()]);
        let rotation_90_yaw = Quaternion::new(
            std::f32::consts::FRAC_1_SQRT_2,
            0.0,
            std::f32::consts::FRAC_1_SQRT_2,
            0.0,
        )
        .unwrap();
        renderer.add_element(group_id, 1, &obj_cfg).unwrap();
        renderer.set_head_rotation(rotation_90_yaw).unwrap();
        renderer
            .update_element_positions(1, &pos, Samples::new(frame_size as u32).unwrap())
            .unwrap();
        let stimulus: Vec<f32> = (0..frame_size).map(|i| ((i as f32) * 0.1).sin() * 0.5).collect();
        let input_refs: [&[f32]; 1] = [stimulus.as_slice()];

        let in_view_a = create_planar_buffer_ref(&input_refs, frame_size);
        let mut out_a = vec![0.0f32; output_channels * frame_size];
        let mut slices_a: Vec<&mut [f32]> =
            (0..output_channels).map(|_| &mut [] as &mut [f32]).collect();
        let mut buf_a =
            create_planar_buffer_mut(&mut out_a, output_channels, frame_size, &mut slices_a);
        renderer.render(&[(1, in_view_a)], &mut buf_a).unwrap();

        renderer.remove_element(1).unwrap();
        assert_eq!(renderer.number_of_audio_elements(), 0);
        renderer.add_element(group_id, 2, &obj_cfg).unwrap();
        renderer
            .update_element_positions(2, &pos, Samples::new(frame_size as u32).unwrap())
            .unwrap();
        let in_view_b = create_planar_buffer_ref(&input_refs, frame_size);
        let mut out_b = vec![0.0f32; output_channels * frame_size];
        let mut slices_b: Vec<&mut [f32]> =
            (0..output_channels).map(|_| &mut [] as &mut [f32]).collect();
        let mut buf_b =
            create_planar_buffer_mut(&mut out_b, output_channels, frame_size, &mut slices_b);
        renderer.render(&[(2, in_view_b)], &mut buf_b).unwrap();

        expect_that!(rms(&out_a), gt(NON_SILENT_RMS_THRESHOLD));
        for (a, b) in out_a.iter().zip(out_b.iter()) {
            expect_that!(*b, near(*a, 1e-6));
        }
    }

    /// Verifies that an audio group containing both a world-locked stereo element and a head-locked
    /// object element renders the exact superposition of the rotated world-locked element and the
    /// unrotated head-locked element.
    #[gtest]
    fn binaural_mixed_world_and_head_locked_group_matches_superposition_under_rotation() {
        let frame_size: usize = 256;
        let sample_rate: u32 = 48000;
        let output_channels: usize = 2;
        let samples = Samples::new(frame_size as u32).unwrap();
        let config =
            Config::new(OutputLayout::Binaural, samples, SampleRate::new(sample_rate).unwrap())
                .unwrap();
        let world_locked_stereo_cfg = AudioElementConfig::ChannelBased(ChannelBasedConfig {
            layout: InputLayout::Stereo,
            downmix_info: None,
            rendering_config: Some(ElementRenderingConfig {
                headphones_rendering_mode: HeadphonesRenderingMode::WorldLocked,
                binaural_filter_profile: BinauralFilterProfile::Direct,
            }),
        });
        let head_locked_obj_cfg = AudioElementConfig::ObjectBased(
            ObjectBasedConfig::new(
                1,
                Some(ElementRenderingConfig {
                    headphones_rendering_mode: HeadphonesRenderingMode::HeadLocked,
                    binaural_filter_profile: BinauralFilterProfile::Direct,
                }),
            )
            .unwrap(),
        );

        let obj_pos =
            ObjectPosition::Polar(vec![PolarCoordinate::new_from_floats(-90.0, 0.0, 1.0).unwrap()]);
        let rotation_90_yaw = Quaternion::new(
            std::f32::consts::FRAC_1_SQRT_2,
            0.0,
            0.0,
            std::f32::consts::FRAC_1_SQRT_2,
        )
        .unwrap();
        let stereo_ch0: Vec<f32> =
            (0..frame_size).map(|i| ((i as f32) * 0.07).sin() * 0.4).collect();
        let stereo_ch1: Vec<f32> =
            (0..frame_size).map(|i| ((i as f32) * 0.11).cos() * 0.3).collect();
        let obj_ch0: Vec<f32> = (0..frame_size).map(|i| ((i as f32) * 0.17).sin() * 0.5).collect();
        let stereo_refs: [&[f32]; 2] = [stereo_ch0.as_slice(), stereo_ch1.as_slice()];
        let obj_refs: [&[f32]; 1] = [obj_ch0.as_slice()];

        let mut mixed_renderer = RoarRenderer::create(&config).unwrap();
        let mixed_group = mixed_renderer.add_audio_group().unwrap();
        mixed_renderer.enable_head_tracking(true).unwrap();
        mixed_renderer.add_element(mixed_group, 10, &world_locked_stereo_cfg).unwrap();
        mixed_renderer.add_element(mixed_group, 20, &head_locked_obj_cfg).unwrap();
        mixed_renderer.update_element_positions(20, &obj_pos, samples).unwrap();
        mixed_renderer.set_head_rotation(rotation_90_yaw).unwrap();
        let mut out_mixed = vec![0.0f32; output_channels * frame_size];
        let mut slices_mixed: Vec<&mut [f32]> =
            (0..output_channels).map(|_| &mut [] as &mut [f32]).collect();
        let mut buf_mixed = create_planar_buffer_mut(
            &mut out_mixed,
            output_channels,
            frame_size,
            &mut slices_mixed,
        );
        mixed_renderer
            .render(
                &[
                    (10, create_planar_buffer_ref(&stereo_refs, frame_size)),
                    (20, create_planar_buffer_ref(&obj_refs, frame_size)),
                ],
                &mut buf_mixed,
            )
            .unwrap();

        let mut wl_renderer = RoarRenderer::create(&config).unwrap();
        let wl_group = wl_renderer.add_audio_group().unwrap();
        wl_renderer.enable_head_tracking(true).unwrap();
        wl_renderer.add_element(wl_group, 10, &world_locked_stereo_cfg).unwrap();
        wl_renderer.set_head_rotation(rotation_90_yaw).unwrap();
        let mut out_wl = vec![0.0f32; output_channels * frame_size];
        let mut slices_wl: Vec<&mut [f32]> =
            (0..output_channels).map(|_| &mut [] as &mut [f32]).collect();
        let mut buf_wl =
            create_planar_buffer_mut(&mut out_wl, output_channels, frame_size, &mut slices_wl);
        wl_renderer
            .render(&[(10, create_planar_buffer_ref(&stereo_refs, frame_size))], &mut buf_wl)
            .unwrap();

        let mut hl_renderer = RoarRenderer::create(&config).unwrap();
        let hl_group = hl_renderer.add_audio_group().unwrap();
        hl_renderer.add_element(hl_group, 20, &head_locked_obj_cfg).unwrap();
        hl_renderer.update_element_positions(20, &obj_pos, samples).unwrap();
        let mut out_hl = vec![0.0f32; output_channels * frame_size];
        let mut slices_hl: Vec<&mut [f32]> =
            (0..output_channels).map(|_| &mut [] as &mut [f32]).collect();
        let mut buf_hl =
            create_planar_buffer_mut(&mut out_hl, output_channels, frame_size, &mut slices_hl);
        hl_renderer
            .render(&[(20, create_planar_buffer_ref(&obj_refs, frame_size))], &mut buf_hl)
            .unwrap();

        for i in 0..(output_channels * frame_size) {
            expect_that!(out_mixed[i], near(out_wl[i] + out_hl[i], 1e-5));
        }
    }
}
