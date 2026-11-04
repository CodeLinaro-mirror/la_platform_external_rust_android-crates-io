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

/// Integration tests for the look-ahead peak limiter.
#[cfg(test)]
mod test {

    use crate::test_helpers::{
        create_channel_based_config, create_planar_buffer_mut, create_planar_buffer_ref, TestRoar,
    };
    use aom_roar::{
        c_types::{roar_audio_block_t, roar_layout_t},
        ffi::{
            roar_add_audio_element, roar_add_audio_group, roar_enable_limiter, roar_flush,
            roar_get_limiter_delay, roar_render, roar_set_limiter_threshold,
            roar_update_audio_element_data,
        },
        AudioElementConfig, ChannelBasedConfig, Config, Decibels, InputLayout, OarError,
        OutputLayout, RoarRenderer, SampleRate, Samples,
    };
    use googletest::prelude::*;

    const LIMITER_VALUE: f32 = 1.0;
    const VALUE_GREATER_THAN_LIMIT: f32 = 2.0;
    const VALUE_UNDER_LIMITER_VALUE: f32 = 0.75;
    // 0.005 s * 48000 Hz = 240 samples.
    const EXPECTED_DELAY_SAMPLES: usize = 240;

    fn get_max_abs(buffer: &[f32]) -> f32 {
        buffer.iter().map(|x| x.abs()).fold(0.0, f32::max)
    }

    // ===== C FFI API Tests =====

    /// Verifies that the peak limiter, when enabled via the C FFI API, keeps the output
    /// amplitude <= limiter value even when fed with high-amplitude inputs, and that it passes the
    /// unattenuated signal when disabled.
    #[gtest]
    fn ffi_limiter_enabled_prevents_output_clipping() -> Result<()> {
        let frame_size: usize = 128;
        let sample_rate: u32 = 48000;
        let input_channels: usize = 2;
        let output_channels: usize = 2;
        let element_id: u32 = 10;
        // Create test ROAR instance.
        let oar =
            TestRoar::new(roar_layout_t::ck_roar_layout_stereo, frame_size as u32, sample_rate);
        // SAFETY: `oar.ptr()` is a live, valid renderer instance created above with exclusive
        // access.
        let group_id = unsafe { roar_add_audio_group(oar.ptr()) };
        verify_that!(group_id, ge(0))?;
        let element_cfg = create_channel_based_config(roar_layout_t::ck_roar_layout_stereo);
        // SAFETY: `oar.ptr()` is a live, valid renderer instance created above with exclusive
        // access, and `element_cfg` is a valid initialized `roar_audio_element_config_t`.
        let ret =
            unsafe { roar_add_audio_element(oar.ptr(), group_id as u32, element_id, &element_cfg) };
        verify_eq!(ret, 0)?;
        // Very loud input.
        let mut input_buffer = vec![VALUE_GREATER_THAN_LIMIT; input_channels * frame_size];
        let mut input_data = roar_audio_block_t {
            data: input_buffer.as_mut_ptr(),
            channels: input_channels as u32,
            samples_per_channel: frame_size as u32,
        };
        let mut output_no_limiter = vec![0.0f32; output_channels * frame_size];
        let mut out_block_1 = roar_audio_block_t {
            data: output_no_limiter.as_mut_ptr(),
            channels: output_channels as u32,
            samples_per_channel: frame_size as u32,
        };
        let mut output_with_limiter = vec![0.0f32; output_channels * frame_size];
        let mut out_block_2 = roar_audio_block_t {
            data: output_with_limiter.as_mut_ptr(),
            channels: output_channels as u32,
            samples_per_channel: frame_size as u32,
        };

        // 1. Render with limiter DISABLED
        // SAFETY: `oar.ptr()` is a live, valid renderer instance created above with exclusive
        // access.
        let disable_ret = unsafe { roar_enable_limiter(oar.ptr(), 0) };
        verify_eq!(disable_ret, 0)?;
        // SAFETY: `oar.ptr()` is a live, valid renderer instance created above, and
        // `input_data.data` points to `input_buffer` with `input_channels * frame_size`
        // initialized, aligned `f32` elements, both exclusively owned during this call.
        let update_ret_1 =
            unsafe { roar_update_audio_element_data(oar.ptr(), element_id, &mut input_data) };
        verify_eq!(update_ret_1, 0)?;
        // SAFETY: `oar.ptr()` is a live, valid renderer instance created above, and
        // `out_block_1.data` points to `output_no_limiter` with capacity for
        // `output_channels * frame_size` aligned `f32` elements, both exclusively owned during
        // this call.
        let render_ret_1 = unsafe { roar_render(oar.ptr(), &mut out_block_1) };
        verify_eq!(render_ret_1, 0)?;

        // 2. Render with limiter ENABLED
        // SAFETY: `oar.ptr()` is a live, valid renderer instance created above with exclusive
        // access.
        let enable_ret = unsafe { roar_enable_limiter(oar.ptr(), 1) };
        verify_eq!(enable_ret, 0)?;
        // Render a few blocks to let the limiter stabilize.
        for _ in 0..10 {
            // SAFETY: `oar.ptr()` is a live, valid renderer instance created above, and
            // `input_data.data` points to `input_buffer` with `input_channels * frame_size`
            // initialized, aligned `f32` elements, both exclusively owned during this call.
            let update_ret =
                unsafe { roar_update_audio_element_data(oar.ptr(), element_id, &mut input_data) };
            verify_eq!(update_ret, 0)?;
            // SAFETY: `oar.ptr()` is a live, valid renderer instance created above, and
            // `out_block_2.data` points to `output_with_limiter` with capacity for
            // `output_channels * frame_size` aligned `f32` elements, both exclusively owned during
            // this call.
            let render_ret = unsafe { roar_render(oar.ptr(), &mut out_block_2) };
            verify_eq!(render_ret, 0)?;
        }

        let max_no_limiter = get_max_abs(&output_no_limiter);
        expect_gt!(max_no_limiter, LIMITER_VALUE);
        let max_with_limiter = get_max_abs(&output_with_limiter);
        expect_le!(max_with_limiter, LIMITER_VALUE);
        expect_gt!(max_with_limiter, VALUE_UNDER_LIMITER_VALUE);
        Ok(())
    }

