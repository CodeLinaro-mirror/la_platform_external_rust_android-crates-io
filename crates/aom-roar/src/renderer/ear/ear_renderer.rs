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

//! A spatially-oriented headphone/binaural renderer (`EAR` library port).
//!
//! Direct Rust port of `liboar/src/renderer/ear/ear.c` and `ear.h`.
//! Responsible for applying exact multi-channel to multi-channel and
//! High-Order Ambisonics to multi-channel rendering to channel-based
//! and scene-based audio elements according to target loudspeaker layouts.

use crate::common::definitions::{
    AudioElementConfig, OarError, OutputLayout, PlanarBufferMut, PlanarBufferRef,
    MAX_OUTPUT_CHANNEL_COUNT,
};
use crate::renderer::audio_renderer_api::AudioRenderer;
use crate::renderer::ear::ambisonic_to_channel_renderer::{
    element_renderer_get_ambi_to_chan_matrix, element_renderer_render_ambi_to_chan,
    AmbisonicToChannelRenderer,
};
use crate::renderer::ear::channel_to_channel_renderer::{
    element_renderer_get_chan_to_chan_custom_matrix, element_renderer_get_chan_to_chan_matrix,
    element_renderer_render_chan_to_chan, element_renderer_render_chan_to_chan_custom,
    ChannelToChannelRenderer,
};

/// A spatially-oriented headphone/binaural renderer.
///
/// `EarRenderer` applies target-layout conversion matrices
/// to spatialized planar audio outputs, matching `ear_renderer_t` of `ear.c`.
#[derive(Debug, Clone)]
pub struct EarRenderer {
    /// Target output layout for this renderer instance.
    pub target_layout: OutputLayout,
    /// Active rendering matrix mode configured for the current audio element.
    active_mode: Option<EarRenderingMode>,
}

/// Enumeration of configured internal rendering algorithms and tables.
#[derive(Debug, Clone)]
pub enum EarRenderingMode {
    /// Predefined channel-to-channel matrix (`sp_type == 0`).
    ChannelPredefined(ChannelToChannelRenderer),
    /// Custom subset layout channel matrix (`sp_type == 1`).
    ChannelCustom {
        /// Superset-layout matrix structure with `m` set to the subset channel count.
        matrix: ChannelToChannelRenderer,
        /// Channel indices of the subset layout within its superset layout.
        channels_map: Vec<usize>,
    },
    /// High-Order Ambisonics to multichannel matrix.
    Scene(AmbisonicToChannelRenderer),
}

impl EarRenderer {
    /// Creates a new `EarRenderer` configured with default Stereo output.
    pub fn new() -> Self {
        Self::with_output_layout(OutputLayout::Stereo)
    }

    /// Creates a new `EarRenderer` with specified target output layout.
    pub fn with_output_layout(target_layout: OutputLayout) -> Self {
        Self { target_layout, active_mode: None }
    }
}

impl Default for EarRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioRenderer for EarRenderer {
    /// Configures an audio element within the renderer (`_open` equivalent).
    fn add_element(&mut self, _id: u32, config: &AudioElementConfig) -> Result<(), OarError> {
        self.active_mode = None;
        let out_system = self.target_layout;

        match *config {
            AudioElementConfig::ChannelBased(cb) => {
                if cb.layout.superset_layout().is_some() {
                    let (matrix, channels_map) =
                        element_renderer_get_chan_to_chan_custom_matrix(cb.layout, out_system)?;
                    self.active_mode =
                        Some(EarRenderingMode::ChannelCustom { matrix, channels_map });
                } else {
                    let matrix = element_renderer_get_chan_to_chan_matrix(cb.layout, out_system)?;
                    self.active_mode = Some(EarRenderingMode::ChannelPredefined(matrix));
                }
                Ok(())
            }
            AudioElementConfig::SceneBased(sb) => {
                let ambi_to_chan = element_renderer_get_ambi_to_chan_matrix(sb.order, out_system)?;
                self.active_mode = Some(EarRenderingMode::Scene(ambi_to_chan));
                Ok(())
            }
            AudioElementConfig::ObjectBased(_) => Err(OarError::NotSupported),
        }
    }

    /// Processes input planar buffers (`_render` equivalent) and writes spatialized outputs.
    ///
    /// Executes allocation-free on the real-time audio thread by packing slice
    /// references into a fixed stack array (`[MAX_OUTPUT_CHANNEL_COUNT]`).
    fn render(
        &mut self,
        inputs: PlanarBufferRef<'_, '_>,
        output: &mut PlanarBufferMut<'_, '_>,
    ) -> Result<(), OarError> {
        if output.num_samples() != inputs.num_samples() {
            return Err(OarError::InvalidParameter);
        }
        let (expected_in_channels, expected_out_channels) = match &self.active_mode {
            Some(EarRenderingMode::ChannelPredefined(chan_to_chan)) => {
                (chan_to_chan.m, chan_to_chan.n)
            }
            Some(EarRenderingMode::ChannelCustom { matrix, .. }) => (matrix.m, matrix.n),
            Some(EarRenderingMode::Scene(ambi_to_chan)) => (ambi_to_chan.m, ambi_to_chan.channels),
            None => return Err(OarError::NotSupported),
        };
        if inputs.num_channels() != expected_in_channels
            || output.num_channels() != expected_out_channels
            || inputs.num_channels() > MAX_OUTPUT_CHANNEL_COUNT
            || output.num_channels() > MAX_OUTPUT_CHANNEL_COUNT
        {
            return Err(OarError::InvalidParameter);
        }

        match &self.active_mode {
            Some(EarRenderingMode::ChannelPredefined(chan_to_chan)) => {
                element_renderer_render_chan_to_chan(chan_to_chan, inputs, output)
            }
            Some(EarRenderingMode::ChannelCustom { matrix, channels_map }) => {
                element_renderer_render_chan_to_chan_custom(matrix, inputs, output, channels_map)
            }
            Some(EarRenderingMode::Scene(ambi_to_chan)) => {
                element_renderer_render_ambi_to_chan(ambi_to_chan, inputs, output)
            }
            None => Err(OarError::NotSupported),
        }
    }
}

