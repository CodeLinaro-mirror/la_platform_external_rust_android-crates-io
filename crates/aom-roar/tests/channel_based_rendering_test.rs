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

/// Integration tests for channel-based audio rendering (loudspeaker layouts and downmixing).
#[cfg(test)]
mod test {

    use crate::test_helpers::{
        create_channel_based_config, create_channel_based_config_with_downmix,
        create_channel_based_config_with_downmix_and_headphones_mode, create_downmix_mode_metadata,
        create_flat_sine_input, create_object_based_config, create_planar_buffer_mut,
        create_planar_buffer_ref, create_sine_input_channels, rms, TestRoar,
    };
    use aom_roar::c_types::{roar_audio_block_t, roar_headphones_rendering_mode_t, roar_layout_t};
    use aom_roar::ffi::*;
    use aom_roar::{
        AudioElementConfig, BinauralFilterProfile, ChannelBasedConfig, Config, DownmixInfo,
        DownmixMode, ElementRenderingConfig, HeadphonesRenderingMode, InputLayout,
        ObjectBasedConfig, OutputLayout, RoarRenderer, SampleRate, Samples,
    };
    use googletest::prelude::*;

    /// Expect at least this value for non-silent output.
    const NON_SILENT_RMS_THRESHOLD: f32 = 0.1;

    // ===== FFI C API Tests =====

    /// Verifies channel-based rendering via the C API by rendering a 5.1 layout input to a Stereo
    /// layout output.
    #[gtest]
    fn ffi_rendering_51_input_to_stereo_output_succeeds() {
        let frame_size: usize = 256;
        let sample_rate: u32 = 48000;
        let input_channels: usize = 6;
        let output_channels: usize = 2;

        // Create test ROAR instance.
        let oar =
            TestRoar::new(roar_layout_t::ck_roar_layout_stereo, frame_size as u32, sample_rate);
        // Create audio group and audio element.
        let element_cfg = create_channel_based_config(roar_layout_t::ck_roar_layout_51);
        // SAFETY: `oar.ptr()` is a valid pointer initialized by `TestRoar::new`.
        let group_id = unsafe { roar_add_audio_group(oar.ptr()) };
        assert!(group_id >= 0);
        let element_id = 1u32;
        // SAFETY: `oar.ptr()` is valid, and `&element_cfg` is a valid temporary reference.
        let ret =
            unsafe { roar_add_audio_element(oar.ptr(), group_id as u32, element_id, &element_cfg) };
        assert_eq!(ret, 0);

        // SAFETY: `oar.ptr()` is valid.
        let actual_input_channels =
            unsafe { roar_get_number_of_audio_element_channels(oar.ptr(), element_id) };
        assert_eq!(actual_input_channels, input_channels as u32);
        // SAFETY: `oar.ptr()` is valid.
        let actual_output_channels = unsafe { roar_get_number_of_output_channels(oar.ptr()) };
        assert_eq!(actual_output_channels, output_channels as u32);

        // Create input buffer, fill with sine wave, create input audio block struct.
        let amplitude: f32 = 1.0;
        let freq: f32 = 440.0;
        let mut input_buffer =
            create_flat_sine_input(input_channels, frame_size, amplitude, freq, sample_rate as f32);
        let mut input_data = roar_audio_block_t {
            data: input_buffer.as_mut_ptr(),
            channels: input_channels as u32,
            samples_per_channel: frame_size as u32,
        };
        // Create output audio block.
        let mut output_buffer = vec![0.0f32; output_channels * frame_size];
        let mut output_data = roar_audio_block_t {
            data: output_buffer.as_mut_ptr(),
            channels: output_channels as u32,
            samples_per_channel: frame_size as u32,
        };

        // SAFETY: `oar.ptr()` is valid; `input_data` buffer pointer is valid for this call.
        let update_ret =
            unsafe { roar_update_audio_element_data(oar.ptr(), element_id, &mut input_data) };
        // SAFETY: `oar.ptr()` is valid; `output_data` buffer pointer is valid for this call.
        let render_ret = unsafe { roar_render(oar.ptr(), &mut output_data) };

        assert_eq!(update_ret, 0);
        assert_eq!(render_ret, 0);
        // Expect non-zero output in both channels.
        expect_that!(rms(&output_buffer[..frame_size]), gt(NON_SILENT_RMS_THRESHOLD));
        expect_that!(rms(&output_buffer[frame_size..]), gt(NON_SILENT_RMS_THRESHOLD));
    }

