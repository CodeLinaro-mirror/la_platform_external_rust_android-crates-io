// Copyright (c) 2026, Alliance for Open Media. All rights reserved
//
// This source code is subject to the terms of the BSD 3-Clause Clear License
// and the Alliance for Open Media Patent License 1.0. If the BSD 3-Clause Clear
// License was not distributed with this source code in the LICENSE file, you
// can obtain it at www.aomedia.org/license/software-license/bsd-3-c-c. If the
// Alliance for Open Media Patent License 1.0 was not distributed with this
// source code in the PATENTS file, you can obtain it at
// www.aomedia.org/license/patent.

#![forbid(unsafe_code)]

//! Top-level OBR (Open Binaural RoarRenderer) orchestrator.
//!
//! This module implements the main entry point and coordinator for the OBR subsystem.
//! It manages audio elements, groups them into processing groups, performs Ambisonic
//! encoding, handles head tracking rotation, and performs frequency-domain binaural
//! decoding.
//!
//! Crucially, `ObrImpl` is designed to be allocation-free on its real-time audio thread
//! processing loop once initialized.

use super::audio_element_config::ObrAudioElement;
use super::processing_group::{ProcessingGroup, ProcessingGroupKey};
use crate::common::audio_buffer::simd_utils::add_pointwise_in_place;
use crate::common::audio_buffer::AudioBuffer;
use crate::common::definitions::{
    AudioElementConfig, ChannelCount, Gain, OarError, ObjectPosition, PlanarBufferMut,
    PlanarBufferRef, PolarCoordinate, Quaternion, SampleRate, Samples,
};
use crate::renderer::audio_renderer_api::AudioRenderer;
use crate::renderer::gain_parameter::{generate_gain_multipliers, GainParameter, GainUpdate};
use crate::renderer::obr::ambisonic_binaural_decoder::{FftManager, Resampler};
use crate::renderer::obr::common::constants::{
    MAX_SUPPORTED_NUM_INPUT_CHANNELS, NUM_BINAURAL_CHANNELS,
};
use std::collections::BTreeMap;
use std::collections::HashMap;

/// Top-level coordinator for the OBR rendering pipeline.
///
/// Orchestrates the process of mapping and rendering multiple configured audio elements.
/// It groups elements by their filter profiles and Ambisonic orders to optimize the number
/// of DSP operations required. Supports real-time updates for object positions and
/// head rotation.
#[derive(Debug)]
pub struct ObrImpl {
    buffer_size_per_channel: Samples,
    sampling_rate: SampleRate,
    head_tracking_enabled: bool,
    world_rotation: Quaternion,
    audio_elements: Vec<ObrAudioElement>,
    processing_groups: Vec<ProcessingGroup>,
    resampler: Resampler,
    fft_manager: FftManager,
    scratch_group_output: AudioBuffer,
}

impl ObrImpl {
    /// Constructs a new `ObrImpl` instance.
    ///
    /// # Parameters
    /// * `buffer_size_per_channel` - Number of audio frames per block.
    /// * `sampling_rate` - Audio sample rate.
    pub fn new(buffer_size_per_channel: Samples, sampling_rate: SampleRate) -> Self {
        let size_usize = buffer_size_per_channel.value() as usize;
        Self {
            buffer_size_per_channel,
            sampling_rate,
            head_tracking_enabled: false,
            world_rotation: Quaternion::identity(),
            audio_elements: Vec::new(),
            processing_groups: Vec::new(),
            resampler: Resampler::new(),
            fft_manager: FftManager::new(size_usize),
            scratch_group_output: AudioBuffer::new(2, size_usize),
        }
    }

    /// Updates the processing block size per channel and re-initializes DSP state if elements
    /// exist.
    ///
    /// # Parameters
    /// * `buffer_size_per_channel` - Number of audio frames per processing block.
    pub fn set_buffer_size_per_channel(
        &mut self,
        buffer_size_per_channel: Samples,
    ) -> Result<(), OarError> {
        if self.buffer_size_per_channel == buffer_size_per_channel {
            return Ok(());
        }
        self.buffer_size_per_channel = buffer_size_per_channel;
        let size_usize = buffer_size_per_channel.value() as usize;
        self.fft_manager = FftManager::new(size_usize);
        self.scratch_group_output = AudioBuffer::new(2, size_usize);
        if !self.audio_elements.is_empty() {
            self.initialize_dsp()?;
        }
        Ok(())
    }

    /// Resets all DSP state and clears processing groups.
    pub fn reset_dsp(&mut self) {
        self.processing_groups.clear();
    }

    /// Re-initialises DSP resources after configuration changes.
    pub fn initialize_dsp(&mut self) -> Result<(), OarError> {
        self.reset_dsp();
        if self.audio_elements.is_empty() {
            return Err(OarError::InvalidParameter);
        }
        let input_channels = self.get_number_of_input_channels();
        if input_channels == 0 {
            return Err(OarError::InvalidParameter);
        }
        self.create_processing_groups();
        for group in &mut self.processing_groups {
            group.initialize(&mut self.fft_manager, &mut self.resampler)?;
            group.update_ambisonic_encoder(&self.audio_elements)?;
        }
        Ok(())
    }

