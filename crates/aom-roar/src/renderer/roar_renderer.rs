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

//! RoarRenderer, main orchestrator of the rendering pipeline.
//!
//! This module implements runtime state boundaries, dynamic updates (gain, head rotation, object
//! position, etc) and rendering loops.

use crate::common::definitions::ChannelCount;
use crate::common::definitions::{
    AudioElementConfig, Config, Decibels, DownmixMode, Gain, GroupId, HeadphonesRenderingMode,
    LinearGain, OarError, ObjectPosition, OutputLayout, PlanarBufferMut, PlanarBufferRef,
    Quaternion, Samples,
};
use crate::limiter::oar_limiter::OarLimiter;
use crate::renderer::audio_elements_renderer::AudioElementsRenderer;
use crate::renderer::audio_renderer_api::AudioRenderer;
use crate::renderer::downmix::downmix_renderer::DownmixRenderer;
use crate::renderer::ear::ear_renderer::EarRenderer;
use crate::renderer::gain_parameter::{generate_gain_multipliers, GainParameter, GainUpdate};
use crate::renderer::obr::common::constants::MAX_SUPPORTED_NUM_INPUT_CHANNELS;
use crate::renderer::obr::renderer::ObrAudioRendererAdapter;
use crate::renderer::olr::object_renderer::ObjectRenderer;
use std::collections::HashMap;

const MAX_OUTPUT_CHANNELS: usize = 25;

/// Status of the OAR rendering engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Configuring,
    Rendering,
}

/// Internal helper representing an active renderer instance for an element.
pub(crate) struct ActiveRenderer {
    pub(crate) element_id: Option<u32>,
    pub(crate) renderer: Box<dyn AudioRenderer>,
}

/// Central rendering orchestrator for ROAR.
///
/// Manages the lifecycle of audio elements and groups, coordinates
/// the audio rendering process, and applies metadata updates.
/// Uses an internal state machine (`Status`) to transition from
/// configuration to rendering, ensuring allocations are performed eagerly.
///
/// # Examples
///
/// ```
/// use aom_roar::{
///     AudioElementConfig, ChannelBasedConfig, Config, InputLayout, OutputLayout, PlanarBufferMut,
///     PlanarBufferRef, RoarRenderer, SampleRate, Samples,
/// };
///
/// let samples = Samples::new(256).unwrap();
/// let config = Config::new(OutputLayout::Stereo, samples, SampleRate::new(48000).unwrap()).unwrap();
///
/// let mut rdr = RoarRenderer::create(&config).unwrap();
/// let group_id = rdr.add_audio_group().unwrap();
///
/// let element_cfg = AudioElementConfig::ChannelBased(ChannelBasedConfig {
///     layout: InputLayout::Stereo,
///     downmix_info: None,
///     rendering_config: None,
/// });
/// rdr.add_element(group_id, 42, &element_cfg).unwrap();
///
/// let in_chans = [vec![0.0f32; 256], vec![0.0f32; 256]];
/// let in_slices = [in_chans[0].as_slice(), in_chans[1].as_slice()];
/// let inputs = [(42, PlanarBufferRef::new(&in_slices, 2, samples).unwrap())];
///
/// let (mut out_l, mut out_r) = (vec![0.0f32; 256], vec![0.0f32; 256]);
/// let mut out_slices = [out_l.as_mut_slice(), out_r.as_mut_slice()];
/// let mut output = PlanarBufferMut::new(&mut out_slices, 2, samples).unwrap();
/// rdr.render(&inputs, &mut output).unwrap();
/// ```
pub struct RoarRenderer {
    /// Renderer configuration (output layout, sample rate, etc.).
    config: Config,
    /// Current state of the renderer lifecycle.
    status: Status,
    /// Active audio elements and their configurations.
    active_elements: HashMap<u32, AudioElementConfig>,
    /// List of active element IDs.
    active_element_ids: Vec<u32>,
    /// Mapping from element ID to its parent group ID.
    pub(crate) element_to_group: HashMap<u32, GroupId>,
    /// List of active group IDs.
    active_groups: Vec<GroupId>,
    /// Renderers associated with each group.
    group_renderers: HashMap<GroupId, Vec<ActiveRenderer>>,
    /// Scratch buffer for element-level rendering.
    renderer_scratch: Vec<f32>,
    /// Scratch buffer for group-level mixing.
    group_scratch: Vec<f32>,
    /// Toggle for the output peak limiter.
    limiter_enabled: bool,
    /// Toggle for listener head tracking (binaural output only).
    pub(crate) head_tracking_enabled: bool,
    /// Toggle for group loudness normalization.
    pub(crate) loudness_processor_enabled: bool,
    /// Loudness normalization gain adjustments per group.
    pub(crate) group_loudness: HashMap<GroupId, LinearGain>,
    /// Output peak limiter.
    limiter: OarLimiter,
    /// Dynamic gain parameters for elements, keyed by element ID and gain ID.
    element_gains: HashMap<u32, HashMap<u32, GainParameter>>,
    /// Dynamic gain parameters for groups.
    group_gains: HashMap<GroupId, GainParameter>,
    /// Scratch buffer for gain interpolation.
    gain_scratch_buf: Vec<f32>,
    /// Interval in samples for processing object position metadata.
    pub(crate) object_position_metadata_samples: Samples,
}