    /// Verifies that rendering a Stereo element to Stereo output using the C FFI API acts as
    /// an identity pass-through (no downmixing or spatialization).
    #[gtest]
    fn ffi_rendering_stereo_input_to_stereo_output_acts_as_identity() {
        let frame_size: usize = 128;
        let sample_rate: u32 = 48000;
        let channels: usize = 2;

        // Create test ROAR instance.
        let oar =
            TestRoar::new(roar_layout_t::ck_roar_layout_stereo, frame_size as u32, sample_rate);
        // Create audio group and audio element.
        let element_cfg = create_channel_based_config(roar_layout_t::ck_roar_layout_stereo);
        // SAFETY: `oar.ptr()` is a valid pointer initialized by `TestRoar::new`.
        let group_id = unsafe { roar_add_audio_group(oar.ptr()) };
        assert!(group_id >= 0);
        let element_id = 10u32;
        // SAFETY: `oar.ptr()` is valid, and `&element_cfg` is a valid temporary reference.
        let ret =
            unsafe { roar_add_audio_element(oar.ptr(), group_id as u32, element_id, &element_cfg) };
        assert_eq!(ret, 0);
        // Create input buffer, fill with constant values, create input audio block struct.
        let mut input_buffer = vec![0.0f32; channels * frame_size];
        // Fill left and right with distinct, constant values.
        let left_fill_val: f32 = 0.5;
        let right_fill_val: f32 = 0.75;
        input_buffer[..frame_size].fill(left_fill_val);
        input_buffer[frame_size..].fill(right_fill_val);
        let mut input_data = roar_audio_block_t {
            data: input_buffer.as_mut_ptr(),
            channels: channels as u32,
            samples_per_channel: frame_size as u32,
        };
        // Create output audio block.
        let mut output_buffer = vec![0.0f32; channels * frame_size];
        let mut output_data = roar_audio_block_t {
            data: output_buffer.as_mut_ptr(),
            channels: channels as u32,
            samples_per_channel: frame_size as u32,
        };

        // SAFETY: `oar.ptr()` is valid; `input_data` buffer pointer is valid for this call.
        let update_ret =
            unsafe { roar_update_audio_element_data(oar.ptr(), element_id, &mut input_data) };
        // SAFETY: `oar.ptr()` is valid; `output_data` buffer pointer is valid for this call.
        let render_ret = unsafe { roar_render(oar.ptr(), &mut output_data) };

        assert_eq!(update_ret, 0);
        assert_eq!(render_ret, 0);
        // Verify planar output: L is first frame_size, R is next frame_size.
        expect_that!(output_buffer[..frame_size], each(eq(&left_fill_val)));
        expect_that!(output_buffer[frame_size..], each(eq(&right_fill_val)));
    }

