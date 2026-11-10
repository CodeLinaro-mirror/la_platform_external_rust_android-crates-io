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

//! Public output and input layout configurations.

use crate::common::definitions::{ChannelCount, Degrees, OarError};

/// The angular position and LFE classification of a loudspeaker in a layout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpeakerPosition {
    /// The azimuth angle of the loudspeaker.
    pub azimuth: Degrees,
    /// The elevation angle of the loudspeaker.
    pub elevation: Degrees,
    /// Whether this loudspeaker is a Low-Frequency Effects (LFE) channel.
    pub is_lfe: bool,
}

// ITU-R BS.2051 loudspeaker positions (`SP_<Layer><Sign><Azimuth>`):
// - Layer: `M` = middle (0°), `U` = upper (+30°), `UH` = upper high (+45°), `B` = bottom (-30°),
//   `T` = top (+90°).
// - Sign:  `P` = plus (+azimuth, left), `M` = minus (-azimuth, right).

/// ITU-R BS.2051 `M+000`: Middle layer, 0° azimuth (Front Center / `C` / `FC`).
pub(crate) const SP_MP000: SpeakerPosition =
    SpeakerPosition { azimuth: Degrees::ZERO, elevation: Degrees::ZERO, is_lfe: false };
/// ITU-R BS.2051 `M+025`: Middle layer, +25° azimuth (Left Screen Edge, Sound System G).
pub(crate) const SP_MP025: SpeakerPosition =
    SpeakerPosition { azimuth: Degrees::from_const(25.0), elevation: Degrees::ZERO, is_lfe: false };
/// ITU-R BS.2051 `M-025`: Middle layer, -25° azimuth (Right Screen Edge, Sound System G).
pub(crate) const SP_MM025: SpeakerPosition = SpeakerPosition {
    azimuth: Degrees::from_const(-25.0),
    elevation: Degrees::ZERO,
    is_lfe: false,
};
/// ITU-R BS.2051 `M-030`: Middle layer, -30° azimuth (Front Right / `R` / `FRc`).
pub(crate) const SP_MM030: SpeakerPosition = SpeakerPosition {
    azimuth: Degrees::from_const(-30.0),
    elevation: Degrees::ZERO,
    is_lfe: false,
};
/// ITU-R BS.2051 `M+030`: Middle layer, +30° azimuth (Front Left / `L` / `FLc`).
pub(crate) const SP_MP030: SpeakerPosition =
    SpeakerPosition { azimuth: Degrees::from_const(30.0), elevation: Degrees::ZERO, is_lfe: false };
/// ITU-R BS.2051 `M-060`: Middle layer, -60° azimuth (Front Right Wide / `FR`).
pub(crate) const SP_MM060: SpeakerPosition = SpeakerPosition {
    azimuth: Degrees::from_const(-60.0),
    elevation: Degrees::ZERO,
    is_lfe: false,
};
/// ITU-R BS.2051 `M+060`: Middle layer, +60° azimuth (Front Left Wide / `FL`).
pub(crate) const SP_MP060: SpeakerPosition =
    SpeakerPosition { azimuth: Degrees::from_const(60.0), elevation: Degrees::ZERO, is_lfe: false };
/// ITU-R BS.2051 `M-090`: Middle layer, -90° azimuth (Side Right / `Rss` / `SiR`).
pub(crate) const SP_MM090: SpeakerPosition = SpeakerPosition {
    azimuth: Degrees::from_const(-90.0),
    elevation: Degrees::ZERO,
    is_lfe: false,
};
/// ITU-R BS.2051 `M+090`: Middle layer, +90° azimuth (Side Left / `Lss` / `SiL`).
pub(crate) const SP_MP090: SpeakerPosition =
    SpeakerPosition { azimuth: Degrees::from_const(90.0), elevation: Degrees::ZERO, is_lfe: false };
/// ITU-R BS.2051 `M-110`: Middle layer, -110° azimuth (Right Surround / `Rs`).
pub(crate) const SP_MM110: SpeakerPosition = SpeakerPosition {
    azimuth: Degrees::from_const(-110.0),
    elevation: Degrees::ZERO,
    is_lfe: false,
};
/// ITU-R BS.2051 `M+110`: Middle layer, +110° azimuth (Left Surround / `Ls`).
pub(crate) const SP_MP110: SpeakerPosition = SpeakerPosition {
    azimuth: Degrees::from_const(110.0),
    elevation: Degrees::ZERO,
    is_lfe: false,
};
/// ITU-R BS.2051 `M-135`: Middle layer, -135° azimuth (Rear Right / `Rrs` / `BR`).
pub(crate) const SP_MM135: SpeakerPosition = SpeakerPosition {
    azimuth: Degrees::from_const(-135.0),
    elevation: Degrees::ZERO,
    is_lfe: false,
};
/// ITU-R BS.2051 `M+135`: Middle layer, +135° azimuth (Rear Left / `Lrs` / `BL`).
pub(crate) const SP_MP135: SpeakerPosition = SpeakerPosition {
    azimuth: Degrees::from_const(135.0),
    elevation: Degrees::ZERO,
    is_lfe: false,
};
/// ITU-R BS.2051 `M+180`: Middle layer, 180° azimuth (Back Center / `BC`).
pub(crate) const SP_MP180: SpeakerPosition = SpeakerPosition {
    azimuth: Degrees::from_const(180.0),
    elevation: Degrees::ZERO,
    is_lfe: false,
};