// ===== Tests =====

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::definitions::{
        ChannelBasedConfig, HighOrderAmbisonics, InputLayout, ObjectBasedConfig, Samples,
        SceneBasedConfig,
    };
    use googletest::prelude::*;

    /// Helper to run a function with PlanarBufferRef and PlanarBufferMut with the given buffers.
    fn with_planar_buffers<F, R>(
        in_buf: &[f32],
        in_channels: usize,
        out_buf: &mut [f32],
        out_channels: usize,
        num_samples: usize,
        f: F,
    ) -> R
    where
        F: FnOnce(PlanarBufferRef<'_, '_>, &mut PlanarBufferMut<'_, '_>) -> R,
    {
        assert_eq!(in_buf.len(), in_channels * num_samples);
        assert_eq!(out_buf.len(), out_channels * num_samples);

        let mut in_chunks = in_buf.chunks_exact(num_samples);
        let in_slices: Vec<&[f32]> = (0..in_channels).map(|_| in_chunks.next().unwrap()).collect();
        let in_planar = PlanarBufferRef::new(
            &in_slices,
            in_channels,
            Samples::new(num_samples as u32).unwrap(),
        )
        .unwrap();

        let mut out_chunks = out_buf.chunks_exact_mut(num_samples);
        let mut out_slices: Vec<&mut [f32]> =
            (0..out_channels).map(|_| out_chunks.next().unwrap()).collect();
        let mut out_planar = PlanarBufferMut::new(
            &mut out_slices,
            out_channels,
            Samples::new(num_samples as u32).unwrap(),
        )
        .unwrap();

        f(in_planar, &mut out_planar)
    }

    #[gtest]
    fn ear_renderer_add_element_channel_predefined_sets_active_mode_and_renders() {
        let mut rdr = EarRenderer::new();
        let config = AudioElementConfig::ChannelBased(ChannelBasedConfig {
            layout: InputLayout::Mono,
            downmix_info: None,
            rendering_config: None,
        });
        let in_buf = [10.0f32];
        let mut out_buf = [0.0f32; 2];
        let add_result = rdr.add_element(10, &config);
        let render_result =
            with_planar_buffers(&in_buf, 1, &mut out_buf, 2, 1, |inputs, outputs| {
                rdr.render(inputs, outputs)
            });

        assert_ok!(add_result);
        assert_ok!(render_result);
        expect_that!(out_buf[0], near(7.071068f32, 1e-4));
        expect_that!(out_buf[1], near(7.071068f32, 1e-4));
    }

    #[gtest]
    fn ear_renderer_add_element_scene_based_sets_active_mode_and_renders() {
        let mut rdr = EarRenderer::with_output_layout(OutputLayout::Layout51);
        let config = AudioElementConfig::SceneBased(SceneBasedConfig {
            order: HighOrderAmbisonics::Zoa,
            rendering_config: None,
        });
        let in_buf = [2.0f32];
        let mut out_buf = [0.0f32; 6];
        let add_result = rdr.add_element(20, &config);
        let render_result =
            with_planar_buffers(&in_buf, 1, &mut out_buf, 6, 1, |inputs, outputs| {
                rdr.render(inputs, outputs)
            });

        assert_ok!(add_result);
        assert_ok!(render_result);
        expect_that!(out_buf[0], near(0.6526711f32, 1e-4));
        expect_that!(out_buf[1], near(0.6526767f32, 1e-4));
        expect_that!(out_buf[2], near(0.43026306f32, 1e-4));
        expect_that!(out_buf[3], eq(0.0f32));
    }

    #[gtest]
    fn ear_renderer_add_element_unsupported_layout_returns_not_supported() {
        let mut rdr = EarRenderer::new();
        let config = AudioElementConfig::ObjectBased(ObjectBasedConfig::new(1, None).unwrap());

        let result = rdr.add_element(30, &config);

        expect_that!(result, eq(Err(OarError::NotSupported)));
    }

    #[gtest]
    fn ear_renderer_add_element_channel_custom_sets_active_mode_and_renders() {
        let mut rdr = EarRenderer::with_output_layout(OutputLayout::Layout51);
        let config = AudioElementConfig::ChannelBased(ChannelBasedConfig {
            layout: InputLayout::StereoS,
            downmix_info: None,
            rendering_config: None,
        });
        let in_buf = [1.0f32, 2.0f32];
        let mut out_buf = [0.0f32; 6];
        let add_result = rdr.add_element(40, &config);
        let render_result =
            with_planar_buffers(&in_buf, 2, &mut out_buf, 6, 1, |inputs, outputs| {
                rdr.render(inputs, outputs)
            });

        assert_ok!(add_result);
        assert_ok!(render_result);
        expect_that!(out_buf[0], eq(0.0f32));
        expect_that!(out_buf[1], eq(0.0f32));
        expect_that!(out_buf[2], eq(0.0f32));
        expect_that!(out_buf[3], eq(0.0f32));
        expect_that!(out_buf[4], eq(1.0f32));
        expect_that!(out_buf[5], eq(2.0f32));
    }
}
