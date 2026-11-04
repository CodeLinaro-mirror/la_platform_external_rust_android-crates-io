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

/// Integration tests for scene-based (HOA) audio rendering.
#[cfg(test)]
mod test {

    use crate::test_helpers::{
        create_flat_sine_input, create_planar_buffer_mut, create_planar_buffer_ref,
        create_scene_based_config, create_sine_input_channels, rms, TestRoar,
    };
    use aom_roar::c_types::{roar_audio_block_t, roar_hoa_t, roar_layout_t};
    use aom_roar::ffi::*;
    use aom_roar::{
        AudioElementConfig, Config, HighOrderAmbisonics, OutputLayout, RoarRenderer, SampleRate,
        Samples, SceneBasedConfig,
    };
    use googletest::prelude::*;

    /// Expect at least this value for non-silent output.
    const NON_SILENT_RMS_THRESHOLD: f32 = 0.01;

    // ===== FFI C API Tests =====

    /// Verifies scene-based (Ambisonics) rendering via the C FFI API by rendering a 1st order
    /// HOA input (4 channels) to a Stereo layout output.
    #[gtest]
    fn ffi_rendering_1oa_input_to_stereo_output_succeeds() {
        let frame_size: usize = 256;
        let sample_rate: u32 = 48000;
        let input_channels: usize = 4;
        let output_channels: usize = 2;
        let element_id: u32 = 1;

        // Create test ROAR instance.
        let oar =
            TestRoar::new(roar_layout_t::ck_roar_layout_stereo, frame_size as u32, sample_rate);
        // Create audio group and audio element.
        let element_cfg = create_scene_based_config(roar_hoa_t::ck_roar_1oa);
        // SAFETY: `oar.ptr()` is a valid pointer initialized by `TestRoar::new`.
        let group_id = unsafe { roar_add_audio_group(oar.ptr()) };
        assert!(group_id >= 0);
        // SAFETY: `oar.ptr()` is valid, and `&element_cfg` is a valid temporary reference.
        let ret =
            unsafe { roar_add_audio_element(oar.ptr(), group_id as u32, element_id, &element_cfg) };
        assert_eq!(ret, 0);

        // SAFETY: `oar.ptr()` is valid.
        let actual_input_channels =
            unsafe { roar_get_number_of_audio_element_channels(oar.ptr(), element_id) };
        assert_eq!(actual_input_channels, input_channels as u32); // 1OA has 4 channels

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
        // Verify output is non-silent.
        expect_that!(rms(&output_buffer[..frame_size]), gt(NON_SILENT_RMS_THRESHOLD));
        expect_that!(rms(&output_buffer[frame_size..]), gt(NON_SILENT_RMS_THRESHOLD));
    }

    // ===== Rust API Tests =====

    /// Verifies scene-based rendering via the Rust API by rendering a 1st order HOA element
    /// to Stereo output.
    #[gtest]
    fn rust_rendering_1oa_input_to_stereo_output_succeeds() {
        let frame_size: usize = 128;
        let sample_rate: u32 = 48000;
        let input_channels: usize = 4;
        let output_channels: usize = 2;
        let element_id: u32 = 15;

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
        let element_cfg = AudioElementConfig::SceneBased(SceneBasedConfig {
            order: HighOrderAmbisonics::Order1,
            rendering_config: None,
        });
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

        let render_result = renderer.render(&[(element_id, input_buffer_ref)], &mut output_buffer);

        expect_ok!(render_result);
        // Verify output is non-silent.
        expect_that!(rms(output_buffer.channel_mut(0)), gt(NON_SILENT_RMS_THRESHOLD));
        expect_that!(rms(output_buffer.channel_mut(1)), gt(NON_SILENT_RMS_THRESHOLD));
    }
}
