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

//! Downmix renderer implementation.
//!
//! This module implements the `DownmixRenderer` which supports all standard IAMF
//! downmix configurations such as 5.1 to Stereo, 7.1 to 5.1, Mono downmix, and all
//! height/surround layout transitions adhering to exact IAMF demixing modes and weight tables.

use super::ia_channel::IAChannel;
use crate::common::definitions::ChannelCount;
use crate::common::definitions::{
    AudioElementConfig, Coeff, DownmixMode, InputLayout, MixFactors, OarError, OutputLayout,
    PlanarBufferMut, PlanarBufferRef, Samples, WeightIndex, WeightIndexShift,
    METADATA_QUEUE_CAPACITY,
};
use crate::renderer::audio_renderer_api::AudioRenderer;

const WIDX2W_TABLE: [f32; 11] =
    [0.0, 0.0179, 0.0391, 0.0658, 0.1038, 0.25, 0.3962, 0.4342, 0.4609, 0.4821, 0.5];

fn get_w(w_idx: Option<WeightIndex>) -> f32 {
    match w_idx {
        None => WIDX2W_TABLE[0],
        Some(w) => WIDX2W_TABLE[w.value() as usize],
    }
}

/// Computes the next weight index based on the chosen mode's shift direction.
///
/// If there is no previous weight index (i.e. `None` representing default uninitialized state),
/// it defaults to the minimum index (`WeightIndex::MIN`).
/// Otherwise, it increments or decrements the index (clamped within `0..=10`) based on
/// whether `weight_index_shift` is positive (shifting up) or negative (shifting down).
fn calculate_next_weight_index(
    weight_index_shift: WeightIndexShift,
    w_idx_prev: Option<WeightIndex>,
) -> WeightIndex {
    match w_idx_prev {
        None => WeightIndex::MIN,
        Some(w) => match weight_index_shift {
            WeightIndexShift::Positive => w.increment(),
            WeightIndexShift::Negative => w.decrement(),
        },
    }
}

fn get_dep_list(ch: IAChannel) -> &'static [(IAChannel, Coeff)] {
    match ch {
        IAChannel::Mono => {
            &[(IAChannel::R2, Coeff::Fixed(0.5)), (IAChannel::L2, Coeff::Fixed(0.5))]
        }
        IAChannel::L2 => &[(IAChannel::L3, Coeff::Fixed(1.0)), (IAChannel::C, Coeff::Fixed(0.707))],
        IAChannel::R2 => &[(IAChannel::R3, Coeff::Fixed(1.0)), (IAChannel::C, Coeff::Fixed(0.707))],
        IAChannel::Tl => &[(IAChannel::Hl, Coeff::Fixed(1.0)), (IAChannel::Sl5, Coeff::DeltaW)],
        IAChannel::Tr => &[(IAChannel::Hr, Coeff::Fixed(1.0)), (IAChannel::Sr5, Coeff::DeltaW)],
        IAChannel::L3 => &[(IAChannel::L7, Coeff::Fixed(1.0)), (IAChannel::Sl5, Coeff::Delta)],
        IAChannel::R3 => &[(IAChannel::R7, Coeff::Fixed(1.0)), (IAChannel::Sr5, Coeff::Delta)],
        IAChannel::Sl5 => &[(IAChannel::Sl7, Coeff::Alpha), (IAChannel::Bl7, Coeff::Beta)],
        IAChannel::Sr5 => &[(IAChannel::Sr7, Coeff::Alpha), (IAChannel::Br7, Coeff::Beta)],
        IAChannel::Hl => &[(IAChannel::Hfl, Coeff::Fixed(1.0)), (IAChannel::Hbl, Coeff::Gamma)],
        IAChannel::Hr => &[(IAChannel::Hfr, Coeff::Fixed(1.0)), (IAChannel::Hbr, Coeff::Gamma)],
        _ => &[],
    }
}

