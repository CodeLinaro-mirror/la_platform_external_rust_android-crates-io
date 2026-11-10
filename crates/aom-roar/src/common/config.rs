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

use super::downmix::DownmixInfo;
use super::hoa::HighOrderAmbisonics;
use super::layout::{InputLayout, OutputLayout};
use super::units::{SampleRate, Samples};
use crate::common::definitions::{OarError, MAX_SAMPLES_PER_CHANNEL};
use std::num::NonZeroUsize;

/// Number of channels in an audio element or layout or similar.
pub type ChannelCount = NonZeroUsize;

/// Configuration for creating the ROAR instance, equivalent of C API `roar_config_t`.
#[derive(Debug, Clone, Copy)]
pub struct Config {
    /// The target output layout.
    target_layout: OutputLayout,
    /// Number of samples per channel per render call, [32, 16384].
    samples_per_channel: Samples,
    /// The sample rate in Hz.
    sample_rate: SampleRate,
}

impl Config {
    pub fn new(
        target_layout: OutputLayout,
        samples_per_channel: Samples,
        sample_rate: SampleRate,
    ) -> Result<Self, OarError> {
        if samples_per_channel.value() > MAX_SAMPLES_PER_CHANNEL {
            return Err(OarError::InvalidParameter);
        }
        Ok(Config { target_layout, samples_per_channel, sample_rate })
    }

    /// Get output (target) layout.
    pub fn output_layout(&self) -> OutputLayout {
        self.target_layout
    }

    /// Get samples per channel (per render call).
    pub fn samples_per_channel(&self) -> Samples {
        self.samples_per_channel
    }

    /// Get sample rate.
    pub fn sample_rate(&self) -> SampleRate {
        self.sample_rate
    }
}

/// Headphones rendering mode as specified by IAMF.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HeadphonesRenderingMode {
    /// Stereo rendering for this element (no binaural rendering).
    #[default]
    WorldLockedRestricted = 0,
    /// Binaural rendering with full head-tracking (world-locked).
    WorldLocked = 1,
    /// Binaural rendering locked to the listener's head.
    HeadLocked = 2,
    /// Reserved mode.
    Reserved = 3,
}

impl HeadphonesRenderingMode {
    /// Returns `true` if this mode locks the audio element to the listener's head.
    pub fn is_head_locked(self) -> bool {
        matches!(self, Self::HeadLocked)
    }
}

/// Selected binaural filter profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum BinauralFilterProfile {
    /// Ambient binaural filter profile.
    #[default]
    Ambient = 0,
    /// Direct binaural filter profile.
    Direct = 1,
    /// Reverberant binaural filter profile.
    Reverberant = 2,
}

/// Configures how a channel-based audio element is rendered on headphones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ElementRenderingConfig {
    /// The headphones rendering mode to use.
    pub headphones_rendering_mode: HeadphonesRenderingMode,
    /// The binaural filter profile to apply.
    pub binaural_filter_profile: BinauralFilterProfile,
}

/// Configuration for channel-based audio element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelBasedConfig {
    /// The input loudspeaker or channel-subset layout configuration.
    pub layout: InputLayout,
    /// Optional initial IAMF downmix parameters.
    ///
    /// When `Some`, selects `DownmixRenderer` if `layout` and the effective loudspeaker target
    /// layout form a supported IAMF downmix pair (including `WorldLockedRestricted` elements
    /// rendered to stereo on `OutputLayout::Binaural`). When `None` or when the layout pair is not
    /// a supported downmix (such as `5.1.2 -> Stereo` or `in == out`), rendering falls back to EAR
    /// (or binaural OBR) and runtime downmix mode updates are accepted as advisory no-ops.
    pub downmix_info: Option<DownmixInfo>,
    /// Optional binaural rendering config.
    pub rendering_config: Option<ElementRenderingConfig>,
}

/// Configuration for scene-based (HOA) audio element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneBasedConfig {
    /// The higher order ambisonics order.
    pub order: HighOrderAmbisonics,
    ///Optional binaural rendering config.
    pub rendering_config: Option<ElementRenderingConfig>,
}

/// Configuration for object-based audio element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectBasedConfig {
    /// The number of objects (1 or 2).
    num_objects: ChannelCount,
    /// Optional binaural rendering config.
    pub rendering_config: Option<ElementRenderingConfig>,
}