    /// Verifies that IAMF downmix mode updates on an EAR fallback path (`5.1.2 -> stereo` with
    /// `downmix_info`) return `0` as an advisory no-op and produce bit-exact identical output to
    /// rendering without downmix mode updates.
    #[gtest]
    fn ffi_downmix_mode_update_on_ear_fallback_512_to_stereo_is_noop_and_preserves_output() {
        let frame_size: usize = 64;
        let sample_rate: u32 = 48000;
        let input_channels: usize = 8;
        let output_channels: usize = 2;
        let element_id: u32 = 1;
        let baseline_oar =
            TestRoar::new(roar_layout_t::ck_roar_layout_stereo, frame_size as u32, sample_rate);
        let updated_oar =
            TestRoar::new(roar_layout_t::ck_roar_layout_stereo, frame_size as u32, sample_rate);
        let element_cfg =
            create_channel_based_config_with_downmix(roar_layout_t::ck_roar_layout_512, 0);
        let baseline_gid = unsafe { roar_add_audio_group(baseline_oar.ptr()) };
        let updated_gid = unsafe { roar_add_audio_group(updated_oar.ptr()) };
        assert!(baseline_gid >= 0);
        assert!(updated_gid >= 0);
        assert_eq!(
            unsafe {
                roar_add_audio_element(
                    baseline_oar.ptr(),
                    baseline_gid as u32,
                    element_id,
                    &element_cfg,
                )
            },
            0
        );
        assert_eq!(
            unsafe {
                roar_add_audio_element(
                    updated_oar.ptr(),
                    updated_gid as u32,
                    element_id,
                    &element_cfg,
                )
            },
            0
        );
        let mut baseline_input =
            create_flat_sine_input(input_channels, frame_size, 0.8, 330.0, sample_rate as f32);
        let mut updated_input = baseline_input.clone();
        let mut baseline_input_block = roar_audio_block_t {
            data: baseline_input.as_mut_ptr(),
            channels: input_channels as u32,
            samples_per_channel: frame_size as u32,
        };
        let mut updated_input_block = roar_audio_block_t {
            data: updated_input.as_mut_ptr(),
            channels: input_channels as u32,
            samples_per_channel: frame_size as u32,
        };
        let mut baseline_output = vec![0.0f32; output_channels * frame_size];
        let mut updated_output = vec![0.0f32; output_channels * frame_size];
        let mut baseline_output_block = roar_audio_block_t {
            data: baseline_output.as_mut_ptr(),
            channels: output_channels as u32,
            samples_per_channel: frame_size as u32,
        };
        let mut updated_output_block = roar_audio_block_t {
            data: updated_output.as_mut_ptr(),
            channels: output_channels as u32,
            samples_per_channel: frame_size as u32,
        };
        let dmix_meta = create_downmix_mode_metadata(1, frame_size as i32);

        let first_meta_ret = unsafe {
            roar_update_audio_element_metadata(updated_oar.ptr(), element_id, &dmix_meta)
        };
        let second_meta_ret = unsafe {
            roar_update_audio_element_metadata(updated_oar.ptr(), element_id, &dmix_meta)
        };
        let baseline_data_ret = unsafe {
            roar_update_audio_element_data(
                baseline_oar.ptr(),
                element_id,
                &mut baseline_input_block,
            )
        };
        let updated_data_ret = unsafe {
            roar_update_audio_element_data(updated_oar.ptr(), element_id, &mut updated_input_block)
        };
        let baseline_render_ret =
            unsafe { roar_render(baseline_oar.ptr(), &mut baseline_output_block) };
        let updated_render_ret =
            unsafe { roar_render(updated_oar.ptr(), &mut updated_output_block) };

        expect_that!(first_meta_ret, eq(0));
        expect_that!(second_meta_ret, eq(0));
        expect_that!(baseline_data_ret, eq(0));
        expect_that!(updated_data_ret, eq(0));
        expect_that!(baseline_render_ret, eq(0));
        expect_that!(updated_render_ret, eq(0));
        expect_that!(updated_output, eq(&baseline_output));
    }

    /// Verifies that an IAMF downmix mode update when input and output layouts match
    /// (`5.1.2 -> 5.1.2`, handled by EAR pass-through) returns `0` and renders identity output.
    #[gtest]
    fn ffi_downmix_mode_update_on_matching_512_layout_is_noop_and_renders_audio() {
        let frame_size: usize = 32;
        let sample_rate: u32 = 48000;
        let channels: usize = 8;
        let element_id: u32 = 1;
        let oar = TestRoar::new(roar_layout_t::ck_roar_layout_512, frame_size as u32, sample_rate);
        let element_cfg =
            create_channel_based_config_with_downmix(roar_layout_t::ck_roar_layout_512, 0);
        let group_id = unsafe { roar_add_audio_group(oar.ptr()) };
        assert!(group_id >= 0);
        assert_eq!(
            unsafe { roar_add_audio_element(oar.ptr(), group_id as u32, element_id, &element_cfg) },
            0
        );
        let mut input_buffer = vec![0.5f32; channels * frame_size];
        let mut input_block = roar_audio_block_t {
            data: input_buffer.as_mut_ptr(),
            channels: channels as u32,
            samples_per_channel: frame_size as u32,
        };
        let mut output_buffer = vec![0.0f32; channels * frame_size];
        let mut output_block = roar_audio_block_t {
            data: output_buffer.as_mut_ptr(),
            channels: channels as u32,
            samples_per_channel: frame_size as u32,
        };
        let dmix_meta = create_downmix_mode_metadata(2, frame_size as i32);

        let meta_ret =
            unsafe { roar_update_audio_element_metadata(oar.ptr(), element_id, &dmix_meta) };
        let data_ret =
            unsafe { roar_update_audio_element_data(oar.ptr(), element_id, &mut input_block) };
        let render_ret = unsafe { roar_render(oar.ptr(), &mut output_block) };

        expect_that!(meta_ret, eq(0));
        expect_that!(data_ret, eq(0));
        expect_that!(render_ret, eq(0));
        expect_that!(output_buffer, each(eq(&0.5f32)));
    }