fn get_channel_weight(
    target_ch: IAChannel,
    input_chs: &[IAChannel],
    target_in_idx: usize,
    mode: DownmixMode,
    w_val: f32,
) -> f32 {
    if let Some(pos) = input_chs.iter().position(|&c| c == target_ch) {
        return if pos == target_in_idx { 1.0 } else { 0.0 };
    }

    let mut sum = 0.0;
    let MixFactors { alpha, beta, gamma, delta, .. } = mode.mix_factors();
    for &(dep_ch, coeff_type) in get_dep_list(target_ch) {
        let factor = match coeff_type {
            Coeff::Fixed(v) => v,
            Coeff::Alpha => alpha,
            Coeff::Beta => beta,
            Coeff::Gamma => gamma,
            Coeff::Delta => delta,
            Coeff::DeltaW => delta * w_val,
        };
        sum += factor * get_channel_weight(dep_ch, input_chs, target_in_idx, mode, w_val);
    }
    sum
}

/// A polymorphic audio renderer that downmixes multi-channel audio to target loudspeaker layouts.
///
/// Downmixing is performed allocation-free by pre-computing coefficients.  Pre-allocates and
/// dynamically updates all matrix weights (`weights`) during construction (`new`) and metadata
/// updates (`update_metadata`) to adhere to a zero-allocation policy inside the `render` loop.
#[derive(Debug, Clone)]
pub struct DownmixRenderer {
    input_layout: InputLayout,
    output_layout: OutputLayout,
    input_channels: ChannelCount,
    output_channels: ChannelCount,
    weights: Vec<f32>,
    supported: bool,
    mode: DownmixMode,
    weight_index: Option<WeightIndex>,
    metadata_duration: Option<u32>,
    metadata_queue: std::collections::VecDeque<(DownmixMode, Option<Samples>)>,
}

impl DownmixRenderer {
    /// Creates a new `DownmixRenderer` with specified input and output layouts (`DMRenderer_open`
    /// equivalent).
    ///
    /// Precomputes and pre-allocates the downmix coefficient matrix using default IAMF mode 0
    /// ($w_{\text{idx}} = -1$).
    ///
    /// # Parameters
    ///
    /// * `input_layout`: The source layout config of input audio.
    /// * `output_layout`: The target layout config of output audio.
    pub fn new(input_layout: InputLayout, output_layout: OutputLayout) -> Self {
        let input_channels = input_layout.channels();
        let output_channels = output_layout.channels();

        let weights = vec![0.0; input_channels.get() * output_channels.get()];
        let supported = input_layout.is_valid_downmix_to(output_layout);

        let mut rdr = Self {
            input_layout,
            output_layout,
            input_channels,
            output_channels,
            weights,
            supported,
            mode: DownmixMode::Mode1NegOffset,
            weight_index: None,
            metadata_duration: Some(0),
            metadata_queue: std::collections::VecDeque::with_capacity(METADATA_QUEUE_CAPACITY),
        };

        rdr.compute_weights();

        rdr
    }

    /// Sets the demixing mode and weight index (`DMRenderer_set_mode_weight` equivalent).
    ///
    /// # Parameters
    ///
    /// * `mode`: The target downmix mode.
    /// * `w_idx`: Optional weight index. If `None`, the next weight index is calculated
    ///   automatically based on the mode's shift direction.
    pub fn set_mode_weight(
        &mut self,
        mode: DownmixMode,
        w_idx: Option<WeightIndex>,
    ) -> Result<(), OarError> {
        if !self.supported {
            return Err(OarError::NotSupported);
        }

        self.mode = mode;
        let factors = mode.mix_factors();

        self.weight_index = match w_idx {
            Some(w) => Some(w),
            None => {
                Some(calculate_next_weight_index(factors.weight_index_shift, self.weight_index))
            }
        };

        if OutputLayout::try_from(self.input_layout).ok() != Some(self.output_layout) {
            self.compute_weights();
        }
        Ok(())
    }

    fn compute_weights(&mut self) {
        if !self.supported {
            return;
        }

        if OutputLayout::try_from(self.input_layout).ok() == Some(self.output_layout) {
            // Identity matrix pass-through
            self.weights.fill(0.0);
            for i in 0..self.input_channels.get() {
                self.weights[i * self.input_channels.get() + i] = 1.0;
            }
            return;
        }

        let Some(in_chs) = self.input_layout.downmix_channels() else {
            return;
        };
        let Some(out_chs) = self.output_layout.downmix_channels() else {
            return;
        };

        let w_val = get_w(self.weight_index);

        for (out_idx, &target_ch) in out_chs.iter().enumerate() {
            for (in_idx, _) in in_chs.iter().enumerate() {
                let weight = get_channel_weight(target_ch, in_chs, in_idx, self.mode, w_val);
                self.weights[out_idx * self.input_channels.get() + in_idx] = weight;
            }
        }
    }
}