impl ObjectBasedConfig {
    /// Creates a new `ObjectBasedConfig` if `num_objects` is 1 or 2.
    pub fn new(
        num_objects: usize,
        rendering_config: Option<ElementRenderingConfig>,
    ) -> Result<Self, OarError> {
        if !(1..=2).contains(&num_objects) {
            return Err(OarError::InvalidParameter);
        }
        Ok(Self { num_objects: ChannelCount::new(num_objects).unwrap(), rendering_config })
    }

    /// Returns the number of objects (1 or 2).
    pub fn num_objects(&self) -> ChannelCount {
        self.num_objects
    }
}

/// Configuration for an audio element, equivalent of C API `roar_audio_element_config_t`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioElementConfig {
    /// Channel-based layout configuration.
    ChannelBased(ChannelBasedConfig),
    /// Scene-based (Ambisonics) configuration.
    SceneBased(SceneBasedConfig),
    /// Object-based configuration.
    ObjectBased(ObjectBasedConfig),
}

impl AudioElementConfig {
    /// Returns the number of channels for this element configuration.
    pub fn channels(&self) -> ChannelCount {
        match self {
            AudioElementConfig::ChannelBased(cbc) => cbc.layout.channels(),
            AudioElementConfig::SceneBased(sbc) => sbc.order.channels(),
            AudioElementConfig::ObjectBased(obc) => obc.num_objects,
        }
    }

    /// Returns the headphones rendering configuration if present.
    pub fn rendering_config(&self) -> Option<ElementRenderingConfig> {
        match self {
            AudioElementConfig::ChannelBased(cbc) => cbc.rendering_config,
            AudioElementConfig::SceneBased(sbc) => sbc.rendering_config,
            AudioElementConfig::ObjectBased(obc) => obc.rendering_config,
        }
    }
}

// ===== Tests =====

#[cfg(test)]
mod test {

    use super::*;
    use googletest::prelude::*;

    #[gtest]
    fn test_oar_config_accessors() {
        let config = Config::new(
            OutputLayout::Layout714,
            Samples::new(512).unwrap(),
            SampleRate::new(48000).unwrap(),
        )
        .unwrap();

        expect_that!(config.output_layout(), eq(OutputLayout::Layout714));
        expect_that!(config.sample_rate().value(), eq(48000));
        expect_that!(config.samples_per_channel().value(), eq(512));
    }

    #[gtest]
    fn test_oar_config_validation() {
        // Valid configuration
        expect_ok!(Config::new(
            OutputLayout::Stereo,
            Samples::new(512).unwrap(),
            SampleRate::new(48000).unwrap(),
        ));
        // Invalid samples (too large for config limit of 16384)
        expect_true!(Config::new(
            OutputLayout::Stereo,
            Samples::new(20000).unwrap(),
            SampleRate::new(48000).unwrap(),
        )
        .is_err());
    }

    #[gtest]
    fn test_audio_element_config_comparisons_and_traits() {
        let ch_config = ChannelBasedConfig {
            layout: InputLayout::Stereo,
            downmix_info: None,
            rendering_config: None,
        };
        let sc_config =
            SceneBasedConfig { order: HighOrderAmbisonics::Order1, rendering_config: None };
        let obj_config = ObjectBasedConfig::new(2, None).unwrap();

        let el_ch = AudioElementConfig::ChannelBased(ch_config);
        let el_sc = AudioElementConfig::SceneBased(sc_config);
        let el_obj = AudioElementConfig::ObjectBased(obj_config);

        expect_that!(el_ch, not(eq(el_sc)));
        expect_that!(el_ch, not(eq(el_obj)));

        let el_ch_clone = el_ch;
        expect_that!(el_ch, eq(el_ch_clone));
    }

    #[gtest]
    fn test_object_based_config_validation() {
        expect_eq!(ObjectBasedConfig::new(0, None), Err(OarError::InvalidParameter));
        expect_ok!(ObjectBasedConfig::new(1, None));
        expect_ok!(ObjectBasedConfig::new(2, None));
        expect_eq!(ObjectBasedConfig::new(3, None), Err(OarError::InvalidParameter));
    }

    #[gtest]
    fn headphones_rendering_mode_is_head_locked_returns_true_only_for_head_locked() {
        expect_false!(HeadphonesRenderingMode::WorldLockedRestricted.is_head_locked());
        expect_false!(HeadphonesRenderingMode::WorldLocked.is_head_locked());
        expect_true!(HeadphonesRenderingMode::HeadLocked.is_head_locked());
        expect_false!(HeadphonesRenderingMode::Reserved.is_head_locked());
    }
}
