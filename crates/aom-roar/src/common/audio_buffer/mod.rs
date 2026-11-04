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

//! Audio buffer representations
//!
//! This module provides the core types used for representing and passing multi-channel planar audio
//! data throughout the renderer.
//!
//! * AudioBuffer for owned memory internally.
//!     * Manages a single heap-allocated flat `Vec<f32>` with 64-byte alignment suitable for SIMD
//!       operations.
//!     * It partitions this flat vector into channels using [`ChannelSpec`] offsets.
//!     * Usage: Primarily used internally by the renderer for scratch memory (e.g., intermediate
//!       downmix blocks or sub-frame buffers) or int tests.  It is *not* passed directly to the
//!       core rendering methods.
//!
//! * Views into non-owned memory: `PlanarBufferRef` and `PlanarBufferMut`
//!     * Zero-allocation wrappers around standard Rust slices of slices: `&[&[f32]]` and `&mut
//!       [&mut [f32]]`.
//!     * They borrow memory owned by the caller (e.g., FFI buffer pointers, stack arrays, or slices
//!       of an `AudioBuffer`).
//!     * Validation Guarantees: Upon construction (via `::new()`), they validate that:
//!         1. The number of channels exactly matches the expected channel count.
//!         2. All channel slices have the exact same expected number of samples (represented by
//!            [`Samples`]).
//!     * Usage: These views are the primary types passed to renderers and processors.  Because
//!       validation is enforced at the boundary, all downstream renderers can skip runtime bounds
//!       and length checks, ensuring memory safety and enabling compiler loop vectorization.
//!
//! * Views of individual channels
//!     * `ChannelView` for immutable access
//!     * `ChannelViewMut` for mutable access

pub mod channel_view;
pub mod simd_utils;

pub use channel_view::{ChannelSpec, ChannelView, ChannelViewMut};

pub const MEMORY_ALIGNMENT_BYTES: usize = 64;

use crate::common::definitions::{OarError, Samples};
use simd_utils::{
    add_pointwise_in_place, find_next_aligned_array_index, subtract_pointwise_in_place,
};
use std::ops::{AddAssign, Index, IndexMut, SubAssign};

/// Multi-channel planar floating-point audio buffer.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AudioBuffer {
    num_frames: usize,
    data: Vec<f32>,
    channel_specs: Vec<ChannelSpec>,
}

/// Validated read-only view of planar audio channels.
/// Guarantees that the buffer has the expected channel count and all channels
/// have the exact expected sample length.
#[derive(Debug, Clone, Copy)]
pub struct PlanarBufferRef<'a, 'b> {
    channels: &'b [&'a [f32]],
}

/// Validated mutable view of planar audio channels.
/// Guarantees that the buffer has the expected channel count and all channels
/// have the exact expected sample length.
#[derive(Debug)]
pub struct PlanarBufferMut<'a, 'b> {
    channels: &'b mut [&'a mut [f32]],
}

impl AudioBuffer {
    /// Constructs a new `AudioBuffer` with given channels and frames counts.
    pub fn new(num_channels: usize, num_frames: usize) -> Self {
        let mut buffer = Self { num_frames, data: Vec::new(), channel_specs: Vec::new() };
        buffer.init_channel_views(num_channels);
        buffer
    }

    /// Allocates memory and initializes `ChannelSpec` specifications.
    fn init_channel_views(&mut self, num_channels: usize) {
        let stride = find_next_aligned_array_index(
            self.num_frames,
            std::mem::size_of::<f32>(),
            MEMORY_ALIGNMENT_BYTES,
        );
        let data_size = num_channels * stride;
        self.data.resize(data_size, 0.0);
        self.channel_specs.clear();
        self.channel_specs.reserve(num_channels);
        for i in 0..num_channels {
            self.channel_specs.push(ChannelSpec::new(i * stride, self.num_frames));
        }
    }

    /// Returns the number of audio channels.
    pub fn num_channels(&self) -> usize {
        self.channel_specs.len()
    }

    /// Returns the number of audio frames per channel.
    pub fn num_frames(&self) -> usize {
        self.num_frames
    }

    /// Returns the byte/word stride size between channels.
    pub fn get_channel_stride(&self) -> usize {
        find_next_aligned_array_index(
            self.num_frames,
            std::mem::size_of::<f32>(),
            MEMORY_ALIGNMENT_BYTES,
        )
    }

