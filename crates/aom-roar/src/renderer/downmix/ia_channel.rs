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

//! Internal IAMF downmix channel definitions and layout downmix compatibility rules.

use crate::common::layout::{InputLayout, OutputLayout};

/// Internal representation of physical IAMF channels (`IAChannel` equivalent in `liboar`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum IAChannel {
    /// Left channel (7.x loudspeaker configuration).
    L7,
    /// Right channel (7.x loudspeaker configuration).
    R7,
    /// Center channel.
    C,
    /// Low-frequency effects (LFE) channel.
    Lfe,
    /// Surround Left channel (7.x loudspeaker configuration).
    Sl7,
    /// Surround Right channel (7.x loudspeaker configuration).
    Sr7,
    /// Back Left channel (7.x loudspeaker configuration).
    Bl7,
    /// Back Right channel (7.x loudspeaker configuration).
    Br7,
    /// Height Front Left channel.
    Hfl,
    /// Height Front Right channel.
    Hfr,
    /// Height Back Left channel.
    Hbl,
    /// Height Back Right channel.
    Hbr,
    /// Mono channel.
    Mono,
    /// Left channel (Stereo/2.0 loudspeaker configuration).
    L2,
    /// Right channel (Stereo/2.0 loudspeaker configuration).
    R2,
    /// Top Left channel.
    Tl,
    /// Top Right channel.
    Tr,
    /// Left channel (3.x loudspeaker configuration).
    L3,
    /// Right channel (3.x loudspeaker configuration).
    R3,
    /// Surround Left channel (5.x loudspeaker configuration).
    Sl5,
    /// Surround Right channel (5.x loudspeaker configuration).
    Sr5,
    /// Height Left channel.
    Hl,
    /// Height Right channel.
    Hr,
}

impl IAChannel {
    pub(super) const L5: Self = Self::L7;
    pub(super) const R5: Self = Self::R7;
}

impl InputLayout {
    /// Returns the `(height, surround)` channel counts for layouts participating in IAMF
    /// downmixing.
    fn downmix_height_and_surround(&self) -> Option<(usize, usize)> {
        match self {
            Self::Mono => Some((0, 1)),
            Self::Stereo => Some((0, 2)),
            Self::Layout51 => Some((0, 5)),
            Self::Layout512 => Some((2, 5)),
            Self::Layout514 => Some((4, 5)),
            Self::Layout71 => Some((0, 7)),
            Self::Layout712 => Some((2, 7)),
            Self::Layout714 => Some((4, 7)),
            Self::Layout312 => Some((2, 3)),
            _ => None,
        }
    }

    /// Checks whether IAMF downmixing from this input layout to `target` is supported.
    ///
    /// Matches `DMRenderer_open` in `liboar`, which rejects identical input/output layouts
    /// (`in == out`, handled as pass-through by EAR), non-IAMF-downmix layouts,
    /// height-to-non-height mixes (`hi > 0 && ho == 0`), and upmixes (`si < so || hi < ho`).
    pub(crate) fn is_valid_downmix_to(&self, target: OutputLayout) -> bool {
        if InputLayout::try_from(target).ok() == Some(*self) {
            return false;
        }
        let (Some((hi, si)), Some((ho, so))) =
            (self.downmix_height_and_surround(), target.downmix_height_and_surround())
        else {
            return false;
        };
        if hi > 0 && ho == 0 {
            return false;
        }
        si >= so && hi >= ho
    }