/// ITU-R BS.2051 `U+000`: Upper layer (+30° elevation), 0° azimuth (Top Front Center / `TpFC`).
pub(crate) const SP_UP000: SpeakerPosition =
    SpeakerPosition { azimuth: Degrees::ZERO, elevation: Degrees::from_const(30.0), is_lfe: false };
/// ITU-R BS.2051 `U-030`: Upper layer (+30° elevation), -30° azimuth (Right Top Front / `Rtf`).
pub(crate) const SP_UM030: SpeakerPosition = SpeakerPosition {
    azimuth: Degrees::from_const(-30.0),
    elevation: Degrees::from_const(30.0),
    is_lfe: false,
};
/// ITU-R BS.2051 `U+030`: Upper layer (+30° elevation), +30° azimuth (Left Top Front / `Ltf`).
pub(crate) const SP_UP030: SpeakerPosition = SpeakerPosition {
    azimuth: Degrees::from_const(30.0),
    elevation: Degrees::from_const(30.0),
    is_lfe: false,
};
/// ITU-R BS.2051 `U-045`: Upper layer (+30° elevation), -45° azimuth (Right Top Front / `Rtf` /
/// `TpFR`).
pub(crate) const SP_UM045: SpeakerPosition = SpeakerPosition {
    azimuth: Degrees::from_const(-45.0),
    elevation: Degrees::from_const(30.0),
    is_lfe: false,
};
/// ITU-R BS.2051 `U+045`: Upper layer (+30° elevation), +45° azimuth (Left Top Front / `Ltf` /
/// `TpFL`).
pub(crate) const SP_UP045: SpeakerPosition = SpeakerPosition {
    azimuth: Degrees::from_const(45.0),
    elevation: Degrees::from_const(30.0),
    is_lfe: false,
};
/// ITU-R BS.2051 `U-090`: Upper layer (+30° elevation), -90° azimuth (Top Side Right / `TpSiR`).
pub(crate) const SP_UM090: SpeakerPosition = SpeakerPosition {
    azimuth: Degrees::from_const(-90.0),
    elevation: Degrees::from_const(30.0),
    is_lfe: false,
};
/// ITU-R BS.2051 `U+090`: Upper layer (+30° elevation), +90° azimuth (Top Side Left / `TpSiL`).
pub(crate) const SP_UP090: SpeakerPosition = SpeakerPosition {
    azimuth: Degrees::from_const(90.0),
    elevation: Degrees::from_const(30.0),
    is_lfe: false,
};
/// ITU-R BS.2051 `U-110`: Upper layer (+30° elevation), -110° azimuth (Right Top Rear / `Rtr`).
pub(crate) const SP_UM110: SpeakerPosition = SpeakerPosition {
    azimuth: Degrees::from_const(-110.0),
    elevation: Degrees::from_const(30.0),
    is_lfe: false,
};
/// ITU-R BS.2051 `U+110`: Upper layer (+30° elevation), +110° azimuth (Left Top Rear / `Ltr`).
pub(crate) const SP_UP110: SpeakerPosition = SpeakerPosition {
    azimuth: Degrees::from_const(110.0),
    elevation: Degrees::from_const(30.0),
    is_lfe: false,
};
/// ITU-R BS.2051 `U-135`: Upper layer (+30° elevation), -135° azimuth (Right Top Back / `Rtb` /
/// `TpBR`).
pub(crate) const SP_UM135: SpeakerPosition = SpeakerPosition {
    azimuth: Degrees::from_const(-135.0),
    elevation: Degrees::from_const(30.0),
    is_lfe: false,
};
/// ITU-R BS.2051 `U+135`: Upper layer (+30° elevation), +135° azimuth (Left Top Back / `Ltb` /
/// `TpBL`).
pub(crate) const SP_UP135: SpeakerPosition = SpeakerPosition {
    azimuth: Degrees::from_const(135.0),
    elevation: Degrees::from_const(30.0),
    is_lfe: false,
};
/// ITU-R BS.2051 `U+180`: Upper layer (+30° elevation), 180° azimuth (Top Back Center / `TpBC`).
pub(crate) const SP_UP180: SpeakerPosition = SpeakerPosition {
    azimuth: Degrees::from_const(180.0),
    elevation: Degrees::from_const(30.0),
    is_lfe: false,
};

/// ITU-R BS.2051 `U+180` (45° elevation): Back Center Height for Sound System F.
pub(crate) const SP_UHP180: SpeakerPosition = SpeakerPosition {
    azimuth: Degrees::from_const(180.0),
    elevation: Degrees::from_const(45.0),
    is_lfe: false,
};

