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

//! Runtime audio element state and configuration queries for OBR.
//!
//! Wraps [`AudioElementConfig`] with the element's packed input channel offset and mutable
//! spatial coordinates for dynamic audio object channels.

use crate::common::definitions::{
    AudioElementConfig, BinauralFilterProfile, HighOrderAmbisonics, InputLayout, OarError,
    PolarCoordinate, SpeakerPosition,
};

/// Maximum number of object channels supported by a single OBR object-based audio element
/// (mono or dual objects).
const MAX_OBJECT_CHANNELS: usize = 2;

/// Runtime state and configuration for an individual audio element in OBR.
///
/// Wraps [`AudioElementConfig`] with the element's packed input channel offset and mutable polar
/// coordinates for dynamic object channels. Because loudspeaker positions are streamed directly
/// from [`InputLayout::obr_speakers`] and object elements support at most two channels, this
/// struct requires zero heap allocations (`Copy`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObrAudioElement {
    config: AudioElementConfig,
    first_channel_index: usize,
    object_positions: [PolarCoordinate; MAX_OBJECT_CHANNELS],
}

impl ObrAudioElement {
    /// Creates a new `ObrAudioElement` from a common [`AudioElementConfig`].
    ///
    /// # Parameters
    /// * `config` - Element configuration; object-based configurations must specify 1 or 2 objects.
    ///
    /// # Errors
    /// Returns [`OarError::InvalidParameter`] if `config` is object-based with more than 2
    /// objects.
    pub fn new(config: AudioElementConfig) -> Result<Self, OarError> {
        Ok(Self {
            config,
            first_channel_index: 0,
            object_positions: [PolarCoordinate::FRONT; MAX_OBJECT_CHANNELS],
        })
    }

    /// Returns `true` if this element is a periphonic Ambisonic scene (orders 1..=4).
    pub fn is_ambisonics(&self) -> bool {
        matches!(
            self.config,
            AudioElementConfig::SceneBased(sbc) if sbc.order != HighOrderAmbisonics::Zoa
        )
    }

    /// Returns `true` if this element bypasses Ambisonic spatial processing (`Zoa` mono
    /// passthrough or `InputLayout::Binaural` stereo passthrough).
    pub fn is_passthrough(&self) -> bool {
        matches!(
            self.config,
            AudioElementConfig::SceneBased(sbc) if sbc.order == HighOrderAmbisonics::Zoa
        ) || matches!(
            self.config,
            AudioElementConfig::ChannelBased(cbc) if cbc.layout == InputLayout::Binaural
        )
    }

    /// Returns `true` if this element is encoded via the Ambisonic encoder (non-binaural
    /// loudspeaker layouts or spatialized objects).
    pub fn uses_ambisonic_encoder(&self) -> bool {
        match self.config {
            AudioElementConfig::ChannelBased(cbc) => cbc.layout != InputLayout::Binaural,
            AudioElementConfig::ObjectBased(_) => true,
            AudioElementConfig::SceneBased(_) => false,
        }
    }

    /// Sets the base `first_channel_index` for this element within OBR's packed input buffer.
    pub fn set_first_channel_index(&mut self, first_channel: usize) {
        self.first_channel_index = first_channel;
    }

    /// Returns the offset index of the first input channel for this element.
    pub fn get_first_channel_index(&self) -> usize {
        self.first_channel_index
    }

    /// Returns the total number of input channels for this element.
    pub fn get_number_of_input_channels(&self) -> usize {
        self.config.channels().get()
    }

    /// Returns an iterator over virtual loudspeaker positions if this is a non-binaural
    /// channel-based element, or an empty iterator otherwise.
    pub fn get_loudspeaker_channels(&self) -> impl Iterator<Item = SpeakerPosition> + use<> {
        match self.config {
            AudioElementConfig::ChannelBased(cbc) => cbc.layout.obr_speakers(),
            _ => None,
        }
        .into_iter()
        .flatten()
    }

