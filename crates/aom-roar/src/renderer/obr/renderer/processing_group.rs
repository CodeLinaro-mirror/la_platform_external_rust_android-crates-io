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

//! Processing groups grouping audio elements.
//!
//! Audio elements that share the same Ambisonic order and binaural filter profile are grouped
//! into a single `ProcessingGroup`. This optimizes DSP processing by reducing the number of
//! rotations, convolutions, and FFT operations.

use super::audio_element_config::ObrAudioElement;
use crate::common::audio_buffer::AudioBuffer;
use crate::common::definitions::{
    BinauralFilterProfile, Distance, LinearGain, OarError, Quaternion, SampleRate, Samples,
};
use crate::renderer::obr::ambisonic_binaural_decoder::{
    create_sh_hrirs_from_assets, AmbisonicBinauralDecoder, FftManager, Resampler,
};
use crate::renderer::obr::ambisonic_encoder::AmbisonicEncoder;
use crate::renderer::obr::ambisonic_rotator::AmbisonicRotator;
use crate::renderer::obr::common::ambisonic_utils::get_num_periphonic_components;
use crate::renderer::obr::common::constants::{
    MAX_SUPPORTED_AMBISONIC_ORDER, MIN_SUPPORTED_AMBISONIC_ORDER, NUM_BINAURAL_CHANNELS,
};

/// Key representing the unique attributes of a processing group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProcessingGroupKey {
    /// The Ambisonic order of the group's input elements.
    pub ambisonic_order: i32,
    /// The target binaural filter profile.
    pub filter_profile: BinauralFilterProfile,
}

/// Coordinator for DSP operations within a single processing group.
///
/// Manages the Ambisonic encoder, rotator, and binaural decoder resources shared
/// by all audio elements assigned to this group.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessingGroup {
    key: ProcessingGroupKey,
    audio_element_indices: Vec<usize>,
    buffer_size_per_channel: Samples,
    sampling_rate: SampleRate,
    ambisonic_mix_bed: AudioBuffer,
    ambisonic_mix_bed_head_locked: AudioBuffer,
    ambisonic_encoder: Option<AmbisonicEncoder>,
    ambisonic_encoder_input_buffer: AudioBuffer,
    ambisonic_encoder_head_locked: Option<AmbisonicEncoder>,
    ambisonic_encoder_input_buffer_head_locked: AudioBuffer,
    ambisonic_rotator: Option<AmbisonicRotator>,
    ambisonic_binaural_decoder: Option<AmbisonicBinauralDecoder>,
    world_locked_indices: Vec<usize>,
    head_locked_indices: Vec<usize>,
}

impl ProcessingGroup {
    /// Constructs a new `ProcessingGroup`.
    pub fn new(
        key: ProcessingGroupKey,
        audio_element_indices: Vec<usize>,
        buffer_size_per_channel: Samples,
        sampling_rate: SampleRate,
    ) -> Self {
        let size_usize = buffer_size_per_channel.value() as usize;
        let num_ambisonic_channels = get_num_periphonic_components(key.ambisonic_order);
        Self {
            key,
            audio_element_indices,
            buffer_size_per_channel,
            sampling_rate,
            ambisonic_mix_bed: AudioBuffer::new(num_ambisonic_channels, size_usize),
            ambisonic_mix_bed_head_locked: AudioBuffer::new(num_ambisonic_channels, size_usize),
            ambisonic_encoder: None,
            ambisonic_encoder_input_buffer: AudioBuffer::default(),
            ambisonic_encoder_head_locked: None,
            ambisonic_encoder_input_buffer_head_locked: AudioBuffer::default(),
            ambisonic_rotator: None,
            ambisonic_binaural_decoder: None,
            // TODO(b/525080422): Pre-allocate these vectors to avoid heap allocations in the render
            // path. Could we know the number of input channels before creation to avoid allocating
            // a maximum capacity?
            world_locked_indices: Vec::new(),
            head_locked_indices: Vec::new(),
        }
    }

    /// Initializes DSP resources (rotator and binaural decoder) using given assets/resampler.
    ///
    /// # Parameters
    /// * `fft_manager` - Context for FFT and IFFT transforms.
    /// * `resampler` - State resampler for loading filters at custom sample rates.
    pub fn initialize(
        &mut self,
        fft_manager: &mut FftManager,
        resampler: &mut Resampler,
    ) -> Result<(), OarError> {
        let order = self.key.ambisonic_order;
        if !(MIN_SUPPORTED_AMBISONIC_ORDER..=MAX_SUPPORTED_AMBISONIC_ORDER).contains(&order) {
            return Err(OarError::InvalidParameter);
        }

        let frames_per_buffer = self.buffer_size_per_channel.value() as usize;
        self.ambisonic_rotator = Some(AmbisonicRotator::new(order, frames_per_buffer));

        let profile_str = match self.key.filter_profile {
            BinauralFilterProfile::Direct => "Direct",
            BinauralFilterProfile::Ambient => "Ambient",
            BinauralFilterProfile::Reverberant => "Reverberant",
        };

        let hrir_l_name = format!("{}OA{}L", order, profile_str);
        let hrir_r_name = format!("{}OA{}R", order, profile_str);

        let sh_hrirs_l = create_sh_hrirs_from_assets(&hrir_l_name, self.sampling_rate, resampler)
            .ok_or(OarError::InvalidParameter)?;
        let sh_hrirs_r = create_sh_hrirs_from_assets(&hrir_r_name, self.sampling_rate, resampler)
            .ok_or(OarError::InvalidParameter)?;

        assert_eq!(sh_hrirs_l.num_channels(), sh_hrirs_r.num_channels());
        assert_eq!(sh_hrirs_l.num_frames(), sh_hrirs_r.num_frames());

        self.ambisonic_binaural_decoder = Some(AmbisonicBinauralDecoder::new(
            &sh_hrirs_l,
            &sh_hrirs_r,
            frames_per_buffer,
            fft_manager,
        ));

        Ok(())
    }

