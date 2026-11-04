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

//! Channel view wrappers and specifications.

use super::simd_utils::{
    add_pointwise_in_place, multiply_pointwise_in_place, subtract_pointwise_in_place,
};
use std::ops::{Index, IndexMut};

/// Metadata specification for a channel view offset stored inside an `AudioBuffer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelSpec {
    /// Offset index into the underlying 1D data buffer.
    pub offset: usize,
    /// Size of the channel stride in frames.
    pub size: usize,
    /// Indicates whether this channel is enabled and contains active audio.
    pub enabled: bool,
}

impl ChannelSpec {
    /// Constructs a new `ChannelSpec`.
    pub fn new(offset: usize, size: usize) -> Self {
        Self { offset, size, enabled: true }
    }
}

/// An immutable view onto an individual audio channel stride.
#[derive(Debug, Clone, Copy)]
pub struct ChannelView<'a> {
    /// Reference to the underlying slice.
    pub data: &'a [f32],
    /// Indicates whether the channel is enabled.
    pub enabled: bool,
}

impl<'a> ChannelView<'a> {
    /// Constructs a new `ChannelView`.
    pub fn new(data: &'a [f32], enabled: bool) -> Self {
        Self { data, enabled }
    }

    /// Returns the number of frames in the view.
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Returns whether the view contains zero frames.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns whether the channel is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Accesses the underlying data as a raw slice.
    ///
    /// # Panics
    /// Panics if the channel is disabled.
    pub fn as_slice(&self) -> &'a [f32] {
        assert!(self.enabled, "Accessed disabled ChannelView");
        self.data
    }
}

impl<'a> Index<usize> for ChannelView<'a> {
    type Output = f32;
    fn index(&self, index: usize) -> &Self::Output {
        assert!(self.enabled, "Accessed disabled ChannelView");
        &self.data[index]
    }
}

/// A mutable view onto an individual audio channel stride.
///
/// Supports pointwise vector arithmetic operations.
#[derive(Debug)]
pub struct ChannelViewMut<'a> {
    /// Mutable reference to the underlying slice.
    pub data: &'a mut [f32],
    /// Indicates whether the channel is enabled.
    pub enabled: bool,
}

impl<'a> ChannelViewMut<'a> {
    /// Constructs a new `ChannelViewMut`.
    pub fn new(data: &'a mut [f32], enabled: bool) -> Self {
        Self { data, enabled }
    }

    /// Returns the number of frames in the view.
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Returns whether the view contains zero frames.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns whether the channel is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Sets whether the channel is enabled.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Fills the channel view with zero samples.
    ///
    /// # Panics
    /// Panics if the channel is disabled.
    pub fn clear(&mut self) {
        assert!(self.enabled, "Accessed disabled ChannelViewMut");
        self.data.fill(0.0);
    }

    /// Accesses the underlying data as an immutable slice.
    ///
    /// # Panics
    /// Panics if the channel is disabled.
    pub fn as_slice(&self) -> &[f32] {
        assert!(self.enabled, "Accessed disabled ChannelViewMut");
        self.data
    }

    /// Accesses the underlying data as a mutable slice.
    ///
    /// # Panics
    /// Panics if the channel is disabled.
    pub fn as_mut_slice(&mut self) -> &mut [f32] {
        assert!(self.enabled, "Accessed disabled ChannelViewMut");
        self.data
    }

    /// Consumes the view and returns the underlying mutable slice with lifetime 'a.
    ///
    /// # Panics
    /// Panics if the channel is disabled.
    pub fn into_mut_slice(self) -> &'a mut [f32] {
        assert!(self.enabled, "Accessed disabled ChannelViewMut");
        self.data
    }

    /// Performs pointwise addition with another immutable channel view (`self += other`).
    pub fn add_assign_view(&mut self, other: &ChannelView<'_>) {
        assert!(self.enabled && other.enabled);
        assert_eq!(self.data.len(), other.data.len());
        add_pointwise_in_place(self.data, other.data);
    }

    /// Performs pointwise addition with another mutable channel view (`self += other`).
    pub fn add_assign_mut_view(&mut self, other: &ChannelViewMut<'_>) {
        assert!(self.enabled && other.enabled);
        assert_eq!(self.data.len(), other.data.len());
        add_pointwise_in_place(self.data, other.data);
    }

    /// Performs pointwise subtraction with another channel view (`self -= other`).
    pub fn sub_assign_view(&mut self, other: &ChannelView<'_>) {
        assert!(self.enabled && other.enabled);
        assert_eq!(self.data.len(), other.data.len());
        subtract_pointwise_in_place(self.data, other.data);
    }

    /// Performs pointwise multiplication with another channel view (`self *= other`).
    pub fn mul_assign_view(&mut self, other: &ChannelView<'_>) {
        assert!(self.enabled && other.enabled);
        assert_eq!(self.data.len(), other.data.len());
        multiply_pointwise_in_place(self.data, other.data);
    }

    /// Performs pointwise multiplication with a raw float slice (`self *= other`).
    pub fn mul_assign_slice(&mut self, other: &[f32]) {
        assert!(self.enabled);
        assert_eq!(self.data.len(), other.len());
        multiply_pointwise_in_place(self.data, other);
    }
}

impl<'a> Index<usize> for ChannelViewMut<'a> {
    type Output = f32;
    fn index(&self, index: usize) -> &Self::Output {
        assert!(self.enabled, "Accessed disabled ChannelViewMut");
        &self.data[index]
    }
}

impl<'a> IndexMut<usize> for ChannelViewMut<'a> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        assert!(self.enabled, "Accessed disabled ChannelViewMut");
        &mut self.data[index]
    }
}