    /// Fills all channels with zero samples and sets them to enabled.
    pub fn clear(&mut self) {
        for spec in &mut self.channel_specs {
            spec.enabled = true;
        }
        self.data.fill(0.0);
    }

    /// Fills all channels with given value (doesn't change enabled status).
    pub fn fill(&mut self, val: f32) {
        self.data.fill(val);
    }

    /// Sets whether a specific channel should be enabled or disabled.
    pub fn set_channel_enabled(&mut self, channel: usize, enabled: bool) {
        self.channel_specs[channel].enabled = enabled;
    }

    /// Returns whether the specified channel is enabled.
    pub fn is_channel_enabled(&self, channel: usize) -> bool {
        self.channel_specs[channel].enabled
    }

    /// Returns an immutable channel view wrapper (`ChannelView`) for the specified channel index.
    pub fn channel(&self, channel: usize) -> ChannelView<'_> {
        let spec = &self.channel_specs[channel];
        ChannelView::new(&self.data[spec.offset..spec.offset + spec.size], spec.enabled)
    }

    /// Returns a mutable channel view wrapper (`ChannelViewMut`) for the specified channel index.
    pub fn channel_mut(&mut self, channel: usize) -> ChannelViewMut<'_> {
        let spec = &self.channel_specs[channel];
        let offset = spec.offset;
        let size = spec.size;
        let enabled = spec.enabled;
        ChannelViewMut::new(&mut self.data[offset..offset + size], enabled)
    }

    /// Copies sample data from a 2D float slice.
    pub fn copy_from_2d(&mut self, other: &[Vec<f32>]) {
        assert_eq!(other.len(), self.num_channels());
        for (channel_idx, src_channel) in other.iter().enumerate() {
            assert_eq!(src_channel.len(), self.num_frames);
            let spec = &self.channel_specs[channel_idx];
            assert!(spec.enabled);
            let dst = &mut self.data[spec.offset..spec.offset + spec.size];
            dst.copy_from_slice(src_channel);
        }
    }

    /// Converts this buffer to a 2D float vector representation.
    pub fn to_2d(&self) -> Vec<Vec<f32>> {
        (0..self.num_channels()).map(|i| self.channel(i).as_slice().to_vec()).collect()
    }

    /// Resizes the buffer to a new number of channels and frames.
    pub fn resize(&mut self, num_channels: usize, num_frames: usize) {
        if self.num_channels() != num_channels || self.num_frames != num_frames {
            self.num_frames = num_frames;
            self.init_channel_views(num_channels);
        }
    }

    /// Safely populates a destination slice of immutable slices with references to channels.
    pub fn as_slices<'a>(&'a self, dst: &mut [&'a [f32]]) {
        assert!(
            dst.len() >= self.num_channels(),
            "Destination slice is too small (expected at least {}, got {})",
            self.num_channels(),
            dst.len()
        );
        for (i, spec) in self.channel_specs.iter().enumerate() {
            dst[i] = &self.data[spec.offset..spec.offset + spec.size];
        }
    }

    /// Safely populates a destination slice of mutable slices with references to channels
    /// (allocation-free).
    pub fn as_slices_mut<'a>(&'a mut self, dst: &mut [&'a mut [f32]]) {
        assert!(
            dst.len() >= self.num_channels(),
            "Destination slice is too small (expected at least {}, got {})",
            self.num_channels(),
            dst.len()
        );
        let mut data_slice = &mut self.data[..];
        let mut last_offset = 0;
        for (i, spec) in self.channel_specs.iter().enumerate() {
            let offset = spec.offset - last_offset;
            let (_, rest) = data_slice.split_at_mut(offset);
            let (chan, rest2) = rest.split_at_mut(spec.size);
            dst[i] = chan;
            data_slice = rest2;
            last_offset = spec.offset + spec.size;
        }
    }
}

impl Index<usize> for AudioBuffer {
    type Output = [f32];
    fn index(&self, index: usize) -> &Self::Output {
        self.channel(index).data
    }
}

impl IndexMut<usize> for AudioBuffer {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.channel_mut(index).data
    }
}

impl AddAssign<&AudioBuffer> for AudioBuffer {
    fn add_assign(&mut self, other: &AudioBuffer) {
        assert_eq!(other.num_channels(), self.num_channels());
        assert_eq!(other.num_frames(), self.num_frames());
        for i in 0..self.num_channels() {
            let src = other.channel(i);
            assert!(src.enabled);
            let dst = self.channel_mut(i);
            assert!(dst.enabled);
            add_pointwise_in_place(dst.data, src.data);
        }
    }
}