    fn create_processing_groups(&mut self) {
        self.processing_groups.clear();
        let mut groups_map: BTreeMap<ProcessingGroupKey, Vec<usize>> = BTreeMap::new();
        for (i, elem) in self.audio_elements.iter().enumerate() {
            if elem.is_passthrough() {
                continue;
            }
            let key = ProcessingGroupKey {
                ambisonic_order: elem.get_binaural_filters_ambisonic_order(),
                filter_profile: elem.get_binaural_filter_profile(),
            };
            groups_map.entry(key).or_default().push(i);
        }
        for (key, indices) in groups_map {
            self.processing_groups.push(ProcessingGroup::new(
                key,
                indices,
                self.buffer_size_per_channel,
                self.sampling_rate,
            ));
        }
    }

    /// Adds an audio element to the renderer and updates the DSP lifecycle.
    ///
    /// # Parameters
    /// * `config` - Configuration for the audio element to add.
    pub fn add_audio_element(&mut self, config: &AudioElementConfig) -> Result<(), OarError> {
        let mut cfg = ObrAudioElement::try_from(config)?;
        if !self.audio_elements.is_empty() {
            let last = self.audio_elements.last().unwrap();
            let first_idx = last.get_first_channel_index() + last.get_number_of_input_channels();
            cfg.set_first_channel_index(first_idx);
        }
        let total_inputs: usize =
            self.audio_elements.iter().map(|e| e.get_number_of_input_channels()).sum::<usize>()
                + cfg.get_number_of_input_channels();
        if total_inputs > MAX_SUPPORTED_NUM_INPUT_CHANNELS {
            return Err(OarError::InvalidParameter);
        }
        self.audio_elements.push(cfg);
        self.initialize_dsp()
    }

    /// Removes the audio element at `index` and re-initialises DSP resources.
    ///
    /// # Parameters
    /// * `index` - Zero-based index of the audio element to remove.
    pub fn remove_audio_element(&mut self, index: usize) -> Result<(), OarError> {
        if index >= self.audio_elements.len() {
            return Err(OarError::InvalidParameter);
        }
        self.audio_elements.remove(index);
        let mut first_idx = 0;
        for elem in &mut self.audio_elements {
            elem.set_first_channel_index(first_idx);
            first_idx += elem.get_number_of_input_channels();
        }
        if self.audio_elements.is_empty() {
            self.reset_dsp();
            Ok(())
        } else {
            self.initialize_dsp()
        }
    }

    /// Sets the position of a specific channel inside a multi-channel object.
    ///
    /// # Parameters
    /// * `audio_element_index` - Index of the audio element.
    /// * `channel_index` - Channel index within the object.
    /// * `azimuth` - Azimuth angle in degrees.
    /// * `elevation` - Elevation angle in degrees.
    /// * `distance` - Distance from the listener.
    pub fn update_object_channel_position(
        &mut self,
        audio_element_index: usize,
        channel_index: usize,
        azimuth: f32,
        elevation: f32,
        distance: f32,
    ) -> Result<(), OarError> {
        let elem =
            self.audio_elements.get_mut(audio_element_index).ok_or(OarError::InvalidParameter)?;
        let slot =
            elem.object_positions_mut().get_mut(channel_index).ok_or(OarError::InvalidParameter)?;
        *slot = PolarCoordinate::new_from_floats(azimuth, elevation, distance)?;
        for g in &mut self.processing_groups {
            g.update_ambisonic_encoder(&self.audio_elements)?;
        }
        Ok(())
    }

    /// Enables or disables head tracking rotation of the sound field.
    pub fn enable_head_tracking(&mut self, enable: bool) {
        self.head_tracking_enabled = enable;
    }

    /// Sets the head rotation quaternion for counter-rotating sound fields.
    ///
    /// # Parameters
    /// * `rotation` - Quaternion representing head orientation.
    pub fn set_head_rotation(&mut self, rotation: Quaternion) -> Result<(), OarError> {
        self.world_rotation = rotation.conjugate();
        Ok(())
    }

