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

//! Static assets wrapper for embedded binaural filters.
//!
//! Provides the `BinauralFiltersWrapper` struct which resolves static embedded Rice-encoded
//! filter data by name.

/// Wrapper struct providing access to embedded binaural filter asset files.
#[derive(Debug, Default, Clone, Copy)]
pub struct BinauralFiltersWrapper;

impl BinauralFiltersWrapper {
    /// Constructs a new `BinauralFiltersWrapper` instance.
    pub fn new() -> Self {
        Self
    }

    /// Retrieves the binary content of an embedded Rice-encoded filter asset by name.
    ///
    /// Returns `Some(&[u8])` if the asset exists, or `None` otherwise.
    pub fn get_file(&self, filename: &str) -> Option<&'static [u8]> {
        match filename {
            "1OAAmbientL" => {
                Some(super::binaural_filters_1_oa_ambient_l::get_binauralfilters1oaambientl())
            }
            "1OAAmbientR" => {
                Some(super::binaural_filters_1_oa_ambient_r::get_binauralfilters1oaambientr())
            }
            "1OADirectL" => {
                Some(super::binaural_filters_1_oa_direct_l::get_binauralfilters1oadirectl())
            }
            "1OADirectR" => {
                Some(super::binaural_filters_1_oa_direct_r::get_binauralfilters1oadirectr())
            }
            "1OAReverberantL" => Some(
                super::binaural_filters_1_oa_reverberant_l::get_binauralfilters1oareverberantl(),
            ),
            "1OAReverberantR" => Some(
                super::binaural_filters_1_oa_reverberant_r::get_binauralfilters1oareverberantr(),
            ),
            "2OAAmbientL" => {
                Some(super::binaural_filters_2_oa_ambient_l::get_binauralfilters2oaambientl())
            }
            "2OAAmbientR" => {
                Some(super::binaural_filters_2_oa_ambient_r::get_binauralfilters2oaambientr())
            }
            "2OADirectL" => {
                Some(super::binaural_filters_2_oa_direct_l::get_binauralfilters2oadirectl())
            }
            "2OADirectR" => {
                Some(super::binaural_filters_2_oa_direct_r::get_binauralfilters2oadirectr())
            }
            "2OAReverberantL" => Some(
                super::binaural_filters_2_oa_reverberant_l::get_binauralfilters2oareverberantl(),
            ),
            "2OAReverberantR" => Some(
                super::binaural_filters_2_oa_reverberant_r::get_binauralfilters2oareverberantr(),
            ),
            "3OAAmbientL" => {
                Some(super::binaural_filters_3_oa_ambient_l::get_binauralfilters3oaambientl())
            }
            "3OAAmbientR" => {
                Some(super::binaural_filters_3_oa_ambient_r::get_binauralfilters3oaambientr())
            }
            "3OADirectL" => {
                Some(super::binaural_filters_3_oa_direct_l::get_binauralfilters3oadirectl())
            }
            "3OADirectR" => {
                Some(super::binaural_filters_3_oa_direct_r::get_binauralfilters3oadirectr())
            }
            "3OAReverberantL" => Some(
                super::binaural_filters_3_oa_reverberant_l::get_binauralfilters3oareverberantl(),
            ),
            "3OAReverberantR" => Some(
                super::binaural_filters_3_oa_reverberant_r::get_binauralfilters3oareverberantr(),
            ),
            "4OAAmbientL" => {
                Some(super::binaural_filters_4_oa_ambient_l::get_binauralfilters4oaambientl())
            }
            "4OAAmbientR" => {
                Some(super::binaural_filters_4_oa_ambient_r::get_binauralfilters4oaambientr())
            }
            "4OADirectL" => {
                Some(super::binaural_filters_4_oa_direct_l::get_binauralfilters4oadirectl())
            }
            "4OADirectR" => {
                Some(super::binaural_filters_4_oa_direct_r::get_binauralfilters4oadirectr())
            }
            "4OAReverberantL" => Some(
                super::binaural_filters_4_oa_reverberant_l::get_binauralfilters4oareverberantl(),
            ),
            "4OAReverberantR" => Some(
                super::binaural_filters_4_oa_reverberant_r::get_binauralfilters4oareverberantr(),
            ),
            _ => None,
        }
    }
}