    /// Verifies that updating IAMF downmix mode from `0` (`delta = 0.707`) to `2`
    /// (`delta = 0.866`) on an active `5.1 -> stereo` downmix path takes effect in rendered audio.
    #[gtest]
    fn ffi_downmix_mode_update_on_51_to_stereo_changes_rendered_output() {
        let frame_size: usize = 32;
        let sample_rate: u32 = 48000;
        let input_channels: usize = 6;
        let output_channels: usize = 2;
        let element_id: u32 = 1;
        let oar =
            TestRoar::new(roar_layout_t::ck_roar_layout_stereo, frame_size as u32, sample_rate);
        let element_cfg =
            create_channel_based_config_with_downmix(roar_layout_t::ck_roar_layout_51, 0);
        let group_id = unsafe { roar_add_audio_group(oar.ptr()) };
        assert!(group_id >= 0);
        assert_eq!(
            unsafe { roar_add_audio_element(oar.ptr(), group_id as u32, element_id, &element_cfg) },
            0
        );
        // Channel 4 in 5.1 is side-left surround (`Sl5`), mixed into left stereo with weight
        // `delta`.
        let mut input_buffer = vec![0.0f32; input_channels * frame_size];
        input_buffer[4 * frame_size..5 * frame_size].fill(1.0);
        let mut input_block = roar_audio_block_t {
            data: input_buffer.as_mut_ptr(),
            channels: input_channels as u32,
            samples_per_channel: frame_size as u32,
        };
        let mut output_buffer = vec![0.0f32; output_channels * frame_size];
        let mut output_block = roar_audio_block_t {
            data: output_buffer.as_mut_ptr(),
            channels: output_channels as u32,
            samples_per_channel: frame_size as u32,
        };
        let dmix_meta = create_downmix_mode_metadata(2, frame_size as i32);

        let meta_ret =
            unsafe { roar_update_audio_element_metadata(oar.ptr(), element_id, &dmix_meta) };
        let data_ret =
            unsafe { roar_update_audio_element_data(oar.ptr(), element_id, &mut input_block) };
        let render_ret = unsafe { roar_render(oar.ptr(), &mut output_block) };

        expect_that!(meta_ret, eq(0));
        expect_that!(data_ret, eq(0));
        expect_that!(render_ret, eq(0));
        expect_that!(output_buffer[..frame_size], each(near(0.866f32, 1e-4)));
        expect_that!(output_buffer[frame_size..], each(near(0.0f32, 1e-6)));
    }

    /// Verifies that an IAMF downmix mode update on the binaural multi-element OBR path is
    /// accepted and ignored (`0`).
    #[gtest]
    fn ffi_downmix_mode_update_on_binaural_obr_element_returns_ok() {
        let oar = TestRoar::new(roar_layout_t::ck_roar_layout_binaural, 256, 48000);
        let group_id = unsafe { roar_add_audio_group(oar.ptr()) };
        assert!(group_id >= 0);
        let element_cfg = create_object_based_config(1);
        assert_eq!(
            unsafe { roar_add_audio_element(oar.ptr(), group_id as u32, 1, &element_cfg) },
            0
        );
        let dmix_meta = create_downmix_mode_metadata(1, 256);

        let meta_ret = unsafe { roar_update_audio_element_metadata(oar.ptr(), 1, &dmix_meta) };

        expect_that!(meta_ret, eq(0));
    }