    /// Processes planar audio data from `input_buffer` to a 2-channel binaural `output_buffer`.
    pub fn process(&mut self, input_buffer: &AudioBuffer, output_buffer: &mut AudioBuffer) {
        let frames_usize = self.buffer_size_per_channel.value() as usize;
        assert_eq!(input_buffer.num_frames(), frames_usize);
        assert_eq!(output_buffer.num_channels(), 2);
        assert_eq!(output_buffer.num_frames(), frames_usize);
        output_buffer.clear();

        let expected_channels = self.get_number_of_input_channels();
        if input_buffer.num_channels() != expected_channels {
            return;
        }

        for audio_element in &self.audio_elements {
            if !audio_element.is_passthrough() {
                continue;
            }
            let first_ch = audio_element.get_first_channel_index();
            // TODO(b/525080422): Optimize by retrieving output buffer slices once at the start
            // of process using as_slices_mut to avoid repeated channel_mut calls in the loop.
            // Mono passthrough (ZOA) duplicates its single channel to both ears; stereo
            // passthrough (Binaural) maps channels 0 and 1 to left and right ears respectively.
            let right_ch = if audio_element.get_number_of_input_channels() == 2 {
                first_ch + 1
            } else {
                first_ch
            };
            let out_l = output_buffer.channel_mut(0);
            add_pointwise_in_place(out_l.data, input_buffer.channel(first_ch).data);
            let out_r = output_buffer.channel_mut(1);
            add_pointwise_in_place(out_r.data, input_buffer.channel(right_ch).data);
        }

        if self.processing_groups.is_empty() {
            return;
        }

        let audio_elements = &self.audio_elements;
        let head_tracking_enabled = self.head_tracking_enabled;
        let world_rotation = &self.world_rotation;

        for group in &mut self.processing_groups {
            self.scratch_group_output.clear();
            group.process(
                input_buffer,
                audio_elements,
                head_tracking_enabled,
                world_rotation,
                &mut self.scratch_group_output,
                &mut self.fft_manager,
            );

            // TODO(b/525080422): Optimize by retrieving output buffer slices once at the start of
            // process using as_slices_mut to avoid repeated channel_mut calls in the loop.
            {
                let src = self.scratch_group_output.channel(0);
                let dst = output_buffer.channel_mut(0);
                add_pointwise_in_place(dst.data, src.data);
            }
            {
                let src = self.scratch_group_output.channel(1);
                let dst = output_buffer.channel_mut(1);
                add_pointwise_in_place(dst.data, src.data);
            }
        }
    }

    /// Returns the buffer size per channel.
    pub fn get_buffer_size_per_channel(&self) -> Samples {
        self.buffer_size_per_channel
    }

    /// Returns the total number of input channels across all elements.
    pub fn get_number_of_input_channels(&self) -> usize {
        self.audio_elements.iter().map(|e| e.get_number_of_input_channels()).sum()
    }

    /// Returns the number of output channels (always 2 for stereo binaural).
    #[cfg(test)]
    pub fn get_number_of_output_channels(&self) -> usize {
        NUM_BINAURAL_CHANNELS
    }

    /// Returns the number of configured audio elements.
    pub fn get_number_of_audio_elements(&self) -> usize {
        self.audio_elements.len()
    }
}

/// Struct to adapt and expose OBR through the AudioRenderer Trait.
#[derive(Debug)]
pub struct ObrAudioRendererAdapter {
    pub obr: ObrImpl,
    pub id_to_element_index: HashMap<u32, usize>,
    element_channels: HashMap<u32, ChannelCount>,
    scratch_input: AudioBuffer,
    scratch_output: AudioBuffer,
    element_gains: HashMap<u32, HashMap<u32, GainParameter>>,
    gain_scratch_buf: Vec<f32>,
}

impl ObrAudioRendererAdapter {
    /// Constructs a new `ObrAudioRendererAdapter` instance.
    pub fn new(samples_per_channel: Samples, sampling_rate: SampleRate) -> Self {
        let size_usize = samples_per_channel.value() as usize;
        Self {
            obr: ObrImpl::new(samples_per_channel, sampling_rate),
            id_to_element_index: HashMap::new(),
            element_channels: HashMap::new(),
            scratch_input: AudioBuffer::default(),
            scratch_output: AudioBuffer::new(NUM_BINAURAL_CHANNELS, size_usize),
            element_gains: HashMap::new(),
            gain_scratch_buf: vec![1.0f32; size_usize],
        }
    }
    fn get_element_channel_offset(&self, id: u32) -> usize {
        let target_idx = self.id_to_element_index.get(&id).copied().unwrap_or(0);
        let mut offset = 0;
        for (&eid, &idx) in &self.id_to_element_index {
            if idx < target_idx {
                offset += self.element_channels.get(&eid).map_or(0, |c| c.get());
            }
        }
        offset
    }
}

impl AudioRenderer for ObrAudioRendererAdapter {
    fn add_element(&mut self, id: u32, config: &AudioElementConfig) -> Result<(), OarError> {
        if self.id_to_element_index.contains_key(&id) {
            return Err(OarError::InvalidParameter);
        }
        self.obr.add_audio_element(config)?;
        let idx = self.obr.get_number_of_audio_elements() - 1;
        self.id_to_element_index.insert(id, idx);
        self.element_channels.insert(id, config.channels());

        let expected_ins = self.obr.get_number_of_input_channels();
        let frames_usize = self.obr.get_buffer_size_per_channel().value() as usize;
        self.scratch_input.resize(expected_ins, frames_usize);
        Ok(())
    }

    fn remove_element(&mut self, id: u32) -> Result<(), OarError> {
        let &removed_idx = self.id_to_element_index.get(&id).ok_or(OarError::InvalidParameter)?;
        self.obr.remove_audio_element(removed_idx)?;
        self.id_to_element_index.remove(&id);
        for idx in self.id_to_element_index.values_mut() {
            if *idx > removed_idx {
                *idx -= 1;
            }
        }
        self.element_channels.remove(&id);
        self.element_gains.remove(&id);
        let expected_ins = self.obr.get_number_of_input_channels();
        let frames_usize = self.obr.get_buffer_size_per_channel().value() as usize;
        self.scratch_input.resize(expected_ins, frames_usize);
        Ok(())
    }