    #[gtest]
    fn ffi_get_limiter_delay_returns_configured_look_ahead_samples() {
        let oar = TestRoar::new(roar_layout_t::ck_roar_layout_stereo, 480, 48000);

        // SAFETY: `oar.ptr()` is a live, valid renderer instance created above with no concurrent
        // mutation.
        let delay = unsafe { roar_get_limiter_delay(oar.ptr()) };

        expect_eq!(delay, EXPECTED_DELAY_SAMPLES as i32);
    }

    #[gtest]
    fn ffi_set_limiter_threshold_updates_output_ceiling_and_rejects_nan() -> Result<()> {
        let frame_size: usize = 480;
        let sample_rate: u32 = 48000;
        let channels: usize = 2;
        let element_id: u32 = 10;
        let oar =
            TestRoar::new(roar_layout_t::ck_roar_layout_stereo, frame_size as u32, sample_rate);
        // SAFETY: `oar.ptr()` is a live, valid renderer instance created above with exclusive
        // access.
        let group_id = unsafe { roar_add_audio_group(oar.ptr()) };
        verify_that!(group_id, ge(0))?;
        let element_cfg = create_channel_based_config(roar_layout_t::ck_roar_layout_stereo);
        // SAFETY: `oar.ptr()` is a live, valid renderer instance created above with exclusive
        // access, and `element_cfg` is a valid initialized `roar_audio_element_config_t`.
        let add_ret =
            unsafe { roar_add_audio_element(oar.ptr(), group_id as u32, element_id, &element_cfg) };
        verify_eq!(add_ret, 0)?;
        let mut input_buffer = vec![0.8f32; channels * frame_size];
        let mut input_data = roar_audio_block_t {
            data: input_buffer.as_mut_ptr(),
            channels: channels as u32,
            samples_per_channel: frame_size as u32,
        };
        let mut render_output = vec![0.0f32; channels * frame_size];
        let mut render_block = roar_audio_block_t {
            data: render_output.as_mut_ptr(),
            channels: channels as u32,
            samples_per_channel: frame_size as u32,
        };

        // SAFETY: `oar.ptr()` is a live, valid renderer instance created above with exclusive
        // access.
        let enable_ret = unsafe { roar_enable_limiter(oar.ptr(), 1) };
        verify_eq!(enable_ret, 0)?;
        // SAFETY: `oar.ptr()` is a live, valid renderer instance created above with exclusive
        // access.
        let nan_thresh_ret = unsafe { roar_set_limiter_threshold(oar.ptr(), f32::NAN) };
        // SAFETY: `oar.ptr()` is a live, valid renderer instance created above with exclusive
        // access.
        let thresh_ret = unsafe { roar_set_limiter_threshold(oar.ptr(), -6.0) };
        verify_eq!(thresh_ret, 0)?;
        // SAFETY: `oar.ptr()` is a live, valid renderer instance created above, and
        // `input_data.data` points to `input_buffer` with `channels * frame_size` initialized,
        // aligned `f32` elements, both exclusively owned during this call.
        let update_ret =
            unsafe { roar_update_audio_element_data(oar.ptr(), element_id, &mut input_data) };
        verify_eq!(update_ret, 0)?;
        // SAFETY: `oar.ptr()` is a live, valid renderer instance created above, and
        // `render_block.data` points to `render_output` with capacity for `channels * frame_size`
        // aligned `f32` elements, both exclusively owned during this call.
        let render_ret = unsafe { roar_render(oar.ptr(), &mut render_block) };
        verify_eq!(render_ret, 0)?;

        expect_eq!(nan_thresh_ret, OarError::InvalidParameter.into());
        // -6.0 dBFS corresponds to ~0.501187 linear ceiling.
        expect_le!(get_max_abs(&render_output), 0.5012);
        expect_gt!(get_max_abs(&render_output), 0.49);
        Ok(())
    }

