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

//! Dynamic Audio Element Router (AudioElementsRenderer) implementation.
//!
//! This module coordinates assigning audio elements (channel-based, object-based,
//! scene-based) dynamically into rendering groups and routes them to sub-renderers,
//! applying sub-frame temporal block slicing (`sub_frame_samples`) and dynamic
//! polar coordinate trajectory updates during block rendering (`render_sub_frames`).

use crate::common::definitions::ChannelCount;
use crate::common::definitions::{
    AudioElementConfig, DownmixMode, Gain, Interpolate, OarError, ObjectPosition, PlanarBufferMut,
    PlanarBufferRef, Quaternion, Samples, MAX_OUTPUT_CHANNEL_COUNT, METADATA_QUEUE_CAPACITY,
};
use crate::renderer::audio_renderer_api::AudioRenderer;
use crate::utility::oar_utils::cartesian_to_polar_sector_float32;
use std::collections::{BTreeMap, HashMap, VecDeque};

/// An individual item in an audio element's position metadata queue.
#[derive(Debug, Clone, PartialEq)]
pub struct PositionsMetadataItem {
    /// The spatial positions.
    pub positions: ObjectPosition,
    /// Duration in samples for which this metadata is active/valid.
    pub duration: Samples,
}

// TODO(b/525080422): Use the `Samples` type for duration/offset values.
/// Dynamic positions tracking queue and temporal context across blocks.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ElementPositionsContext {
    /// Relative start sample offset within current block or accumulated elapsed time.
    pub start: u32,
    /// Sum of durations of all queued metadata items in `positions`.
    pub duration: u32,
    /// FIFO queue of metadata items representing spatial trajectories over time.
    pub positions: VecDeque<PositionsMetadataItem>,
}

/// Manages and routes active audio elements within rendering groups while applying
/// sub-frame temporal block slicing and dynamic polar coordinate trajectory updates.
///
/// Adheres strictly to zero dynamic heap allocations in the real-time `render()` loop,
/// slicing planar input slices (`&inputs[ch][offset..offset + sub_samples]`) without copying.
pub struct AudioElementsRenderer {
    active_elements: HashMap<u32, AudioElementConfig>,
    positions_queues: BTreeMap<u32, ElementPositionsContext>,
    sub_frame_samples: Option<Samples>,
    sub_out_buffer: Vec<f32>,
    num_out_channels: ChannelCount,
    /// Underlying spatial rendering backend (e.g. `ObjectRenderer`, `EarRenderer`).
    /// Note: manual Clone impl sets this field to None when cloned.
    pub sub_renderer: Option<Box<dyn AudioRenderer>>,
}

impl std::fmt::Debug for AudioElementsRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioElementsRenderer")
            .field("active_elements", &self.active_elements)
            .field("positions_queues", &self.positions_queues)
            .field("sub_frame_samples", &self.sub_frame_samples)
            .field(
                "sub_renderer",
                &if self.sub_renderer.is_some() { "Some(Box<dyn AudioRenderer>)" } else { "None" },
            )
            .finish()
    }
}

impl Clone for AudioElementsRenderer {
    fn clone(&self) -> Self {
        Self {
            active_elements: self.active_elements.clone(),
            positions_queues: self.positions_queues.clone(),
            sub_frame_samples: self.sub_frame_samples,
            sub_out_buffer: Vec::new(),
            num_out_channels: self.num_out_channels,
            sub_renderer: None,
        }
    }
}

impl AudioElementsRenderer {
    /// Creates a new, empty `AudioElementsRenderer` with specified output channel count and
    /// sub-frame size.
    ///
    /// # Parameters
    ///
    /// * `num_out_channels`: The number of target output channels. Must be `<=
    ///   MAX_OUTPUT_CHANNEL_COUNT`.
    /// * `sub_frame_samples`: The size of the sub-frame blocks in samples. If `Some(size)`, enables
    ///   slicing input blocks into sub-frames of this size during rendering. If `None`, sub-frame
    ///   slicing is disabled (inputs are rendered as a single block, and no scratch memory is
    ///   allocated).
    pub fn new(num_out_channels: ChannelCount, sub_frame_samples: Option<Samples>) -> Self {
        assert!(
            num_out_channels.get() <= MAX_OUTPUT_CHANNEL_COUNT,
            "Output channels {} exceeds maximum {}",
            num_out_channels,
            MAX_OUTPUT_CHANNEL_COUNT
        );
        let samples_val = sub_frame_samples.map(|s| s.value() as usize).unwrap_or(0);
        let req_size = num_out_channels.get() * samples_val;
        Self {
            active_elements: HashMap::new(),
            positions_queues: BTreeMap::new(),
            sub_frame_samples,
            sub_out_buffer: vec![0.0; req_size],
            num_out_channels,
            sub_renderer: None,
        }
    }