    fn update_element_gain(
        &mut self,
        id: u32,
        gain_id: u32,
        gain: &Gain,
        duration: Samples,
    ) -> Result<(), OarError> {
        if !self.id_to_element_index.contains_key(&id) {
            return Err(OarError::InvalidParameter);
        }
        let params = self.element_gains.entry(id).or_default();
        let param = params.entry(gain_id).or_insert_with(|| GainParameter::new(gain_id));
        param.updates.push_back(GainUpdate { gain: gain.variant.clone(), duration, elapsed: 0 });
        Ok(())
    }

    fn update_element_positions(
        &mut self,
        id: u32,
        positions: &ObjectPosition,
        _duration: Samples,
    ) -> Result<(), OarError> {
        let &idx = self.id_to_element_index.get(&id).ok_or(OarError::InvalidParameter)?;
        match positions {
            ObjectPosition::Polar(polar_coords) => {
                for (i, polar_coord) in
                    polar_coords.iter().enumerate().take(polar_coords.len().min(2))
                {
                    self.obr.update_object_channel_position(
                        idx,
                        i,
                        polar_coord.azimuth().value(),
                        polar_coord.elevation().value(),
                        polar_coord.distance().value(),
                    )?;
                }
                Ok(())
            }
            _ => Err(OarError::NotSupported),
        }
    }

    fn render(
        &mut self,
        inputs: PlanarBufferRef<'_, '_>,
        output: &mut PlanarBufferMut<'_, '_>,
    ) -> Result<(), OarError> {
        let expected_ins = self.obr.get_number_of_input_channels();
        if inputs.num_channels() != expected_ins {
            return Err(OarError::InvalidParameter);
        }
        let frames = self.obr.get_buffer_size_per_channel();
        let frames_usize = frames.value() as usize;
        // TODO(b/525080422): Avoid resizing scratch buffers in the render path as it can cause heap
        // allocations. We should know the number of samples needed from the first creation of the
        // ROAR instance and can use that to initialize buffers as needed.
        if self.scratch_input.num_channels() != expected_ins
            || self.scratch_input.num_frames() != frames_usize
        {
            self.scratch_input.resize(expected_ins, frames_usize);
        }
        for ch in 0..expected_ins {
            let src = inputs.channel(ch);
            let mut dst = self.scratch_input.channel_mut(ch);
            let len = src.len().min(frames_usize);
            dst.as_mut_slice()[..len].copy_from_slice(&src[..len]);
        }
        for &eid in self.id_to_element_index.keys() {
            let offset = self.get_element_channel_offset(eid);
            let num_chans = self.element_channels.get(&eid).map_or(0, |c| c.get());
            if let Some(gains) = self.element_gains.get_mut(&eid) {
                for param in gains.values_mut() {
                    generate_gain_multipliers(param, frames_usize, &mut self.gain_scratch_buf);
                    for ch in offset..offset + num_chans {
                        let mut dst = self.scratch_input.channel_mut(ch);
                        for (sample, &gain) in
                            dst.as_mut_slice().iter_mut().zip(self.gain_scratch_buf.iter())
                        {
                            *sample *= gain;
                        }
                    }
                }
            }
        }
        self.obr.process(&self.scratch_input, &mut self.scratch_output);
        if output.num_channels() < 2 || output.num_samples() < frames_usize {
            return Err(OarError::InvalidParameter);
        }
        let ch0 = self.scratch_output.channel(0);
        let ch1 = self.scratch_output.channel(1);
        let s0 = ch0.as_slice();
        let s1 = ch1.as_slice();
        output.channel_mut(0)[..frames_usize].copy_from_slice(&s0[..frames_usize]);
        output.channel_mut(1)[..frames_usize].copy_from_slice(&s1[..frames_usize]);
        Ok(())
    }

    fn enable_head_tracking(&mut self, enable: bool) -> Result<(), OarError> {
        self.obr.enable_head_tracking(enable);
        Ok(())
    }

    fn set_head_rotation(&mut self, rotation: Quaternion) -> Result<(), OarError> {
        self.obr.set_head_rotation(rotation)
    }

    fn set_metadata_unit_to_process(&mut self, samples: Samples) -> Result<(), OarError> {
        if self.obr.get_buffer_size_per_channel() == samples {
            return Ok(());
        }
        self.obr.set_buffer_size_per_channel(samples)?;
        let size_usize = samples.value() as usize;
        let expected_ins = self.obr.get_number_of_input_channels();
        self.scratch_input.resize(expected_ins, size_usize);
        self.scratch_output = AudioBuffer::new(NUM_BINAURAL_CHANNELS, size_usize);
        self.gain_scratch_buf.resize(size_usize, 1.0);
        Ok(())
    }
}

// ===== Tests =====

#[cfg(test)]
mod test {

    use super::*;
    use crate::common::definitions::{
        BinauralFilterProfile, ChannelBasedConfig, ElementRenderingConfig, HeadphonesRenderingMode,
        HighOrderAmbisonics, InputLayout, ObjectBasedConfig, SceneBasedConfig,
    };
    use googletest::prelude::*;
    use std::f32::consts::PI;

    const BUFFER_SIZE: usize = 128;
    const SAMPLING_RATE: i32 = 48000;