    #[gtest]
    fn ffi_flush_drains_delayed_samples_when_enabled_and_returns_zero_when_disabled() -> Result<()>
    {
        let frame_size: usize = 480;
        let sample_rate: u32 = 48000;
        let channels: usize = 2;
        let element_id: u32 = 10;
        let oar =
            TestRoar::new(roar_layout_t::ck_roar_layout_stereo, frame_size as u32, sample_rate);
        // SAFETY: `oar.ptr()` is a live, valid renderer instance created above with exclusive
        // access.
        let group_id = unsafe { roar_add_audio_group(oar.ptr()) };
        verify_that!(group_id, ge(0))?;
        let element_cfg = create_channel_based_config(roar_layout_t::ck_roar_layout_stereo);
        // SAFETY: `oar.ptr()` is a live, valid renderer instance created above with exclusive
        // access, and `element_cfg` is a valid initialized `roar_audio_element_config_t`.
        let add_ret =
            unsafe { roar_add_audio_element(oar.ptr(), group_id as u32, element_id, &element_cfg) };
        verify_eq!(add_ret, 0)?;
        let mut input_buffer = vec![0.5f32; channels * frame_size];
        let mut input_data = roar_audio_block_t {
            data: input_buffer.as_mut_ptr(),
            channels: channels as u32,
            samples_per_channel: frame_size as u32,
        };
        let mut render_output = vec![0.0f32; channels * frame_size];
        let mut render_block = roar_audio_block_t {
            data: render_output.as_mut_ptr(),
            channels: channels as u32,
            samples_per_channel: frame_size as u32,
        };
        let mut flush_output = vec![0.0f32; channels * EXPECTED_DELAY_SAMPLES];
        let mut flush_block = roar_audio_block_t {
            data: flush_output.as_mut_ptr(),
            channels: channels as u32,
            samples_per_channel: 0,
        };
        let mut disabled_flush_block = roar_audio_block_t {
            data: flush_output.as_mut_ptr(),
            channels: channels as u32,
            samples_per_channel: 99,
        };

        // 1. Flush with limiter DISABLED.
        // SAFETY: `oar.ptr()` is a live, valid renderer instance created above.
        // `disabled_flush_block.data` points to `flush_output` with capacity for
        // `channels * EXPECTED_DELAY_SAMPLES` aligned `f32` elements, exclusively owned during
        // this call, which satisfies `roar_flush`'s buffer capacity requirement.
        let disabled_flush_ret = unsafe { roar_flush(oar.ptr(), &mut disabled_flush_block) };
        verify_eq!(disabled_flush_ret, 0)?;

        // 2. Enable limiter, render one block to fill look-ahead delay, then flush.
        // SAFETY: `oar.ptr()` is a live, valid renderer instance created above with exclusive
        // access.
        let enable_ret = unsafe { roar_enable_limiter(oar.ptr(), 1) };
        verify_eq!(enable_ret, 0)?;
        // SAFETY: `oar.ptr()` is a live, valid renderer instance created above, and
        // `input_data.data` points to `input_buffer` with `channels * frame_size` initialized,
        // aligned `f32` elements, both exclusively owned during this call.
        let update_ret =
            unsafe { roar_update_audio_element_data(oar.ptr(), element_id, &mut input_data) };
        verify_eq!(update_ret, 0)?;
        // SAFETY: `oar.ptr()` is a live, valid renderer instance created above, and
        // `render_block.data` points to `render_output` with capacity for `channels * frame_size`
        // aligned `f32` elements, both exclusively owned during this call.
        let render_ret = unsafe { roar_render(oar.ptr(), &mut render_block) };
        verify_eq!(render_ret, 0)?;
        // SAFETY: `oar.ptr()` is a live, valid renderer instance created above. `flush_block.data`
        // points to `flush_output` with capacity for `channels * EXPECTED_DELAY_SAMPLES` aligned
        // `f32` elements, exclusively owned during this call, which satisfies `roar_flush`'s
        // buffer capacity requirement.
        let flush_ret = unsafe { roar_flush(oar.ptr(), &mut flush_block) };
        verify_eq!(flush_ret, 0)?;

        expect_eq!(disabled_flush_block.samples_per_channel, 0);
        expect_eq!(flush_block.samples_per_channel, EXPECTED_DELAY_SAMPLES as u32);
        expect_eq!(&flush_output[..], &[0.5f32; 2 * EXPECTED_DELAY_SAMPLES]);
        Ok(())
    }