/// ITU-R BS.2051 `B+000`: Bottom layer (-30° elevation), 0° azimuth (Bottom Front Center / `BtFC`).
pub(crate) const SP_BP000: SpeakerPosition = SpeakerPosition {
    azimuth: Degrees::ZERO,
    elevation: Degrees::from_const(-30.0),
    is_lfe: false,
};
/// ITU-R BS.2051 `B-045`: Bottom layer (-30° elevation), -45° azimuth (Bottom Front Right /
/// `BtFR`).
pub(crate) const SP_BM045: SpeakerPosition = SpeakerPosition {
    azimuth: Degrees::from_const(-45.0),
    elevation: Degrees::from_const(-30.0),
    is_lfe: false,
};
/// ITU-R BS.2051 `B+045`: Bottom layer (-30° elevation), +45° azimuth (Bottom Front Left / `BtFL`).
pub(crate) const SP_BP045: SpeakerPosition = SpeakerPosition {
    azimuth: Degrees::from_const(45.0),
    elevation: Degrees::from_const(-30.0),
    is_lfe: false,
};
/// ITU-R BS.2051 `B-135`: Bottom layer (-30° elevation), -135° azimuth (Bottom Back Right /
/// `BtBR`).
pub(crate) const SP_BM135: SpeakerPosition = SpeakerPosition {
    azimuth: Degrees::from_const(-135.0),
    elevation: Degrees::from_const(-30.0),
    is_lfe: false,
};
/// ITU-R BS.2051 `B+135`: Bottom layer (-30° elevation), +135° azimuth (Bottom Back Left / `BtBL`).
pub(crate) const SP_BP135: SpeakerPosition = SpeakerPosition {
    azimuth: Degrees::from_const(135.0),
    elevation: Degrees::from_const(-30.0),
    is_lfe: false,
};

/// ITU-R BS.2051 `T+000`: Top layer (+90° elevation), 0° azimuth (Top Center / Voice of God /
/// `TpC`).
pub(crate) const SP_TP000: SpeakerPosition =
    SpeakerPosition { azimuth: Degrees::ZERO, elevation: Degrees::from_const(90.0), is_lfe: false };
/// Single Low-Frequency Effects (LFE) channel at 0° azimuth, -30° elevation.
pub(crate) const SP_LFE: SpeakerPosition =
    SpeakerPosition { azimuth: Degrees::ZERO, elevation: Degrees::from_const(-30.0), is_lfe: true };
/// Primary Low-Frequency Effects (`LFE1`) channel at +45° azimuth, -30° elevation.
pub(crate) const SP_LFE1: SpeakerPosition = SpeakerPosition {
    azimuth: Degrees::from_const(45.0),
    elevation: Degrees::from_const(-30.0),
    is_lfe: true,
};
/// Secondary Low-Frequency Effects (`LFE2`) channel at -45° azimuth, -30° elevation.
pub(crate) const SP_LFE2: SpeakerPosition = SpeakerPosition {
    azimuth: Degrees::from_const(-45.0),
    elevation: Degrees::from_const(-30.0),
    is_lfe: true,
};

/// Target output loudspeaker and headphone layouts supported by the renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OutputLayout {
    /// Mono layout (1.0).
    Mono,
    /// Stereo layout (2.0).
    Stereo,
    /// 5.1 surround sound layout.
    Layout51,
    /// 5.1.2 surround sound layout (with height channels).
    Layout512,
    /// 5.1.4 surround sound layout (with height channels).
    Layout514,
    /// 7.1 surround sound layout.
    Layout71,
    /// 7.1.2 surround sound layout (with height channels).
    Layout712,
    /// 7.1.4 surround sound layout (with height channels).
    Layout714,
    /// 3.1.2 surround sound layout.
    Layout312,
    /// 9.1.6 surround sound layout.
    Layout916,
    /// 22.2 surround sound layout (10.2.9.3 / A293).
    LayoutA293,
    /// 7.1.5.4 surround sound layout.
    Layout7154,
    /// ITU-R BS.2051 Sound System E (4+5+1).
    SoundSystemE451,
    /// ITU-R BS.2051 Sound System F (3+7+0 with 2 LFE channels).
    SoundSystemF370,
    /// ITU-R BS.2051 Sound System G (4+9+0).
    SoundSystemG490,
    /// Binaural headphones rendering layout (2 channels).
    Binaural,
}

impl OutputLayout {
    /// Sound system A (0.2.0), mapping to [`OutputLayout::Stereo`].
    pub const SOUND_SYSTEM_A_020: Self = Self::Stereo;
    /// Sound system B (0.5.0), mapping to [`OutputLayout::Layout51`].
    pub const SOUND_SYSTEM_B_050: Self = Self::Layout51;
    /// Sound system C (2.5.0), mapping to [`OutputLayout::Layout512`].
    pub const SOUND_SYSTEM_C_250: Self = Self::Layout512;
    /// Sound system D (4.5.0), mapping to [`OutputLayout::Layout514`].
    pub const SOUND_SYSTEM_D_450: Self = Self::Layout514;
    /// Sound system I (0.7.0), mapping to [`OutputLayout::Layout71`].
    pub const SOUND_SYSTEM_I_070: Self = Self::Layout71;
    /// Sound system J (4.7.0), mapping to [`OutputLayout::Layout714`].
    pub const SOUND_SYSTEM_J_470: Self = Self::Layout714;
    /// Sound system H (9.a.3), mapping to [`OutputLayout::LayoutA293`].
    pub const SOUND_SYSTEM_H_9A3: Self = Self::LayoutA293;