    /// Creates a new `AudioElementsRenderer` encapsulating an underlying spatial backend
    /// (`AudioRenderer`) with specified output channel count and sub-frame size.
    ///
    /// # Parameters
    ///
    /// * `sub`: The underlying spatial renderer backend (e.g. `ObjectRenderer`, `EarRenderer`).
    /// * `num_out_channels`: The number of target output channels. Must be `<=
    ///   MAX_OUTPUT_CHANNEL_COUNT`.
    /// * `sub_frame_samples`: The size of the sub-frame blocks in samples. If `Some(size)`, enables
    ///   slicing input blocks into sub-frames of this size during rendering. If `None`, sub-frame
    ///   slicing is disabled (inputs are rendered as a single block, and no scratch memory is
    ///   allocated).
    pub fn with_backend(
        sub: Box<dyn AudioRenderer>,
        num_out_channels: ChannelCount,
        sub_frame_samples: Option<Samples>,
    ) -> Self {
        assert!(
            num_out_channels.get() <= MAX_OUTPUT_CHANNEL_COUNT,
            "Output channels {} exceeds maximum {}",
            num_out_channels,
            MAX_OUTPUT_CHANNEL_COUNT
        );
        let samples_val = sub_frame_samples.map(|s| s.value() as usize).unwrap_or(0);
        let req_size = num_out_channels.get() * samples_val;
        Self {
            active_elements: HashMap::new(),
            positions_queues: BTreeMap::new(),
            sub_frame_samples,
            sub_out_buffer: vec![0.0; req_size],
            num_out_channels,
            sub_renderer: Some(sub),
        }
    }

    /// Sets or updates the sub-frame block size in samples.
    pub fn set_sub_frame_samples(&mut self, samples: Option<Samples>) {
        self.sub_frame_samples = samples;
        let samples_val = samples.map(|s| s.value() as usize).unwrap_or(0);
        let req_size = self.num_out_channels.get() * samples_val;
        if self.sub_out_buffer.len() < req_size {
            self.sub_out_buffer.resize(req_size, 0.0);
        }
    }

    /// Adds/configures an audio element in the router.
    ///
    /// # Errors
    ///
    /// Returns `OarError::InvalidParameter` if the element ID is already registered.
    pub fn add_element(&mut self, id: u32, config: &AudioElementConfig) -> Result<(), OarError> {
        if self.active_elements.contains_key(&id) {
            return Err(OarError::InvalidParameter);
        }
        self.active_elements.insert(id, *config);
        self.positions_queues.insert(
            id,
            ElementPositionsContext {
                positions: VecDeque::with_capacity(METADATA_QUEUE_CAPACITY),
                start: 0,
                duration: 0,
            },
        );
        if let Some(ref mut sub) = self.sub_renderer {
            sub.add_element(id, config)?;
        }
        Ok(())
    }

    /// Removes an audio element from the router and its underlying sub-renderer.
    ///
    /// # Errors
    ///
    /// Returns `OarError::InvalidParameter` if `id` is not registered, or propagates any error
    /// from the underlying sub-renderer.
    pub fn remove_element(&mut self, id: u32) -> Result<(), OarError> {
        if !self.active_elements.contains_key(&id) {
            return Err(OarError::InvalidParameter);
        }
        if let Some(ref mut sub) = self.sub_renderer {
            sub.remove_element(id)?;
        }
        self.active_elements.remove(&id);
        self.positions_queues.remove(&id);
        Ok(())
    }