    #[gtest]
    fn ffi_flush_returns_invalid_parameter_for_invalid_output_block() {
        let oar = TestRoar::new(roar_layout_t::ck_roar_layout_stereo, 480, 48000);
        let mut buf = vec![0.0f32; 2 * EXPECTED_DELAY_SAMPLES];
        let mut null_data_block = roar_audio_block_t {
            data: std::ptr::null_mut(),
            channels: 2,
            samples_per_channel: 240,
        };
        let mut wrong_channels_block =
            roar_audio_block_t { data: buf.as_mut_ptr(), channels: 1, samples_per_channel: 240 };

        // SAFETY: `oar.ptr()` is a live, valid renderer instance created above, and
        // `null_data_block` is a valid initialized `roar_audio_block_t` with null `data`, which
        // `roar_flush` explicitly permits under its safety contract.
        let null_data_ret = unsafe { roar_flush(oar.ptr(), &mut null_data_block) };
        // SAFETY: `oar.ptr()` is a live, valid renderer instance created above.
        // `wrong_channels_block.data` points to `buf` with capacity for
        // `2 * EXPECTED_DELAY_SAMPLES` aligned `f32` elements (exceeding
        // `wrong_channels_block.channels * EXPECTED_DELAY_SAMPLES`), exclusively owned during this
        // call, which satisfies `roar_flush`'s buffer capacity requirement.
        let wrong_ch_ret = unsafe { roar_flush(oar.ptr(), &mut wrong_channels_block) };

        expect_eq!(null_data_ret, OarError::InvalidParameter.into());
        expect_eq!(wrong_ch_ret, OarError::InvalidParameter.into());
    }

    // ===== Rust API Tests =====

    /// Verifies that the peak limiter, when enabled via the Rust API, keeps the output
    /// amplitude <= limited value even when fed with high-amplitude inputs, and that it passes the
    /// unattenuated signal when disabled.
    #[gtest]
    fn rust_limiter_enabled_prevents_output_clipping() -> Result<()> {
        let frame_size: usize = 128;
        let sample_rate: u32 = 48000;
        let input_channels: usize = 2;
        let output_channels: usize = 2;
        let element_id: u32 = 10;
        let config = Config::new(
            OutputLayout::Stereo,
            Samples::new(frame_size as u32)?,
            SampleRate::new(sample_rate)?,
        )?;
        let mut renderer = RoarRenderer::create(&config)?;
        let group_id = renderer.add_audio_group()?;
        let element_cfg = AudioElementConfig::ChannelBased(ChannelBasedConfig {
            layout: InputLayout::Stereo,
            downmix_info: None,
            rendering_config: None,
        });
        renderer.add_element(group_id, element_id, &element_cfg)?;
        // Very loud input.
        let inputs = vec![vec![VALUE_GREATER_THAN_LIMIT; frame_size]; input_channels];
        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let input_buffer_ref = create_planar_buffer_ref(&input_refs[..], frame_size);
        let mut output_no_limiter = vec![0.0; output_channels * frame_size];
        let mut output_slices_1: [&mut [f32]; 2] = [&mut [], &mut []];
        let mut output_buffer_1 = create_planar_buffer_mut(
            &mut output_no_limiter,
            output_channels,
            frame_size,
            &mut output_slices_1,
        );
        let mut output_with_limiter = vec![0.0; output_channels * frame_size];
        let mut output_slices_2: [&mut [f32]; 2] = [&mut [], &mut []];
        let mut output_buffer_2 = create_planar_buffer_mut(
            &mut output_with_limiter,
            output_channels,
            frame_size,
            &mut output_slices_2,
        );

        // 1. Render with limiter DISABLED
        renderer.enable_limiter(false)?;
        renderer.render(&[(element_id, input_buffer_ref)], &mut output_buffer_1)?;

        // 2. Render with limiter ENABLED
        renderer.enable_limiter(true)?;
        // Render a few blocks to let the limiter stabilize.
        for _ in 0..10 {
            let input_buffer_ref_loop = create_planar_buffer_ref(&input_refs[..], frame_size);
            renderer.render(&[(element_id, input_buffer_ref_loop)], &mut output_buffer_2)?;
        }

        let max_no_limiter = get_max_abs(&output_no_limiter);
        expect_gt!(max_no_limiter, LIMITER_VALUE);
        let max_with_limiter = get_max_abs(&output_with_limiter);
        expect_le!(max_with_limiter, LIMITER_VALUE);
        expect_gt!(max_with_limiter, VALUE_UNDER_LIMITER_VALUE);
        Ok(())
    }