    /// Processes world/head-locked audio elements in this group into a binaural `output_buffer`.
    pub fn process(
        &mut self,
        input_buffer: &AudioBuffer,
        audio_elements: &[ObrAudioElement],
        head_tracking_enabled: bool,
        world_rotation: &Quaternion,
        output_buffer: &mut AudioBuffer,
        fft_manager: &mut FftManager,
    ) {
        assert_eq!(output_buffer.num_channels(), NUM_BINAURAL_CHANNELS);
        assert_eq!(output_buffer.num_frames(), self.buffer_size_per_channel.value() as usize);

        self.ambisonic_mix_bed.clear();
        self.ambisonic_mix_bed_head_locked.clear();

        self.world_locked_indices.clear();
        Self::populate_encoder_source_channel_indices(
            &self.audio_element_indices,
            audio_elements,
            Some(false),
            &mut self.world_locked_indices,
        );
        if !self.world_locked_indices.is_empty() && self.ambisonic_encoder.is_some() {
            for (i, &src_idx) in self.world_locked_indices.iter().enumerate() {
                let src = input_buffer.channel(src_idx);
                let mut dst = self.ambisonic_encoder_input_buffer.channel_mut(i);
                dst.as_mut_slice().copy_from_slice(src.as_slice());
            }
            if let Some(enc) = &mut self.ambisonic_encoder {
                enc.process_planar_audio_data(
                    &self.ambisonic_encoder_input_buffer,
                    &mut self.ambisonic_mix_bed,
                );
            }
        }

        for &ae_index in &self.audio_element_indices {
            let audio_element = &audio_elements[ae_index];
            if audio_element.is_ambisonics() && !audio_element.is_head_locked() {
                for channel in 0..audio_element.get_number_of_input_channels() {
                    let mut bed_ch = self.ambisonic_mix_bed.channel_mut(channel);
                    let src_ch =
                        input_buffer.channel(audio_element.get_first_channel_index() + channel);
                    bed_ch.add_assign_view(&src_ch);
                }
            }
        }

        self.head_locked_indices.clear();
        Self::populate_encoder_source_channel_indices(
            &self.audio_element_indices,
            audio_elements,
            Some(true),
            &mut self.head_locked_indices,
        );
        if !self.head_locked_indices.is_empty() && self.ambisonic_encoder_head_locked.is_some() {
            for (i, &src_idx) in self.head_locked_indices.iter().enumerate() {
                let src = input_buffer.channel(src_idx);
                let mut dst = self.ambisonic_encoder_input_buffer_head_locked.channel_mut(i);
                dst.as_mut_slice().copy_from_slice(src.as_slice());
            }
            if let Some(enc) = &mut self.ambisonic_encoder_head_locked {
                enc.process_planar_audio_data(
                    &self.ambisonic_encoder_input_buffer_head_locked,
                    &mut self.ambisonic_mix_bed_head_locked,
                );
            }
        }

        for &ae_index in &self.audio_element_indices {
            let audio_element = &audio_elements[ae_index];
            if audio_element.is_ambisonics() && audio_element.is_head_locked() {
                for channel in 0..audio_element.get_number_of_input_channels() {
                    let mut bed_ch = self.ambisonic_mix_bed_head_locked.channel_mut(channel);
                    let src_ch =
                        input_buffer.channel(audio_element.get_first_channel_index() + channel);
                    bed_ch.add_assign_view(&src_ch);
                }
            }
        }

        if let Some(rot) = &mut self.ambisonic_rotator {
            if head_tracking_enabled {
                rot.process_in_place(world_rotation, &mut self.ambisonic_mix_bed);
            } else {
                rot.mark_audio_rendered();
            }
        }

        self.ambisonic_mix_bed += &self.ambisonic_mix_bed_head_locked;

        if let Some(dec) = &mut self.ambisonic_binaural_decoder {
            dec.process_audio_buffer(&self.ambisonic_mix_bed, output_buffer, fft_manager);
        }
    }

    /// Updates Ambisonic encoder source positions and channel inputs for both world-locked and
    /// head-locked elements in this processing group.
    pub fn update_ambisonic_encoder(
        &mut self,
        audio_elements: &[ObrAudioElement],
    ) -> Result<(), OarError> {
        let ambisonic_order = self.key.ambisonic_order as usize;
        Self::update_encoder_for_lock_mode(
            &self.audio_element_indices,
            self.buffer_size_per_channel,
            ambisonic_order,
            audio_elements,
            false,
            &mut self.ambisonic_encoder,
            &mut self.ambisonic_encoder_input_buffer,
        );
        Self::update_encoder_for_lock_mode(
            &self.audio_element_indices,
            self.buffer_size_per_channel,
            ambisonic_order,
            audio_elements,
            true,
            &mut self.ambisonic_encoder_head_locked,
            &mut self.ambisonic_encoder_input_buffer_head_locked,
        );
        Ok(())
    }
}