    /// Returns the number of output channels for this layout.
    pub fn channels(&self) -> ChannelCount {
        let count = self.olr_speakers().map_or(2, <[SpeakerPosition]>::len);
        ChannelCount::new(count).unwrap()
    }

    /// Returns the OLR loudspeaker positions for this output layout, or `None` for binaural.
    pub(crate) fn olr_speakers(&self) -> Option<&'static [SpeakerPosition]> {
        if let Ok(input_layout) = InputLayout::try_from(*self) {
            return input_layout.speakers();
        }
        match self {
            Self::SoundSystemE451 => Some(&[
                SP_MP030, SP_MM030, SP_MP000, SP_LFE, SP_MP110, SP_MM110, SP_UP030, SP_UM030,
                SP_UP110, SP_UM110, SP_BP000,
            ]),
            Self::SoundSystemF370 => Some(&[
                SP_MP000, SP_MP030, SP_MM030, SP_UP045, SP_UM045, SP_MP090, SP_MM090, SP_MP135,
                SP_MM135, SP_UHP180, SP_LFE1, SP_LFE2,
            ]),
            Self::SoundSystemG490 => Some(&[
                SP_MP030, SP_MM030, SP_MP000, SP_LFE, SP_MP090, SP_MM090, SP_MP135, SP_MM135,
                SP_UP045, SP_UM045, SP_UP135, SP_UM135, SP_MP025, SP_MM025,
            ]),
            _ => unreachable!(),
        }
    }
}

/// Input loudspeaker and IAMF channel-subset layouts supported for channel-based audio elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputLayout {
    /// Mono layout (1.0).
    Mono,
    /// Stereo layout (2.0).
    Stereo,
    /// 5.1 surround sound layout.
    Layout51,
    /// 5.1.2 surround sound layout (with height channels).
    Layout512,
    /// 5.1.4 surround sound layout (with height channels).
    Layout514,
    /// 7.1 surround sound layout.
    Layout71,
    /// 7.1.2 surround sound layout (with height channels).
    Layout712,
    /// 7.1.4 surround sound layout (with height channels).
    Layout714,
    /// 3.1.2 surround sound layout.
    Layout312,
    /// 9.1.6 surround sound layout.
    Layout916,
    /// 22.2 surround sound layout (10.2.9.3 / A293).
    LayoutA293,
    /// 7.1.5.4 surround sound layout.
    Layout7154,
    /// Binaural 2-channel layout.
    Binaural,
    /// Low-frequency effects subset (LFE) of 7.1.4ch.
    Lfe,
    /// Surround subset (Ls/Rs) of 5.1.4ch.
    StereoS,
    /// Side surround subset (Lss/Rss) of 7.1.4ch.
    StereoSs,
    /// Rear surround subset (Lrs/Rrs) of 7.1.4ch.
    StereoRs,
    /// Top front subset (Ltf/Rtf) of 7.1.4ch.
    StereoTf,
    /// Top back subset (Ltb/Rtb) of 7.1.4ch.
    StereoTb,
    /// Top 4 channels (Ltf/Rtf/Ltb/Rtb) of 7.1.4ch.
    Top4ch,
    /// Front 3 channels (L/C/R) of 7.1.4ch.
    ThreeCh,
    /// Front subset (FL/FR) of 9.1.6ch.
    StereoF,
    /// Side subset (SiL/SiR) of 9.1.6ch.
    StereoSi,
    /// Top side subset (TpSiL/TpSiR) of 9.1.6ch.
    StereoTpsi,
    /// Top 6 channels (TpFL/TpFR/TpBL/TpBR/TpSiL/TpSiR) of 9.1.6ch.
    Top6ch,
    /// Low-frequency effects subset (LFE1/LFE2) of 10.2.9.3ch.
    LfePair,
    /// Bottom 3 channels (BtFC/BtFL/BtFR) of 10.2.9.3ch.
    Bottom3ch,
    /// Bottom 4 channels (BtFL/BtFR/BtBL/BtBR) of 7.1.5.4ch.
    Bottom4ch,
    /// Top subset (TpC) of 7.1.5.4ch.
    Top1ch,
    /// Top 5 channels (Ltf/Rtf/TpC/Ltb/Rtb) of 7.1.5.4ch.
    Top5ch,
}

impl InputLayout {
    /// Sound system A (0.2.0), mapping to [`InputLayout::Stereo`].
    pub const SOUND_SYSTEM_A_020: Self = Self::Stereo;
    /// Sound system B (0.5.0), mapping to [`InputLayout::Layout51`].
    pub const SOUND_SYSTEM_B_050: Self = Self::Layout51;
    /// Sound system C (2.5.0), mapping to [`InputLayout::Layout512`].
    pub const SOUND_SYSTEM_C_250: Self = Self::Layout512;
    /// Sound system D (4.5.0), mapping to [`InputLayout::Layout514`].
    pub const SOUND_SYSTEM_D_450: Self = Self::Layout514;
    /// Sound system I (0.7.0), mapping to [`InputLayout::Layout71`].
    pub const SOUND_SYSTEM_I_070: Self = Self::Layout71;
    /// Sound system J (4.7.0), mapping to [`InputLayout::Layout714`].
    pub const SOUND_SYSTEM_J_470: Self = Self::Layout714;
    /// Sound system H (9.a.3), mapping to [`InputLayout::LayoutA293`].
    pub const SOUND_SYSTEM_H_9A3: Self = Self::LayoutA293;