impl SubAssign<&AudioBuffer> for AudioBuffer {
    fn sub_assign(&mut self, other: &AudioBuffer) {
        assert_eq!(other.num_channels(), self.num_channels());
        assert_eq!(other.num_frames(), self.num_frames());
        for i in 0..self.num_channels() {
            let src = other.channel(i);
            assert!(src.enabled);
            let dst = self.channel_mut(i);
            assert!(dst.enabled);
            subtract_pointwise_in_place(dst.data, src.data);
        }
    }
}

impl<'a, 'b> PlanarBufferRef<'a, 'b> {
    /// Creates a new validated view, checking that shape matches expectations.
    /// Returns `OarError::InvalidParameter` if validation fails.
    pub fn new(
        channels: &'b [&'a [f32]],
        expected_channels: usize,
        expected_samples_per_channel: Samples,
    ) -> Result<Self, OarError> {
        let expected_samples: usize = expected_samples_per_channel.into();
        if channels.len() != expected_channels {
            return Err(OarError::InvalidParameter);
        }
        for ch in channels {
            if ch.len() != expected_samples {
                return Err(OarError::InvalidParameter);
            }
        }
        Ok(Self { channels })
    }

    /// Returns the number of channels.
    pub fn num_channels(&self) -> usize {
        self.channels.len()
    }

    /// Returns the number of samples per channel.
    pub fn num_samples(&self) -> usize {
        self.channels.first().map(|ch| ch.len()).unwrap_or(0)
    }

    /// Accesses the data slice of the specified channel index.
    pub fn channel(&self, idx: usize) -> &'a [f32] {
        self.channels[idx]
    }

    /// Safely populates a destination slice of immutable slices with references to channels
    /// (allocation-free).
    pub fn as_slices<'c>(&'c self, dst: &mut [&'c [f32]]) {
        assert!(
            dst.len() >= self.num_channels(),
            "Destination slice is too small (expected at least {}, got {})",
            self.num_channels(),
            dst.len()
        );
        for (i, dst_channel) in dst.iter_mut().enumerate().take(self.num_channels()) {
            *dst_channel = self.channel(i);
        }
    }
}

impl<'a, 'b> PlanarBufferMut<'a, 'b> {
    /// Creates a new validated mutable view, checking that shape matches expectations.
    /// Returns `OarError::InvalidParameter` if validation fails.
    pub fn new(
        channels: &'b mut [&'a mut [f32]],
        expected_channels: usize,
        expected_samples_per_channel: Samples,
    ) -> Result<Self, OarError> {
        let expected_samples: usize = expected_samples_per_channel.into();
        if channels.len() != expected_channels {
            return Err(OarError::InvalidParameter);
        }
        for ch in channels.iter() {
            if ch.len() != expected_samples {
                return Err(OarError::InvalidParameter);
            }
        }
        Ok(Self { channels })
    }

    /// Returns the number of channels.
    pub fn num_channels(&self) -> usize {
        self.channels.len()
    }

    /// Returns the number of samples per channel.
    pub fn num_samples(&self) -> usize {
        self.channels.first().map(|ch| ch.len()).unwrap_or(0)
    }

    /// Accesses the mutable data slice of the specified channel index.
    pub fn channel_mut(&mut self, idx: usize) -> &mut [f32] {
        self.channels[idx]
    }

    /// Fills all channels with a constant value.
    pub fn fill(&mut self, val: f32) {
        for ch in self.channels.iter_mut() {
            ch.fill(val);
        }
    }

    /// Safely populates a destination slice of mutable slices with references to channels
    /// (allocation-free).
    pub fn as_slices_mut<'c>(&'c mut self, dst: &mut [&'c mut [f32]]) {
        let num_chans = self.num_channels();
        assert!(
            dst.len() >= num_chans,
            "Destination slice is too small (expected at least {}, got {})",
            num_chans,
            dst.len()
        );
        let mut rem = &mut *self.channels;
        for slot in dst.iter_mut().take(num_chans) {
            let (first, rest) = rem.split_first_mut().unwrap();
            *slot = &mut **first;
            rem = rest;
        }
    }
}