impl ProcessingGroup {
    /// Configures the Ambisonic encoder and input scratch buffer for elements matching
    /// `head_locked`. Separate encoders prevent head-locked and world-locked passes from
    /// overwriting each other's source positions, input channels, or ramp state.
    fn update_encoder_for_lock_mode(
        audio_element_indices: &[usize],
        buffer_size_per_channel: Samples,
        ambisonic_order: usize,
        audio_elements: &[ObrAudioElement],
        head_locked: bool,
        encoder: &mut Option<AmbisonicEncoder>,
        input_buffer: &mut AudioBuffer,
    ) {
        let mut expected_channels = 0;
        for &ae_index in audio_element_indices {
            let ae = &audio_elements[ae_index];
            if ae.is_head_locked() == head_locked && ae.uses_ambisonic_encoder() {
                expected_channels += ae.get_number_of_input_channels();
            }
        }

        if expected_channels == 0 {
            *encoder = None;
            *input_buffer = AudioBuffer::default();
            return;
        }

        if encoder.is_none() || input_buffer.num_channels() != expected_channels {
            *input_buffer =
                AudioBuffer::new(expected_channels, buffer_size_per_channel.value() as usize);
            *encoder = Some(AmbisonicEncoder::new(expected_channels, ambisonic_order));
        }

        if let Some(enc) = encoder {
            let mut enc_idx = 0;
            for &ae_index in audio_element_indices {
                let audio_element = &audio_elements[ae_index];
                if audio_element.is_head_locked() != head_locked {
                    continue;
                }
                for source in audio_element.get_loudspeaker_channels() {
                    enc.set_source(
                        enc_idx,
                        LinearGain::ONE,
                        source.azimuth,
                        source.elevation,
                        Distance::ONE,
                    );
                    enc_idx += 1;
                }
                for source in audio_element.object_positions() {
                    enc.set_source(
                        enc_idx,
                        LinearGain::ONE,
                        source.azimuth(),
                        source.elevation(),
                        source.distance(),
                    );
                    enc_idx += 1;
                }
            }
        }
    }
}

impl ProcessingGroup {
    fn populate_encoder_source_channel_indices(
        audio_element_indices: &[usize],
        audio_elements: &[ObrAudioElement],
        filter_head_locked: Option<bool>,
        out_indices: &mut Vec<usize>,
    ) {
        for &ae_index in audio_element_indices {
            let ae = &audio_elements[ae_index];
            if let Some(target_locked) = filter_head_locked
                && ae.is_head_locked() != target_locked
            {
                continue;
            }
            if ae.uses_ambisonic_encoder() {
                for i in 0..ae.get_number_of_input_channels() {
                    out_indices.push(ae.get_first_channel_index() + i);
                }
            }
        }
    }
}

#[cfg(test)]
impl ProcessingGroup {
    pub fn get_key(&self) -> ProcessingGroupKey {
        self.key
    }

    pub fn get_audio_element_indices(&self) -> &[usize] {
        &self.audio_element_indices
    }

    pub fn get_ambisonic_order(&self) -> i32 {
        self.key.ambisonic_order
    }

    pub fn get_filter_profile(&self) -> BinauralFilterProfile {
        self.key.filter_profile
    }
}

// ===== Tests =====

#[cfg(test)]
mod test {

