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

/// Integration tests for object-based audio rendering.
#[cfg(test)]
mod test {

    use crate::test_helpers::{
        create_flat_sine_input, create_object_based_config, create_planar_buffer_mut,
        create_planar_buffer_ref, create_polar_position_metadata, create_polar_positions,
        create_sine_input_channels, rms, TestRoar,
    };
    use aom_roar::c_types::{roar_audio_block_t, roar_layout_t, roar_metadata_type_t};
    use aom_roar::ffi::*;
    use aom_roar::{
        AudioElementConfig, Config, OarError, ObjectBasedConfig, OutputLayout, RoarRenderer,
        SampleRate, Samples,
    };
    use googletest::prelude::*;

    /// Expect at least this value for non-silent output.
    const NON_SILENT_RMS_THRESHOLD: f32 = 0.1;

    /// Creates a C FFI `TestRoar` instance with a group and object-based audio element registered.
    fn create_ffi_object_renderer(
        layout: roar_layout_t,
        samples_per_channel: Samples,
        sample_rate: SampleRate,
        element_id: u32,
        num_objects: usize,
    ) -> TestRoar {
        let oar = TestRoar::new(layout, samples_per_channel.value(), sample_rate.value());
        let element_cfg = create_object_based_config(num_objects);
        // SAFETY: `oar.ptr()` was initialized by `TestRoar::new` and is non-null and valid.
        // `&element_cfg` is an aligned reference to a live stack struct. `group_id` is verified
        // non-negative before casting to u32.
        unsafe {
            let group_id = roar_add_audio_group(oar.ptr());
            assert!(group_id >= 0);
            let ret = roar_add_audio_element(oar.ptr(), group_id as u32, element_id, &element_cfg);
            assert_eq!(ret, 0);
        }
        oar
    }

    /// Creates a Rust `RoarRenderer` instance with a group and object-based audio element
    /// registered.
    fn create_rust_object_renderer(
        layout: OutputLayout,
        samples_per_channel: Samples,
        sample_rate: SampleRate,
        element_id: u32,
        num_objects: usize,
    ) -> Result<RoarRenderer> {
        let config = Config::new(layout, samples_per_channel, sample_rate)?;
        let mut renderer = RoarRenderer::create(&config)?;
        let group_id = renderer.add_audio_group()?;
        let element_cfg =
            AudioElementConfig::ObjectBased(ObjectBasedConfig::new(num_objects, None)?);
        renderer.add_element(group_id, element_id, &element_cfg)?;
        Ok(renderer)
    }

    // ===== FFI C API Tests =====

    /// Verifies object-based rendering via the C FFI API with 2 objects positioned at left and
    /// right 45°, and asserts that output channels contain non-zero energy.
    #[gtest]
    fn ffi_rendering_multi_object_input_to_stereo_output_pans_objects() -> Result<()> {
        let frame_size = Samples::new(128)?;
        let sample_rate = SampleRate::new(48000)?;
        let num_objects: usize = 2;
        let output_channels: usize = 2;
        let element_id: u32 = 10;

        let oar = create_ffi_object_renderer(
            roar_layout_t::ck_roar_layout_stereo,
            frame_size,
            sample_rate,
            element_id,
            num_objects,
        );
        let pos_meta = create_polar_position_metadata(
            &[(45.0, 0.0, 1.0), (-45.0, 0.0, 1.0)],
            frame_size.value() as usize,
        );
        let mut input_buffer = create_flat_sine_input(
            num_objects,
            frame_size.value() as usize,
            1.0,
            440.0,
            sample_rate.value() as f32,
        );
        let mut input_data = roar_audio_block_t {
            data: input_buffer.as_mut_ptr(),
            channels: num_objects as u32,
            samples_per_channel: frame_size.value(),
        };
        let mut output_buffer = vec![0.0f32; output_channels * frame_size.value() as usize];
        let mut output_data = roar_audio_block_t {
            data: output_buffer.as_mut_ptr(),
            channels: output_channels as u32,
            samples_per_channel: frame_size.value(),
        };

        // SAFETY: `oar.ptr()`, metadata, input, and output pointers are valid.
        let update_meta_ret =
            unsafe { roar_update_audio_element_metadata(oar.ptr(), element_id, &pos_meta) };
        let update_data_ret =
            unsafe { roar_update_audio_element_data(oar.ptr(), element_id, &mut input_data) };
        let render_ret = unsafe { roar_render(oar.ptr(), &mut output_data) };

        expect_eq!(update_meta_ret, 0);
        expect_eq!(update_data_ret, 0);
        expect_eq!(render_ret, 0);
        expect_that!(
            rms(&output_buffer[..frame_size.value() as usize]),
            gt(NON_SILENT_RMS_THRESHOLD)
        );
        expect_that!(
            rms(&output_buffer[frame_size.value() as usize..]),
            gt(NON_SILENT_RMS_THRESHOLD)
        );
        Ok(())
    }