impl AudioRenderer for DownmixRenderer {
    /// Configures an audio element within this specific renderer backend (`_open` equivalent).
    fn add_element(&mut self, _id: u32, config: &AudioElementConfig) -> Result<(), OarError> {
        if !self.supported {
            return Err(OarError::NotSupported);
        }
        match config {
            AudioElementConfig::ChannelBased(cfg) => {
                if cfg.layout == self.input_layout {
                    Ok(())
                } else {
                    Err(OarError::InvalidParameter)
                }
            }
            _ => Err(OarError::NotSupported),
        }
    }

    /// Updates dynamic metadata parameters (such as IAMF downmix mode changes).
    fn update_element_downmix_mode(
        &mut self,
        _id: u32,
        mode: DownmixMode,
        duration: Option<Samples>,
    ) -> Result<(), OarError> {
        if !self.supported {
            return Err(OarError::NotSupported);
        }
        self.metadata_queue.push_back((mode, duration));
        Ok(())
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
    /// Returns an `OarError::InvalidParameter` if parameter shapes or lengths are mismatched,
    /// or if layout configuration is not supported.
    fn render(
        &mut self,
        inputs: PlanarBufferRef<'_, '_>,
        output: &mut PlanarBufferMut<'_, '_>,
    ) -> Result<(), OarError> {
        if !self.supported {
            return Err(OarError::NotSupported);
        }
        if inputs.num_channels() != self.input_channels.get()
            || output.num_channels() != self.output_channels.get()
            || output.num_samples() != inputs.num_samples()
        {
            return Err(OarError::InvalidParameter);
        }
        let samples_per_channel = inputs.num_samples();
        if samples_per_channel == 0 {
            return Err(OarError::InvalidParameter);
        }

        output.fill(0.0);
        let mut offset_into_buffers = 0usize;
        while offset_into_buffers < samples_per_channel {
            // Default: process all remaining samples in the block.
            let mut chunk_samples = samples_per_channel - offset_into_buffers;

            // Pop the next metadata update if either:
            // - The current finite metadata has expired (duration is Some(0)).
            // - The current metadata is infinite (duration is None) and there is a new update.
            let need_pop = match self.metadata_duration {
                Some(0) => true,
                None => !self.metadata_queue.is_empty(),
                _ => false,
            };

            if need_pop && let Some((mode, dur)) = self.metadata_queue.pop_front() {
                self.set_mode_weight(mode, None)?;
                self.metadata_duration = dur.map(|s| s.value());
            }

            // If the current metadata has a finite remaining duration, limit the chunk size
            // to ensure we apply the next metadata update at the correct sample boundary.
            if let Some(rem_dur) = self.metadata_duration
                && rem_dur > 0
            {
                chunk_samples = chunk_samples.min(rem_dur as usize);
            }
            // Perform the downmixing matrix multiplication safely across [offset..offset +
            // chunk_samples] without allocating any memory (`#alloc` safe).
            for (out_idx, weights_row) in
                self.weights.chunks_exact(self.input_channels.get()).enumerate()
            {
                let out_slice = &mut output.channel_mut(out_idx)
                    [offset_into_buffers..offset_into_buffers + chunk_samples];
                for (in_idx, &weight) in weights_row.iter().enumerate() {
                    if weight != 0.0 {
                        let src_slice = &inputs.channel(in_idx)
                            [offset_into_buffers..offset_into_buffers + chunk_samples];
                        out_slice.iter_mut().zip(src_slice.iter()).for_each(
                            |(out_val, &in_val)| {
                                *out_val += in_val * weight;
                            },
                        );
                    }
                }
            }
            // Decrement remaining duration for finite metadata updates.
            if let Some(ref mut rem_dur) = self.metadata_duration
                && *rem_dur > 0
            {
                *rem_dur -= chunk_samples as u32;
            }
            offset_into_buffers += chunk_samples;
        }

        Ok(())
    }
}

// ===== Tests =====

#[cfg(test)]
mod test {

    use super::*;
    use crate::common::definitions::{AudioElementConfig, ChannelBasedConfig};
    use crate::renderer::audio_renderer_api::AudioRenderer;
    use googletest::prelude::*;