    /// Returns the number of input channels for this layout.
    pub fn channels(&self) -> ChannelCount {
        let count = self.speakers().map_or(2, <[SpeakerPosition]>::len);
        ChannelCount::new(count).unwrap()
    }

    /// Returns the standard ITU-R BS.2051 loudspeaker positions for this input layout, or `None`
    /// for binaural.
    pub(crate) fn speakers(&self) -> Option<&'static [SpeakerPosition]> {
        let speakers: &'static [SpeakerPosition] = match self {
            Self::Binaural => return None,
            Self::Mono => &[SP_MP000],
            Self::Stereo => &[SP_MP030, SP_MM030],
            Self::Layout51 => &[SP_MP030, SP_MM030, SP_MP000, SP_LFE, SP_MP110, SP_MM110],
            Self::Layout512 => {
                &[SP_MP030, SP_MM030, SP_MP000, SP_LFE, SP_MP110, SP_MM110, SP_UP030, SP_UM030]
            }
            Self::Layout514 => &[
                SP_MP030, SP_MM030, SP_MP000, SP_LFE, SP_MP110, SP_MM110, SP_UP030, SP_UM030,
                SP_UP110, SP_UM110,
            ],
            Self::Layout71 => {
                &[SP_MP030, SP_MM030, SP_MP000, SP_LFE, SP_MP090, SP_MM090, SP_MP135, SP_MM135]
            }
            Self::Layout712 => &[
                SP_MP030, SP_MM030, SP_MP000, SP_LFE, SP_MP090, SP_MM090, SP_MP135, SP_MM135,
                SP_UP045, SP_UM045,
            ],
            Self::Layout714 => &[
                SP_MP030, SP_MM030, SP_MP000, SP_LFE, SP_MP090, SP_MM090, SP_MP135, SP_MM135,
                SP_UP045, SP_UM045, SP_UP135, SP_UM135,
            ],
            Self::Layout312 => &[SP_MP030, SP_MM030, SP_MP000, SP_LFE, SP_UP045, SP_UM045],
            Self::Layout916 => &[
                SP_MP060, SP_MM060, SP_MP000, SP_LFE1, SP_MP135, SP_MM135, SP_MP030, SP_MM030,
                SP_MP090, SP_MM090, SP_UP045, SP_UM045, SP_UP135, SP_UM135, SP_UP090, SP_UM090,
            ],
            Self::LayoutA293 => &[
                SP_MP060, SP_MM060, SP_MP000, SP_LFE1, SP_MP135, SP_MM135, SP_MP030, SP_MM030,
                SP_MP180, SP_LFE2, SP_MP090, SP_MM090, SP_UP045, SP_UM045, SP_UP000, SP_TP000,
                SP_UP135, SP_UM135, SP_UP090, SP_UM090, SP_UP180, SP_BP000, SP_BP045, SP_BM045,
            ],
            Self::Layout7154 => &[
                SP_MP030, SP_MM030, SP_MP000, SP_LFE, SP_MP090, SP_MM090, SP_MP135, SP_MM135,
                SP_UP045, SP_UM045, SP_TP000, SP_UP135, SP_UM135, SP_BP045, SP_BM045, SP_BP135,
                SP_BM135,
            ],
            Self::Lfe => &[SP_LFE],
            Self::StereoS => &[SP_MP110, SP_MM110],
            Self::StereoSs => &[SP_MP090, SP_MM090],
            Self::StereoRs => &[SP_MP135, SP_MM135],
            Self::StereoTf => &[SP_UP045, SP_UM045],
            Self::StereoTb => &[SP_UP135, SP_UM135],
            Self::Top4ch => &[SP_UP045, SP_UM045, SP_UP135, SP_UM135],
            Self::ThreeCh => &[SP_MP030, SP_MM030, SP_MP000],
            Self::StereoF => &[SP_MP060, SP_MM060],
            Self::StereoSi => &[SP_MP090, SP_MM090],
            Self::StereoTpsi => &[SP_UP090, SP_UM090],
            Self::Top6ch => &[SP_UP045, SP_UM045, SP_UP135, SP_UM135, SP_UP090, SP_UM090],
            Self::LfePair => &[SP_LFE1, SP_LFE2],
            Self::Bottom3ch => &[SP_BP000, SP_BP045, SP_BM045],
            Self::Bottom4ch => &[SP_BP045, SP_BM045, SP_BP135, SP_BM135],
            Self::Top1ch => &[SP_TP000],
            Self::Top5ch => &[SP_UP045, SP_UM045, SP_TP000, SP_UP135, SP_UM135],
        };
        Some(speakers)
    }

    /// Returns the virtual loudspeaker positions used by OBR for binaural rendering.
    ///
    /// Uses the standard ITU-R BS.2051 speaker positions from [`Self::speakers`], with two
    /// intentional OBR overrides matching `liboar`:
    /// - `Layout514` places its four upper speakers at `±45°` and `±135°` azimuth instead of `±30°`
    ///   and `±110°`.
    /// - All upper-layer speakers (`30°` elevation) are elevated to `45°`.
    pub(crate) fn obr_speakers(self) -> Option<impl ExactSizeIterator<Item = SpeakerPosition>> {
        let base_speakers = match self {
            Self::Layout514 => &[
                SP_MP030, SP_MM030, SP_MP000, SP_LFE, SP_MP110, SP_MM110, SP_UP045, SP_UM045,
                SP_UP135, SP_UM135,
            ],
            other => other.speakers()?,
        };
        Some(base_speakers.iter().map(|&sp| {
            let elevation = if sp.elevation == Degrees::from_const(30.0) {
                Degrees::from_const(45.0)
            } else {
                sp.elevation
            };
            SpeakerPosition { elevation, ..sp }
        }))
    }

    /// Returns the canonical IAMF superset [`InputLayout`] for a channel-subset layout, or `None`
    /// for full layouts.
    pub(crate) const fn superset_layout(self) -> Option<Self> {
        match self {
            Self::Mono
            | Self::Stereo
            | Self::Layout51
            | Self::Layout512
            | Self::Layout514
            | Self::Layout71
            | Self::Layout712
            | Self::Layout714
            | Self::Layout312
            | Self::Layout916
            | Self::LayoutA293
            | Self::Layout7154
            | Self::Binaural => None,
            Self::StereoS => Some(Self::Layout514),
            Self::Lfe
            | Self::StereoSs
            | Self::StereoRs
            | Self::StereoTf
            | Self::StereoTb
            | Self::Top4ch
            | Self::ThreeCh => Some(Self::Layout714),
            Self::StereoF | Self::StereoSi | Self::StereoTpsi | Self::Top6ch => {
                Some(Self::Layout916)
            }
            Self::LfePair | Self::Bottom3ch => Some(Self::LayoutA293),
            Self::Bottom4ch | Self::Top1ch | Self::Top5ch => Some(Self::Layout7154),
        }
    }

    /// For a layout defines as the subset of another layout, returns its superset [`InputLayout`]
    /// and an iterator yielding each subset channel's index within that superset layout.
    ///
    /// Returns `None` for full (non-subset) layouts.
    pub(crate) fn superset_and_subset_channels(
        self,
    ) -> Option<(Self, impl ExactSizeIterator<Item = usize> + use<>)> {
        let superset = self.superset_layout()?;
        let superset_speakers = superset.speakers()?;
        let subset_speakers = self.speakers()?;
        let indices = subset_speakers.iter().map(move |sp| {
            superset_speakers
                .iter()
                .position(|superset_sp| superset_sp == sp)
                .expect("subset speaker must exist in its superset layout")
        });
        Some((superset, indices))
    }
}

