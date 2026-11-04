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

//! API definitions for polymorphic audio renderers.
//!
//! This module contains the `AudioRenderer` trait which defines the unified
//! interface for rendering audio elements using different rendering backends.

use crate::common::definitions::{
    AudioElementConfig, DownmixMode, Gain, OarError, ObjectPosition, PlanarBufferMut,
    PlanarBufferRef, Quaternion, Samples,
};

/// A trait representing a polymorphic audio renderer backend.
///
/// An `AudioRenderer` handles configuring audio elements, updating their
/// metadata, and rendering the spatialized output from input planar audio buffers.
///
/// Implementations must be thread-safe (`Send + Sync`).
pub trait AudioRenderer: Send + Sync {
    /// Configures an audio element within this specific renderer backend.
    ///
    /// # Parameters
    ///
    /// * `id`: Unique identifier for the audio element.
    /// * `config`: Configuration details for the audio element.
    ///
    /// # Errors
    ///
    /// Returns an `OarError` if configuration fails, e.g., if the backend
    /// cannot support the specified layout or if the ID is already configured.
    fn add_element(&mut self, id: u32, config: &AudioElementConfig) -> Result<(), OarError>;

    /// Removes an audio element from a multi-element renderer backend.
    ///
    /// Only multi-element backends (`AudioElementsRenderer` and `ObrAudioRendererAdapter`, which
    /// multiplex all binaural elements in an audio group into a shared OBR instance) implement
    /// in-place element removal. Single-element loudspeaker backends (`EarRenderer`,
    /// `DownmixRenderer`, `ObjectRenderer`) are instantiated per element and dropped as a unit when
    /// their element is removed, so they use the default `Err(OarError::NotSupported)`
    /// implementation.
    ///
    /// # Parameters
    ///
    /// * `id`: Unique identifier for the audio element to remove.
    fn remove_element(&mut self, _id: u32) -> Result<(), OarError> {
        Err(OarError::NotSupported)
    }

    /// Updates dynamic gain parameters.
    fn update_element_gain(
        &mut self,
        _id: u32,
        _gain_id: u32,
        _gain: &Gain,
        _duration: Samples,
    ) -> Result<(), OarError> {
        Err(OarError::NotSupported)
    }

    /// Updates dynamic object positions.
    fn update_element_positions(
        &mut self,
        _id: u32,
        _positions: &ObjectPosition,
        _duration: Samples,
    ) -> Result<(), OarError> {
        Err(OarError::NotSupported)
    }

    /// Updates the dynamic IAMF downmix mode for an audio element.
    ///
    /// Only backends with an IAMF downmix stage (`DownmixRenderer`) consume this update. Other
    /// backends return the default `Err(OarError::NotSupported)`, which `RoarRenderer` accepts and
    /// ignores as an advisory no-op (`Ok(())`).
    ///
    /// # Parameters
    ///
    /// * `id`: Unique identifier for the audio element.
    /// * `mode`: Target IAMF downmix mode.
    /// * `duration`: Duration in samples for which `mode` applies, or `None` to apply `mode`
    ///   indefinitely until superseded by a subsequent update.
    fn update_element_downmix_mode(
        &mut self,
        _id: u32,
        _mode: DownmixMode,
        _duration: Option<Samples>,
    ) -> Result<(), OarError> {
        Err(OarError::NotSupported)
    }
    /// Updates dynamic group gain.
    fn update_group_gain(
        &mut self,
        _gid: u32,
        _gain: &Gain,
        _duration: Samples,
    ) -> Result<(), OarError> {
        Err(OarError::NotSupported)
    }

    /// Processes input planar buffers and writes spatialized outputs into destination buffers.
    ///
    /// # Parameters
    ///
    /// * `inputs`: A slice of planar input buffers (one slice of `f32` per input channel).
    /// * `output`: The destination output buffer where the spatialized output will be written.
    ///
    /// # Errors
    ///
    /// Returns an `OarError` if rendering fails.
    fn render(
        &mut self,
        inputs: PlanarBufferRef<'_, '_>,
        output: &mut PlanarBufferMut<'_, '_>,
    ) -> Result<(), OarError>;

    /// Enables or disables head tracking in the renderer.
    ///
    /// Default implementation returns `Err(OarError::NotSupported)`.
    fn enable_head_tracking(&mut self, _enable: bool) -> Result<(), OarError> {
        Err(OarError::NotSupported)
    }

    /// Sets head rotation quaternion.
    ///
    /// Default implementation returns `Err(OarError::NotSupported)`.
    fn set_head_rotation(&mut self, _rotation: Quaternion) -> Result<(), OarError> {
        Err(OarError::NotSupported)
    }

    /// Sets the metadata processing unit size in samples.
    fn set_metadata_unit_to_process(&mut self, _samples: Samples) -> Result<(), OarError> {
        Ok(())
    }
}