    /// Verifies object-based rendering via the C FFI API by rendering a single-object input.
    #[gtest]
    fn ffi_rendering_single_object_input_to_stereo_output_succeeds() -> Result<()> {
        let frame_size = Samples::new(128)?;
        let sample_rate = SampleRate::new(48000)?;
        let num_objects: usize = 1;
        let output_channels: usize = 2;
        let element_id: u32 = 10;

        let oar = create_ffi_object_renderer(
            roar_layout_t::ck_roar_layout_stereo,
            frame_size,
            sample_rate,
            element_id,
            num_objects,
        );
        let pos_meta =
            create_polar_position_metadata(&[(15.0, 0.0, 1.0)], frame_size.value() as usize);
        let mut input_buffer = create_flat_sine_input(
            num_objects,
            frame_size.value() as usize,
            1.0,
            440.0,
            sample_rate.value() as f32,
        );
        let mut input_data = roar_audio_block_t {
            data: input_buffer.as_mut_ptr(),
            channels: num_objects as u32,
            samples_per_channel: frame_size.value(),
        };
        let mut output_buffer = vec![0.0f32; output_channels * frame_size.value() as usize];
        let mut output_data = roar_audio_block_t {
            data: output_buffer.as_mut_ptr(),
            channels: output_channels as u32,
            samples_per_channel: frame_size.value(),
        };

        // SAFETY: `oar.ptr()`, metadata, input, and output pointers are valid.
        let update_meta_ret =
            unsafe { roar_update_audio_element_metadata(oar.ptr(), element_id, &pos_meta) };
        let update_data_ret =
            unsafe { roar_update_audio_element_data(oar.ptr(), element_id, &mut input_data) };
        let render_ret = unsafe { roar_render(oar.ptr(), &mut output_data) };

        expect_eq!(update_meta_ret, 0);
        expect_eq!(update_data_ret, 0);
        expect_eq!(render_ret, 0);
        expect_that!(
            rms(&output_buffer[..frame_size.value() as usize]),
            gt(rms(&output_buffer[frame_size.value() as usize..]))
        );
        expect_that!(
            rms(&output_buffer[frame_size.value() as usize..]),
            gt(NON_SILENT_RMS_THRESHOLD)
        );
        Ok(())
    }

    #[gtest]
    fn ffi_set_metadata_unit_to_process_allows_binaural_output() -> Result<()> {
        let frame_size = Samples::new(256)?;
        let sub_frame_size: u32 = 64;
        let sample_rate = SampleRate::new(48000)?;
        let num_objects: usize = 1;
        let output_channels: usize = 2;
        let element_id: u32 = 10;

        let oar = create_ffi_object_renderer(
            roar_layout_t::ck_roar_layout_binaural,
            frame_size,
            sample_rate,
            element_id,
            num_objects,
        );
        let pos_meta =
            create_polar_position_metadata(&[(30.0, 0.0, 1.0)], frame_size.value() as usize);
        let mut input_buffer = create_flat_sine_input(
            num_objects,
            frame_size.value() as usize,
            1.0,
            440.0,
            sample_rate.value() as f32,
        );
        let mut input_data = roar_audio_block_t {
            data: input_buffer.as_mut_ptr(),
            channels: num_objects as u32,
            samples_per_channel: frame_size.value(),
        };
        let mut output_buffer = vec![0.0f32; output_channels * frame_size.value() as usize];
        let mut output_data = roar_audio_block_t {
            data: output_buffer.as_mut_ptr(),
            channels: output_channels as u32,
            samples_per_channel: frame_size.value(),
        };

        // SAFETY: `oar.ptr()` is initialized and valid from `create_ffi_object_renderer`.
        // `pos_meta`, `input_data`, and `output_data` point to valid, initialized, and
        // correctly aligned data structures that remain live for the duration of these calls.
        let (set_unit_ret, update_meta_ret, update_data_ret, render_ret) = unsafe {
            (
                roar_set_metadata_unit_to_process(
                    oar.ptr(),
                    roar_metadata_type_t::ck_roar_metadata_object_positions as u32,
                    sub_frame_size,
                ),
                roar_update_audio_element_metadata(oar.ptr(), element_id, &pos_meta),
                roar_update_audio_element_data(oar.ptr(), element_id, &mut input_data),
                roar_render(oar.ptr(), &mut output_data),
            )
        };

        expect_eq!(set_unit_ret, 0);
        expect_eq!(update_meta_ret, 0);
        expect_eq!(update_data_ret, 0);
        expect_eq!(render_ret, 0);
        expect_that!(rms(&output_buffer[..frame_size.value() as usize]), gt(0.0));
        expect_that!(rms(&output_buffer[frame_size.value() as usize..]), gt(0.0));
        Ok(())
    }