    #[gtest]
    fn rust_limiter_delay_returns_configured_look_ahead_samples() -> Result<()> {
        let config =
            Config::new(OutputLayout::Stereo, Samples::new(480)?, SampleRate::new(48000)?)?;
        let renderer = RoarRenderer::create(&config)?;

        let delay = renderer.limiter_delay();

        expect_eq!(delay, EXPECTED_DELAY_SAMPLES);
        Ok(())
    }

    #[gtest]
    fn rust_set_limiter_threshold_updates_output_ceiling() -> Result<()> {
        let frame_size: usize = 480;
        let sample_rate: u32 = 48000;
        let channels: usize = 2;
        let element_id: u32 = 10;
        let config = Config::new(
            OutputLayout::Stereo,
            Samples::new(frame_size as u32)?,
            SampleRate::new(sample_rate)?,
        )?;
        let mut renderer = RoarRenderer::create(&config)?;
        let group_id = renderer.add_audio_group()?;
        let element_cfg = AudioElementConfig::ChannelBased(ChannelBasedConfig {
            layout: InputLayout::Stereo,
            downmix_info: None,
            rendering_config: None,
        });
        renderer.add_element(group_id, element_id, &element_cfg)?;
        let inputs = vec![vec![0.8f32; frame_size]; channels];
        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let input_buffer_ref = create_planar_buffer_ref(&input_refs[..], frame_size);
        let mut render_output = vec![0.0f32; channels * frame_size];
        let mut render_slices: [&mut [f32]; 2] = [&mut [], &mut []];
        let mut render_buf =
            create_planar_buffer_mut(&mut render_output, channels, frame_size, &mut render_slices);

        renderer.enable_limiter(true)?;
        renderer.set_limiter_threshold(Decibels::new(-6.0)?)?;
        renderer.render(&[(element_id, input_buffer_ref)], &mut render_buf)?;

        expect_le!(get_max_abs(&render_output), 0.5012);
        expect_gt!(get_max_abs(&render_output), 0.49);
        Ok(())
    }