    fn get_sample_rate() -> SampleRate {
        SampleRate::new(SAMPLING_RATE as u32).unwrap()
    }

    fn get_buffer_size() -> Samples {
        Samples::new(BUFFER_SIZE as u32).unwrap()
    }

    fn ambisonics_config(
        order: HighOrderAmbisonics,
        binaural_filter_profile: BinauralFilterProfile,
    ) -> AudioElementConfig {
        AudioElementConfig::SceneBased(SceneBasedConfig {
            order,
            rendering_config: Some(ElementRenderingConfig {
                headphones_rendering_mode: HeadphonesRenderingMode::WorldLockedRestricted,
                binaural_filter_profile,
            }),
        })
    }

    fn layout_config(
        layout: InputLayout,
        binaural_filter_profile: BinauralFilterProfile,
    ) -> AudioElementConfig {
        AudioElementConfig::ChannelBased(ChannelBasedConfig {
            layout,
            downmix_info: None,
            rendering_config: Some(ElementRenderingConfig {
                headphones_rendering_mode: HeadphonesRenderingMode::WorldLockedRestricted,
                binaural_filter_profile,
            }),
        })
    }

    fn mono_object_config(
        binaural_filter_profile: BinauralFilterProfile,
        headphones_rendering_mode: HeadphonesRenderingMode,
    ) -> AudioElementConfig {
        AudioElementConfig::ObjectBased(
            ObjectBasedConfig::new(
                1,
                Some(ElementRenderingConfig { headphones_rendering_mode, binaural_filter_profile }),
            )
            .unwrap(),
        )
    }

    fn generate_sine(sampling_rate: i32, channel: &mut [f32]) {
        let len = channel.len();
        let freq = 440.0;
        let amplitude = 0.5;
        for (i, chan) in channel.iter_mut().enumerate().take(len) {
            *chan = amplitude * (2.0 * PI * freq * (i as f32) / (sampling_rate as f32)).sin();
        }
    }

    fn has_non_zero_output(buffer: &AudioBuffer) -> bool {
        for ch in 0..buffer.num_channels() {
            for &sample in buffer.channel(ch).as_slice() {
                if sample.abs() > 1e-6 {
                    return true;
                }
            }
        }
        false
    }

    #[gtest]
    fn test_initialization() {
        let renderer = ObrImpl::new(get_buffer_size(), get_sample_rate());
        expect_that!(renderer.buffer_size_per_channel, eq(get_buffer_size()));
        expect_that!(renderer.sampling_rate, eq(get_sample_rate()));
        expect_that!(renderer.get_number_of_input_channels(), eq(0));
        expect_that!(renderer.get_number_of_output_channels(), eq(2));
    }

    #[gtest]
    fn test_add_and_remove_audio_element() {
        let mut renderer = ObrImpl::new(get_buffer_size(), get_sample_rate());
        let add_result = renderer.add_audio_element(&ambisonics_config(
            HighOrderAmbisonics::Order3,
            BinauralFilterProfile::Direct,
        ));

        expect_true!(add_result.is_ok());
        expect_that!(renderer.get_number_of_input_channels(), eq(16));

        let remove_result = renderer.remove_audio_element(0);

        expect_true!(remove_result.is_ok());
        expect_that!(renderer.get_number_of_input_channels(), eq(0));
    }

    #[gtest]
    fn test_add_and_remove_multiple_audio_elements() {
        let mut renderer = ObrImpl::new(get_buffer_size(), get_sample_rate());
        let add1 = renderer.add_audio_element(&ambisonics_config(
            HighOrderAmbisonics::Order3,
            BinauralFilterProfile::Direct,
        ));

        expect_true!(add1.is_ok());
        expect_that!(renderer.get_number_of_input_channels(), eq(16));

        let add2 = renderer.add_audio_element(&layout_config(
            InputLayout::Layout714,
            BinauralFilterProfile::Direct,
        ));

        expect_true!(add2.is_ok());
        expect_that!(renderer.get_number_of_input_channels(), eq(28));
        expect_that!(renderer.audio_elements.len(), eq(2));

        let remove1 = renderer.remove_audio_element(1);

        expect_true!(remove1.is_ok());
        expect_that!(renderer.get_number_of_input_channels(), eq(16));
        expect_that!(renderer.audio_elements.len(), eq(1));

        let remove2 = renderer.remove_audio_element(0);

        expect_true!(remove2.is_ok());
        expect_that!(renderer.get_number_of_input_channels(), eq(0));
        expect_that!(renderer.audio_elements.len(), eq(0));

        let remove3 = renderer.remove_audio_element(0);

        expect_true!(remove3.is_err());
    }