    use super::*;
    use crate::common::definitions::{
        AudioElementConfig, ChannelBasedConfig, ElementRenderingConfig, HeadphonesRenderingMode,
        HighOrderAmbisonics, InputLayout, ObjectBasedConfig, PolarCoordinate, SceneBasedConfig,
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

    fn ambisonics_element(
        order: HighOrderAmbisonics,
        binaural_filter_profile: BinauralFilterProfile,
        headphones_rendering_mode: HeadphonesRenderingMode,
    ) -> ObrAudioElement {
        ObrAudioElement::new(AudioElementConfig::SceneBased(SceneBasedConfig {
            order,
            rendering_config: Some(ElementRenderingConfig {
                headphones_rendering_mode,
                binaural_filter_profile,
            }),
        }))
        .unwrap()
    }

    fn layout_element(
        layout: InputLayout,
        binaural_filter_profile: BinauralFilterProfile,
        headphones_rendering_mode: HeadphonesRenderingMode,
    ) -> ObrAudioElement {
        ObrAudioElement::new(AudioElementConfig::ChannelBased(ChannelBasedConfig {
            layout,
            downmix_info: None,
            rendering_config: Some(ElementRenderingConfig {
                headphones_rendering_mode,
                binaural_filter_profile,
            }),
        }))
        .unwrap()
    }

    fn mono_object_element(
        binaural_filter_profile: BinauralFilterProfile,
        headphones_rendering_mode: HeadphonesRenderingMode,
    ) -> ObrAudioElement {
        ObrAudioElement::new(AudioElementConfig::ObjectBased(
            ObjectBasedConfig::new(
                1,
                Some(ElementRenderingConfig { headphones_rendering_mode, binaural_filter_profile }),
            )
            .unwrap(),
        ))
        .unwrap()
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
    fn test_equality() {
        let key1 = ProcessingGroupKey {
            ambisonic_order: 1,
            filter_profile: BinauralFilterProfile::Direct,
        };
        let key2 = ProcessingGroupKey {
            ambisonic_order: 1,
            filter_profile: BinauralFilterProfile::Direct,
        };
        let key3 = ProcessingGroupKey {
            ambisonic_order: 2,
            filter_profile: BinauralFilterProfile::Direct,
        };
        let key4 = ProcessingGroupKey {
            ambisonic_order: 1,
            filter_profile: BinauralFilterProfile::Ambient,
        };

        expect_that!(key1 == key2, eq(true));
        expect_that!(key1 == key3, eq(false));
        expect_that!(key1 == key4, eq(false));
    }

    #[gtest]
    fn test_less_than() {
        let key1 = ProcessingGroupKey {
            ambisonic_order: 1,
            filter_profile: BinauralFilterProfile::Ambient,
        };
        let key2 = ProcessingGroupKey {
            ambisonic_order: 2,
            filter_profile: BinauralFilterProfile::Ambient,
        };
        let key3 = ProcessingGroupKey {
            ambisonic_order: 1,
            filter_profile: BinauralFilterProfile::Direct,
        };

        expect_that!(key1 < key2, eq(true));
        expect_that!(key2 < key1, eq(false));
        expect_that!(key1 < key3, eq(true));
    }

    #[gtest]
    fn test_construction() {
        let key = ProcessingGroupKey {
            ambisonic_order: 2,
            filter_profile: BinauralFilterProfile::Direct,
        };
        let indices = vec![0, 1];

        let group =
            ProcessingGroup::new(key, indices.clone(), get_buffer_size(), get_sample_rate());

        expect_that!(group.get_key().ambisonic_order, eq(2));
        expect_that!(group.get_key().filter_profile, eq(BinauralFilterProfile::Direct));
        expect_that!(group.get_audio_element_indices(), eq(&indices));
        expect_that!(group.get_ambisonic_order(), eq(2));
        expect_that!(group.get_filter_profile(), eq(BinauralFilterProfile::Direct));
    }

    #[gtest]
    fn test_initialize_direct_filter() {
        let key = ProcessingGroupKey {
            ambisonic_order: 1,
            filter_profile: BinauralFilterProfile::Direct,
        };
        let indices = vec![0];
        let mut group = ProcessingGroup::new(key, indices, get_buffer_size(), get_sample_rate());
        let mut fft_manager = FftManager::new(BUFFER_SIZE);
        let mut resampler = Resampler::default();

        expect_true!(group.initialize(&mut fft_manager, &mut resampler).is_ok());
    }

    #[gtest]
    fn test_initialize_ambient_filter() {
        let key = ProcessingGroupKey {
            ambisonic_order: 2,
            filter_profile: BinauralFilterProfile::Ambient,
        };
        let indices = vec![0, 1];
        let mut group = ProcessingGroup::new(key, indices, get_buffer_size(), get_sample_rate());
        let mut fft_manager = FftManager::new(BUFFER_SIZE);
        let mut resampler = Resampler::default();

        expect_true!(group.initialize(&mut fft_manager, &mut resampler).is_ok());
    }

    #[gtest]
    fn test_initialize_reverberant_filter() {
        let key = ProcessingGroupKey {
            ambisonic_order: 3,
            filter_profile: BinauralFilterProfile::Reverberant,
        };
        let indices = vec![0];
        let mut group = ProcessingGroup::new(key, indices, get_buffer_size(), get_sample_rate());
        let mut fft_manager = FftManager::new(BUFFER_SIZE);
        let mut resampler = Resampler::default();

        expect_true!(group.initialize(&mut fft_manager, &mut resampler).is_ok());
    }

    #[gtest]
    fn test_initialize_different_orders() {
        let orders = vec![1, 2, 3, 4];
        let mut fft_manager = FftManager::new(BUFFER_SIZE);
        let mut resampler = Resampler::default();

        for order in orders {
            let key = ProcessingGroupKey {
                ambisonic_order: order,
                filter_profile: BinauralFilterProfile::Direct,
            };
            let indices = vec![0];
            let mut group =
                ProcessingGroup::new(key, indices, get_buffer_size(), get_sample_rate());

            expect_true!(group.initialize(&mut fft_manager, &mut resampler).is_ok());
        }
    }

    #[gtest]
    fn test_process_ambisonics_only() {
        let key = ProcessingGroupKey {
            ambisonic_order: 1,
            filter_profile: BinauralFilterProfile::Direct,
        };
        let indices = vec![0];
        let mut group = ProcessingGroup::new(key, indices, get_buffer_size(), get_sample_rate());
        let mut fft_manager = FftManager::new(BUFFER_SIZE);
        let mut resampler = Resampler::default();
        assert!(group.initialize(&mut fft_manager, &mut resampler).is_ok());

        let audio_elements = vec![ambisonics_element(
            HighOrderAmbisonics::Order1,
            BinauralFilterProfile::Direct,
            HeadphonesRenderingMode::WorldLockedRestricted,
        )];

        let mut input_buffer = AudioBuffer::new(4, BUFFER_SIZE);
        let mut output_buffer = AudioBuffer::new(2, BUFFER_SIZE);

        for ch in 0..input_buffer.num_channels() {
            generate_sine(SAMPLING_RATE, input_buffer.channel_mut(ch).as_mut_slice());
        }

        let rotation = Quaternion::default();
        group.process(
            &input_buffer,
            &audio_elements,
            false,
            &rotation,
            &mut output_buffer,
            &mut fft_manager,
        );

        expect_that!(has_non_zero_output(&output_buffer), eq(true));
    }

    #[gtest]
    fn test_process_loudspeaker_layout() {
        let key = ProcessingGroupKey {
            ambisonic_order: 2,
            filter_profile: BinauralFilterProfile::Direct,
        };
        let indices = vec![0];
        let mut group = ProcessingGroup::new(key, indices, get_buffer_size(), get_sample_rate());
        let mut fft_manager = FftManager::new(BUFFER_SIZE);
        let mut resampler = Resampler::default();
        assert!(group.initialize(&mut fft_manager, &mut resampler).is_ok());

        let audio_elements = vec![layout_element(
            InputLayout::Stereo,
            BinauralFilterProfile::Direct,
            HeadphonesRenderingMode::WorldLockedRestricted,
        )];

        expect_true!(group.update_ambisonic_encoder(&audio_elements).is_ok());

        let mut input_buffer = AudioBuffer::new(2, BUFFER_SIZE);
        let mut output_buffer = AudioBuffer::new(2, BUFFER_SIZE);

        for ch in 0..input_buffer.num_channels() {
            generate_sine(SAMPLING_RATE, input_buffer.channel_mut(ch).as_mut_slice());
        }

        let rotation = Quaternion::default();
        group.process(
            &input_buffer,
            &audio_elements,
            false,
            &rotation,
            &mut output_buffer,
            &mut fft_manager,
        );

        expect_that!(has_non_zero_output(&output_buffer), eq(true));
    }

    #[gtest]
    fn test_process_audio_object() {
        let key = ProcessingGroupKey {
            ambisonic_order: 1,
            filter_profile: BinauralFilterProfile::Direct,
        };
        let indices = vec![0];
        let mut group = ProcessingGroup::new(key, indices, get_buffer_size(), get_sample_rate());
        let mut fft_manager = FftManager::new(BUFFER_SIZE);
        let mut resampler = Resampler::default();
        assert!(group.initialize(&mut fft_manager, &mut resampler).is_ok());
        let mut audio_elements = vec![mono_object_element(
            BinauralFilterProfile::Direct,
            HeadphonesRenderingMode::WorldLockedRestricted,
        )];
        audio_elements[0].object_positions_mut()[0] =
            PolarCoordinate::new_from_floats(45.0, 0.0, 1.0).unwrap();

        expect_true!(group.update_ambisonic_encoder(&audio_elements).is_ok());

        let mut input_buffer = AudioBuffer::new(1, BUFFER_SIZE);
        let mut output_buffer = AudioBuffer::new(2, BUFFER_SIZE);

        generate_sine(SAMPLING_RATE, input_buffer.channel_mut(0).as_mut_slice());

        let rotation = Quaternion::default();
        group.process(
            &input_buffer,
            &audio_elements,
            false,
            &rotation,
            &mut output_buffer,
            &mut fft_manager,
        );

        expect_that!(has_non_zero_output(&output_buffer), eq(true));
    }

    #[gtest]
    fn test_process_with_head_tracking() {
        let key = ProcessingGroupKey {
            ambisonic_order: 1,
            filter_profile: BinauralFilterProfile::Direct,
        };
        let indices = vec![0];
        let mut group = ProcessingGroup::new(key, indices, get_buffer_size(), get_sample_rate());
        let mut fft_manager = FftManager::new(BUFFER_SIZE);
        let mut resampler = Resampler::default();
        assert!(group.initialize(&mut fft_manager, &mut resampler).is_ok());

        let audio_elements = vec![ambisonics_element(
            HighOrderAmbisonics::Order1,
            BinauralFilterProfile::Direct,
            HeadphonesRenderingMode::WorldLockedRestricted,
        )];

        let mut input_buffer = AudioBuffer::new(4, BUFFER_SIZE);
        let mut output_buffer = AudioBuffer::new(2, BUFFER_SIZE);

        for ch in 0..input_buffer.num_channels() {
            generate_sine(SAMPLING_RATE, input_buffer.channel_mut(ch).as_mut_slice());
        }

        let rotation = Quaternion::new(0.707, 0.0, 0.707, 0.0).unwrap();

        group.process(
            &input_buffer,
            &audio_elements,
            true,
            &rotation,
            &mut output_buffer,
            &mut fft_manager,
        );

        expect_that!(has_non_zero_output(&output_buffer), eq(true));
    }

    #[gtest]
    fn test_process_mixed_content() {
        let key = ProcessingGroupKey {
            ambisonic_order: 2,
            filter_profile: BinauralFilterProfile::Direct,
        };
        let indices = vec![0, 1];
        let mut group = ProcessingGroup::new(key, indices, get_buffer_size(), get_sample_rate());
        let mut fft_manager = FftManager::new(BUFFER_SIZE);
        let mut resampler = Resampler::default();
        assert!(group.initialize(&mut fft_manager, &mut resampler).is_ok());

        let mut audio_elements = vec![
            ambisonics_element(
                HighOrderAmbisonics::Order2,
                BinauralFilterProfile::Direct,
                HeadphonesRenderingMode::WorldLockedRestricted,
            ),
            mono_object_element(
                BinauralFilterProfile::Direct,
                HeadphonesRenderingMode::WorldLockedRestricted,
            ),
        ];

        audio_elements[1].set_first_channel_index(9);
        audio_elements[1].object_positions_mut()[0] =
            PolarCoordinate::new_from_floats(90.0, 30.0, 0.5).unwrap();

        expect_true!(group.update_ambisonic_encoder(&audio_elements).is_ok());

        let mut input_buffer = AudioBuffer::new(10, BUFFER_SIZE);
        let mut output_buffer = AudioBuffer::new(2, BUFFER_SIZE);

        for ch in 0..input_buffer.num_channels() {
            generate_sine(SAMPLING_RATE, input_buffer.channel_mut(ch).as_mut_slice());
        }

        let rotation = Quaternion::default();
        group.process(
            &input_buffer,
            &audio_elements,
            false,
            &rotation,
            &mut output_buffer,
            &mut fft_manager,
        );

        expect_that!(has_non_zero_output(&output_buffer), eq(true));
    }

    #[gtest]
    fn test_update_encoder_no_sources() {
        let key = ProcessingGroupKey {
            ambisonic_order: 1,
            filter_profile: BinauralFilterProfile::Direct,
        };
        let indices = vec![0];
        let mut group = ProcessingGroup::new(key, indices, get_buffer_size(), get_sample_rate());
        let mut fft_manager = FftManager::new(BUFFER_SIZE);
        let mut resampler = Resampler::default();
        assert!(group.initialize(&mut fft_manager, &mut resampler).is_ok());

        let audio_elements = vec![ambisonics_element(
            HighOrderAmbisonics::Order1,
            BinauralFilterProfile::Direct,
            HeadphonesRenderingMode::WorldLockedRestricted,
        )];

        expect_true!(group.update_ambisonic_encoder(&audio_elements).is_ok());
    }

    #[gtest]
    fn test_update_encoder_object_positions() {
        let key = ProcessingGroupKey {
            ambisonic_order: 2,
            filter_profile: BinauralFilterProfile::Direct,
        };
        let indices = vec![0];
        let mut group = ProcessingGroup::new(key, indices, get_buffer_size(), get_sample_rate());
        let mut fft_manager = FftManager::new(BUFFER_SIZE);
        let mut resampler = Resampler::default();
        assert!(group.initialize(&mut fft_manager, &mut resampler).is_ok());

        let mut audio_elements = vec![mono_object_element(
            BinauralFilterProfile::Direct,
            HeadphonesRenderingMode::WorldLockedRestricted,
        )];

        expect_true!(group.update_ambisonic_encoder(&audio_elements).is_ok());

        audio_elements[0].object_positions_mut()[0] =
            PolarCoordinate::new_from_floats(180.0, 45.0, 0.8).unwrap();

        expect_true!(group.update_ambisonic_encoder(&audio_elements).is_ok());
    }

    #[gtest]
    fn test_all_filter_profiles_with_same_input() {
        let profiles = vec![
            BinauralFilterProfile::Direct,
            BinauralFilterProfile::Ambient,
            BinauralFilterProfile::Reverberant,
        ];
        let mut fft_manager = FftManager::new(BUFFER_SIZE);
        let mut resampler = Resampler::default();

        for profile in profiles {
            let key = ProcessingGroupKey { ambisonic_order: 1, filter_profile: profile };
            let indices = vec![0];
            let mut group =
                ProcessingGroup::new(key, indices, get_buffer_size(), get_sample_rate());
            assert!(group.initialize(&mut fft_manager, &mut resampler).is_ok());

            let audio_elements = vec![ambisonics_element(
                HighOrderAmbisonics::Order1,
                profile,
                HeadphonesRenderingMode::WorldLockedRestricted,
            )];

            let mut input_buffer = AudioBuffer::new(4, BUFFER_SIZE);
            let mut output_buffer = AudioBuffer::new(2, BUFFER_SIZE);

            for ch in 0..input_buffer.num_channels() {
                generate_sine(SAMPLING_RATE, input_buffer.channel_mut(ch).as_mut_slice());
            }

            let rotation = Quaternion::default();
            group.process(
                &input_buffer,
                &audio_elements,
                false,
                &rotation,
                &mut output_buffer,
                &mut fft_manager,
            );

            expect_that!(has_non_zero_output(&output_buffer), eq(true));
        }
    }

    #[gtest]
    fn test_world_locked_element_with_rotation() {
        let key = ProcessingGroupKey {
            ambisonic_order: 1,
            filter_profile: BinauralFilterProfile::Direct,
        };
        let indices = vec![0];
        let mut group = ProcessingGroup::new(key, indices, get_buffer_size(), get_sample_rate());
        let mut fft_manager = FftManager::new(BUFFER_SIZE);
        let mut resampler = Resampler::default();
        assert!(group.initialize(&mut fft_manager, &mut resampler).is_ok());

        let audio_elements = vec![ambisonics_element(
            HighOrderAmbisonics::Order1,
            BinauralFilterProfile::Direct,
            HeadphonesRenderingMode::WorldLockedRestricted,
        )];
        expect_that!(audio_elements[0].is_head_locked(), eq(false));

        let mut input_buffer = AudioBuffer::new(4, BUFFER_SIZE);
        let mut output_buffer1 = AudioBuffer::new(2, BUFFER_SIZE);
        let mut output_buffer2 = AudioBuffer::new(2, BUFFER_SIZE);

        for ch in 0..input_buffer.num_channels() {
            generate_sine(SAMPLING_RATE, input_buffer.channel_mut(ch).as_mut_slice());
        }

        let identity_rotation = Quaternion::default();
        group.process(
            &input_buffer,
            &audio_elements,
            true,
            &identity_rotation,
            &mut output_buffer1,
            &mut fft_manager,
        );

        let rotated = Quaternion::new(0.707, 0.0, 0.707, 0.0).unwrap();
        group.process(
            &input_buffer,
            &audio_elements,
            true,
            &rotated,
            &mut output_buffer2,
            &mut fft_manager,
        );

        expect_that!(has_non_zero_output(&output_buffer1), eq(true));
        expect_that!(has_non_zero_output(&output_buffer2), eq(true));

        let mut outputs_differ = false;
        for ch in 0..2 {
            for i in 0..BUFFER_SIZE {
                if (output_buffer1.channel(ch).as_slice()[i]
                    - output_buffer2.channel(ch).as_slice()[i])
                    .abs()
                    > 1e-5
                {
                    outputs_differ = true;
                    break;
                }
            }
            if outputs_differ {
                break;
            }
        }
        expect_that!(outputs_differ, eq(true));
    }

    #[gtest]
    fn test_head_locked_element_with_rotation() {
        let key = ProcessingGroupKey {
            ambisonic_order: 1,
            filter_profile: BinauralFilterProfile::Direct,
        };
        let indices = vec![0];
        let mut group = ProcessingGroup::new(key, indices, get_buffer_size(), get_sample_rate());
        let mut fft_manager = FftManager::new(BUFFER_SIZE);
        let mut resampler = Resampler::default();
        assert!(group.initialize(&mut fft_manager, &mut resampler).is_ok());

        let audio_elements = vec![ambisonics_element(
            HighOrderAmbisonics::Order1,
            BinauralFilterProfile::Direct,
            HeadphonesRenderingMode::HeadLocked,
        )];
        expect_that!(audio_elements[0].is_head_locked(), eq(true));

        let mut input_buffer = AudioBuffer::new(4, BUFFER_SIZE);
        let mut output_buffer = AudioBuffer::new(2, BUFFER_SIZE);

        for ch in 0..input_buffer.num_channels() {
            generate_sine(SAMPLING_RATE, input_buffer.channel_mut(ch).as_mut_slice());
        }

        let rotated = Quaternion::new(0.707, 0.0, 0.707, 0.0).unwrap();
        group.process(
            &input_buffer,
            &audio_elements,
            true,
            &rotated,
            &mut output_buffer,
            &mut fft_manager,
        );

        expect_that!(has_non_zero_output(&output_buffer), eq(true));
    }

    #[gtest]
    fn test_mixed_world_and_head_locked_elements() {
        let key = ProcessingGroupKey {
            ambisonic_order: 1,
            filter_profile: BinauralFilterProfile::Direct,
        };
        let mut mixed_group =
            ProcessingGroup::new(key, vec![0, 1], get_buffer_size(), get_sample_rate());
        let mut world_only_group =
            ProcessingGroup::new(key, vec![0], get_buffer_size(), get_sample_rate());
        let mut head_only_group =
            ProcessingGroup::new(key, vec![0], get_buffer_size(), get_sample_rate());
        let mut fft_manager = FftManager::new(BUFFER_SIZE);
        let mut resampler = Resampler::default();
        assert!(mixed_group.initialize(&mut fft_manager, &mut resampler).is_ok());
        assert!(world_only_group.initialize(&mut fft_manager, &mut resampler).is_ok());
        assert!(head_only_group.initialize(&mut fft_manager, &mut resampler).is_ok());

        let mut mixed_elements = vec![
            layout_element(
                InputLayout::Stereo,
                BinauralFilterProfile::Direct,
                HeadphonesRenderingMode::WorldLockedRestricted,
            ),
            mono_object_element(BinauralFilterProfile::Direct, HeadphonesRenderingMode::HeadLocked),
        ];
        mixed_elements[1].set_first_channel_index(2);
        mixed_elements[1].object_positions_mut()[0] =
            PolarCoordinate::new_from_floats(-90.0, 0.0, 1.0).unwrap();

        let world_only_elements = vec![mixed_elements[0]];
        let mut head_only_element = mixed_elements[1];
        head_only_element.set_first_channel_index(0);
        let head_only_elements = vec![head_only_element];

        expect_true!(mixed_group.update_ambisonic_encoder(&mixed_elements).is_ok());
        expect_true!(world_only_group.update_ambisonic_encoder(&world_only_elements).is_ok());
        expect_true!(head_only_group.update_ambisonic_encoder(&head_only_elements).is_ok());

        let mut mixed_input = AudioBuffer::new(3, BUFFER_SIZE);
        generate_sine(SAMPLING_RATE, mixed_input.channel_mut(0).as_mut_slice());
        generate_sine(SAMPLING_RATE, mixed_input.channel_mut(1).as_mut_slice());
        generate_sine(SAMPLING_RATE, mixed_input.channel_mut(2).as_mut_slice());
        let mut world_input = AudioBuffer::new(2, BUFFER_SIZE);
        world_input
            .channel_mut(0)
            .as_mut_slice()
            .copy_from_slice(mixed_input.channel(0).as_slice());
        world_input
            .channel_mut(1)
            .as_mut_slice()
            .copy_from_slice(mixed_input.channel(1).as_slice());
        let mut head_input = AudioBuffer::new(1, BUFFER_SIZE);
        head_input.channel_mut(0).as_mut_slice().copy_from_slice(mixed_input.channel(2).as_slice());

        let mut mixed_output = AudioBuffer::new(2, BUFFER_SIZE);
        let mut world_output = AudioBuffer::new(2, BUFFER_SIZE);
        let mut head_output = AudioBuffer::new(2, BUFFER_SIZE);
        let rotated = Quaternion::new(0.70710677, 0.0, 0.0, 0.70710677).unwrap();

        mixed_group.process(
            &mixed_input,
            &mixed_elements,
            true,
            &rotated,
            &mut mixed_output,
            &mut fft_manager,
        );
        world_only_group.process(
            &world_input,
            &world_only_elements,
            true,
            &rotated,
            &mut world_output,
            &mut fft_manager,
        );
        head_only_group.process(
            &head_input,
            &head_only_elements,
            false,
            &Quaternion::default(),
            &mut head_output,
            &mut fft_manager,
        );

        expect_that!(has_non_zero_output(&mixed_output), eq(true));
        for ch in 0..2 {
            for i in 0..BUFFER_SIZE {
                let expected =
                    world_output.channel(ch).as_slice()[i] + head_output.channel(ch).as_slice()[i];
                expect_that!(mixed_output.channel(ch).as_slice()[i], near(expected, 1e-5));
            }
        }
    }

    #[gtest]
    fn test_all_elements_world_locked() {
        let key = ProcessingGroupKey {
            ambisonic_order: 1,
            filter_profile: BinauralFilterProfile::Direct,
        };
        let indices = vec![0];
        let mut group = ProcessingGroup::new(key, indices, get_buffer_size(), get_sample_rate());
        let mut fft_manager = FftManager::new(BUFFER_SIZE);
        let mut resampler = Resampler::default();
        assert!(group.initialize(&mut fft_manager, &mut resampler).is_ok());

        let audio_elements = vec![ambisonics_element(
            HighOrderAmbisonics::Order1,
            BinauralFilterProfile::Direct,
            HeadphonesRenderingMode::WorldLockedRestricted,
        )];

        let mut input_buffer = AudioBuffer::new(4, BUFFER_SIZE);
        let mut output_buffer = AudioBuffer::new(2, BUFFER_SIZE);

        for ch in 0..input_buffer.num_channels() {
            generate_sine(SAMPLING_RATE, input_buffer.channel_mut(ch).as_mut_slice());
        }

        let rotated = Quaternion::new(0.707, 0.0, 0.707, 0.0).unwrap();
        group.process(
            &input_buffer,
            &audio_elements,
            true,
            &rotated,
            &mut output_buffer,
            &mut fft_manager,
        );

        expect_that!(has_non_zero_output(&output_buffer), eq(true));
    }

    #[gtest]
    fn test_all_elements_head_locked() {
        let key = ProcessingGroupKey {
            ambisonic_order: 1,
            filter_profile: BinauralFilterProfile::Direct,
        };
        let indices = vec![0];
        let mut group = ProcessingGroup::new(key, indices, get_buffer_size(), get_sample_rate());
        let mut fft_manager = FftManager::new(BUFFER_SIZE);
        let mut resampler = Resampler::default();
        assert!(group.initialize(&mut fft_manager, &mut resampler).is_ok());

        let audio_elements = vec![ambisonics_element(
            HighOrderAmbisonics::Order1,
            BinauralFilterProfile::Direct,
            HeadphonesRenderingMode::HeadLocked,
        )];

        let mut input_buffer = AudioBuffer::new(4, BUFFER_SIZE);
        let mut output_buffer = AudioBuffer::new(2, BUFFER_SIZE);

        for ch in 0..input_buffer.num_channels() {
            generate_sine(SAMPLING_RATE, input_buffer.channel_mut(ch).as_mut_slice());
        }

        let rotated = Quaternion::new(0.707, 0.0, 0.707, 0.0).unwrap();
        group.process(
            &input_buffer,
            &audio_elements,
            true,
            &rotated,
            &mut output_buffer,
            &mut fft_manager,
        );

        expect_that!(has_non_zero_output(&output_buffer), eq(true));
    }

    #[gtest]
    fn test_head_tracking_disabled_override() {
        let key = ProcessingGroupKey {
            ambisonic_order: 1,
            filter_profile: BinauralFilterProfile::Direct,
        };
        let indices = vec![0];
        let mut group = ProcessingGroup::new(key, indices, get_buffer_size(), get_sample_rate());
        let mut fft_manager = FftManager::new(BUFFER_SIZE);
        let mut resampler = Resampler::default();
        assert!(group.initialize(&mut fft_manager, &mut resampler).is_ok());

        let audio_elements = vec![ambisonics_element(
            HighOrderAmbisonics::Order1,
            BinauralFilterProfile::Direct,
            HeadphonesRenderingMode::WorldLockedRestricted,
        )];

        let mut input_buffer = AudioBuffer::new(4, BUFFER_SIZE);
        let mut output_buffer1 = AudioBuffer::new(2, BUFFER_SIZE);
        let mut output_buffer2 = AudioBuffer::new(2, BUFFER_SIZE);

        for ch in 0..input_buffer.num_channels() {
            generate_sine(SAMPLING_RATE, input_buffer.channel_mut(ch).as_mut_slice());
        }

        let rotated = Quaternion::new(0.707, 0.0, 0.707, 0.0).unwrap();

        group.process(
            &input_buffer,
            &audio_elements,
            true,
            &rotated,
            &mut output_buffer1,
            &mut fft_manager,
        );
        group.process(
            &input_buffer,
            &audio_elements,
            false,
            &rotated,
            &mut output_buffer2,
            &mut fft_manager,
        );

        let mut outputs_differ = false;
        for ch in 0..2 {
            for i in 0..BUFFER_SIZE {
                if (output_buffer1.channel(ch).as_slice()[i]
                    - output_buffer2.channel(ch).as_slice()[i])
                    .abs()
                    > 1e-5
                {
                    outputs_differ = true;
                    break;
                }
            }
            if outputs_differ {
                break;
            }
        }
        expect_that!(outputs_differ, eq(true));
    }
}
