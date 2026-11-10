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

//! Core definitions, layout configurations, and error types for the OAR library.

use std::fmt;

/// C-compatible status code indicating successful operations (`ERoarStatus::ck_roar_ok`).
pub const ROAR_STATUS_OK: std::os::raw::c_int = 0;

/// Maximum number of samples per channel supported by the renderer.
pub const MAX_SAMPLES_PER_CHANNEL: u32 = 16384;

/// Maximum output channel count, used to allocate vectors of pointers.
pub const MAX_OUTPUT_CHANNEL_COUNT: usize = 25;

// TODO(b/512062316): Rename to RoarError.
/// Errors returned by the ROAR library, mapping to C error status codes (`ERoarStatus`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum OarError {
    /// Out of memory error.
    NoMem = -12,
    /// Resource busy or unavailable error or pre-requisite function not called.
    Busy = -16,
    /// Invalid parameters error.
    InvalidParameter = -22,
    /// Function not implemented error.
    NoSys = -38,
    /// Operation not supported error.
    NotSupported = -95,
}

/// Returns the raw integer C-compatible status code (`ERoarStatus`).
impl From<OarError> for std::os::raw::c_int {
    fn from(error: OarError) -> Self {
        error as std::os::raw::c_int
    }
}

impl fmt::Display for OarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OarError::NoMem => write!(f, "Out of memory error"),
            OarError::Busy => write!(
                f,
                "Resource busy or unavailable error or pre-requisite function not called."
            ),
            OarError::InvalidParameter => write!(f, "Invalid parameters error"),
            OarError::NoSys => write!(f, "Function not implemented error"),
            OarError::NotSupported => write!(f, "Operation not supported error"),
        }
    }
}

impl std::error::Error for OarError {}

/// Type-safe identifier for rendering groups (restricted to 0 or 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GroupId {
    Zero = 0,
    One = 1,
}

impl TryFrom<u32> for GroupId {
    type Error = OarError;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(GroupId::Zero),
            1 => Ok(GroupId::One),
            _ => Err(OarError::InvalidParameter),
        }
    }
}

impl From<GroupId> for u32 {
    fn from(id: GroupId) -> Self {
        id as u32
    }
}

impl From<GroupId> for i32 {
    fn from(id: GroupId) -> Self {
        id as i32
    }
}

/// Default maximum number of dynamic audio objects supported by the renderer.
pub const DEF_MAX_NUMBER_OF_OBJECTS: usize = 2;

/// Default capacity for update queues (gains, positions, etc.) to prevent heap allocations during
/// rendering.
pub const METADATA_QUEUE_CAPACITY: usize = 4;

// Re-export submodules from super
pub use super::animation::*;
pub use super::audio_buffer::*;
pub use super::config::*;
pub use super::coordinates::*;
pub use super::downmix::*;
pub use super::gains::*;
pub use super::hoa::*;
pub use super::layout::*;
pub use super::object_positions::*;
pub use super::quaternion::*;
pub use super::units::*;

// ===== Tests =====

#[cfg(test)]
mod test {

    use super::*;
    use googletest::prelude::*;
    use std::error::Error;

    #[gtest]
    fn test_oar_error_discriminants() {
        expect_that!(ROAR_STATUS_OK, eq(0));
        expect_that!(OarError::NoMem as i32, eq(-12));
        expect_that!(OarError::Busy as i32, eq(-16));
        expect_that!(OarError::InvalidParameter as i32, eq(-22));
        expect_that!(OarError::NoSys as i32, eq(-38));
        expect_that!(OarError::NotSupported as i32, eq(-95));
    }

    #[gtest]
    fn test_oar_error_display() {
        expect_that!(
            format!("{}", OarError::NoMem).to_lowercase(),
            contains_substring("out of memory")
        );
        expect_that!(format!("{}", OarError::Busy).to_lowercase(), contains_substring("busy"));
        expect_that!(
            format!("{}", OarError::InvalidParameter).to_lowercase(),
            contains_substring("invalid parameters")
        );
        expect_that!(
            format!("{}", OarError::NoSys).to_lowercase(),
            contains_substring("not implemented")
        );
        expect_that!(
            format!("{}", OarError::NotSupported).to_lowercase(),
            contains_substring("not supported")
        );
    }

    #[gtest]
    fn test_oar_error_implements_std_error() {
        let err: OarError = OarError::InvalidParameter;
        let err_trait: &dyn Error = &err;
        expect_that!(format!("{}", err_trait), eq("Invalid parameters error"));
    }

    #[gtest]
    fn test_group_id_try_from() {
        expect_that!(GroupId::try_from(0), ok(eq(GroupId::Zero)));
        expect_that!(GroupId::try_from(1), ok(eq(GroupId::One)));
        expect_that!(GroupId::try_from(2), err(eq(OarError::InvalidParameter)));
    }

    #[gtest]
    fn test_group_id_into() {
        expect_that!(u32::from(GroupId::Zero), eq(0));
        expect_that!(u32::from(GroupId::One), eq(1));
        expect_that!(i32::from(GroupId::Zero), eq(0));
        expect_that!(i32::from(GroupId::One), eq(1));
    }
}