    #[gtest]
    fn remove_first_audio_element_updates_remaining_channel_offsets() {
        let mut renderer = ObrImpl::new(get_buffer_size(), get_sample_rate());
        expect_true!(renderer
            .add_audio_element(&ambisonics_config(
                HighOrderAmbisonics::Order3,
                BinauralFilterProfile::Direct,
            ))
            .is_ok());
        expect_true!(renderer
            .add_audio_element(&layout_config(InputLayout::Stereo, BinauralFilterProfile::Direct))
            .is_ok());
        expect_that!(renderer.get_number_of_input_channels(), eq(18));

        let remove_first = renderer.remove_audio_element(0);

        expect_true!(remove_first.is_ok());
        expect_that!(renderer.audio_elements.len(), eq(1));
        expect_that!(renderer.get_number_of_input_channels(), eq(2));
        expect_that!(renderer.audio_elements[0].get_first_channel_index(), eq(0));

        let mut input_buffer = AudioBuffer::new(2, BUFFER_SIZE);
        generate_sine(SAMPLING_RATE, input_buffer.channel_mut(0).as_mut_slice());
        generate_sine(SAMPLING_RATE, input_buffer.channel_mut(1).as_mut_slice());
        let mut output_buffer = AudioBuffer::new(2, BUFFER_SIZE);
        renderer.process(&input_buffer, &mut output_buffer);
        expect_that!(has_non_zero_output(&output_buffer), eq(true));
    }

    #[gtest]
    fn test_process_audio_buffer_with_wrong_number_of_channels() {
        let mut renderer = ObrImpl::new(get_buffer_size(), get_sample_rate());
        let add = renderer.add_audio_element(&ambisonics_config(
            HighOrderAmbisonics::Order3,
            BinauralFilterProfile::Direct,
        ));

        expect_true!(add.is_ok());

        let input_buffer = AudioBuffer::new(17, BUFFER_SIZE);
        let mut output_buffer = AudioBuffer::new(2, BUFFER_SIZE);
        output_buffer.channel_mut(0).as_mut_slice().fill(1.0);
        output_buffer.channel_mut(1).as_mut_slice().fill(1.0);

        renderer.process(&input_buffer, &mut output_buffer);

        for ch in 0..2 {
            for &sample in output_buffer.channel(ch).as_slice() {
                expect_that!(sample, eq(0.0));
            }
        }
    }

    #[gtest]
    fn test_add_multiple_audio_elements_with_different_types_same_filter_type_succeeds() {
        let mut renderer = ObrImpl::new(get_buffer_size(), get_sample_rate());
        let add1 = renderer.add_audio_element(&ambisonics_config(
            HighOrderAmbisonics::Order3,
            BinauralFilterProfile::Direct,
        ));

        expect_true!(add1.is_ok());

        let add2 = renderer.add_audio_element(&ambisonics_config(
            HighOrderAmbisonics::Order1,
            BinauralFilterProfile::Direct,
        ));

        expect_true!(add2.is_ok());

        expect_that!(renderer.get_number_of_input_channels(), eq(20));
        expect_that!(renderer.audio_elements.len(), eq(2));
        expect_that!(renderer.processing_groups.len(), eq(2));
    }

    #[gtest]
    fn test_add_multiple_audio_elements_same_type_different_filter_types_succeeds() {
        let mut renderer = ObrImpl::new(get_buffer_size(), get_sample_rate());
        let add1 = renderer.add_audio_element(&ambisonics_config(
            HighOrderAmbisonics::Order3,
            BinauralFilterProfile::Direct,
        ));

        expect_true!(add1.is_ok());

        let add2 = renderer.add_audio_element(&ambisonics_config(
            HighOrderAmbisonics::Order3,
            BinauralFilterProfile::Reverberant,
        ));

        expect_true!(add2.is_ok());

        expect_that!(renderer.get_number_of_input_channels(), eq(32));
        expect_that!(renderer.audio_elements.len(), eq(2));
        expect_that!(renderer.processing_groups.len(), eq(2));
    }

    #[gtest]
    fn test_ambisonics_and_loudspeaker_layouts_same_filter_profile() {
        let mut renderer = ObrImpl::new(get_buffer_size(), get_sample_rate());
        let add1 = renderer.add_audio_element(&ambisonics_config(
            HighOrderAmbisonics::Order2,
            BinauralFilterProfile::Direct,
        ));

        expect_true!(add1.is_ok());

        let add2 = renderer.add_audio_element(&layout_config(
            InputLayout::Layout714,
            BinauralFilterProfile::Direct,
        ));

        expect_true!(add2.is_ok());

        expect_that!(renderer.get_number_of_input_channels(), eq(21));
        expect_that!(renderer.processing_groups.len(), eq(2));
    }

    #[gtest]
    fn test_passthrough_mono() {
        let mut renderer = ObrImpl::new(get_buffer_size(), get_sample_rate());
        let add = renderer.add_audio_element(&ambisonics_config(
            HighOrderAmbisonics::Zoa,
            BinauralFilterProfile::Direct,
        ));

        expect_true!(add.is_ok());

        let mut input_buffer = AudioBuffer::new(1, BUFFER_SIZE);
        generate_sine(SAMPLING_RATE, input_buffer.channel_mut(0).as_mut_slice());

        let mut output_buffer = AudioBuffer::new(2, BUFFER_SIZE);
        renderer.process(&input_buffer, &mut output_buffer);

        for i in 0..BUFFER_SIZE {
            expect_that!(
                output_buffer.channel(0).as_slice()[i],
                eq(input_buffer.channel(0).as_slice()[i])
            );
            expect_that!(
                output_buffer.channel(1).as_slice()[i],
                eq(input_buffer.channel(0).as_slice()[i])
            );
        }
    }