    /// Queues position updates for an audio element.
    ///
    /// # Errors
    ///
    /// Returns `OarError::InvalidParameter` if the element ID is not registered or queue update
    ///         fails.
    pub fn update_element_positions(
        &mut self,
        id: u32,
        positions: &ObjectPosition,
        duration: Samples,
    ) -> Result<(), OarError> {
        if !self.active_elements.contains_key(&id) {
            return Err(OarError::InvalidParameter);
        }

        // Convert cartesian to polar before sending to sub-renderers.
        let safe_positions = match positions {
            ObjectPosition::Cartesian(cart_coords) => {
                let mut polar_coords = Vec::with_capacity(cart_coords.len());
                for &cart in cart_coords {
                    polar_coords.push(cartesian_to_polar_sector_float32(cart)?);
                }
                ObjectPosition::Polar(polar_coords)
            }
            _ => positions.clone(),
        };

        let context = self.positions_queues.entry(id).or_default();
        if context.positions.is_empty() {
            context.start = 0;
        }
        context.positions.push_back(PositionsMetadataItem { positions: safe_positions, duration });
        context.duration += u32::from(duration);
        Ok(())
    }

    /// Elapses time counters across queued metadata items after processing blocks
    /// (`metadata_item_elapse`).
    pub fn elapser_tick(&mut self, samples: u32) {
        for context in self.positions_queues.values_mut() {
            context.start += samples;
            while !context.positions.is_empty() {
                if let Some(first) = context.positions.front() {
                    let dur_u32 = u32::from(first.duration);
                    if dur_u32 <= context.start {
                        context.start -= dur_u32;
                        context.duration = context.duration.saturating_sub(dur_u32);
                        context.positions.pop_front();
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        }
    }

    /// Helper to create constant or spherical-linearly interpolated polar coordinate metadata
    /// specific to a sub-frame interval (`metadata_constant_polar_positions_create`).
    fn create_subframe_positions(
        pos: &ObjectPosition,
        next_pos: Option<&ObjectPosition>,
        relative_pos: u32,
        total_entry_duration: Samples,
        sub_duration: u32,
    ) -> Result<ObjectPosition, OarError> {
        if sub_duration == 0 {
            return Ok(pos.clone());
        }
        let total_dur_u32 = u32::from(total_entry_duration);
        match pos {
            ObjectPosition::Polar(curr_polar) => {
                if let Some(next) = next_pos
                    && let ObjectPosition::Polar(next_polar) = next
                {
                    let c = (relative_pos as f32) / (total_dur_u32 as f32);
                    if !c.is_nan() && (0.0..=1.0).contains(&c) {
                        let mut interp_polar = curr_polar.clone();
                        for i in 0..curr_polar.len().min(next_polar.len()) {
                            interp_polar[i] = curr_polar[i].lerp(next_polar[i], c);
                        }
                        return Ok(ObjectPosition::Polar(interp_polar));
                    }
                }
                Ok(pos.clone())
            }
            ObjectPosition::AnimatedPolar(curr_anim_polar) => {
                let c = (relative_pos as f32) / (total_dur_u32 as f32);
                let mut evaluated_polar = Vec::with_capacity(curr_anim_polar.len());
                for anim in curr_anim_polar {
                    let p = anim.sample(c);
                    evaluated_polar.push(p);
                }
                Ok(ObjectPosition::Polar(evaluated_polar))
            }
            ObjectPosition::AnimatedCartesian(curr_anim_cart) => {
                let c = (relative_pos as f32) / (total_dur_u32 as f32);
                let mut evaluated_polar = Vec::with_capacity(curr_anim_cart.len());
                for anim in curr_anim_cart {
                    let cart = anim.sample(c);
                    let polar = cartesian_to_polar_sector_float32(cart)?;
                    evaluated_polar.push(polar);
                }
                Ok(ObjectPosition::Polar(evaluated_polar))
            }
            _ => Ok(pos.clone()),
        }
    }

    /// Renders planar input channels across temporal sub-frames (`_sub_frames_apply_positions`).
    ///
    /// Slices input channels using stack references (`&ch[offset..offset + sub_samples]`)
    /// without heap allocation on the critical audio processing loop.
    pub fn render_sub_frames(
        &mut self,
        inputs: PlanarBufferRef<'_, '_>,
        output: &mut PlanarBufferMut<'_, '_>,
    ) -> Result<(), OarError> {
        if inputs.num_channels() == 0 || output.num_channels() == 0 {
            return Err(OarError::InvalidParameter);
        }
        let total_samples = inputs.num_samples();
        if total_samples == 0 {
            return Err(OarError::InvalidParameter);
        }

        if output.num_samples() != total_samples {
            return Err(OarError::InvalidParameter);
        }
        let out_channels = output.num_channels();

        let sub_frame_samples =
            self.sub_frame_samples.map(|s| s.value() as usize).unwrap_or(total_samples);

        const MAX_STACK_CHANNELS: usize = 256;
        if inputs.num_channels() > MAX_STACK_CHANNELS {
            return Err(OarError::InvalidParameter);
        }

        let mut processed_samples: usize = 0;

        while processed_samples < total_samples {
            let current_unit_samples = (total_samples - processed_samples).min(sub_frame_samples);

            // 1. Sub-frames coordinate update iteration (_sub_frames_apply_positions)
            for (&id, ctx) in &self.positions_queues {
                let sample_position = processed_samples as u32 + ctx.start;
                let mut accumulated_duration = 0u32;
                let mut active_pos_ref: Option<&ObjectPosition> = None;
                let mut interpolated_pos: Option<ObjectPosition> = None;

                for (entry_idx, entry) in ctx.positions.iter().enumerate() {
                    let dur_u32 = u32::from(entry.duration);
                    if sample_position >= accumulated_duration
                        && sample_position < accumulated_duration.saturating_add(dur_u32)
                    {
                        let relative_pos = sample_position - accumulated_duration;
                        let next_entry_pos = if entry_idx + 1 < ctx.positions.len() {
                            Some(&ctx.positions[entry_idx + 1].positions)
                        } else {
                            None
                        };

                        let needs_eval = match &entry.positions {
                            ObjectPosition::Polar(_) => next_entry_pos.is_some(),
                            ObjectPosition::AnimatedPolar(_) => true,
                            ObjectPosition::AnimatedCartesian(_) => true,
                            _ => false,
                        };

                        if needs_eval {
                            interpolated_pos = Some(Self::create_subframe_positions(
                                &entry.positions,
                                next_entry_pos,
                                relative_pos,
                                entry.duration,
                                current_unit_samples as u32,
                            )?);
                        } else {
                            active_pos_ref = Some(&entry.positions);
                        }
                        break;
                    }
                    accumulated_duration = accumulated_duration.saturating_add(dur_u32);
                }

                let final_pos_ref =
                    if let Some(p) = active_pos_ref { Some(p) } else { interpolated_pos.as_ref() };

                if let Some(pos) = final_pos_ref
                    && let Some(ref mut sub) = self.sub_renderer
                {
                    let _ = sub.update_element_positions(
                        id,
                        pos,
                        Samples::new(current_unit_samples as u32)?,
                    );
                }
            }

            // 2. Zero-allocation input slicing via &[ch][offset..offset + sub_samples]
            let mut stack_sub_inputs: [&[f32]; MAX_STACK_CHANNELS] = [&[]; MAX_STACK_CHANNELS];
            for (i, sub_input) in
                stack_sub_inputs.iter_mut().enumerate().take(inputs.num_channels())
            {
                let ch_buf = inputs.channel(i);
                if processed_samples + current_unit_samples > ch_buf.len() {
                    return Err(OarError::InvalidParameter);
                }
                *sub_input = &ch_buf[processed_samples..processed_samples + current_unit_samples];
            }
            let sub_inputs = &stack_sub_inputs[..inputs.num_channels()];
            let sub_inputs_ref = PlanarBufferRef::new(
                sub_inputs,
                sub_inputs.len(),
                Samples::new(current_unit_samples as u32)?,
            )?;

            // 3. Sub-frame rendering and output buffer write
            if let Some(ref mut sub) = self.sub_renderer {
                if current_unit_samples == total_samples && processed_samples == 0 {
                    sub.render(sub_inputs_ref, output)?;
                } else if out_channels <= 1 {
                    let sub_out = &mut output.channel_mut(0)
                        [processed_samples..processed_samples + current_unit_samples];
                    let mut sub_out_slices = [sub_out];
                    let mut sub_out_buf = PlanarBufferMut::new(
                        &mut sub_out_slices,
                        1,
                        Samples::new(current_unit_samples as u32)?,
                    )?;
                    sub.render(sub_inputs_ref, &mut sub_out_buf)?;
                } else {
                    if out_channels > self.num_out_channels.get() {
                        return Err(OarError::InvalidParameter);
                    }
                    let req_size = out_channels * current_unit_samples;
                    debug_assert!(self.sub_out_buffer.len() >= req_size);
                    let sub_out = &mut self.sub_out_buffer[..req_size];
                    sub_out.fill(0.0);
                    let mut sub_out_slices: [&mut [f32]; MAX_OUTPUT_CHANNEL_COUNT] =
                        std::array::from_fn(|_| &mut [] as &mut [f32]);
                    let mut chunks = sub_out.chunks_exact_mut(current_unit_samples);
                    for slice in sub_out_slices.iter_mut().take(out_channels) {
                        *slice = chunks.next().ok_or(OarError::InvalidParameter)?;
                    }
                    let mut sub_out_buf = PlanarBufferMut::new(
                        &mut sub_out_slices[..out_channels],
                        out_channels,
                        Samples::new(current_unit_samples as u32)?,
                    )?;
                    sub.render(sub_inputs_ref, &mut sub_out_buf)?;

                    for ch in 0..out_channels {
                        let src =
                            &sub_out[ch * current_unit_samples..(ch + 1) * current_unit_samples];
                        let dst = &mut output.channel_mut(ch)
                            [processed_samples..processed_samples + current_unit_samples];
                        dst.copy_from_slice(src);
                    }
                }
            } else {
                if processed_samples == 0 && current_unit_samples == total_samples {
                    output.fill(0.0);
                } else {
                    for ch in 0..out_channels {
                        let dst = &mut output.channel_mut(ch)
                            [processed_samples..processed_samples + current_unit_samples];
                        dst.fill(0.0);
                    }
                }
            }

            processed_samples += current_unit_samples;
        }

        // 4. Advance timeline counters across all queued metadata (`metadata_item_elapse`)
        self.elapser_tick(total_samples as u32);

        Ok(())
    }
}

impl Default for AudioElementsRenderer {
    fn default() -> Self {
        Self::new(ChannelCount::new(MAX_OUTPUT_CHANNEL_COUNT).unwrap(), None)
    }
}

impl AudioRenderer for AudioElementsRenderer {
    fn add_element(&mut self, id: u32, config: &AudioElementConfig) -> Result<(), OarError> {
        self.add_element(id, config)
    }

    fn remove_element(&mut self, id: u32) -> Result<(), OarError> {
        self.remove_element(id)
    }

    fn update_element_gain(
        &mut self,
        id: u32,
        gain_id: u32,
        gain: &Gain,
        duration: Samples,
    ) -> Result<(), OarError> {
        if let Some(ref mut sub) = self.sub_renderer {
            sub.update_element_gain(id, gain_id, gain, duration)
        } else {
            Err(OarError::NotSupported)
        }
    }

    fn update_element_positions(
        &mut self,
        id: u32,
        positions: &ObjectPosition,
        duration: Samples,
    ) -> Result<(), OarError> {
        self.update_element_positions(id, positions, duration)
    }

    fn update_element_downmix_mode(
        &mut self,
        id: u32,
        mode: DownmixMode,
        duration: Option<Samples>,
    ) -> Result<(), OarError> {
        if let Some(ref mut sub) = self.sub_renderer {
            sub.update_element_downmix_mode(id, mode, duration)
        } else {
            Err(OarError::NotSupported)
        }
    }

    fn render(
        &mut self,
        inputs: PlanarBufferRef<'_, '_>,
        output: &mut PlanarBufferMut<'_, '_>,
    ) -> Result<(), OarError> {
        self.render_sub_frames(inputs, output)
    }

    fn enable_head_tracking(&mut self, enable: bool) -> Result<(), OarError> {
        if let Some(ref mut sub) = self.sub_renderer {
            sub.enable_head_tracking(enable)
        } else {
            Err(OarError::NotSupported)
        }
    }

    fn set_head_rotation(&mut self, rotation: Quaternion) -> Result<(), OarError> {
        if let Some(ref mut sub) = self.sub_renderer {
            sub.set_head_rotation(rotation)
        } else {
            Err(OarError::NotSupported)
        }
    }

    fn set_metadata_unit_to_process(&mut self, samples: Samples) -> Result<(), OarError> {
        if let Some(ref mut sub) = self.sub_renderer {
            sub.set_metadata_unit_to_process(samples)?;
        }
        self.set_sub_frame_samples(Some(samples));
        Ok(())
    }
}

// ===== Tests =====

#[cfg(test)]
mod test {

    use super::*;
    use crate::common::definitions::{
        AudioElementConfig, CartesianCoordinate, ChannelBasedConfig, Degrees, Distance,
        InputLayout, OarError, ObjectBasedConfig, ObjectPosition, PolarCoordinate,
    };
    use googletest::prelude::*;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct MockState {
        elements: Vec<u32>,
        renders: Vec<Vec<f32>>,
        positions_updates: Vec<(u32, ObjectPosition, Samples)>,
    }

    struct SharedMockRenderer {
        state: Arc<Mutex<MockState>>,
    }

    impl SharedMockRenderer {
        fn new(state: Arc<Mutex<MockState>>) -> Self {
            Self { state }
        }
    }

    impl AudioRenderer for SharedMockRenderer {
        fn add_element(&mut self, id: u32, _config: &AudioElementConfig) -> Result<(), OarError> {
            self.state.lock().unwrap().elements.push(id);
            Ok(())
        }
        fn remove_element(&mut self, id: u32) -> Result<(), OarError> {
            let mut state = self.state.lock().unwrap();
            let pos =
                state.elements.iter().position(|&x| x == id).ok_or(OarError::InvalidParameter)?;
            state.elements.remove(pos);
            Ok(())
        }
        fn update_element_positions(
            &mut self,
            id: u32,
            positions: &ObjectPosition,
            duration: Samples,
        ) -> Result<(), OarError> {
            self.state.lock().unwrap().positions_updates.push((id, positions.clone(), duration));
            Ok(())
        }
        fn render(
            &mut self,
            inputs: PlanarBufferRef<'_, '_>,
            output: &mut PlanarBufferMut<'_, '_>,
        ) -> Result<(), OarError> {
            if inputs.num_channels() > 0 {
                self.state.lock().unwrap().renders.push(inputs.channel(0).to_vec());
            }
            output.fill(1.0);
            Ok(())
        }
    }

    #[gtest]
    fn slices_input_into_sub_frames_during_render() {
        let state = Arc::new(Mutex::new(MockState::default()));
        let mock = Box::new(SharedMockRenderer::new(state.clone()));
        let mut router = AudioElementsRenderer::with_backend(
            mock,
            ChannelCount::new(1).unwrap(),
            Some(Samples::new(4).unwrap()),
        );
        let config = AudioElementConfig::ObjectBased(ObjectBasedConfig::new(1, None).unwrap());
        router.add_element(10, &config).unwrap();
        let input_data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0];
        let inputs = [&input_data[..]];
        let mut output = [0.0; 12];
        let mut out_slices = [&mut output[..]];
        let mut out_buf =
            PlanarBufferMut::new(&mut out_slices, 1, Samples::new(12).unwrap()).unwrap();
        let validated_inputs = PlanarBufferRef::new(&inputs, 1, Samples::new(12).unwrap()).unwrap();
        router.render(validated_inputs, &mut out_buf).unwrap();

        let state_lock = state.lock().unwrap();
        expect_that!(state_lock.renders.len(), eq(3));
        expect_that!(state_lock.renders[0].as_slice(), eq(&[1.0f32, 2.0f32, 3.0f32, 4.0f32][..]));
        expect_that!(state_lock.renders[1].as_slice(), eq(&[5.0f32, 6.0f32, 7.0f32, 8.0f32][..]));
        expect_that!(
            state_lock.renders[2].as_slice(),
            eq(&[9.0f32, 10.0f32, 11.0f32, 12.0f32][..])
        );
    }

    #[gtest]
    fn interpolates_metadata_across_sub_frames() -> Result<()> {
        let state = Arc::new(Mutex::new(MockState::default()));
        let mock = Box::new(SharedMockRenderer::new(state.clone()));
        let samples_4 = Samples::new(4).unwrap();
        let samples_8 = Samples::new(8).unwrap();
        let mut router = AudioElementsRenderer::with_backend(
            mock,
            ChannelCount::new(1).unwrap(),
            Some(samples_4),
        );
        let config = AudioElementConfig::ObjectBased(ObjectBasedConfig::new(1, None).unwrap());
        router.add_element(10, &config).unwrap();
        // Queue positions 1: azimuth 0.0, duration 8
        let pos1 =
            ObjectPosition::Polar(vec![PolarCoordinate::new_from_floats(0.0, 0.0, 1.0).unwrap()]);
        router.update_element_positions(10, &pos1, samples_8).unwrap();
        // Positions 2: azimuth 90.0, duration 8
        let pos2 =
            ObjectPosition::Polar(vec![PolarCoordinate::new_from_floats(90.0, 0.0, 1.0).unwrap()]);
        router.update_element_positions(10, &pos2, samples_8).unwrap();
        // Render 12 samples (3 sub-frames)
        let input_data = [0.0; 12];
        let inputs = [&input_data[..]];
        let mut output = [0.0; 12];
        let mut out_slices = [&mut output[..]];
        let mut out_buf =
            PlanarBufferMut::new(&mut out_slices, 1, Samples::new(12).unwrap()).unwrap();
        let validated_inputs = PlanarBufferRef::new(&inputs, 1, Samples::new(12).unwrap()).unwrap();
        router.render(validated_inputs, &mut out_buf).unwrap();

        let state_lock = state.lock().unwrap();
        verify_that!(
            state_lock.positions_updates,
            elements_are![
                // Sub-frame 0: c = 0.0 -> azimuth 0.0
                (
                    eq(&10),
                    pat!(ObjectPosition::Polar(elements_are![pat!(PolarCoordinate {
                        azimuth(): pat!(Degrees { value(): near(0.0, 1e-5) }),
                        elevation(): pat!(Degrees { value(): near(0.0, 1e-5) }),
                        distance(): pat!(Distance { value(): near(1.0, 1e-5) }),
                    })])),
                    eq(&samples_4),
                ),
                // Sub-frame 1: c = 0.5 -> azimuth 45.0
                (
                    eq(&10),
                    pat!(ObjectPosition::Polar(elements_are![pat!(PolarCoordinate {
                        azimuth(): pat!(Degrees { value(): near(45.0, 1e-5) }),
                        elevation(): pat!(Degrees { value(): near(0.0, 1e-5) }),
                        distance(): pat!(Distance { value(): near(1.0, 1e-5) }),
                    })])),
                    eq(&samples_4),
                ),
                // Sub-frame 2: c = 0.0 (in pos2) -> azimuth 90.0
                (
                    eq(&10),
                    pat!(ObjectPosition::Polar(elements_are![pat!(PolarCoordinate {
                        azimuth(): pat!(Degrees { value(): near(90.0, 1e-5) }),
                        elevation(): pat!(Degrees { value(): near(0.0, 1e-5) }),
                        distance(): pat!(Distance { value(): near(1.0, 1e-5) }),
                    })])),
                    eq(&samples_4),
                ),
            ]
        )
    }

    #[gtest]
    fn audio_elements_router_fails_to_add_duplicate_element() {
        let mut router = AudioElementsRenderer::new(ChannelCount::new(2).unwrap(), None);
        let config = AudioElementConfig::ChannelBased(ChannelBasedConfig {
            layout: InputLayout::Stereo,
            downmix_info: None,
            rendering_config: None,
        });
        router.add_element(42, &config).unwrap();

        let result = router.add_element(42, &config);

        expect_that!(result, err(eq(OarError::InvalidParameter)));
    }

    #[gtest]
    fn converts_cartesian_to_polar_at_ingestion() -> Result<()> {
        let state = Arc::new(Mutex::new(MockState::default()));
        let mock = Box::new(SharedMockRenderer::new(state.clone()));
        let samples_12 = Samples::new(12).unwrap();
        let mut router = AudioElementsRenderer::with_backend(
            mock,
            ChannelCount::new(1).unwrap(),
            Some(samples_12),
        );
        let config = AudioElementConfig::ObjectBased(ObjectBasedConfig::new(1, None).unwrap());
        router.add_element(10, &config).unwrap();

        let pos = ObjectPosition::Cartesian(vec![CartesianCoordinate::new(0.0, 1.0, 0.0).unwrap()]);
        router.update_element_positions(10, &pos, samples_12).unwrap();

        let input_data = [0.0; 12];
        let inputs = [&input_data[..]];
        let mut output = [0.0; 12];
        let mut out_slices = [&mut output[..]];
        let mut out_buf = PlanarBufferMut::new(&mut out_slices, 1, samples_12).unwrap();
        let validated_inputs = PlanarBufferRef::new(&inputs, 1, samples_12).unwrap();
        router.render(validated_inputs, &mut out_buf).unwrap();

        let state_lock = state.lock().unwrap();
        verify_that!(
            state_lock.positions_updates,
            elements_are![(
                eq(&10),
                pat!(ObjectPosition::Polar(elements_are![pat!(PolarCoordinate {
                    azimuth(): pat!(Degrees { value(): near(0.0, 1e-3) }),
                    elevation(): pat!(Degrees { value(): near(0.0, 1e-3) }),
                    distance(): pat!(Distance { value(): near(1.0, 1e-3) }),
                })])),
                eq(&samples_12),
            )]
        )
    }

    #[gtest]
    fn remove_element_stops_removed_element_and_preserves_remaining_positions() -> Result<()> {
        let state = Arc::new(Mutex::new(MockState::default()));
        let mock = Box::new(SharedMockRenderer::new(state.clone()));
        let samples_4 = Samples::new(4).unwrap();
        let mut router = AudioElementsRenderer::with_backend(
            mock,
            ChannelCount::new(1).unwrap(),
            Some(samples_4),
        );
        let config = AudioElementConfig::ObjectBased(ObjectBasedConfig::new(1, None).unwrap());
        router.add_element(10, &config).unwrap();
        router.add_element(20, &config).unwrap();
        let pos10 =
            ObjectPosition::Polar(vec![PolarCoordinate::new_from_floats(30.0, 0.0, 1.0).unwrap()]);
        let pos20 =
            ObjectPosition::Polar(vec![PolarCoordinate::new_from_floats(-45.0, 0.0, 1.0).unwrap()]);
        router.update_element_positions(10, &pos10, samples_4).unwrap();
        router.update_element_positions(20, &pos20, samples_4).unwrap();
        let input_data = [0.0; 4];
        let inputs = [&input_data[..]];
        let mut output = [0.0; 4];
        let mut out_slices = [&mut output[..]];
        let mut out_buf = PlanarBufferMut::new(&mut out_slices, 1, samples_4).unwrap();
        let validated_inputs = PlanarBufferRef::new(&inputs, 1, samples_4).unwrap();

        router.remove_element(10).unwrap();
        let duplicate_remove = router.remove_element(10);
        router.render(validated_inputs, &mut out_buf).unwrap();

        expect_that!(duplicate_remove, err(eq(OarError::InvalidParameter)));
        let state_lock = state.lock().unwrap();
        expect_that!(state_lock.elements.as_slice(), eq(&[20u32][..]));
        verify_that!(
            state_lock.positions_updates,
            elements_are![(
                eq(&20),
                pat!(ObjectPosition::Polar(elements_are![pat!(PolarCoordinate {
                    azimuth(): pat!(Degrees { value(): near(-45.0, 1e-5) }),
                    elevation(): pat!(Degrees { value(): near(0.0, 1e-5) }),
                    distance(): pat!(Distance { value(): near(1.0, 1e-5) }),
                })])),
                eq(&samples_4),
            )]
        )
    }
}