    /// Returns the active object polar coordinates if this is an object-based element, or an
    /// empty slice otherwise.
    pub fn object_positions(&self) -> &[PolarCoordinate] {
        match self.config {
            AudioElementConfig::ObjectBased(obc) => {
                &self.object_positions[..obc.num_objects().get()]
            }
            _ => &[],
        }
    }

    /// Returns a mutable slice of the active object polar coordinates if this is an object-based
    /// element, or an empty slice otherwise.
    pub fn object_positions_mut(&mut self) -> &mut [PolarCoordinate] {
        match self.config {
            AudioElementConfig::ObjectBased(obc) => {
                &mut self.object_positions[..obc.num_objects().get()]
            }
            _ => &mut [],
        }
    }

    /// Returns the Ambisonic order used for rendering this element's binaural filters.
    pub fn get_binaural_filters_ambisonic_order(&self) -> i32 {
        match self.config {
            AudioElementConfig::SceneBased(sbc) => sbc.order as i32,
            AudioElementConfig::ChannelBased(cbc) if cbc.layout == InputLayout::Binaural => 0,
            AudioElementConfig::ChannelBased(_) | AudioElementConfig::ObjectBased(_) => 3,
        }
    }

    /// Returns the binaural filter profile assigned to this element.
    pub fn get_binaural_filter_profile(&self) -> BinauralFilterProfile {
        self.config.rendering_config().unwrap_or_default().binaural_filter_profile
    }

    /// Returns whether this element's channels are locked to the listener's head.
    ///
    /// Passthrough elements (`Zoa` and `InputLayout::Binaural`) bypass Ambisonic spatial
    /// processing and head rotation, so they are always head-locked.
    pub fn is_head_locked(&self) -> bool {
        self.is_passthrough()
            || self
                .config
                .rendering_config()
                .unwrap_or_default()
                .headphones_rendering_mode
                .is_head_locked()
    }
}

impl TryFrom<&AudioElementConfig> for ObrAudioElement {
    type Error = OarError;

    fn try_from(config: &AudioElementConfig) -> Result<Self, Self::Error> {
        Self::new(*config)
    }
}

// ===== Tests =====

#[cfg(test)]
mod test {

    use super::*;
    use crate::common::definitions::{
        ChannelBasedConfig, ElementRenderingConfig, HeadphonesRenderingMode, ObjectBasedConfig,
        SceneBasedConfig,
    };
    use googletest::prelude::*;

    #[gtest]
    fn obr_audio_element_ambisonics_initializes_order_and_channel_count() {
        let common_config = AudioElementConfig::SceneBased(SceneBasedConfig {
            order: HighOrderAmbisonics::Order1,
            rendering_config: Some(ElementRenderingConfig {
                headphones_rendering_mode: HeadphonesRenderingMode::WorldLocked,
                binaural_filter_profile: BinauralFilterProfile::Direct,
            }),
        });

        let elem = ObrAudioElement::new(common_config).unwrap();

        expect_true!(elem.is_ambisonics());
        expect_false!(elem.is_passthrough());
        expect_false!(elem.uses_ambisonic_encoder());
        expect_eq!(elem.get_number_of_input_channels(), 4);
        expect_eq!(elem.get_binaural_filters_ambisonic_order(), 1);
        expect_eq!(elem.get_binaural_filter_profile(), BinauralFilterProfile::Direct);
        expect_false!(elem.is_head_locked());
    }

    #[gtest]
    fn obr_audio_element_object_initializes_front_positions_and_third_order() {
        let obj_config = AudioElementConfig::ObjectBased(ObjectBasedConfig::new(2, None).unwrap());

        let elem = ObrAudioElement::new(obj_config).unwrap();

        expect_false!(elem.is_ambisonics());
        expect_true!(elem.uses_ambisonic_encoder());
        expect_eq!(elem.get_number_of_input_channels(), 2);
        expect_eq!(elem.object_positions(), &[PolarCoordinate::FRONT, PolarCoordinate::FRONT]);
        expect_eq!(elem.get_binaural_filters_ambisonic_order(), 3);
        expect_eq!(elem.get_binaural_filter_profile(), BinauralFilterProfile::Ambient);
    }