    #[gtest]
    fn test_passthrough_stereo() {
        let mut renderer = ObrImpl::new(get_buffer_size(), get_sample_rate());
        let add = renderer.add_audio_element(&layout_config(
            InputLayout::Binaural,
            BinauralFilterProfile::Direct,
        ));

        expect_true!(add.is_ok());

        let mut input_buffer = AudioBuffer::new(2, BUFFER_SIZE);
        generate_sine(SAMPLING_RATE, input_buffer.channel_mut(0).as_mut_slice());
        for i in 0..BUFFER_SIZE {
            input_buffer.channel_mut(1).as_mut_slice()[i] = -input_buffer.channel(0).as_slice()[i];
        }

        let mut output_buffer = AudioBuffer::new(2, BUFFER_SIZE);
        renderer.process(&input_buffer, &mut output_buffer);

        for i in 0..BUFFER_SIZE {
            expect_that!(
                output_buffer.channel(0).as_slice()[i],
                eq(input_buffer.channel(0).as_slice()[i])
            );
            expect_that!(
                output_buffer.channel(1).as_slice()[i],
                eq(input_buffer.channel(1).as_slice()[i])
            );
        }
    }

    #[gtest]
    fn test_process_mixed_ambisonics_and_object() {
        let mut renderer = ObrImpl::new(get_buffer_size(), get_sample_rate());
        let add1 = renderer.add_audio_element(&ambisonics_config(
            HighOrderAmbisonics::Order1,
            BinauralFilterProfile::Direct,
        ));

        expect_true!(add1.is_ok());

        let add2 = renderer.add_audio_element(&mono_object_config(
            BinauralFilterProfile::Direct,
            HeadphonesRenderingMode::WorldLockedRestricted,
        ));

        expect_true!(add2.is_ok());

        let mut input_buffer = AudioBuffer::new(5, BUFFER_SIZE);
        for ch in 0..5 {
            generate_sine(SAMPLING_RATE, input_buffer.channel_mut(ch).as_mut_slice());
        }

        let mut output_buffer = AudioBuffer::new(2, BUFFER_SIZE);
        renderer.process(&input_buffer, &mut output_buffer);

        expect_that!(has_non_zero_output(&output_buffer), eq(true));
    }

    #[gtest]
    fn set_buffer_size_per_channel_with_same_value_preserves_buffers() {
        let mut renderer = ObrImpl::new(get_buffer_size(), get_sample_rate());
        let initial_group_ptr = renderer.scratch_group_output.channel(0).as_slice().as_ptr();

        let result = renderer.set_buffer_size_per_channel(get_buffer_size());

        expect_ok!(result);
        expect_eq!(renderer.scratch_group_output.channel(0).as_slice().as_ptr(), initial_group_ptr);
    }

    #[gtest]
    fn set_buffer_size_per_channel_with_new_value_resizes_buffers() -> Result<()> {
        let mut renderer = ObrImpl::new(get_buffer_size(), get_sample_rate());
        let new_size = Samples::new(64)?;

        renderer.set_buffer_size_per_channel(new_size)?;

        expect_eq!(renderer.get_buffer_size_per_channel(), new_size);
        expect_eq!(renderer.scratch_group_output.num_frames(), 64);
        Ok(())
    }

    #[gtest]
    fn add_element_with_binaural_layout_configures_stereo_passthrough() {
        let mut adapter = ObrAudioRendererAdapter::new(get_buffer_size(), get_sample_rate());
        let config = AudioElementConfig::ChannelBased(ChannelBasedConfig {
            layout: InputLayout::Binaural,
            downmix_info: None,
            rendering_config: None,
        });

        let result = adapter.add_element(1, &config);

        expect_ok!(result);
        expect_eq!(adapter.obr.get_number_of_input_channels(), 2);
        expect_true!(adapter.obr.audio_elements[0].is_passthrough());
        expect_true!(adapter.obr.audio_elements[0].is_head_locked());
    }

    #[gtest]
    fn add_audio_element_with_head_locked_ignores_head_rotation() {
        let yaw_90 = Quaternion::new(0.70710677, 0.0, 0.0, 0.70710677).unwrap();
        let mut renderer = ObrImpl::new(get_buffer_size(), get_sample_rate());
        renderer
            .add_audio_element(&mono_object_config(
                BinauralFilterProfile::Direct,
                HeadphonesRenderingMode::HeadLocked,
            ))
            .unwrap();
        renderer.update_object_channel_position(0, 0, 90.0, 0.0, 1.0).unwrap();
        renderer.enable_head_tracking(true);
        renderer.set_head_rotation(yaw_90).unwrap();

        let mut unrotated_reference = ObrImpl::new(get_buffer_size(), get_sample_rate());
        unrotated_reference
            .add_audio_element(&mono_object_config(
                BinauralFilterProfile::Direct,
                HeadphonesRenderingMode::WorldLockedRestricted,
            ))
            .unwrap();
        unrotated_reference.update_object_channel_position(0, 0, 90.0, 0.0, 1.0).unwrap();

        let mut input = AudioBuffer::new(1, BUFFER_SIZE);
        generate_sine(SAMPLING_RATE, input.channel_mut(0).as_mut_slice());
        let mut head_locked_output = AudioBuffer::new(2, BUFFER_SIZE);
        let mut reference_output = AudioBuffer::new(2, BUFFER_SIZE);

        renderer.process(&input, &mut head_locked_output);
        unrotated_reference.process(&input, &mut reference_output);

        for ch in 0..2 {
            expect_eq!(
                head_locked_output.channel(ch).as_slice(),
                reference_output.channel(ch).as_slice()
            );
        }
    }