    /// Verifies that a `ck_roar_world_locked_restricted` `5.1` channel element with `downmix_info`
    /// on binaural output uses the Downmix renderer and applies dynamic downmix mode updates.
    #[gtest]
    fn ffi_downmix_mode_update_on_binaural_world_locked_restricted_51_applies_mode() {
        let frame_size: usize = 32;
        let sample_rate: u32 = 48000;
        let input_channels: usize = 6;
        let output_channels: usize = 2;
        let element_id: u32 = 1;
        let oar =
            TestRoar::new(roar_layout_t::ck_roar_layout_binaural, frame_size as u32, sample_rate);
        let element_cfg = create_channel_based_config_with_downmix_and_headphones_mode(
            roar_layout_t::ck_roar_layout_51,
            0,
            roar_headphones_rendering_mode_t::ck_roar_world_locked_restricted,
        );
        let group_id = unsafe { roar_add_audio_group(oar.ptr()) };
        assert!(group_id >= 0);
        assert_eq!(
            unsafe { roar_add_audio_element(oar.ptr(), group_id as u32, element_id, &element_cfg) },
            0
        );
        let mut input_buffer = vec![0.0f32; input_channels * frame_size];
        input_buffer[4 * frame_size..5 * frame_size].fill(1.0);
        let mut input_block = roar_audio_block_t {
            data: input_buffer.as_mut_ptr(),
            channels: input_channels as u32,
            samples_per_channel: frame_size as u32,
        };
        let mut output_buffer = vec![0.0f32; output_channels * frame_size];
        let mut output_block = roar_audio_block_t {
            data: output_buffer.as_mut_ptr(),
            channels: output_channels as u32,
            samples_per_channel: frame_size as u32,
        };
        let dmix_meta = create_downmix_mode_metadata(2, frame_size as i32);

        let meta_ret =
            unsafe { roar_update_audio_element_metadata(oar.ptr(), element_id, &dmix_meta) };
        let data_ret =
            unsafe { roar_update_audio_element_data(oar.ptr(), element_id, &mut input_block) };
        let render_ret = unsafe { roar_render(oar.ptr(), &mut output_block) };

        expect_that!(meta_ret, eq(0));
        expect_that!(data_ret, eq(0));
        expect_that!(render_ret, eq(0));
        expect_that!(output_buffer[..frame_size], each(near(0.866f32, 1e-4)));
        expect_that!(output_buffer[frame_size..], each(near(0.0f32, 1e-6)));
    }

    // ===== Rust API Tests =====

    /// Verifies channel-based rendering via the Rust API by rendering a 5.1 layout input
    /// to a Stereo layout output.
    #[gtest]
    fn rust_rendering_51_input_to_stereo_output_succeeds() {
        let frame_size: usize = 256;
        let sample_rate: u32 = 48000;
        let input_channels: usize = 6;
        let output_channels: usize = 2;

        // Create test ROAR instance and renderer.
        let config = Config::new(
            OutputLayout::Stereo,
            Samples::new(frame_size as u32).unwrap(),
            SampleRate::new(sample_rate).unwrap(),
        )
        .unwrap();
        let mut renderer = RoarRenderer::create(&config).unwrap();
        // Create audio group and audio element.
        let group_id = renderer.add_audio_group().unwrap();
        let element_cfg = AudioElementConfig::ChannelBased(ChannelBasedConfig {
            layout: InputLayout::Layout51,
            downmix_info: None,
            rendering_config: None,
        });
        let element_id = 10u32;
        renderer.add_element(group_id, element_id, &element_cfg).unwrap();
        // Create input buffer, fill with sine wave.
        let amplitude: f32 = 1.0;
        let freq: f32 = 440.0;
        let input_data = create_sine_input_channels(
            input_channels,
            frame_size,
            amplitude,
            freq,
            sample_rate as f32,
        );
        let input_refs: Vec<&[f32]> = input_data.iter().map(|v| v.as_slice()).collect();
        let input_buffer_ref = create_planar_buffer_ref(&input_refs[..], frame_size);
        // Create output buffer.
        let mut output = vec![0.0; output_channels * frame_size];
        let mut output_slices: Vec<&mut [f32]> =
            (0..output_channels).map(|_| &mut [] as &mut [f32]).collect();
        let mut output_buffer =
            create_planar_buffer_mut(&mut output, output_channels, frame_size, &mut output_slices);

        let result = renderer.render(&[(element_id, input_buffer_ref)], &mut output_buffer);

        expect_ok!(result);
        expect_that!(rms(output_buffer.channel_mut(0)), gt(NON_SILENT_RMS_THRESHOLD));
        expect_that!(rms(output_buffer.channel_mut(1)), gt(NON_SILENT_RMS_THRESHOLD));
    }