impl RoarRenderer {
    /// Returns a reference to the list of active group IDs.
    pub fn active_groups(&self) -> &[GroupId] {
        &self.active_groups
    }

    /// Returns a reference to the configuration.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Returns a reference to the map of active audio elements.
    pub fn active_elements(&self) -> &HashMap<u32, AudioElementConfig> {
        &self.active_elements
    }

    /// Checks if a group ID is active.
    pub fn has_group(&self, gid: GroupId) -> bool {
        self.active_groups.contains(&gid)
    }

    /// Enables or disables the output peak limiter.
    ///
    /// When transitioning from disabled to enabled, the limiter state and delay buffers are
    /// reset to discard any stale buffered audio.
    ///
    /// # Arguments
    /// * `enable` - `true` to enable output limiting, `false` to bypass it.
    pub fn enable_limiter(&mut self, enable: bool) -> Result<(), OarError> {
        if enable && !self.limiter_enabled {
            self.limiter.reset();
        }
        self.limiter_enabled = enable;
        Ok(())
    }

    /// Flushes the limiter's look-ahead delay buffer into `output` and resets limiter state.
    ///
    /// If the limiter is disabled or has zero look-ahead delay, returns `Ok(0)` without
    /// modifying `output`.
    ///
    /// # Arguments
    /// * `output` - Destination planar buffer with the renderer's output channel count and at least
    ///   `limiter_delay()` samples per channel when the limiter is enabled.
    ///
    /// # Returns
    /// The number of flushed samples per channel (`limiter_delay()` when enabled, `0` when
    /// disabled).
    pub fn flush(&mut self, output: &mut PlanarBufferMut<'_, '_>) -> Result<usize, OarError> {
        let out_channels = self.config.output_layout().channels().get();
        if output.num_channels() != out_channels {
            return Err(OarError::InvalidParameter);
        }
        if !self.limiter_enabled || self.limiter.delay() == 0 {
            return Ok(0);
        }
        self.limiter.flush(output)
    }

    /// Sets the peak limiter threshold ceiling in dBFS.
    ///
    /// Can be called before or after `enable_limiter` and between `render` calls without
    /// resetting the delay buffer or gain envelope state.
    ///
    /// # Arguments
    /// * `threshold_db` - Target threshold in dBFS; values outside `[-60.0, 0.0]` are clamped.
    pub fn set_limiter_threshold(&mut self, threshold_db: Decibels) -> Result<(), OarError> {
        self.limiter.set_threshold(threshold_db);
        Ok(())
    }

    /// Returns the limiter look-ahead delay in samples per channel.
    pub fn limiter_delay(&self) -> usize {
        self.limiter.delay()
    }