    fn make_planar_mut<'a, 'b>(
        flat: &'a mut [f32],
        channels: usize,
        samples: usize,
        slices: &'b mut [&'a mut [f32]],
    ) -> PlanarBufferMut<'a, 'b> {
        let mut chunks = flat.chunks_exact_mut(samples);
        for slice in slices.iter_mut().take(channels) {
            *slice = chunks.next().unwrap();
        }
        PlanarBufferMut::new(slices, channels, Samples::new(samples as u32).unwrap()).unwrap()
    }

    fn make_planar_ref<'a, 'b>(inputs: &'b [&'a [f32]], samples: usize) -> PlanarBufferRef<'a, 'b> {
        PlanarBufferRef::new(inputs, inputs.len(), Samples::new(samples as u32).unwrap()).unwrap()
    }

    #[gtest]
    fn new_initializes_correct_dimensions() {
        let renderer = DownmixRenderer::new(InputLayout::Layout51, OutputLayout::Stereo);

        expect_that!(renderer.input_layout, eq(InputLayout::Layout51));
        expect_that!(renderer.output_layout, eq(OutputLayout::Stereo));
        expect_that!(renderer.input_channels.get(), eq(6));
        expect_that!(renderer.output_channels.get(), eq(2));
    }

    #[gtest]
    fn add_element_succeeds_for_matching_layout() {
        let mut renderer = DownmixRenderer::new(InputLayout::Layout51, OutputLayout::Stereo);
        let config = AudioElementConfig::ChannelBased(ChannelBasedConfig {
            layout: InputLayout::Layout51,
            downmix_info: None,
            rendering_config: None,
        });

        let result = renderer.add_element(1, &config);

        expect_ok!(result);
    }

    #[gtest]
    fn add_element_fails_for_mismatched_layout() {
        let mut renderer = DownmixRenderer::new(InputLayout::Layout51, OutputLayout::Stereo);
        let wrong_config = AudioElementConfig::ChannelBased(ChannelBasedConfig {
            layout: InputLayout::Layout71,
            downmix_info: None,
            rendering_config: None,
        });

        let result = renderer.add_element(2, &wrong_config);

        expect_true!(result.is_err());
    }

    #[gtest]
    fn update_metadata_succeeds() {
        let mut renderer = DownmixRenderer::new(InputLayout::Layout51, OutputLayout::Stereo);
        let result = renderer.update_element_downmix_mode(1, DownmixMode::Mode1NegOffset, None);

        expect_ok!(result);
    }

    #[gtest]
    fn downmix_51_to_stereo_calculates_correct_gains() {
        let mut renderer = DownmixRenderer::new(InputLayout::Layout51, OutputLayout::Stereo);
        let samples = 100;
        let mut inputs = vec![vec![0.0f32; samples]; 6];
        inputs[0].fill(1.0); // L
        inputs[2].fill(1.0); // C
        inputs[3].fill(0.5); // LFE (should be ignored in standard stereo downmix)
        inputs[4].fill(1.0); // Ls
        let mut output = vec![0.0f32; 2 * samples];
        let mut out_slices: [&mut [f32]; 2] = std::array::from_fn(|_| &mut [] as &mut [f32]);
        let mut out_buf = make_planar_mut(&mut output, 2, samples, &mut out_slices);
        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let validated_inputs = make_planar_ref(&input_refs, samples);

        let result = renderer.render(validated_inputs, &mut out_buf);

        expect_ok!(result);
        // L_out = L + 0.707 * C + 0.707 * Ls = 1.0 + 0.707 + 0.707 = 2.414
        // R_out = R + 0.707 * C + 0.707 * Rs = 0.0 + 0.707 + 0.0 = 0.707
        for s in 0..samples {
            let l_out = output[s];
            let r_out = output[samples + s];
            expect_that!(l_out, near(2.414f32, 1e-4));
            expect_that!(r_out, near(0.707f32, 1e-4));
        }
    }

    #[gtest]
    fn downmix_71_to_stereo_calculates_correct_gains() {
        let mut renderer = DownmixRenderer::new(InputLayout::Layout71, OutputLayout::Stereo);
        let samples = 50;
        let mut inputs = vec![vec![0.0f32; samples]; 8];
        inputs[0].fill(1.0); // L
        inputs[2].fill(1.0); // C
        inputs[4].fill(1.0); // Lss
        inputs[6].fill(1.0); // Lrs
        let mut output = vec![0.0f32; 2 * samples];
        let mut out_slices: [&mut [f32]; 2] = std::array::from_fn(|_| &mut [] as &mut [f32]);
        let mut out_buf = make_planar_mut(&mut output, 2, samples, &mut out_slices);
        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let validated_inputs = make_planar_ref(&input_refs, samples);

        let result = renderer.render(validated_inputs, &mut out_buf);

        expect_ok!(result);
        // L_out = 1.0 + 0.707 + 0.707 + 0.707 = 3.121
        // R_out = 0.0 + 0.707 + 0.0 + 0.0 = 0.707
        for s in 0..samples {
            let l_out = output[s];
            let r_out = output[samples + s];
            expect_that!(l_out, near(3.121f32, 1e-3));
            expect_that!(r_out, near(0.707f32, 1e-3));
        }
    }

    #[gtest]
    fn downmix_stereo_to_mono_calculates_correct_gains() {
        let mut renderer = DownmixRenderer::new(InputLayout::Stereo, OutputLayout::Mono);
        let samples = 20;
        let mut inputs = vec![vec![0.0f32; samples]; 2];
        inputs[0].fill(1.0); // L
        inputs[1].fill(0.5); // R
        let mut output = vec![0.0f32; samples];
        let mut out_slices: [&mut [f32]; 1] = std::array::from_fn(|_| &mut [] as &mut [f32]);
        let mut out_buf = make_planar_mut(&mut output, 1, samples, &mut out_slices);
        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let validated_inputs = make_planar_ref(&input_refs, samples);

        let result = renderer.render(validated_inputs, &mut out_buf);

        expect_ok!(result);
        // Mono = 0.5 * L + 0.5 * R = 0.5 * 1.0 + 0.5 * 0.5 = 0.75
        for &out_val in output.iter().take(samples) {
            expect_that!(out_val, near(0.75f32, 1e-4));
        }
    }

    #[gtest]
    fn downmix_71_to_51_default_mode_0_calculates_correct_gains() {
        let mut renderer = DownmixRenderer::new(InputLayout::Layout71, OutputLayout::Layout51);
        let samples = 10;
        let mut inputs = vec![vec![0.0f32; samples]; 8];
        inputs[0].fill(1.0); // L
        inputs[1].fill(1.0); // R
        inputs[2].fill(1.0); // C
        inputs[3].fill(1.0); // LFE
        inputs[4].fill(1.0); // Lss
        inputs[5].fill(1.0); // Rss
        inputs[6].fill(1.0); // Lrs
        inputs[7].fill(1.0); // Rrs
        let mut output = vec![0.0f32; 6 * samples];
        let mut out_slices: [&mut [f32]; 6] = std::array::from_fn(|_| &mut [] as &mut [f32]);
        let mut out_buf = make_planar_mut(&mut output, 6, samples, &mut out_slices);
        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let validated_inputs = make_planar_ref(&input_refs, samples);

        let result = renderer.render(validated_inputs, &mut out_buf);

        expect_ok!(result);
        // Under default mode 0 (`alpha=1.0, beta=1.0`), Ls_out = Lss + Lrs = 2.0
        output.chunks(samples).enumerate().for_each(|(ch, out_slice)| {
            out_slice.iter().for_each(|&val| match ch {
                0..=3 => {
                    expect_that!(val, near(1.0f32, 1e-4));
                }
                4..=5 => {
                    expect_that!(val, near(2.0f32, 1e-4));
                }
                _ => unreachable!(),
            });
        });
    }

    #[gtest]
    fn downmix_71_to_51_mode_1_calculates_correct_gains() {
        let mut renderer = DownmixRenderer::new(InputLayout::Layout71, OutputLayout::Layout51);
        let samples = 10;
        let mut inputs = vec![vec![0.0f32; samples]; 8];
        inputs[0].fill(1.0); // L
        inputs[1].fill(1.0); // R
        inputs[2].fill(1.0); // C
        inputs[3].fill(1.0); // LFE
        inputs[4].fill(1.0); // Lss
        inputs[5].fill(1.0); // Rss
        inputs[6].fill(1.0); // Lrs
        inputs[7].fill(1.0); // Rrs
        let mut output = vec![0.0f32; 6 * samples];
        let mut out_slices: [&mut [f32]; 6] = std::array::from_fn(|_| &mut [] as &mut [f32]);
        let mut out_buf = make_planar_mut(&mut output, 6, samples, &mut out_slices);
        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let validated_inputs = make_planar_ref(&input_refs, samples);
        expect_ok!(renderer.set_mode_weight(DownmixMode::Mode2NegOffset, None));

        let result = renderer.render(validated_inputs, &mut out_buf);

        expect_ok!(result);
        // Test mode 1 (`alpha=0.707, beta=0.707`), Ls_out = 0.707 * Lss + 0.707 * Lrs = 1.414
        output.chunks(samples).enumerate().for_each(|(ch, out_slice)| {
            out_slice.iter().for_each(|&val| match ch {
                0..=3 => {
                    expect_that!(val, near(1.0f32, 1e-4));
                }
                4..=5 => {
                    expect_that!(val, near(1.414f32, 1e-4));
                }
                _ => unreachable!(),
            });
        });
    }

    #[gtest]
    fn downmix_height_layouts_updates_metadata_mode() {
        let mut renderer = DownmixRenderer::new(InputLayout::Layout714, OutputLayout::Layout312);
        let samples = 100;
        let inputs = vec![vec![0.0f32; samples]; 12];
        let inputs_ref: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let mut output = vec![0.0f32; 6 * samples];
        let mut out_slices: [&mut [f32]; 6] = std::array::from_fn(|_| &mut [] as &mut [f32]);
        let mut out_buf = make_planar_mut(&mut output, 6, samples, &mut out_slices);
        let validated_inputs = make_planar_ref(&inputs_ref, samples);

        let update_result = renderer.update_element_downmix_mode(
            1,
            DownmixMode::Mode3NegOffset,
            Some(Samples::new(100).unwrap()),
        );
        let render_result = renderer.render(validated_inputs, &mut out_buf);

        expect_ok!(update_result);
        expect_ok!(render_result);
        expect_that!(renderer.input_channels.get(), eq(12));
        expect_that!(renderer.output_channels.get(), eq(6));
        expect_true!(renderer.supported);
        expect_that!(renderer.mode, eq(DownmixMode::Mode3NegOffset));
    }

    #[gtest]
    fn render_fails_with_empty_inputs() {
        let mut renderer = DownmixRenderer::new(InputLayout::Layout51, OutputLayout::Stereo);
        let mut output = vec![0.0f32; 20];
        let mut out_slices: [&mut [f32]; 2] = std::array::from_fn(|_| &mut [] as &mut [f32]);
        let mut out_buf = make_planar_mut(&mut output, 2, 10, &mut out_slices);

        let validated_inputs = PlanarBufferRef::new(&[], 0, Samples::new(10).unwrap()).unwrap();
        let result = renderer.render(validated_inputs, &mut out_buf);
        expect_that!(result, err(eq(OarError::InvalidParameter)));
    }

    #[gtest]
    fn render_fails_with_mismatched_channel_count() {
        let mut renderer = DownmixRenderer::new(InputLayout::Layout51, OutputLayout::Stereo);
        let mut output = vec![0.0f32; 20];
        let inputs_3ch = vec![vec![0.0f32; 10]; 3];
        let input_refs_3ch: Vec<&[f32]> = inputs_3ch.iter().map(|v| v.as_slice()).collect();
        let mut out_slices: [&mut [f32]; 2] = std::array::from_fn(|_| &mut [] as &mut [f32]);
        let mut out_buf = make_planar_mut(&mut output, 2, 10, &mut out_slices);

        let validated_inputs = make_planar_ref(&input_refs_3ch, 10);
        let result = renderer.render(validated_inputs, &mut out_buf);

        expect_that!(result, err(eq(OarError::InvalidParameter)));
    }

    #[gtest]
    fn render_fails_with_mismatched_sample_lengths() {
        let inputs_mismatch = [
            vec![0.0f32; 10],
            vec![0.0f32; 10],
            vec![0.0f32; 9],
            vec![0.0f32; 10],
            vec![0.0f32; 10],
            vec![0.0f32; 10],
        ];
        let input_refs_mismatch: Vec<&[f32]> =
            inputs_mismatch.iter().map(|v| v.as_slice()).collect();

        let result = PlanarBufferRef::new(&input_refs_mismatch, 6, Samples::new(10).unwrap());

        expect_that!(result, err(eq(OarError::InvalidParameter)));
    }
}