    /// Verifies that rendering a Stereo element to Stereo output using the Rust API acts as
    /// an identity pass-through (no downmixing or spatialization).
    #[gtest]
    fn rust_rendering_stereo_input_to_stereo_output_acts_as_identity() {
        let frame_size: usize = 128;
        let sample_rate: u32 = 48000;
        let channels: usize = 2;
        let element_id: u32 = 10;

        // Create test ROAR instance and renderer.
        let config = Config::new(
            OutputLayout::Stereo,
            Samples::new(frame_size as u32).unwrap(),
            SampleRate::new(sample_rate).unwrap(),
        )
        .unwrap();
        let mut renderer = RoarRenderer::create(&config).unwrap();
        // Create audio group and audio element.
        let group_id = renderer.add_audio_group().unwrap();
        let element_cfg = AudioElementConfig::ChannelBased(ChannelBasedConfig {
            layout: InputLayout::Stereo,
            downmix_info: None,
            rendering_config: None,
        });
        renderer.add_element(group_id, element_id, &element_cfg).unwrap();
        // Create input buffer, fill with constant values.
        let left_fill_val: f32 = 0.5;
        let right_fill_val: f32 = 0.75;
        let inputs = [vec![left_fill_val; frame_size], vec![right_fill_val; frame_size]];
        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let input_buffer_ref = create_planar_buffer_ref(&input_refs[..], frame_size);
        // Create output buffer.
        let mut output = vec![0.0; channels * frame_size];
        let mut output_slices: Vec<&mut [f32]> =
            (0..channels).map(|_| &mut [] as &mut [f32]).collect();
        let mut output_buffer =
            create_planar_buffer_mut(&mut output, channels, frame_size, &mut output_slices);

        let result = renderer.render(&[(element_id, input_buffer_ref)], &mut output_buffer);

        expect_ok!(result);
        expect_that!(*output_buffer.channel_mut(0), each(eq(&left_fill_val)));
        expect_that!(*output_buffer.channel_mut(1), each(eq(&right_fill_val)));
    }

    /// Verifies that `RoarRenderer::update_element_downmix_mode` returns `Ok(())` on an EAR
    /// fallback path (`5.1.2 -> stereo` with `downmix_info`) and leaves rendered audio unchanged.
    #[gtest]
    fn rust_downmix_mode_update_on_ear_fallback_512_to_stereo_is_noop_and_preserves_output() {
        let frame_size: usize = 64;
        let sample_rate: u32 = 48000;
        let input_channels: usize = 8;
        let output_channels: usize = 2;
        let element_id: u32 = 1;
        let config = Config::new(
            OutputLayout::Stereo,
            Samples::new(frame_size as u32).unwrap(),
            SampleRate::new(sample_rate).unwrap(),
        )
        .unwrap();
        let mut baseline_renderer = RoarRenderer::create(&config).unwrap();
        let mut updated_renderer = RoarRenderer::create(&config).unwrap();
        let baseline_gid = baseline_renderer.add_audio_group().unwrap();
        let updated_gid = updated_renderer.add_audio_group().unwrap();
        let element_cfg = AudioElementConfig::ChannelBased(ChannelBasedConfig {
            layout: InputLayout::Layout512,
            downmix_info: Some(DownmixInfo::new(DownmixMode::Mode1NegOffset, None)),
            rendering_config: None,
        });
        baseline_renderer.add_element(baseline_gid, element_id, &element_cfg).unwrap();
        updated_renderer.add_element(updated_gid, element_id, &element_cfg).unwrap();
        let input_data =
            create_sine_input_channels(input_channels, frame_size, 0.8, 330.0, sample_rate as f32);
        let input_refs: Vec<&[f32]> = input_data.iter().map(|v| v.as_slice()).collect();
        let baseline_input_ref = create_planar_buffer_ref(&input_refs[..], frame_size);
        let updated_input_ref = create_planar_buffer_ref(&input_refs[..], frame_size);
        let mut baseline_output = vec![0.0f32; output_channels * frame_size];
        let mut baseline_slices: Vec<&mut [f32]> =
            (0..output_channels).map(|_| &mut [] as &mut [f32]).collect();
        let mut baseline_out_buf = create_planar_buffer_mut(
            &mut baseline_output,
            output_channels,
            frame_size,
            &mut baseline_slices,
        );
        let mut updated_output = vec![0.0f32; output_channels * frame_size];
        let mut updated_slices: Vec<&mut [f32]> =
            (0..output_channels).map(|_| &mut [] as &mut [f32]).collect();
        let mut updated_out_buf = create_planar_buffer_mut(
            &mut updated_output,
            output_channels,
            frame_size,
            &mut updated_slices,
        );

        let first_update = updated_renderer.update_element_downmix_mode(
            element_id,
            DownmixMode::Mode2NegOffset,
            Some(Samples::new(frame_size as u32).unwrap()),
        );
        let second_update = updated_renderer.update_element_downmix_mode(
            element_id,
            DownmixMode::Mode3NegOffset,
            None,
        );
        let baseline_render =
            baseline_renderer.render(&[(element_id, baseline_input_ref)], &mut baseline_out_buf);
        let updated_render =
            updated_renderer.render(&[(element_id, updated_input_ref)], &mut updated_out_buf);

        expect_ok!(first_update);
        expect_ok!(second_update);
        expect_ok!(baseline_render);
        expect_ok!(updated_render);
        expect_that!(updated_output, eq(&baseline_output));
    }