    #[gtest]
    fn rust_flush_drains_delayed_samples_when_enabled_and_returns_zero_when_disabled() -> Result<()>
    {
        let frame_size: usize = 480;
        let sample_rate: u32 = 48000;
        let channels: usize = 2;
        let element_id: u32 = 10;
        let config = Config::new(
            OutputLayout::Stereo,
            Samples::new(frame_size as u32)?,
            SampleRate::new(sample_rate)?,
        )?;
        let mut renderer = RoarRenderer::create(&config)?;
        let group_id = renderer.add_audio_group()?;
        let element_cfg = AudioElementConfig::ChannelBased(ChannelBasedConfig {
            layout: InputLayout::Stereo,
            downmix_info: None,
            rendering_config: None,
        });
        renderer.add_element(group_id, element_id, &element_cfg)?;
        let inputs = vec![vec![0.5f32; frame_size]; channels];
        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let input_buffer_ref = create_planar_buffer_ref(&input_refs[..], frame_size);
        let mut disabled_flush_output = vec![0.0f32; channels * EXPECTED_DELAY_SAMPLES];
        let mut disabled_flush_slices: [&mut [f32]; 2] = [&mut [], &mut []];
        let mut disabled_flush_buf = create_planar_buffer_mut(
            &mut disabled_flush_output,
            channels,
            EXPECTED_DELAY_SAMPLES,
            &mut disabled_flush_slices,
        );
        let mut render_output = vec![0.0f32; channels * frame_size];
        let mut render_slices: [&mut [f32]; 2] = [&mut [], &mut []];
        let mut render_buf =
            create_planar_buffer_mut(&mut render_output, channels, frame_size, &mut render_slices);
        let mut enabled_flush_output = vec![0.0f32; channels * EXPECTED_DELAY_SAMPLES];
        let mut enabled_flush_slices: [&mut [f32]; 2] = [&mut [], &mut []];
        let mut enabled_flush_buf = create_planar_buffer_mut(
            &mut enabled_flush_output,
            channels,
            EXPECTED_DELAY_SAMPLES,
            &mut enabled_flush_slices,
        );

        // 1. Flush with limiter DISABLED.
        let disabled_flushed = renderer.flush(&mut disabled_flush_buf)?;

        // 2. Enable limiter, render one block to fill look-ahead delay, then flush.
        renderer.enable_limiter(true)?;
        renderer.render(&[(element_id, input_buffer_ref)], &mut render_buf)?;
        let enabled_flushed = renderer.flush(&mut enabled_flush_buf)?;

        expect_eq!(disabled_flushed, 0);
        expect_eq!(enabled_flushed, EXPECTED_DELAY_SAMPLES);
        expect_eq!(&enabled_flush_output[..], &[0.5f32; 2 * EXPECTED_DELAY_SAMPLES]);
        Ok(())
    }

    #[gtest]
    fn rust_enable_limiter_resets_delay_on_re_enable_but_preserves_state_when_already_enabled(
    ) -> Result<()> {
        let frame_size: usize = 240;
        let sample_rate: u32 = 48000;
        let channels: usize = 2;
        let element_id: u32 = 10;
        let config = Config::new(
            OutputLayout::Stereo,
            Samples::new(frame_size as u32)?,
            SampleRate::new(sample_rate)?,
        )?;
        let mut renderer = RoarRenderer::create(&config)?;
        let group_id = renderer.add_audio_group()?;
        let element_cfg = AudioElementConfig::ChannelBased(ChannelBasedConfig {
            layout: InputLayout::Stereo,
            downmix_info: None,
            rendering_config: None,
        });
        renderer.add_element(group_id, element_id, &element_cfg)?;
        let inputs = vec![vec![0.5f32; frame_size]; channels];
        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let mut block_1 = vec![0.0f32; channels * frame_size];
        let mut slices_1: [&mut [f32]; 2] = [&mut [], &mut []];
        let mut buf_1 = create_planar_buffer_mut(&mut block_1, channels, frame_size, &mut slices_1);
        let mut block_2 = vec![0.0f32; channels * frame_size];
        let mut slices_2: [&mut [f32]; 2] = [&mut [], &mut []];
        let mut buf_2 = create_planar_buffer_mut(&mut block_2, channels, frame_size, &mut slices_2);
        let mut block_3 = vec![0.0f32; channels * frame_size];
        let mut slices_3: [&mut [f32]; 2] = [&mut [], &mut []];
        let mut buf_3 = create_planar_buffer_mut(&mut block_3, channels, frame_size, &mut slices_3);

        // 1. First block primes the look-ahead delay buffer (outputs zeros).
        renderer.enable_limiter(true)?;
        let input_ref_1 = create_planar_buffer_ref(&input_refs[..], frame_size);
        renderer.render(&[(element_id, input_ref_1)], &mut buf_1)?;

        // 2. Calling enable_limiter(true) while already enabled must preserve buffered samples.
        renderer.enable_limiter(true)?;
        let input_ref_2 = create_planar_buffer_ref(&input_refs[..], frame_size);
        renderer.render(&[(element_id, input_ref_2)], &mut buf_2)?;

        // 3. Transitioning disabled -> enabled must reset the delay buffer so block_3 re-primes
        // with zeros.
        renderer.enable_limiter(false)?;
        renderer.enable_limiter(true)?;
        let input_ref_3 = create_planar_buffer_ref(&input_refs[..], frame_size);
        renderer.render(&[(element_id, input_ref_3)], &mut buf_3)?;

        expect_eq!(&block_1[..], &[0.0f32; 2 * 240]);
        expect_eq!(&block_2[..], &[0.5f32; 2 * 240]);
        expect_eq!(&block_3[..], &[0.0f32; 2 * 240]);
        Ok(())
    }
}