    /// Returns whether the output peak limiter is currently enabled.
    pub fn is_limiter_enabled(&self) -> bool {
        self.limiter_enabled
    }

    /// Enables or disables listener head tracking.
    pub fn enable_head_tracking(&mut self, enable: bool) -> Result<(), OarError> {
        if self.config.output_layout() != OutputLayout::Binaural {
            return Err(OarError::NotSupported);
        }
        self.head_tracking_enabled = enable;
        for renderers in self.group_renderers.values_mut() {
            for active_rdr in renderers {
                let _ = active_rdr.renderer.enable_head_tracking(enable);
            }
        }
        Ok(())
    }

    /// Sets listener head rotation quaternion.
    pub fn set_head_rotation(&mut self, rotation: Quaternion) -> Result<(), OarError> {
        if !self.head_tracking_enabled {
            // This error matches the original liboar behavior.
            return Err(OarError::Busy);
        }
        for renderers in self.group_renderers.values_mut() {
            for active_rdr in renderers {
                match active_rdr.renderer.set_head_rotation(rotation) {
                    Ok(()) | Err(OarError::NotSupported) => {}
                    Err(e) => return Err(e),
                }
            }
        }
        Ok(())
    }

    /// Enables or disables group loudness normalization.
    pub fn enable_loudness_processor(&mut self, enable: bool) -> Result<(), OarError> {
        self.loudness_processor_enabled = enable;
        Ok(())
    }

    /// Sets loudness adjustments for a group.
    pub fn set_loudness(
        &mut self,
        gid: GroupId,
        loudness: Decibels,
        target_loudness: Decibels,
    ) -> Result<(), OarError> {
        if !self.has_group(gid) {
            return Err(OarError::InvalidParameter);
        }
        let gain: LinearGain = (target_loudness - loudness).into();
        self.group_loudness.insert(gid, gain);
        Ok(())
    }

    /// Sets the metadata processing unit size in samples for object positions.
    ///
    /// # Errors
    ///
    /// Returns `OarError::InvalidParameter` if `samples` exceeds `samples_per_channel`.
    pub fn set_object_position_metadata_unit_to_process(
        &mut self,
        samples: Samples,
    ) -> Result<(), OarError> {
        if samples > self.config.samples_per_channel() {
            return Err(OarError::InvalidParameter);
        }
        for renderers in self.group_renderers.values_mut() {
            for rdr in renderers {
                rdr.renderer.set_metadata_unit_to_process(samples)?;
            }
        }
        self.object_position_metadata_samples = samples;
        Ok(())
    }

    fn post_process_render(
        &mut self,
        output: &mut PlanarBufferMut<'_, '_>,
    ) -> Result<(), OarError> {
        if self.limiter_enabled {
            self.limiter.process_block(output)?;
        }
        Ok(())
    }

    /// Instantiates a new, uninitialized `RoarRenderer`.
    ///
    /// # Errors
    ///
    /// Returns `OarError::InvalidParameter` if `sample_rate` or `samples_per_channel` is 0.
    pub fn create(config: &Config) -> Result<RoarRenderer, OarError> {
        let samples = config.samples_per_channel().value() as usize;
        let out_channels = config.output_layout().channels();
        let total_samples = out_channels.get() * samples;
        let limiter = OarLimiter::new(config.sample_rate(), out_channels)?;

        Ok(RoarRenderer {
            config: *config,
            status: Status::Configuring,
            active_elements: HashMap::new(),
            active_element_ids: Vec::new(),
            element_to_group: HashMap::new(),
            active_groups: Vec::new(),
            group_renderers: HashMap::new(),
            renderer_scratch: vec![0.0f32; total_samples],
            group_scratch: vec![0.0f32; total_samples],
            limiter_enabled: false,
            head_tracking_enabled: false,
            loudness_processor_enabled: false,
            group_loudness: HashMap::new(),
            limiter,
            element_gains: HashMap::new(),
            group_gains: HashMap::new(),
            gain_scratch_buf: vec![1.0f32; samples],
            object_position_metadata_samples: config.samples_per_channel(),
        })
    }