    /// Verifies that `RoarRenderer::update_element_downmix_mode` returns `Ok(())` when input and
    /// output layouts match (`5.1.2 -> 5.1.2`, handled by EAR pass-through) and renders identity
    /// output.
    #[gtest]
    fn rust_downmix_mode_update_on_matching_512_layout_is_noop_and_renders_audio() {
        let frame_size: usize = 32;
        let sample_rate: u32 = 48000;
        let channels: usize = 8;
        let element_id: u32 = 1;
        let config = Config::new(
            OutputLayout::Layout512,
            Samples::new(frame_size as u32).unwrap(),
            SampleRate::new(sample_rate).unwrap(),
        )
        .unwrap();
        let mut renderer = RoarRenderer::create(&config).unwrap();
        let group_id = renderer.add_audio_group().unwrap();
        let element_cfg = AudioElementConfig::ChannelBased(ChannelBasedConfig {
            layout: InputLayout::Layout512,
            downmix_info: Some(DownmixInfo::new(DownmixMode::Mode1NegOffset, None)),
            rendering_config: None,
        });
        renderer.add_element(group_id, element_id, &element_cfg).unwrap();
        let inputs = vec![vec![0.5f32; frame_size]; channels];
        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let input_buffer_ref = create_planar_buffer_ref(&input_refs[..], frame_size);
        let mut output = vec![0.0f32; channels * frame_size];
        let mut output_slices: Vec<&mut [f32]> =
            (0..channels).map(|_| &mut [] as &mut [f32]).collect();
        let mut output_buffer =
            create_planar_buffer_mut(&mut output, channels, frame_size, &mut output_slices);

        let update_result = renderer.update_element_downmix_mode(
            element_id,
            DownmixMode::Mode3NegOffset,
            Some(Samples::new(frame_size as u32).unwrap()),
        );
        let render_result = renderer.render(&[(element_id, input_buffer_ref)], &mut output_buffer);

        expect_ok!(update_result);
        expect_ok!(render_result);
        expect_that!(output, each(eq(&0.5f32)));
    }