    #[gtest]
    fn add_audio_element_mixed_world_and_head_locked_isolates_encoders() {
        let mut renderer = ObrImpl::new(get_buffer_size(), get_sample_rate());
        renderer
            .add_audio_element(&mono_object_config(
                BinauralFilterProfile::Direct,
                HeadphonesRenderingMode::HeadLocked,
            ))
            .unwrap();
        renderer
            .add_audio_element(&mono_object_config(
                BinauralFilterProfile::Direct,
                HeadphonesRenderingMode::WorldLockedRestricted,
            ))
            .unwrap();
        renderer.update_object_channel_position(0, 0, 90.0, 0.0, 1.0).unwrap();
        renderer.update_object_channel_position(1, 0, -90.0, 0.0, 1.0).unwrap();

        let mut reference = ObrImpl::new(get_buffer_size(), get_sample_rate());
        reference
            .add_audio_element(&mono_object_config(
                BinauralFilterProfile::Direct,
                HeadphonesRenderingMode::WorldLockedRestricted,
            ))
            .unwrap();
        reference.update_object_channel_position(0, 0, -90.0, 0.0, 1.0).unwrap();

        let mut input = AudioBuffer::new(2, BUFFER_SIZE);
        generate_sine(SAMPLING_RATE, input.channel_mut(1).as_mut_slice());
        let mut reference_input = AudioBuffer::new(1, BUFFER_SIZE);
        generate_sine(SAMPLING_RATE, reference_input.channel_mut(0).as_mut_slice());
        let mut output = AudioBuffer::new(2, BUFFER_SIZE);
        let mut reference_output = AudioBuffer::new(2, BUFFER_SIZE);

        renderer.process(&input, &mut output);
        reference.process(&reference_input, &mut reference_output);

        for ch in 0..2 {
            expect_eq!(output.channel(ch).as_slice(), reference_output.channel(ch).as_slice());
        }
    }

    #[gtest]
    fn remove_first_element_in_adapter_shifts_remaining_element_indices() {
        let mut adapter = ObrAudioRendererAdapter::new(get_buffer_size(), get_sample_rate());
        let obj_config =
            mono_object_config(BinauralFilterProfile::Direct, HeadphonesRenderingMode::WorldLocked);
        adapter.add_element(10, &obj_config).unwrap();
        adapter.add_element(20, &obj_config).unwrap();
        let position = ObjectPosition::Polar(vec![
            crate::common::definitions::PolarCoordinate::new_from_floats(45.0, 0.0, 1.0).unwrap(),
        ]);

        let remove_first = adapter.remove_element(10);
        let update_second = adapter.update_element_positions(20, &position, get_buffer_size());
        let remove_second = adapter.remove_element(20);

        expect_ok!(remove_first);
        expect_ok!(update_second);
        expect_ok!(remove_second);
        expect_eq!(adapter.obr.get_number_of_audio_elements(), 0);
    }

    /// Verifies that removing a middle element from `ObrAudioRendererAdapter` shifts only higher
    /// element indices while leaving lower element indices unchanged.
    #[gtest]
    fn remove_middle_element_in_adapter_shifts_only_higher_element_indices() {
        let mut adapter = ObrAudioRendererAdapter::new(get_buffer_size(), get_sample_rate());
        let obj_config =
            mono_object_config(BinauralFilterProfile::Direct, HeadphonesRenderingMode::WorldLocked);
        adapter.add_element(10, &obj_config).unwrap();
        adapter.add_element(20, &obj_config).unwrap();
        adapter.add_element(30, &obj_config).unwrap();
        let position = ObjectPosition::Polar(vec![
            crate::common::definitions::PolarCoordinate::new_from_floats(45.0, 0.0, 1.0).unwrap(),
        ]);

        let remove_middle = adapter.remove_element(20);
        let first_idx_after_middle = adapter.id_to_element_index.get(&10).copied();
        let third_idx_after_middle = adapter.id_to_element_index.get(&30).copied();
        let update_first = adapter.update_element_positions(10, &position, get_buffer_size());
        let update_third = adapter.update_element_positions(30, &position, get_buffer_size());
        let remove_third = adapter.remove_element(30);
        let remove_first = adapter.remove_element(10);

        expect_ok!(remove_middle);
        expect_eq!(first_idx_after_middle, Some(0));
        expect_eq!(third_idx_after_middle, Some(1));
        expect_ok!(update_first);
        expect_ok!(update_third);
        expect_ok!(remove_third);
        expect_ok!(remove_first);
        expect_eq!(adapter.obr.get_number_of_audio_elements(), 0);
    }
}