    fn is_binaural_route_for_config(&self, config: &AudioElementConfig) -> bool {
        if self.config.output_layout() != OutputLayout::Binaural {
            return false;
        }
        if let AudioElementConfig::ChannelBased(cbc) = config
            && let Some(rc) = cbc.rendering_config
            && rc.headphones_rendering_mode == HeadphonesRenderingMode::WorldLockedRestricted
        {
            return false;
        }
        true
    }

    fn is_binaural_route(&self, id: u32) -> Result<bool, OarError> {
        let config = self.active_elements.get(&id).ok_or(OarError::InvalidParameter)?;
        Ok(self.is_binaural_route_for_config(config))
    }

    /// Returns the loudspeaker target layout for an element rendered via an individual
    /// sub-renderer, or `None` when the element routes through the shared binaural OBR renderer.
    fn individual_target_layout_for_config(
        &self,
        config: &AudioElementConfig,
    ) -> Option<OutputLayout> {
        if self.is_binaural_route_for_config(config) {
            None
        } else if self.config.output_layout() == OutputLayout::Binaural {
            // Channel-based elements configured as `WorldLockedRestricted` on binaural output
            // render to stereo via an individual loudspeaker sub-renderer (Downmix or EAR).
            Some(OutputLayout::Stereo)
        } else {
            Some(self.config.output_layout())
        }
    }

    /// Returns whether `config` selects `DownmixRenderer` for `target_layout`.
    fn config_uses_downmix_renderer(
        config: &AudioElementConfig,
        target_layout: OutputLayout,
    ) -> bool {
        matches!(
            config,
            AudioElementConfig::ChannelBased(cbc)
                if target_layout != OutputLayout::Binaural
                    && cbc.downmix_info.is_some()
                    && cbc.layout.is_valid_downmix_to(target_layout)
        )
    }

    /// Creates and activates a new audio group (up to 2 active groups).
    ///
    /// # Errors
    ///
    /// Returns `OarError::InvalidParameter` if the maximum number of audio groups is exceeded.
    // TODO(b/525080422): The groups must be added before elements, so the API can be used wrong if
    // elements are added before adding groups, but it doesn't have to be that way.  I think get rid
    // of this method, create the group the first time an element is pushed to that group.
    pub fn add_audio_group(&mut self) -> Result<GroupId, OarError> {
        let new_id = GroupId::try_from(self.active_groups.len() as u32)?;
        self.active_groups.push(new_id);
        self.group_renderers.insert(new_id, Vec::new());
        Ok(new_id)
    }

    fn instantiate_sub_renderer(
        &self,
        id: u32,
        config: &AudioElementConfig,
        target_layout: OutputLayout,
    ) -> Result<Box<dyn AudioRenderer>, OarError> {
        let sub: Box<dyn AudioRenderer> = match config {
            AudioElementConfig::ChannelBased(cbc) => {
                if Self::config_uses_downmix_renderer(config, target_layout) {
                    let info = cbc.downmix_info.unwrap();
                    let mut dm = DownmixRenderer::new(cbc.layout, target_layout);
                    dm.set_mode_weight(info.mode(), info.weight_index())?;
                    Box::new(dm)
                } else {
                    Box::new(EarRenderer::with_output_layout(target_layout))
                }
            }
            AudioElementConfig::SceneBased(_) => {
                Box::new(EarRenderer::with_output_layout(target_layout))
            }
            AudioElementConfig::ObjectBased(_obc) => Box::new(ObjectRenderer::new(
                target_layout,
                self.config.sample_rate(),
                self.config.samples_per_channel(),
            )?),
        };

        let router = AudioElementsRenderer::with_backend(
            sub,
            target_layout.channels(),
            Some(self.object_position_metadata_samples),
        );

        let mut active_sub: Box<dyn AudioRenderer> = Box::new(router);
        active_sub.add_element(id, config)?;
        let _ = active_sub.enable_head_tracking(self.head_tracking_enabled);
        Ok(active_sub)
    }

