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

//! Embedded binaural filter coefficients.
//!
//! Provides access to embedded spherical harmonic HRIR filter coefficients (direct, ambient, and
//! reverberant) for orders 1-4.

pub mod binaural_filters_1_oa_ambient_l;
pub mod binaural_filters_1_oa_ambient_r;
pub mod binaural_filters_1_oa_direct_l;
pub mod binaural_filters_1_oa_direct_r;
pub mod binaural_filters_1_oa_reverberant_l;
pub mod binaural_filters_1_oa_reverberant_r;
pub mod binaural_filters_2_oa_ambient_l;
pub mod binaural_filters_2_oa_ambient_r;
pub mod binaural_filters_2_oa_direct_l;
pub mod binaural_filters_2_oa_direct_r;
pub mod binaural_filters_2_oa_reverberant_l;
pub mod binaural_filters_2_oa_reverberant_r;
pub mod binaural_filters_3_oa_ambient_l;
pub mod binaural_filters_3_oa_ambient_r;
pub mod binaural_filters_3_oa_direct_l;
pub mod binaural_filters_3_oa_direct_r;
pub mod binaural_filters_3_oa_reverberant_l;
pub mod binaural_filters_3_oa_reverberant_r;
pub mod binaural_filters_4_oa_ambient_l;
pub mod binaural_filters_4_oa_ambient_r;
pub mod binaural_filters_4_oa_direct_l;
pub mod binaural_filters_4_oa_direct_r;
pub mod binaural_filters_4_oa_reverberant_l;
pub mod binaural_filters_4_oa_reverberant_r;
pub mod binaural_filters_wrapper;

pub use binaural_filters_wrapper::BinauralFiltersWrapper;
