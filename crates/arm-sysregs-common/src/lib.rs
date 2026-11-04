// SPDX-FileCopyrightText: Copyright The arm-sysregs Contributors.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Common arm-sysreg components.

#![cfg_attr(not(any(test, feature = "fakes")), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(all(not(any(test, feature = "fakes")), target_arch = "arm"))]
pub mod aarch32;
#[cfg(all(not(any(test, feature = "fakes")), target_arch = "aarch64"))]
pub mod aarch64;
#[cfg(any(test, feature = "fakes"))]
pub mod fake;
pub mod macros;
pub mod types;

#[doc(hidden)]
pub use pastey as _pastey;

/// Shifts `$value` left by `$extra_bits` and then right again by the same amount.
///
/// If `$value` is unsigned then this has the effect of either zeroing out the top `$extra_bits`
/// bits. If it is signed, then the top bits will be set to match the next highest bit, extending
/// the sign of a signed field.
#[macro_export]
macro_rules! mask_extend {
    ($value:expr, $extra_bits:expr) => {
        $value << $extra_bits >> $extra_bits
    };
}