    /// Configures an audio element configuration in the manager.
    ///
    /// # Errors
    ///
    /// Returns `OarError::InvalidParameter` if the group ID does not exist, or `OarError::Busy`
    /// if `id` is already registered.
    pub fn add_element(
        &mut self,
        group_id: GroupId,
        id: u32,
        config: &AudioElementConfig,
    ) -> Result<(), OarError> {
        if !self.has_group(group_id) {
            return Err(OarError::InvalidParameter);
        }
        if self.active_elements.contains_key(&id) {
            return Err(OarError::Busy);
        }

        match self.individual_target_layout_for_config(config) {
            None => {
                let renderers = self.group_renderers.entry(group_id).or_default();
                let shared_obr_idx = renderers.iter().position(|r| r.element_id.is_none());
                if let Some(idx) = shared_obr_idx {
                    renderers[idx].renderer.add_element(id, config)?;
                } else {
                    let obr = ObrAudioRendererAdapter::new(
                        self.object_position_metadata_samples,
                        self.config.sample_rate(),
                    );
                    let router = AudioElementsRenderer::with_backend(
                        Box::new(obr),
                        self.config.output_layout().channels(),
                        Some(self.object_position_metadata_samples),
                    );

                    let mut active_sub: Box<dyn AudioRenderer> = Box::new(router);
                    active_sub.add_element(id, config)?;
                    let _ = active_sub.enable_head_tracking(self.head_tracking_enabled);
                    renderers.push(ActiveRenderer { element_id: None, renderer: active_sub });
                }
            }
            Some(target_layout) => {
                let sub = self.instantiate_sub_renderer(id, config, target_layout)?;
                let renderers = self.group_renderers.entry(group_id).or_default();
                renderers.push(ActiveRenderer { element_id: Some(id), renderer: sub });
            }
        }

        self.active_elements.insert(id, *config);
        self.active_element_ids.push(id);
        self.element_to_group.insert(id, group_id);

        Ok(())
    }

    /// Removes an audio element from the active elements list.
    pub fn remove_element(&mut self, id: u32) -> Result<(), OarError> {
        if !self.active_elements.contains_key(&id) {
            return Err(OarError::InvalidParameter);
        }
        let is_obr_element = self.is_binaural_route(id)?;
        let group_id = self.element_to_group.get(&id).copied().ok_or(OarError::InvalidParameter)?;

        if let Some(renderers) = self.group_renderers.get_mut(&group_id) {
            if is_obr_element {
                if let Some(active_rdr) = renderers.iter_mut().find(|r| r.element_id.is_none()) {
                    active_rdr.renderer.remove_element(id)?;
                }
            } else {
                renderers.retain(|r| r.element_id != Some(id));
            }
        }

        self.active_elements.remove(&id);
        self.active_element_ids.retain(|&x| x != id);
        self.element_to_group.remove(&id);
        self.element_gains.remove(&id);

        Ok(())
    }