    #[gtest]
    fn obr_audio_element_zoa_initializes_mono_passthrough_and_head_locked() {
        let zoa_config = AudioElementConfig::SceneBased(SceneBasedConfig {
            order: HighOrderAmbisonics::Zoa,
            rendering_config: Some(ElementRenderingConfig {
                headphones_rendering_mode: HeadphonesRenderingMode::WorldLocked,
                binaural_filter_profile: BinauralFilterProfile::Reverberant,
            }),
        });

        let elem = ObrAudioElement::new(zoa_config).unwrap();

        expect_true!(elem.is_passthrough());
        expect_false!(elem.is_ambisonics());
        expect_eq!(elem.get_number_of_input_channels(), 1);
        expect_eq!(elem.get_binaural_filters_ambisonic_order(), 0);
        expect_true!(elem.is_head_locked());
    }

    #[gtest]
    fn obr_audio_element_initializes_subset_loudspeaker_layout_channels() {
        let subset_config = AudioElementConfig::ChannelBased(ChannelBasedConfig {
            layout: InputLayout::Top4ch,
            downmix_info: None,
            rendering_config: Some(ElementRenderingConfig {
                headphones_rendering_mode: HeadphonesRenderingMode::WorldLocked,
                binaural_filter_profile: BinauralFilterProfile::Direct,
            }),
        });

        let elem = ObrAudioElement::new(subset_config).unwrap();

        expect_true!(elem.uses_ambisonic_encoder());
        expect_eq!(elem.get_number_of_input_channels(), 4);
        expect_eq!(elem.get_loudspeaker_channels().count(), 4);
        expect_eq!(elem.get_binaural_filters_ambisonic_order(), 3);
    }

    #[gtest]
    fn obr_audio_element_binaural_layout_initializes_stereo_passthrough_and_head_locked() {
        let binaural_config = AudioElementConfig::ChannelBased(ChannelBasedConfig {
            layout: InputLayout::Binaural,
            downmix_info: None,
            rendering_config: Some(ElementRenderingConfig {
                headphones_rendering_mode: HeadphonesRenderingMode::WorldLocked,
                binaural_filter_profile: BinauralFilterProfile::Direct,
            }),
        });

        let elem = ObrAudioElement::new(binaural_config).unwrap();

        expect_true!(elem.is_passthrough());
        expect_false!(elem.uses_ambisonic_encoder());
        expect_eq!(elem.get_loudspeaker_channels().count(), 0);
        expect_eq!(elem.get_number_of_input_channels(), 2);
        expect_eq!(elem.get_binaural_filters_ambisonic_order(), 0);
        expect_true!(elem.is_head_locked());
    }

    #[gtest]
    fn set_first_channel_index_updates_index() {
        let mut elem = ObrAudioElement::new(AudioElementConfig::SceneBased(SceneBasedConfig {
            order: HighOrderAmbisonics::Order1,
            rendering_config: None,
        }))
        .unwrap();

        elem.set_first_channel_index(10);

        expect_eq!(elem.get_first_channel_index(), 10);
    }

    #[gtest]
    fn obr_audio_element_try_from_audio_element_config_maps_rendering_config() {
        let scene_config = AudioElementConfig::SceneBased(SceneBasedConfig {
            order: HighOrderAmbisonics::Order2,
            rendering_config: Some(ElementRenderingConfig {
                headphones_rendering_mode: HeadphonesRenderingMode::HeadLocked,
                binaural_filter_profile: BinauralFilterProfile::Direct,
            }),
        });
        let channel_config = AudioElementConfig::ChannelBased(ChannelBasedConfig {
            layout: InputLayout::Stereo,
            downmix_info: None,
            rendering_config: None,
        });

        let scene_elem = ObrAudioElement::try_from(&scene_config).unwrap();
        let channel_elem = ObrAudioElement::try_from(&channel_config).unwrap();

        expect_true!(scene_elem.is_head_locked());
        expect_eq!(scene_elem.get_binaural_filter_profile(), BinauralFilterProfile::Direct);
        expect_false!(channel_elem.is_head_locked());
        expect_eq!(channel_elem.get_binaural_filter_profile(), BinauralFilterProfile::Ambient);
    }
}