    // ===== Rust API Tests =====

    /// Verifies object-based rendering via the Rust API with 2 objects positioned at 45° left and
    /// right.
    #[gtest]
    fn rust_rendering_multi_object_input_to_stereo_output_pans_objects() -> Result<()> {
        let frame_size = Samples::new(128)?;
        let sample_rate = SampleRate::new(48000)?;
        let num_objects: usize = 2;
        let output_channels: usize = 2;
        let element_id: u32 = 10;

        let mut renderer = create_rust_object_renderer(
            OutputLayout::Stereo,
            frame_size,
            sample_rate,
            element_id,
            num_objects,
        )?;
        let pos = create_polar_positions(&[(45.0, 0.0, 1.0), (-45.0, 0.0, 1.0)]);
        let input_data = create_sine_input_channels(
            num_objects,
            frame_size.value() as usize,
            1.0,
            440.0,
            sample_rate.value() as f32,
        );
        let input_refs: Vec<&[f32]> = input_data.iter().map(|v| v.as_slice()).collect();
        let input_buffer_ref =
            create_planar_buffer_ref(&input_refs[..], frame_size.value() as usize);
        let mut output = vec![0.0; output_channels * frame_size.value() as usize];
        let mut output_slices: Vec<&mut [f32]> =
            (0..output_channels).map(|_| &mut [] as &mut [f32]).collect();
        let mut output_buffer = create_planar_buffer_mut(
            &mut output,
            output_channels,
            frame_size.value() as usize,
            &mut output_slices,
        );

        let position_update_result =
            renderer.update_element_positions(element_id, &pos, frame_size);
        let render_result = renderer.render(&[(element_id, input_buffer_ref)], &mut output_buffer);

        expect_ok!(position_update_result);
        expect_ok!(render_result);
        expect_that!(rms(output_buffer.channel_mut(0)), gt(NON_SILENT_RMS_THRESHOLD));
        expect_that!(rms(output_buffer.channel_mut(1)), gt(NON_SILENT_RMS_THRESHOLD));
        Ok(())
    }

    /// Verifies object-based rendering via the Rust API by rendering a single-object input
    /// and checking that it succeeds.
    #[gtest]
    fn rust_rendering_single_object_input_to_stereo_output_succeeds() -> Result<()> {
        let frame_size = Samples::new(128)?;
        let sample_rate = SampleRate::new(48000)?;
        let num_objects: usize = 1;
        let output_channels: usize = 2;
        let element_id: u32 = 10;

        let mut renderer = create_rust_object_renderer(
            OutputLayout::Stereo,
            frame_size,
            sample_rate,
            element_id,
            num_objects,
        )?;
        let pos = create_polar_positions(&[(15.0, 0.0, 1.0)]);
        let input_data = create_sine_input_channels(
            num_objects,
            frame_size.value() as usize,
            1.0,
            440.0,
            sample_rate.value() as f32,
        );
        let input_refs: Vec<&[f32]> = input_data.iter().map(|v| v.as_slice()).collect();
        let input_buffer_ref =
            create_planar_buffer_ref(&input_refs[..], frame_size.value() as usize);
        let mut output = vec![0.0; output_channels * frame_size.value() as usize];
        let mut output_slices: Vec<&mut [f32]> =
            (0..output_channels).map(|_| &mut [] as &mut [f32]).collect();
        let mut output_buffer = create_planar_buffer_mut(
            &mut output,
            output_channels,
            frame_size.value() as usize,
            &mut output_slices,
        );

        let position_update_result =
            renderer.update_element_positions(element_id, &pos, frame_size);
        let render_result = renderer.render(&[(element_id, input_buffer_ref)], &mut output_buffer);

        expect_ok!(position_update_result);
        expect_ok!(render_result);
        expect_that!(rms(output_buffer.channel_mut(0)), gt(rms(output_buffer.channel_mut(1))));
        expect_that!(rms(output_buffer.channel_mut(0)), gt(NON_SILENT_RMS_THRESHOLD));
        expect_that!(rms(output_buffer.channel_mut(1)), gt(NON_SILENT_RMS_THRESHOLD));
        Ok(())
    }