    /// Processes planar input channels and writes spatialized outputs.
    ///
    /// Adheres to zero-heap-allocation guidelines.
    pub fn render(
        &mut self,
        inputs: &[(u32, PlanarBufferRef<'_, '_>)],
        output: &mut PlanarBufferMut<'_, '_>,
    ) -> Result<(), OarError> {
        self.status = Status::Rendering;

        let samples = self.config.samples_per_channel().value() as usize;
        let out_channels = self.config.output_layout().channels().get();

        if output.num_channels() != out_channels || output.num_samples() != samples {
            return Err(OarError::InvalidParameter);
        }

        if self.active_element_ids.is_empty() || inputs.is_empty() {
            return Err(OarError::InvalidParameter);
        }

        // Validate that all active elements have inputs provided with correct channel count
        for &id in &self.active_element_ids {
            let config = self.active_elements.get(&id).ok_or(OarError::InvalidParameter)?;
            let expected_chans = config.channels().get();
            let elem_input = inputs
                .iter()
                .find(|(eid, _)| *eid == id)
                .map(|&(_, data)| data)
                .ok_or(OarError::InvalidParameter)?;
            if elem_input.num_channels() != expected_chans {
                return Err(OarError::InvalidParameter);
            }
        }

        output.fill(0.0);

        let active_elements = &self.active_elements;
        let element_to_group = &self.element_to_group;
        let output_layout = self.config.output_layout();

        for (&group_id, renderers) in &mut self.group_renderers {
            self.group_scratch.fill(0.0);

            for active_rdr in renderers {
                self.renderer_scratch.fill(0.0);
                let mut scratch_slices_array: [&mut [f32]; MAX_OUTPUT_CHANNELS] =
                    std::array::from_fn(|_| &mut [] as &mut [f32]);
                let mut chunks = self.renderer_scratch.chunks_exact_mut(samples);
                for slice in scratch_slices_array.iter_mut().take(out_channels) {
                    *slice = chunks.next().ok_or(OarError::InvalidParameter)?;
                }
                let mut renderer_scratch_buf = PlanarBufferMut::new(
                    &mut scratch_slices_array[..out_channels],
                    out_channels,
                    self.config.samples_per_channel(),
                )?;
                if let Some(elem_id) = active_rdr.element_id {
                    // Individual renderer (restricted element in binaural output, or loudspeaker
                    // output)
                    let elem_input = inputs
                        .iter()
                        .find(|(eid, _)| *eid == elem_id)
                        .map(|&(_, data)| data)
                        .ok_or(OarError::InvalidParameter)?;
                    active_rdr.renderer.render(elem_input, &mut renderer_scratch_buf)?;
                    if let Some(gains) = self.element_gains.get_mut(&elem_id) {
                        for param in gains.values_mut() {
                            generate_gain_multipliers(param, samples, &mut self.gain_scratch_buf);
                            for c in 0..out_channels {
                                let start = c * samples;
                                let end = start + samples;
                                for (sample, &gain) in self.renderer_scratch[start..end]
                                    .iter_mut()
                                    .zip(self.gain_scratch_buf.iter())
                                {
                                    *sample *= gain;
                                }
                            }
                        }
                    }
                } else {
                    // Shared OBR
                    // TODO(b/512062316): Don't we know how many input channels we have?
                    let mut group_inputs: [&[f32]; MAX_SUPPORTED_NUM_INPUT_CHANNELS] =
                        [&[]; MAX_SUPPORTED_NUM_INPUT_CHANNELS];
                    let mut input_idx = 0;

                    for &id in &self.active_element_ids {
                        let is_binaural_route = if output_layout != OutputLayout::Binaural {
                            false
                        } else {
                            let config =
                                active_elements.get(&id).ok_or(OarError::InvalidParameter)?;
                            if let AudioElementConfig::ChannelBased(cbc) = config {
                                if let Some(rc) = cbc.rendering_config {
                                    rc.headphones_rendering_mode
                                        != HeadphonesRenderingMode::WorldLockedRestricted
                                } else {
                                    true
                                }
                            } else {
                                true
                            }
                        };

                        if element_to_group.get(&id) == Some(&group_id) && is_binaural_route {
                            let elem_input = inputs
                                .iter()
                                .find(|(eid, _)| *eid == id)
                                .map(|&(_, data)| data)
                                .ok_or(OarError::InvalidParameter)?;
                            for c in 0..elem_input.num_channels() {
                                let chan = elem_input.channel(c);
                                if input_idx < MAX_SUPPORTED_NUM_INPUT_CHANNELS {
                                    group_inputs[input_idx] = chan;
                                    input_idx += 1;
                                } else {
                                    return Err(OarError::InvalidParameter);
                                }
                            }
                        }
                    }

                    if input_idx > 0 {
                        let active_inputs = &group_inputs[..input_idx];
                        let validated_inputs = PlanarBufferRef::new(
                            active_inputs,
                            input_idx,
                            self.config.samples_per_channel(),
                        )?;
                        active_rdr.renderer.render(validated_inputs, &mut renderer_scratch_buf)?;
                    }
                }

                for (dst, src) in self.group_scratch.iter_mut().zip(self.renderer_scratch.iter()) {
                    *dst += *src;
                }
            }

            // Apply group gain to group_scratch
            if let Some(param) = self.group_gains.get_mut(&group_id) {
                generate_gain_multipliers(param, samples, &mut self.gain_scratch_buf);
                for c in 0..out_channels {
                    let start = c * samples;
                    let end = start + samples;
                    for (sample, &gain) in
                        self.group_scratch[start..end].iter_mut().zip(self.gain_scratch_buf.iter())
                    {
                        *sample *= gain;
                    }
                }
            }

            if self.loudness_processor_enabled
                && let Some(&gain) = self.group_loudness.get(&group_id)
            {
                // Only apply if the gain is significantly different from 1.0.
                if (gain - 1.0).abs() > 1e-6 {
                    for sample in &mut self.group_scratch {
                        *sample *= gain;
                    }
                }
            }

            for out_idx in 0..out_channels {
                let out_slice = output.channel_mut(out_idx);
                let start = out_idx * samples;
                let src_slice = &self.group_scratch[start..start + samples];
                for (dst, src) in out_slice.iter_mut().zip(src_slice.iter()) {
                    *dst += *src;
                }
            }
        }

        // Post-process final output (limiter)
        self.post_process_render(output)?;

        Ok(())
    }