    /// Returns the ordered IAMF downmix channels for this input layout (`_get_channels` in
    /// `liboar`).
    pub(super) fn downmix_channels(&self) -> Option<&'static [IAChannel]> {
        match self {
            Self::Mono => Some(&[IAChannel::Mono]),
            Self::Stereo => Some(&[IAChannel::L2, IAChannel::R2]),
            Self::Layout51 => Some(&[
                IAChannel::L5,
                IAChannel::R5,
                IAChannel::C,
                IAChannel::Lfe,
                IAChannel::Sl5,
                IAChannel::Sr5,
            ]),
            Self::Layout512 => Some(&[
                IAChannel::L5,
                IAChannel::R5,
                IAChannel::C,
                IAChannel::Lfe,
                IAChannel::Sl5,
                IAChannel::Sr5,
                IAChannel::Hl,
                IAChannel::Hr,
            ]),
            Self::Layout514 => Some(&[
                IAChannel::L5,
                IAChannel::R5,
                IAChannel::C,
                IAChannel::Lfe,
                IAChannel::Sl5,
                IAChannel::Sr5,
                IAChannel::Hfl,
                IAChannel::Hfr,
                IAChannel::Hbl,
                IAChannel::Hbr,
            ]),
            Self::Layout71 => Some(&[
                IAChannel::L7,
                IAChannel::R7,
                IAChannel::C,
                IAChannel::Lfe,
                IAChannel::Sl7,
                IAChannel::Sr7,
                IAChannel::Bl7,
                IAChannel::Br7,
            ]),
            Self::Layout712 => Some(&[
                IAChannel::L7,
                IAChannel::R7,
                IAChannel::C,
                IAChannel::Lfe,
                IAChannel::Sl7,
                IAChannel::Sr7,
                IAChannel::Bl7,
                IAChannel::Br7,
                IAChannel::Hl,
                IAChannel::Hr,
            ]),
            Self::Layout714 => Some(&[
                IAChannel::L7,
                IAChannel::R7,
                IAChannel::C,
                IAChannel::Lfe,
                IAChannel::Sl7,
                IAChannel::Sr7,
                IAChannel::Bl7,
                IAChannel::Br7,
                IAChannel::Hfl,
                IAChannel::Hfr,
                IAChannel::Hbl,
                IAChannel::Hbr,
            ]),
            Self::Layout312 => Some(&[
                IAChannel::L3,
                IAChannel::R3,
                IAChannel::C,
                IAChannel::Lfe,
                IAChannel::Tl,
                IAChannel::Tr,
            ]),
            _ => None,
        }
    }
}

impl OutputLayout {
    /// Returns the `(height, surround)` channel counts for layouts participating in IAMF
    /// downmixing.
    fn downmix_height_and_surround(&self) -> Option<(usize, usize)> {
        InputLayout::try_from(*self).ok()?.downmix_height_and_surround()
    }

    /// Returns the ordered IAMF downmix channels for this output layout (`_get_channels` in
    /// `liboar`).
    pub(super) fn downmix_channels(&self) -> Option<&'static [IAChannel]> {
        InputLayout::try_from(*self).ok()?.downmix_channels()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use googletest::prelude::*;

    #[gtest]
    fn downmix_height_and_surround_returns_expected_counts() {
        expect_that!(InputLayout::Stereo.downmix_height_and_surround(), eq(Some((0, 2))));
        expect_that!(InputLayout::Layout512.downmix_height_and_surround(), eq(Some((2, 5))));
        expect_that!(InputLayout::Layout714.downmix_height_and_surround(), eq(Some((4, 7))));
        expect_that!(OutputLayout::Binaural.downmix_height_and_surround(), none());
    }

    #[gtest]
    fn is_valid_downmix_to_validates_surround_and_height_hierarchy() {
        expect_true!(InputLayout::Layout71.is_valid_downmix_to(OutputLayout::Stereo));
        expect_true!(InputLayout::Layout714.is_valid_downmix_to(OutputLayout::Layout512));
        expect_false!(InputLayout::Stereo.is_valid_downmix_to(OutputLayout::Stereo));
        expect_false!(InputLayout::Layout512.is_valid_downmix_to(OutputLayout::Layout512));
        expect_false!(InputLayout::Layout512.is_valid_downmix_to(OutputLayout::Layout51));
        expect_false!(InputLayout::Layout512.is_valid_downmix_to(OutputLayout::Stereo));
        expect_false!(InputLayout::Stereo.is_valid_downmix_to(OutputLayout::Layout51));
        expect_false!(InputLayout::Layout512.is_valid_downmix_to(OutputLayout::Layout712));
        expect_false!(InputLayout::Layout512.is_valid_downmix_to(OutputLayout::Layout514));
        expect_false!(InputLayout::Layout51.is_valid_downmix_to(OutputLayout::Binaural));
    }
}