    /// Verifies that `RoarRenderer::update_element_downmix_mode` applies `Mode3NegOffset`
    /// (`delta = 0.866`) on an active `5.1 -> stereo` downmix path.
    #[gtest]
    fn rust_downmix_mode_update_on_51_to_stereo_changes_rendered_output() {
        let frame_size: usize = 32;
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
            downmix_info: Some(DownmixInfo::new(DownmixMode::Mode1NegOffset, None)),
            rendering_config: None,
        });
        renderer.add_element(group_id, element_id, &element_cfg).unwrap();
        let mut inputs = vec![vec![0.0f32; frame_size]; input_channels];
        inputs[4].fill(1.0);
        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let input_buffer_ref = create_planar_buffer_ref(&input_refs[..], frame_size);
        let mut output = vec![0.0f32; output_channels * frame_size];
        let mut output_slices: Vec<&mut [f32]> =
            (0..output_channels).map(|_| &mut [] as &mut [f32]).collect();
        let mut output_buffer =
            create_planar_buffer_mut(&mut output, output_channels, frame_size, &mut output_slices);

        let update_result = renderer.update_element_downmix_mode(
            element_id,
            DownmixMode::Mode3NegOffset,
            Some(Samples::new(frame_size as u32).unwrap()),
        );
        let render_result = renderer.render(&[(element_id, input_buffer_ref)], &mut output_buffer);

        expect_ok!(update_result);
        expect_ok!(render_result);
        expect_that!(*output_buffer.channel_mut(0), each(near(0.866f32, 1e-4)));
        expect_that!(*output_buffer.channel_mut(1), each(near(0.0f32, 1e-6)));
    }

    /// Verifies that `RoarRenderer::update_element_downmix_mode` returns `Ok(())` as an advisory
    /// no-op on the binaural multi-element OBR path.
    #[gtest]
    fn rust_downmix_mode_update_on_binaural_obr_element_returns_ok() {
        let config = Config::new(
            OutputLayout::Binaural,
            Samples::new(256).unwrap(),
            SampleRate::new(48000).unwrap(),
        )
        .unwrap();
        let mut renderer = RoarRenderer::create(&config).unwrap();
        let group_id = renderer.add_audio_group().unwrap();
        let element_cfg = AudioElementConfig::ObjectBased(ObjectBasedConfig::new(1, None).unwrap());
        renderer.add_element(group_id, 1, &element_cfg).unwrap();

        let update_result = renderer.update_element_downmix_mode(
            1,
            DownmixMode::Mode2NegOffset,
            Some(Samples::new(256).unwrap()),
        );

        expect_ok!(update_result);
    }

    /// Verifies that `RoarRenderer::update_element_downmix_mode` takes effect on binaural output
    /// when a `5.1` channel element is configured as `WorldLockedRestricted` with `DownmixInfo`.
    #[gtest]
    fn rust_downmix_mode_update_on_binaural_world_locked_restricted_51_applies_mode() {
        let frame_size: usize = 32;
        let sample_rate: u32 = 48000;
        let input_channels: usize = 6;
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
        let element_cfg = AudioElementConfig::ChannelBased(ChannelBasedConfig {
            layout: InputLayout::Layout51,
            downmix_info: Some(DownmixInfo::new(DownmixMode::Mode1NegOffset, None)),
            rendering_config: Some(ElementRenderingConfig {
                headphones_rendering_mode: HeadphonesRenderingMode::WorldLockedRestricted,
                binaural_filter_profile: BinauralFilterProfile::Ambient,
            }),
        });
        renderer.add_element(group_id, element_id, &element_cfg).unwrap();
        let mut inputs = vec![vec![0.0f32; frame_size]; input_channels];
        inputs[4].fill(1.0);
        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let input_buffer_ref = create_planar_buffer_ref(&input_refs[..], frame_size);
        let mut output = vec![0.0f32; output_channels * frame_size];
        let mut output_slices: Vec<&mut [f32]> =
            (0..output_channels).map(|_| &mut [] as &mut [f32]).collect();
        let mut output_buffer =
            create_planar_buffer_mut(&mut output, output_channels, frame_size, &mut output_slices);

        let update_result = renderer.update_element_downmix_mode(
            element_id,
            DownmixMode::Mode3NegOffset,
            Some(Samples::new(frame_size as u32).unwrap()),
        );
        let render_result = renderer.render(&[(element_id, input_buffer_ref)], &mut output_buffer);

        expect_ok!(update_result);
        expect_ok!(render_result);
        expect_that!(*output_buffer.channel_mut(0), each(near(0.866f32, 1e-4)));
        expect_that!(*output_buffer.channel_mut(1), each(near(0.0f32, 1e-6)));
    }
}