impl TryFrom<OutputLayout> for InputLayout {
    type Error = OarError;

    fn try_from(layout: OutputLayout) -> Result<Self, Self::Error> {
        match layout {
            OutputLayout::Mono => Ok(Self::Mono),
            OutputLayout::Stereo => Ok(Self::Stereo),
            OutputLayout::Layout51 => Ok(Self::Layout51),
            OutputLayout::Layout512 => Ok(Self::Layout512),
            OutputLayout::Layout514 => Ok(Self::Layout514),
            OutputLayout::Layout71 => Ok(Self::Layout71),
            OutputLayout::Layout712 => Ok(Self::Layout712),
            OutputLayout::Layout714 => Ok(Self::Layout714),
            OutputLayout::Layout312 => Ok(Self::Layout312),
            OutputLayout::Layout916 => Ok(Self::Layout916),
            OutputLayout::LayoutA293 => Ok(Self::LayoutA293),
            OutputLayout::Layout7154 => Ok(Self::Layout7154),
            OutputLayout::Binaural => Ok(Self::Binaural),
            OutputLayout::SoundSystemE451
            | OutputLayout::SoundSystemF370
            | OutputLayout::SoundSystemG490 => Err(OarError::InvalidParameter),
        }
    }
}

impl TryFrom<InputLayout> for OutputLayout {
    type Error = OarError;