    #[gtest]
    fn rust_set_metadata_unit_to_process_allows_binaural_output() -> Result<()> {
        let frame_size = Samples::new(256)?;
        let sub_frame_size = Samples::new(64)?;
        let sample_rate = SampleRate::new(48000)?;
        let num_objects: usize = 1;
        let output_channels: usize = 2;
        let element_id: u32 = 10;

        let mut renderer = create_rust_object_renderer(
            OutputLayout::Binaural,
            frame_size,
            sample_rate,
            element_id,
            num_objects,
        )?;
        let pos = create_polar_positions(&[(30.0, 0.0, 1.0)]);
        let input_data = create_sine_input_channels(
            num_objects,
            frame_size.value() as usize,
            1.0,
            440.0,
            sample_rate.value() as f32,
        );
        let input_refs: Vec<&[f32]> = input_data.iter().map(|v| v.as_slice()).collect();
        let input_buffer_ref =
            create_planar_buffer_ref(&input_refs[..], frame_size.value() as usize);
        let mut output = vec![0.0; output_channels * frame_size.value() as usize];
        let mut output_slices: Vec<&mut [f32]> =
            (0..output_channels).map(|_| &mut [] as &mut [f32]).collect();
        let mut output_buffer = create_planar_buffer_mut(
            &mut output,
            output_channels,
            frame_size.value() as usize,
            &mut output_slices,
        );

        let set_unit_result = renderer.set_object_position_metadata_unit_to_process(sub_frame_size);
        let position_update_result =
            renderer.update_element_positions(element_id, &pos, frame_size);
        let render_result = renderer.render(&[(element_id, input_buffer_ref)], &mut output_buffer);

        expect_ok!(set_unit_result);
        expect_ok!(position_update_result);
        expect_ok!(render_result);
        expect_that!(rms(output_buffer.channel_mut(0)), gt(0.0));
        expect_that!(rms(output_buffer.channel_mut(1)), gt(0.0));
        Ok(())
    }

    #[gtest]
    fn ffi_set_metadata_unit_to_process_rejects_zero_samples() -> Result<()> {
        let frame_size = Samples::new(256)?;
        let sample_rate = SampleRate::new(48000)?;
        let oar = create_ffi_object_renderer(
            roar_layout_t::ck_roar_layout_binaural,
            frame_size,
            sample_rate,
            10,
            1,
        );

        // SAFETY: `oar.ptr()` was allocated by `create_ffi_object_renderer`, which asserts
        // non-nullness upon construction with `TestRoar::new`, and `oar` remains live across this
        // call.
        let ret = unsafe {
            roar_set_metadata_unit_to_process(
                oar.ptr(),
                roar_metadata_type_t::ck_roar_metadata_object_positions as u32,
                0,
            )
        };

        expect_eq!(ret, OarError::InvalidParameter.into());
        Ok(())
    }

    #[gtest]
    fn rust_set_metadata_unit_to_process_rejects_samples_exceeding_frame_size() -> Result<()> {
        let frame_size = Samples::new(128)?;
        let sample_rate = SampleRate::new(48000)?;
        let mut renderer =
            create_rust_object_renderer(OutputLayout::Stereo, frame_size, sample_rate, 10, 1)?;
        let invalid_samples = Samples::new(256)?;

        let result = renderer.set_object_position_metadata_unit_to_process(invalid_samples);

        expect_that!(result, err(eq(OarError::InvalidParameter)));
        Ok(())
    }
}
