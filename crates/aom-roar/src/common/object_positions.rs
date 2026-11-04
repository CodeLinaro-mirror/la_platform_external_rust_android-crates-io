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

use super::animation::{AnimatedCartesian, AnimatedPolar};
use super::coordinates::{CartesianCoordinate, PolarCoordinate};

/// Object position metadata for object-based audio.
#[derive(Debug, Clone, PartialEq)]
pub enum ObjectPosition {
    /// Static position in polar coordinates.
    Polar(Vec<PolarCoordinate>),
    /// Static position in Cartesian coordinates.
    Cartesian(Vec<CartesianCoordinate>),
    /// Dynamic interpolated position in polar coordinates.
    AnimatedPolar(Vec<AnimatedPolar>),
    /// Dynamic interpolated position in Cartesian coordinates.
    AnimatedCartesian(Vec<AnimatedCartesian>),
}