    /// Queues a dynamic gain update for an active audio element.
    ///
    /// # Parameters
    ///
    /// * `id`: Unique identifier of an active audio element.
    /// * `gain_id`: Identifier of the gain parameter stream on the element.
    /// * `gain`: Constant, per-sample, or animated gain curve in dB.
    /// * `duration`: Duration in samples over which `gain` applies.
    ///
    /// # Errors
    ///
    /// Returns `OarError::InvalidParameter` if `id` is not an active element.
    pub fn update_element_gain(
        &mut self,
        id: u32,
        gain_id: u32,
        gain: &Gain,
        duration: Samples,
    ) -> Result<(), OarError> {
        if !self.active_elements.contains_key(&id) {
            return Err(OarError::InvalidParameter);
        }
        let params = self.element_gains.entry(id).or_default();
        let param = params.entry(gain_id).or_insert_with(|| GainParameter::new(gain_id));
        param.updates.push_back(GainUpdate { gain: gain.variant.clone(), duration, elapsed: 0 });

        let group_id = self.element_to_group.get(&id).copied().ok_or(OarError::InvalidParameter)?;
        let is_obr_element = self.is_binaural_route(id)?;
        if is_obr_element
            && let Some(renderers) = self.group_renderers.get_mut(&group_id)
            && let Some(rdr) = renderers.iter_mut().find(|r| r.element_id.is_none())
        {
            rdr.renderer.update_element_gain(id, gain_id, gain, duration)?;
        }
        Ok(())
    }

    /// Queues dynamic 3D object positions for an active object-based audio element.
    ///
    /// # Parameters
    ///
    /// * `id`: Unique identifier of an active object-based audio element.
    /// * `positions`: Static or animated polar/Cartesian coordinates for the element's objects.
    /// * `duration`: Duration in samples over which `positions` applies.
    ///
    /// # Errors
    ///
    /// Returns `OarError::InvalidParameter` if `id` is not an active element, or
    /// `OarError::NotSupported` if `id` is not an object-based element.
    pub fn update_element_positions(
        &mut self,
        id: u32,
        positions: &ObjectPosition,
        duration: Samples,
    ) -> Result<(), OarError> {
        let config = self.active_elements.get(&id).ok_or(OarError::InvalidParameter)?;
        if !matches!(config, AudioElementConfig::ObjectBased(_)) {
            return Err(OarError::NotSupported);
        }
        let group_id = self.element_to_group.get(&id).copied().ok_or(OarError::InvalidParameter)?;
        let is_obr_element = self.is_binaural_route(id)?;
        if let Some(renderers) = self.group_renderers.get_mut(&group_id) {
            let active_rdr = if is_obr_element {
                renderers.iter_mut().find(|r| r.element_id.is_none())
            } else {
                renderers.iter_mut().find(|r| r.element_id == Some(id))
            };
            if let Some(rdr) = active_rdr {
                rdr.renderer.update_element_positions(id, positions, duration)?;
            }
        }
        Ok(())
    }