    fn try_from(layout: InputLayout) -> Result<Self, Self::Error> {
        match layout {
            InputLayout::Mono => Ok(Self::Mono),
            InputLayout::Stereo => Ok(Self::Stereo),
            InputLayout::Layout51 => Ok(Self::Layout51),
            InputLayout::Layout512 => Ok(Self::Layout512),
            InputLayout::Layout514 => Ok(Self::Layout514),
            InputLayout::Layout71 => Ok(Self::Layout71),
            InputLayout::Layout712 => Ok(Self::Layout712),
            InputLayout::Layout714 => Ok(Self::Layout714),
            InputLayout::Layout312 => Ok(Self::Layout312),
            InputLayout::Layout916 => Ok(Self::Layout916),
            InputLayout::LayoutA293 => Ok(Self::LayoutA293),
            InputLayout::Layout7154 => Ok(Self::Layout7154),
            InputLayout::Binaural => Ok(Self::Binaural),
            InputLayout::Lfe
            | InputLayout::StereoS
            | InputLayout::StereoSs
            | InputLayout::StereoRs
            | InputLayout::StereoTf
            | InputLayout::StereoTb
            | InputLayout::Top4ch
            | InputLayout::ThreeCh
            | InputLayout::StereoF
            | InputLayout::StereoSi
            | InputLayout::StereoTpsi
            | InputLayout::Top6ch
            | InputLayout::LfePair
            | InputLayout::Bottom3ch
            | InputLayout::Bottom4ch
            | InputLayout::Top1ch
            | InputLayout::Top5ch => Err(OarError::InvalidParameter),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use googletest::prelude::*;

    #[gtest]
    fn output_layout_sound_system_aliases_match_standard_layouts() {
        expect_that!(OutputLayout::SOUND_SYSTEM_A_020, eq(OutputLayout::Stereo));
        expect_that!(OutputLayout::SOUND_SYSTEM_B_050, eq(OutputLayout::Layout51));
        expect_that!(OutputLayout::SOUND_SYSTEM_C_250, eq(OutputLayout::Layout512));
        expect_that!(OutputLayout::SOUND_SYSTEM_D_450, eq(OutputLayout::Layout514));
        expect_that!(OutputLayout::SOUND_SYSTEM_I_070, eq(OutputLayout::Layout71));
        expect_that!(OutputLayout::SOUND_SYSTEM_J_470, eq(OutputLayout::Layout714));
        expect_that!(OutputLayout::SOUND_SYSTEM_H_9A3, eq(OutputLayout::LayoutA293));
    }

    #[gtest]
    fn input_layout_sound_system_aliases_match_standard_layouts() {
        expect_that!(InputLayout::SOUND_SYSTEM_A_020, eq(InputLayout::Stereo));
        expect_that!(InputLayout::SOUND_SYSTEM_B_050, eq(InputLayout::Layout51));
        expect_that!(InputLayout::SOUND_SYSTEM_C_250, eq(InputLayout::Layout512));
        expect_that!(InputLayout::SOUND_SYSTEM_D_450, eq(InputLayout::Layout514));
        expect_that!(InputLayout::SOUND_SYSTEM_I_070, eq(InputLayout::Layout71));
        expect_that!(InputLayout::SOUND_SYSTEM_J_470, eq(InputLayout::Layout714));
        expect_that!(InputLayout::SOUND_SYSTEM_H_9A3, eq(InputLayout::LayoutA293));
    }

    #[gtest]
    fn output_layout_channels_returns_expected_channel_counts() {
        expect_that!(OutputLayout::Mono.channels().get(), eq(1));
        expect_that!(OutputLayout::Stereo.channels().get(), eq(2));
        expect_that!(OutputLayout::Layout51.channels().get(), eq(6));
        expect_that!(OutputLayout::Layout714.channels().get(), eq(12));
        expect_that!(OutputLayout::SoundSystemE451.channels().get(), eq(11));
        expect_that!(OutputLayout::SoundSystemF370.channels().get(), eq(12));
        expect_that!(OutputLayout::SoundSystemG490.channels().get(), eq(14));
        expect_that!(OutputLayout::LayoutA293.channels().get(), eq(24));
        expect_that!(OutputLayout::Binaural.channels().get(), eq(2));
    }

    #[gtest]
    fn input_layout_channels_returns_expected_channel_counts() {
        expect_that!(InputLayout::Mono.channels().get(), eq(1));
        expect_that!(InputLayout::Binaural.channels().get(), eq(2));
        expect_that!(InputLayout::Layout51.channels().get(), eq(6));
        expect_that!(InputLayout::Layout916.channels().get(), eq(16));
        expect_that!(InputLayout::Lfe.channels().get(), eq(1));
        expect_that!(InputLayout::Top4ch.channels().get(), eq(4));
        expect_that!(InputLayout::Top6ch.channels().get(), eq(6));
        expect_that!(InputLayout::Bottom3ch.channels().get(), eq(3));
        expect_that!(InputLayout::Top5ch.channels().get(), eq(5));
    }

    #[gtest]
    fn try_from_output_layout_converts_shared_layouts_and_rejects_output_only_layouts() {
        let converted = InputLayout::try_from(OutputLayout::Layout714);
        let converted_binaural = InputLayout::try_from(OutputLayout::Binaural);
        let rejected_e451 = InputLayout::try_from(OutputLayout::SoundSystemE451);

        expect_that!(converted, ok(eq(InputLayout::Layout714)));
        expect_that!(converted_binaural, ok(eq(InputLayout::Binaural)));
        expect_that!(rejected_e451, err(eq(OarError::InvalidParameter)));
    }

    #[gtest]
    fn try_from_input_layout_converts_shared_layouts_and_rejects_subset_layouts() {
        let converted = OutputLayout::try_from(InputLayout::Layout512);
        let converted_binaural = OutputLayout::try_from(InputLayout::Binaural);
        let rejected_subset = OutputLayout::try_from(InputLayout::Top4ch);

        expect_that!(converted, ok(eq(OutputLayout::Layout512)));
        expect_that!(converted_binaural, ok(eq(OutputLayout::Binaural)));
        expect_that!(rejected_subset, err(eq(OarError::InvalidParameter)));
    }

    #[gtest]
    fn olr_speakers_returns_expected_positions_and_none_for_binaural() {
        let stereo = OutputLayout::Stereo.olr_speakers().unwrap();
        let layout51 = OutputLayout::Layout51.olr_speakers().unwrap();
        let f370 = OutputLayout::SoundSystemF370.olr_speakers().unwrap();
        let binaural = OutputLayout::Binaural.olr_speakers();

        expect_that!(stereo.len(), eq(2));
        expect_that!(stereo[0].is_lfe, eq(false));
        expect_that!(layout51.len(), eq(6));
        expect_that!(layout51.iter().filter(|s| s.is_lfe).count(), eq(1));
        expect_that!(f370[9], eq(SP_UHP180));
        expect_that!(binaural, none());
    }

    #[gtest]
    fn obr_speakers_applies_514_azimuth_and_upper_layer_elevation_overrides() {
        let obr_514: Vec<_> = InputLayout::Layout514.obr_speakers().unwrap().collect();
        let obr_top4: Vec<_> = InputLayout::Top4ch.obr_speakers().unwrap().collect();
        let obr_bottom3: Vec<_> = InputLayout::Bottom3ch.obr_speakers().unwrap().collect();
        let obr_binaural = InputLayout::Binaural.obr_speakers();

        expect_that!(obr_514.len(), eq(10));
        expect_that!(obr_514[6].azimuth, eq(Degrees::from_const(45.0)));
        expect_that!(obr_514[6].elevation, eq(Degrees::from_const(45.0)));
        expect_that!(obr_514[8].azimuth, eq(Degrees::from_const(135.0)));
        expect_that!(obr_514[8].elevation, eq(Degrees::from_const(45.0)));
        expect_that!(obr_top4.len(), eq(4));
        expect_that!(obr_top4[0].elevation, eq(Degrees::from_const(45.0)));
        expect_that!(obr_bottom3.len(), eq(3));
        expect_that!(obr_bottom3[0].elevation, eq(Degrees::from_const(-30.0)));
        expect_true!(obr_binaural.is_none());
    }

    #[gtest]
    fn superset_and_subset_channels_returns_none_for_full_input_layouts() {
        let full_layouts = [
            InputLayout::Mono,
            InputLayout::Stereo,
            InputLayout::Layout51,
            InputLayout::Layout512,
            InputLayout::Layout514,
            InputLayout::Layout71,
            InputLayout::Layout712,
            InputLayout::Layout714,
            InputLayout::Layout312,
            InputLayout::Layout916,
            InputLayout::LayoutA293,
            InputLayout::Layout7154,
            InputLayout::Binaural,
        ];

        for layout in full_layouts {
            let result = layout.superset_and_subset_channels();

            expect_true!(result.is_none());
        }
    }

    #[gtest]
    fn superset_and_subset_channels_maps_all_subset_layouts_to_expected_superset_indices() {
        let cases: &[(InputLayout, InputLayout, &[usize])] = &[
            (InputLayout::Lfe, InputLayout::Layout714, &[3]),
            (InputLayout::StereoS, InputLayout::Layout514, &[4, 5]),
            (InputLayout::StereoSs, InputLayout::Layout714, &[4, 5]),
            (InputLayout::StereoRs, InputLayout::Layout714, &[6, 7]),
            (InputLayout::StereoTf, InputLayout::Layout714, &[8, 9]),
            (InputLayout::StereoTb, InputLayout::Layout714, &[10, 11]),
            (InputLayout::Top4ch, InputLayout::Layout714, &[8, 9, 10, 11]),
            (InputLayout::ThreeCh, InputLayout::Layout714, &[0, 1, 2]),
            (InputLayout::StereoF, InputLayout::Layout916, &[0, 1]),
            (InputLayout::StereoSi, InputLayout::Layout916, &[8, 9]),
            (InputLayout::StereoTpsi, InputLayout::Layout916, &[14, 15]),
            (InputLayout::Top6ch, InputLayout::Layout916, &[10, 11, 12, 13, 14, 15]),
            (InputLayout::LfePair, InputLayout::LayoutA293, &[3, 9]),
            (InputLayout::Bottom3ch, InputLayout::LayoutA293, &[21, 22, 23]),
            (InputLayout::Bottom4ch, InputLayout::Layout7154, &[13, 14, 15, 16]),
            (InputLayout::Top1ch, InputLayout::Layout7154, &[10]),
            (InputLayout::Top5ch, InputLayout::Layout7154, &[8, 9, 10, 11, 12]),
        ];

        for &(subset, expected_superset, expected_indices) in cases {
            let (superset, indices) = subset.superset_and_subset_channels().unwrap();
            let collected: Vec<usize> = indices.collect();

            expect_eq!(superset, expected_superset);
            expect_eq!(collected.as_slice(), expected_indices);
        }
    }
}
