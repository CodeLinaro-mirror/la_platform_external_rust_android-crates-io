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

//! Core common data structures and utilities for OAR.

pub mod animation;
pub mod audio_buffer;
pub mod config;
pub mod coordinates;
pub mod definitions;
pub mod downmix;
pub mod gains;
pub mod hoa;
pub mod layout;
pub mod object_positions;
pub mod quaternion;
pub mod units;