    /// Updates the dynamic IAMF downmix mode for an active audio element.
    ///
    /// Downmix mode updates are advisory hints: they take effect only when the element's active
    /// rendering path uses `DownmixRenderer`. When the element is rendered via another backend
    /// (such as EAR fallback, OLR, or binaural OBR), the update is accepted and ignored as a
    /// no-op (`Ok(())`).
    ///
    /// # Parameters
    ///
    /// * `id`: Unique identifier of an active audio element.
    /// * `mode`: Target IAMF downmix mode to apply when a downmix consumer is active.
    /// * `duration`: Duration in samples for which `mode` applies, or `None` to persist
    ///   indefinitely until the next downmix mode update.
    ///
    /// # Errors
    ///
    /// Returns `OarError::InvalidParameter` if `id` is not an active element.
    pub fn update_element_downmix_mode(
        &mut self,
        id: u32,
        mode: DownmixMode,
        duration: Option<Samples>,
    ) -> Result<(), OarError> {
        let is_obr_element = self.is_binaural_route(id)?;
        let group_id = self.element_to_group.get(&id).copied().ok_or(OarError::InvalidParameter)?;
        if let Some(renderers) = self.group_renderers.get_mut(&group_id) {
            let active_rdr = if is_obr_element {
                renderers.iter_mut().find(|r| r.element_id.is_none())
            } else {
                renderers.iter_mut().find(|r| r.element_id == Some(id))
            };
            if let Some(rdr) = active_rdr {
                match rdr.renderer.update_element_downmix_mode(id, mode, duration) {
                    // Downmix mode metadata is advisory: backends without a downmix stage report
                    // `NotSupported`, which is accepted and ignored as a no-op.
                    Ok(()) | Err(OarError::NotSupported) => {}
                    Err(e) => return Err(e),
                }
            }
        }
        Ok(())
    }

    /// Queues a dynamic gain update applied to all output channels of an active audio group.
    ///
    /// # Parameters
    ///
    /// * `gid`: Identifier of an active audio group.
    /// * `gain`: Constant, per-sample, or animated gain curve in dB.
    /// * `duration`: Duration in samples over which `gain` applies.
    ///
    /// # Errors
    ///
    /// Returns `OarError::InvalidParameter` if `gid` is not an active audio group.
    pub fn update_group_gain(
        &mut self,
        gid: GroupId,
        gain: &Gain,
        duration: Samples,
    ) -> Result<(), OarError> {
        if !self.active_groups.contains(&gid) {
            return Err(OarError::InvalidParameter);
        }
        let param = self.group_gains.entry(gid).or_insert_with(|| GainParameter::new(0));
        param.updates.push_back(GainUpdate { gain: gain.variant.clone(), duration, elapsed: 0 });
        Ok(())
    }

    /// Returns the total number of active audio elements.
    pub fn number_of_audio_elements(&self) -> usize {
        self.active_elements.len()
    }

    /// Returns the number of channels for a given element ID.
    pub fn element_channels(&self, id: u32) -> Option<ChannelCount> {
        self.active_elements.get(&id).map(|c| c.channels())
    }

    /// Returns whether the active rendering path for `id` consumes IAMF downmix mode updates.
    ///
    /// # Errors
    ///
    /// Returns `OarError::InvalidParameter` if `id` is not an active element.
    pub(crate) fn element_has_downmix_consumer(&self, id: u32) -> Result<bool, OarError> {
        let config = self.active_elements.get(&id).ok_or(OarError::InvalidParameter)?;
        Ok(self
            .individual_target_layout_for_config(config)
            .is_some_and(|target_layout| Self::config_uses_downmix_renderer(config, target_layout)))
    }
}
