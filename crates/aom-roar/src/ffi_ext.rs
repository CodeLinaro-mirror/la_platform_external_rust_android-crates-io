// Copyright (c) 2026, Alliance for Open Media. All rights reserved
//
// This source code is subject to the terms of the BSD 3-Clause Clear License
// and the Alliance for Open Media Patent License 1.0. If the BSD 3-Clause Clear
// License was not distributed with this source code in the LICENSE file, you
// can obtain it at www.aomedia.org/license/software-license/bsd-3-c-c. If the
// Alliance for Open Media Patent License 1.0 was not distributed with this
// source code in the PATENTS file, you can obtain it at
// www.aomedia.org/license/patent.

//! Translations between C and Rust types for the C API.

use crate::c_types::roar_audio_block_t;
use crate::c_types::roar_audio_element_config_t;

impl roar_audio_block_t {
    /// Views the planar audio block as a collection of safe, immutable float slices.
    ///
    /// # Returns
    /// A `Vec` of immutable slices, one for each channel. If `data` is null, or if
    /// `channels` or `samples_per_channel` are 0, an empty `Vec` is returned.
    ///
    /// # Safety
    /// The caller must guarantee that:
    /// - `self.data` is either null or points to a valid block of contiguous memory containing at
    ///   least `channels * samples_per_channel` elements of type `f32`.
    /// - The memory region remains valid and is not modified for the lifetime of the returned
    ///   slices.
    pub unsafe fn as_slices(&self) -> Vec<&[f32]> {
        if self.data.is_null() || self.channels == 0 || self.samples_per_channel == 0 {
            return Vec::new();
        }

        let total_samples = match self.channels.checked_mul(self.samples_per_channel) {
            Some(t) => t as usize,
            None => return Vec::new(),
        };

        // SAFETY:
        // - We checked that `self.data` is not null and computed a safe `total_samples`.
        // - The caller guarantees that `self.data` points to a block of memory containing at least
        //   `channels * samples_per_channel` valid float values.
        // - The memory region remains unmodified for the duration of the returned immutable slice.
        let full_slice =
            unsafe { std::slice::from_raw_parts(self.data as *const f32, total_samples) };

        full_slice.chunks_exact(self.samples_per_channel as usize).collect()
    }

    /// Views the planar audio block as a collection of safe, mutable maybe uninitialized float
    /// slices.
    ///
    /// # Returns
    /// A `Vec` of mutable slices, one for each channel. If `data` is null, or if
    /// `channels` or `samples_per_channel` are 0, an empty `Vec` is returned.
    ///
    /// # Safety
    /// The caller must guarantee that:
    /// - `self.data` is either null or points to a contiguous block of allocated memory containing
    ///   at least `channels * samples_per_channel` elements of size `f32`.
    /// - The memory region is not accessed via any other pointers or references for the lifetime of
    ///   the returned mutable slices.
    pub unsafe fn as_slices_mut(&mut self) -> Vec<&mut [std::mem::MaybeUninit<f32>]> {
        if self.data.is_null() || self.channels == 0 || self.samples_per_channel == 0 {
            return Vec::new();
        }

        let total_samples = match self.channels.checked_mul(self.samples_per_channel) {
            Some(t) => t as usize,
            None => return Vec::new(),
        };

        // SAFETY:
        // - We checked that `self.data` is not null and computed a safe `total_samples`.
        // - The caller guarantees that `self.data` points to an allocated block of memory
        //   containing at least `channels * samples_per_channel` float elements.
        // - The memory region is accessed exclusively through the returned mutable slice for its
        //   lifetime.
        let full_slice = unsafe {
            std::slice::from_raw_parts_mut(
                self.data as *mut std::mem::MaybeUninit<f32>,
                total_samples,
            )
        };

        full_slice.chunks_exact_mut(self.samples_per_channel as usize).collect()
    }
}

impl roar_audio_element_config_t {
    /// Translates a raw `*const roar_audio_element_config_t` pointer into a safe Rust
    /// `AudioElementConfig` without reading inactive union variants or unflagged parameter blocks.
    ///
    /// Reading fields selectively via raw pointer projections ensures that C callers leaving
    /// `parameters.downmix_info` or `parameters.element_rendering_config` uninitialized when their
    /// corresponding `ROAR_PARAMETER_SET_FLAG_*` bit is unset do not trigger Rust undefined
    /// behavior.
    ///
    /// # Arguments
    /// * `config` - Pointer to the C configuration struct; must be non-null and properly aligned.
    ///   Only `r#type`, `parameters.flags`, the union variant selected by `r#type`, and parameter
    ///   blocks whose flag bit is set in `parameters.flags` are read.
    ///
    /// # Returns
    /// - `Ok(AudioElementConfig)` if the translation is successful.
    /// - `Err(OarError::InvalidParameter)` if `config` is null or misaligned, or if the element
    ///   type or any active configuration field is unsupported, unrecognized, or out of range.
    ///
    /// # Safety
    /// The caller must guarantee that, when `config` is non-null and aligned:
    /// - `config` points to readable memory for a `roar_audio_element_config_t` for the duration of
    ///   this call, with `(*config).r#type`, `(*config).parameters.flags`, and the active union
    ///   variant (`cbc`, `sbc`, or `obc`) initialized.
    /// - If `ROAR_PARAMETER_SET_FLAG_IAMF_ELEMENT_RENDERING_CONFIG` is set in `parameters.flags`,
    ///   `(*config).parameters.element_rendering_config` is initialized.
    /// - If `r#type == ck_roar_channel_based` and `ROAR_PARAMETER_SET_FLAG_IAMF_DOWNMIX_INFO` is
    ///   set in `parameters.flags`, `(*config).parameters.downmix_info` is initialized.
    pub unsafe fn from_raw(
        config: *const Self,
    ) -> Result<crate::common::definitions::AudioElementConfig, crate::common::definitions::OarError>
    {
        use crate::c_types::{
            roar_audio_element_type_t, ROAR_PARAMETER_SET_FLAG_IAMF_DOWNMIX_INFO,
            ROAR_PARAMETER_SET_FLAG_IAMF_ELEMENT_RENDERING_CONFIG,
        };
        use crate::common::definitions::{
            AudioElementConfig, ChannelBasedConfig, DownmixInfo, ElementRenderingConfig,
            HighOrderAmbisonics, InputLayout, OarError, ObjectBasedConfig, SceneBasedConfig,
        };

        if config.is_null() || !config.is_aligned() {
            return Err(OarError::InvalidParameter);
        }

        // SAFETY: `config` was verified non-null and aligned above, and the caller guarantees
        // `r#type` and `parameters.flags` are initialized.
        let (element_type, flags) = unsafe {
            ((&raw const (*config).r#type).read(), (&raw const (*config).parameters.flags).read())
        };

        let rendering_config =
            if (flags & ROAR_PARAMETER_SET_FLAG_IAMF_ELEMENT_RENDERING_CONFIG) != 0 {
                // SAFETY: The `ROAR_PARAMETER_SET_FLAG_IAMF_ELEMENT_RENDERING_CONFIG` flag bit is
                // set, so the caller guarantees `element_rendering_config` is initialized.
                let raw_rendering_config =
                    unsafe { (&raw const (*config).parameters.element_rendering_config).read() };
                Some(ElementRenderingConfig::try_from(raw_rendering_config)?)
            } else {
                None
            };

        const CHANNEL_BASED: i32 = roar_audio_element_type_t::ck_roar_channel_based as i32;
        const SCENE_BASED: i32 = roar_audio_element_type_t::ck_roar_scene_based as i32;
        const OBJECT_BASED: i32 = roar_audio_element_type_t::ck_roar_object_based as i32;

        match element_type {
            CHANNEL_BASED => {
                // SAFETY: `element_type` is `ck_roar_channel_based`, so the caller guarantees the
                // `cbc` union variant is initialized.
                let cbc = unsafe { (&raw const (*config).config.cbc).read() };
                let layout = InputLayout::try_from(cbc.layout)?;
                let downmix_info = if (flags & ROAR_PARAMETER_SET_FLAG_IAMF_DOWNMIX_INFO) != 0 {
                    // SAFETY: `element_type` is `ck_roar_channel_based` and
                    // `ROAR_PARAMETER_SET_FLAG_IAMF_DOWNMIX_INFO` is set, so the caller guarantees
                    // `downmix_info` is initialized.
                    let raw_downmix_info =
                        unsafe { (&raw const (*config).parameters.downmix_info).read() };
                    Some(DownmixInfo::try_from(raw_downmix_info)?)
                } else {
                    None
                };
                Ok(AudioElementConfig::ChannelBased(ChannelBasedConfig {
                    layout,
                    downmix_info,
                    rendering_config,
                }))
            }
            SCENE_BASED => {
                // SAFETY: `element_type` is `ck_roar_scene_based`, so the caller guarantees the
                // `sbc` union variant is initialized.
                let sbc = unsafe { (&raw const (*config).config.sbc).read() };
                let order = HighOrderAmbisonics::try_from(sbc.order)?;

                Ok(AudioElementConfig::SceneBased(SceneBasedConfig { order, rendering_config }))
            }
            OBJECT_BASED => {
                // SAFETY: `element_type` is `ck_roar_object_based`, so the caller guarantees the
                // `obc` union variant is initialized.
                let obc = unsafe { (&raw const (*config).config.obc).read() };
                let num_objects =
                    usize::try_from(obc.num_objects).map_err(|_| OarError::InvalidParameter)?;
                Ok(AudioElementConfig::ObjectBased(ObjectBasedConfig::new(
                    num_objects,
                    rendering_config,
                )?))
            }
            _ => Err(OarError::InvalidParameter),
        }
    }

    /// Translates the C-compatible `roar_audio_element_config_t` configuration into a safe
    /// Rust `AudioElementConfig` representation.
    ///
    /// # Returns
    /// - `Ok(AudioElementConfig)` if the translation is successful.
    /// - `Err(OarError::InvalidParameter)` if the element type is unsupported, unrecognized, or if
    ///   an object-based configuration does not specify between 1 and 2 objects.
    pub fn to_safe(
        &self,
    ) -> Result<crate::common::definitions::AudioElementConfig, crate::common::definitions::OarError>
    {
        // SAFETY: `self` is a valid, aligned reference whose active fields are initialized.
        unsafe { Self::from_raw(self) }
    }
}

impl crate::c_types::roar_metadata_t {
    /// Extracts head rotation from C-compatible `roar_metadata_t`.
    pub fn to_head_rotation(
        &self,
    ) -> Result<crate::common::definitions::Quaternion, crate::common::definitions::OarError> {
        use crate::common::definitions::{OarError, Quaternion};
        match self.r#type {
            4 => {
                // SAFETY:
                // Accessing the `head_rotation` union field is safe because we have verified that
                // `self.type` is 4 (corresponding to `ck_roar_metadata_head_rotation`), meaning
                // this variant of the union is active.
                let q = unsafe { self.value.head_rotation };
                Quaternion::new(q.w, q.x, q.y, q.z)
            }
            _ => Err(OarError::NotSupported),
        }
    }
}

impl crate::c_types::roar_metadata_t {
    /// Extracts gain metadata from a C-compatible `roar_metadata_t`.
    ///
    /// # Returns
    /// - `Ok(Gain)` if `self.r#type` is `ck_roar_metadata_gain` and the active gain payload is
    ///   valid.
    /// - `Err(OarError::InvalidParameter)` if `self.r#type` is not `ck_roar_metadata_gain`, if
    ///   `param_type` is unrecognized, if `gain_array` is null or misaligned, if `duration <= 0` or
    ///   exceeds `isize::MAX / size_of::<f32>()` for `ck_roar_param_multiple`, or if any gain or
    ///   animation parameter is out of range or non-finite.
    ///
    /// # Safety
    /// The caller must guarantee that, when
    /// `self.r#type == roar_metadata_type_t::ck_roar_metadata_gain as u32`:
    /// - `self.value.gain.param_type` is initialized, and if it equals `ck_roar_param_constant`,
    ///   `ck_roar_param_multiple`, or `ck_roar_param_animated`, the corresponding
    ///   `self.value.gain.value` union field (`constant_gain`, `gain_array`, or `animated_gains`)
    ///   is initialized.
    /// - When `self.value.gain.param_type == roar_param_type_t::ck_roar_param_multiple as u32`,
    ///   `self.duration > 0`, and `self.value.gain.value.gain_array` is non-null and properly
    ///   aligned for `f32`:
    ///   - `gain_array` points to a single live allocated object containing at least `self.duration
    ///     as usize` consecutive, properly initialized `f32` elements (`(self.duration as usize) *
    ///     std::mem::size_of::<f32>()` bytes) without address-space wraparound.
    ///   - `gain_array` is valid for reads of those `(self.duration as usize) *
    ///     std::mem::size_of::<f32>()` bytes for the duration of this call (the borrow of `self`),
    ///     and the memory range is not mutated concurrently for the duration of this call.
    pub(crate) unsafe fn to_gain(
        &self,
    ) -> Result<crate::common::definitions::Gain, crate::common::definitions::OarError> {
        use crate::c_types::roar_metadata_type_t;
        use crate::c_types::roar_param_type_t;
        use crate::common::definitions::{AnimatedFloat32, Decibels, Gain, OarError};

        if self.r#type != (roar_metadata_type_t::ck_roar_metadata_gain as u32) {
            return Err(OarError::InvalidParameter);
        }

        const CONSTANT: u32 = roar_param_type_t::ck_roar_param_constant as u32;
        const MULTIPLE: u32 = roar_param_type_t::ck_roar_param_multiple as u32;
        const ANIMATED: u32 = roar_param_type_t::ck_roar_param_animated as u32;

        // SAFETY: `self.r#type` was verified to be `ck_roar_metadata_gain`, so the caller
        // guarantees `self.value.gain.param_type` is initialized. Reading via pointer projection
        // avoids copying inactive union padding by value.
        let param_type = unsafe { (&raw const self.value.gain.param_type).read() };
        match param_type {
            CONSTANT => {
                // SAFETY: `param_type` was verified to be `ck_roar_param_constant`, so the caller
                // guarantees `self.value.gain.value.constant_gain` is initialized.
                let db_gain = unsafe { (&raw const self.value.gain.value.constant_gain).read() };
                Gain::new_constant(Decibels::new(db_gain)?)
            }
            MULTIPLE => {
                // SAFETY: `param_type` was verified to be `ck_roar_param_multiple`, so the caller
                // guarantees `self.value.gain.value.gain_array` is initialized.
                let gain_ptr = unsafe { (&raw const self.value.gain.value.gain_array).read() };
                if gain_ptr.is_null() || !gain_ptr.is_aligned() || self.duration <= 0 {
                    return Err(OarError::InvalidParameter);
                }
                let len = self.duration as usize;
                if len > isize::MAX as usize / std::mem::size_of::<f32>() {
                    return Err(OarError::InvalidParameter);
                }
                // SAFETY:
                // - `gain_ptr` was verified non-null and properly aligned for `f32`, and `len` was
                //   verified to satisfy `len * size_of::<f32>() <= isize::MAX as usize`.
                // - The caller guarantees that `gain_ptr` points to a single live allocation of at
                //   least `len` consecutive, properly initialized `f32` elements without
                //   address-space wraparound, valid for reads and not mutated concurrently for the
                //   duration of this call.
                let db_gains = unsafe { std::slice::from_raw_parts(gain_ptr, len) };
                let decibel_gains =
                    db_gains.iter().map(|&g| Decibels::new(g)).collect::<Result<Vec<_>, _>>()?;
                Gain::new_multiple(decibel_gains)
            }
            ANIMATED => {
                // SAFETY: `param_type` was verified to be `ck_roar_param_animated`, so the caller
                // guarantees `self.value.gain.value.animated_gains` is initialized.
                let anim_gain = unsafe { (&raw const self.value.gain.value.animated_gains).read() };
                let safe_anim = AnimatedFloat32::try_from(anim_gain)?;
                Gain::new_animated(safe_anim)
            }
            _ => Err(OarError::InvalidParameter),
        }
    }

    /// Extracts 3D object positions from a C-compatible `roar_metadata_t`.
    ///
    /// # Returns
    /// - `Ok(ObjectPosition)` if `self.r#type` is `ck_roar_metadata_object_positions` and the
    ///   active object position payload is valid.
    /// - `Err(OarError::InvalidParameter)` if `self.r#type` is not
    ///   `ck_roar_metadata_object_positions`, if `num_objects` exceeds `DEF_MAX_NUMBER_OF_OBJECTS`,
    ///   if `param_type` or `position_type` is unrecognized, or if any coordinate or animation
    ///   parameter is out of range or non-finite.
    /// - `Err(OarError::NotSupported)` if `param_type` is `ck_roar_param_multiple`.
    ///
    /// # Safety
    /// The caller must guarantee that, when
    /// `self.r#type == roar_metadata_type_t::ck_roar_metadata_object_positions as u32`:
    /// - `self.value.object_positions.param_type`, `self.value.object_positions.position_type`, and
    ///   `self.value.object_positions.num_objects` are initialized.
    /// - When `num_objects as usize <= crate::common::definitions::DEF_MAX_NUMBER_OF_OBJECTS` and
    ///   `(param_type, position_type)` selects a supported variant (`ck_roar_param_constant` or
    ///   `ck_roar_param_animated` paired with `ck_roar_polar` or `ck_roar_cartesian`), the first
    ///   `num_objects as usize` elements of the corresponding
    ///   `self.value.object_positions.positions` union array (`polar_positions`,
    ///   `cartesian_positions`, `animated_polar_positions`, or `animated_cartesian_positions`) are
    ///   properly initialized and valid for reads for the duration of this call (the borrow of
    ///   `self`).
    pub(crate) unsafe fn to_positions(
        &self,
    ) -> Result<crate::common::definitions::ObjectPosition, crate::common::definitions::OarError>
    {
        use crate::c_types::roar_coordinate_type_t;
        use crate::c_types::roar_metadata_type_t;
        use crate::c_types::roar_param_type_t;
        use crate::common::definitions::{
            AnimatedCartesian, AnimatedPolar, CartesianCoordinate, OarError, ObjectPosition,
            PolarCoordinate,
        };

        if self.r#type != (roar_metadata_type_t::ck_roar_metadata_object_positions as u32) {
            return Err(OarError::InvalidParameter);
        }

        const CONSTANT: u32 = roar_param_type_t::ck_roar_param_constant as u32;
        const MULTIPLE: u32 = roar_param_type_t::ck_roar_param_multiple as u32;
        const ANIMATED: u32 = roar_param_type_t::ck_roar_param_animated as u32;
        const POLAR: u32 = roar_coordinate_type_t::ck_roar_polar as u32;
        const CARTESIAN: u32 = roar_coordinate_type_t::ck_roar_cartesian as u32;

        // SAFETY: `self.r#type` was verified to be `ck_roar_metadata_object_positions`, so the
        // caller guarantees `param_type`, `position_type`, and `num_objects` are initialized.
        // Reading via pointer projection avoids copying uninitialized trailing elements in
        // `positions`.
        let (param_type, position_type, num_objects_raw) = unsafe {
            (
                (&raw const self.value.object_positions.param_type).read(),
                (&raw const self.value.object_positions.position_type).read(),
                (&raw const self.value.object_positions.num_objects).read(),
            )
        };
        let num_objects = num_objects_raw as usize;
        if num_objects > crate::common::definitions::DEF_MAX_NUMBER_OF_OBJECTS {
            return Err(OarError::InvalidParameter);
        }

        match param_type {
            CONSTANT => match position_type {
                POLAR => {
                    // SAFETY: `self` is a valid, aligned reference and `&raw const` projects to the
                    // `polar_positions` union field without reading uninitialized bytes or forming
                    // an intermediate reference. Reading only the first `num_objects` entries
                    // avoids reading uninitialized trailing slots.
                    let polar_ptr = unsafe {
                        (&raw const self.value.object_positions.positions.polar_positions)
                            .cast::<crate::c_types::roar_polar_t>()
                    };
                    let mut safe_positions = Vec::with_capacity(num_objects);
                    for i in 0..num_objects {
                        // SAFETY: `i < num_objects <= DEF_MAX_NUMBER_OF_OBJECTS` (2), so
                        // `polar_ptr.add(i)` stays within the `[roar_polar_t; 2]` field of `self`
                        // without wraparound and is properly aligned. The caller guarantees the
                        // first `num_objects` polar entries are initialized and valid for reads.
                        let p = unsafe { polar_ptr.add(i).read() };
                        safe_positions.push(PolarCoordinate::new_from_floats(
                            p.azimuth,
                            p.elevation,
                            p.distance,
                        )?);
                    }
                    Ok(ObjectPosition::Polar(safe_positions))
                }
                CARTESIAN => {
                    // SAFETY: `self` is a valid, aligned reference and `&raw const` projects to the
                    // `cartesian_positions` union field without reading uninitialized bytes or
                    // forming an intermediate reference.
                    let cartesian_ptr = unsafe {
                        (&raw const self.value.object_positions.positions.cartesian_positions)
                            .cast::<crate::c_types::roar_cartesian_t>()
                    };
                    let mut safe_positions = Vec::with_capacity(num_objects);
                    for i in 0..num_objects {
                        // SAFETY: `i < num_objects <= DEF_MAX_NUMBER_OF_OBJECTS` (2), so
                        // `cartesian_ptr.add(i)` stays within the `[roar_cartesian_t; 2]` field of
                        // `self` without wraparound and is properly aligned. The caller guarantees
                        // the first `num_objects` Cartesian entries are initialized and valid for
                        // reads.
                        let c = unsafe { cartesian_ptr.add(i).read() };
                        safe_positions.push(CartesianCoordinate::new(c.x, c.y, c.z)?);
                    }
                    Ok(ObjectPosition::Cartesian(safe_positions))
                }
                _ => Err(OarError::InvalidParameter),
            },
            ANIMATED => match position_type {
                POLAR => {
                    // SAFETY: `self` is a valid, aligned reference and `&raw const` projects to the
                    // `animated_polar_positions` union field without reading uninitialized bytes or
                    // forming an intermediate reference.
                    let anim_polar_ptr = unsafe {
                        (&raw const self.value.object_positions.positions.animated_polar_positions)
                            .cast::<crate::c_types::roar_animated_polar_t>()
                    };
                    let mut safe_positions = Vec::with_capacity(num_objects);
                    for i in 0..num_objects {
                        // SAFETY: `i < num_objects <= DEF_MAX_NUMBER_OF_OBJECTS` (2), so
                        // `anim_polar_ptr.add(i)` stays within the `[roar_animated_polar_t; 2]`
                        // field of `self` without wraparound and is properly aligned. The caller
                        // guarantees the first `num_objects` animated polar entries are
                        // initialized and valid for reads.
                        let ap = unsafe { anim_polar_ptr.add(i).read() };
                        safe_positions.push(AnimatedPolar::try_from(ap)?);
                    }
                    Ok(ObjectPosition::AnimatedPolar(safe_positions))
                }
                CARTESIAN => {
                    // SAFETY: `self` is a valid, aligned reference and `&raw const` projects to the
                    // `animated_cartesian_positions` union field without reading uninitialized
                    // bytes or forming an intermediate reference.
                    let anim_cart_ptr = unsafe {
                        (&raw const self
                            .value
                            .object_positions
                            .positions
                            .animated_cartesian_positions)
                            .cast::<crate::c_types::roar_animated_cartesian_t>()
                    };
                    let mut safe_positions = Vec::with_capacity(num_objects);
                    for i in 0..num_objects {
                        // SAFETY: `i < num_objects <= DEF_MAX_NUMBER_OF_OBJECTS` (2), so
                        // `anim_cart_ptr.add(i)` stays within the `[roar_animated_cartesian_t; 2]`
                        // field of `self` without wraparound and is properly aligned. The caller
                        // guarantees the first `num_objects` animated Cartesian entries are
                        // initialized and valid for reads.
                        let ac = unsafe { anim_cart_ptr.add(i).read() };
                        safe_positions.push(AnimatedCartesian::try_from(ac)?);
                    }
                    Ok(ObjectPosition::AnimatedCartesian(safe_positions))
                }
                _ => Err(OarError::InvalidParameter),
            },
            MULTIPLE => Err(OarError::NotSupported),
            _ => Err(OarError::InvalidParameter),
        }
    }

    pub(crate) fn to_downmix_mode(
        &self,
    ) -> Result<crate::common::definitions::DownmixMode, crate::common::definitions::OarError> {
        use crate::c_types::roar_metadata_type_t;
        use crate::common::definitions::{DownmixMode, OarError};
        if self.r#type != (roar_metadata_type_t::ck_roar_metadata_iamf_downmix_mode as u32) {
            return Err(OarError::InvalidParameter);
        }
        // SAFETY: Accessing the `iamf_downmix_mode` union field is safe because we have verified
        // that `self.type` is `ck_roar_metadata_iamf_downmix_mode`.
        let mode = unsafe { self.value.iamf_downmix_mode.mode };
        DownmixMode::try_from(mode)
    }
}

impl TryFrom<i32> for crate::common::definitions::HighOrderAmbisonics {
    type Error = crate::common::definitions::OarError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        use crate::c_types::roar_hoa_t;
        use crate::common::definitions::{HighOrderAmbisonics, OarError};

        const ZOA: i32 = roar_hoa_t::ck_roar_zoa as i32;
        const OA1: i32 = roar_hoa_t::ck_roar_1oa as i32;
        const OA2: i32 = roar_hoa_t::ck_roar_2oa as i32;
        const OA3: i32 = roar_hoa_t::ck_roar_3oa as i32;
        const OA4: i32 = roar_hoa_t::ck_roar_4oa as i32;

        match value {
            ZOA => Ok(HighOrderAmbisonics::Zoa),
            OA1 => Ok(HighOrderAmbisonics::Order1),
            OA2 => Ok(HighOrderAmbisonics::Order2),
            OA3 => Ok(HighOrderAmbisonics::Order3),
            OA4 => Ok(HighOrderAmbisonics::Order4),
            _ => Err(OarError::InvalidParameter),
        }
    }
}

impl TryFrom<crate::c_types::roar_polar_t> for crate::common::definitions::PolarCoordinate {
    type Error = crate::common::definitions::OarError;

    fn try_from(c: crate::c_types::roar_polar_t) -> Result<Self, Self::Error> {
        crate::common::definitions::PolarCoordinate::new_from_floats(
            c.azimuth,
            c.elevation,
            c.distance,
        )
    }
}

impl TryFrom<crate::c_types::roar_cartesian_t> for crate::common::definitions::CartesianCoordinate {
    type Error = crate::common::definitions::OarError;

    fn try_from(c: crate::c_types::roar_cartesian_t) -> Result<Self, Self::Error> {
        crate::common::definitions::CartesianCoordinate::new(c.x, c.y, c.z)
    }
}

impl TryFrom<crate::c_types::roar_quaternion_t> for crate::common::definitions::Quaternion {
    type Error = crate::common::definitions::OarError;

    fn try_from(c: crate::c_types::roar_quaternion_t) -> Result<Self, Self::Error> {
        crate::common::definitions::Quaternion::new(c.w, c.x, c.y, c.z)
    }
}

impl TryFrom<i32> for crate::common::definitions::OutputLayout {
    type Error = crate::common::definitions::OarError;

    fn try_from(layout: i32) -> Result<Self, Self::Error> {
        use crate::c_types::roar_layout_t;
        use crate::common::definitions::{OarError, OutputLayout};

        const MONO: i32 = roar_layout_t::ck_roar_layout_mono as i32;
        const STEREO: i32 = roar_layout_t::ck_roar_layout_stereo as i32;
        const L51: i32 = roar_layout_t::ck_roar_layout_51 as i32;
        const L512: i32 = roar_layout_t::ck_roar_layout_512 as i32;
        const L514: i32 = roar_layout_t::ck_roar_layout_514 as i32;
        const L71: i32 = roar_layout_t::ck_roar_layout_71 as i32;
        const L712: i32 = roar_layout_t::ck_roar_layout_712 as i32;
        const L714: i32 = roar_layout_t::ck_roar_layout_714 as i32;
        const L312: i32 = roar_layout_t::ck_roar_layout_312 as i32;
        const L916: i32 = roar_layout_t::ck_roar_layout_916 as i32;
        const LA293: i32 = roar_layout_t::ck_roar_layout_a293 as i32;
        const L7154: i32 = roar_layout_t::ck_roar_layout_7154 as i32;
        const E451: i32 = roar_layout_t::ck_roar_layout_sound_system_e_451 as i32;
        const F370: i32 = roar_layout_t::ck_roar_layout_sound_system_f_370 as i32;
        const G490: i32 = roar_layout_t::ck_roar_layout_sound_system_g_490 as i32;
        const BINAURAL: i32 = roar_layout_t::ck_roar_layout_binaural as i32;

        match layout {
            MONO => Ok(OutputLayout::Mono),
            STEREO => Ok(OutputLayout::Stereo),
            L51 => Ok(OutputLayout::Layout51),
            L512 => Ok(OutputLayout::Layout512),
            L514 => Ok(OutputLayout::Layout514),
            L71 => Ok(OutputLayout::Layout71),
            L712 => Ok(OutputLayout::Layout712),
            L714 => Ok(OutputLayout::Layout714),
            L312 => Ok(OutputLayout::Layout312),
            L916 => Ok(OutputLayout::Layout916),
            LA293 => Ok(OutputLayout::LayoutA293),
            L7154 => Ok(OutputLayout::Layout7154),
            E451 => Ok(OutputLayout::SoundSystemE451),
            F370 => Ok(OutputLayout::SoundSystemF370),
            G490 => Ok(OutputLayout::SoundSystemG490),
            BINAURAL => Ok(OutputLayout::Binaural),
            _ => Err(OarError::InvalidParameter),
        }
    }
}

impl TryFrom<i32> for crate::common::definitions::InputLayout {
    type Error = crate::common::definitions::OarError;

    fn try_from(layout: i32) -> Result<Self, Self::Error> {
        use crate::c_types::roar_layout_t;
        use crate::common::definitions::{InputLayout, OarError};

        const MONO: i32 = roar_layout_t::ck_roar_layout_mono as i32;
        const STEREO: i32 = roar_layout_t::ck_roar_layout_stereo as i32;
        const L51: i32 = roar_layout_t::ck_roar_layout_51 as i32;
        const L512: i32 = roar_layout_t::ck_roar_layout_512 as i32;
        const L514: i32 = roar_layout_t::ck_roar_layout_514 as i32;
        const L71: i32 = roar_layout_t::ck_roar_layout_71 as i32;
        const L712: i32 = roar_layout_t::ck_roar_layout_712 as i32;
        const L714: i32 = roar_layout_t::ck_roar_layout_714 as i32;
        const L312: i32 = roar_layout_t::ck_roar_layout_312 as i32;
        const L916: i32 = roar_layout_t::ck_roar_layout_916 as i32;
        const LA293: i32 = roar_layout_t::ck_roar_layout_a293 as i32;
        const L7154: i32 = roar_layout_t::ck_roar_layout_7154 as i32;
        const BINAURAL: i32 = roar_layout_t::ck_roar_layout_binaural as i32;
        const LFE: i32 = roar_layout_t::ck_roar_layout_lfe as i32;
        const STEREOS: i32 = roar_layout_t::ck_roar_layout_stereo_s as i32;
        const STEREOSS: i32 = roar_layout_t::ck_roar_layout_stereo_ss as i32;
        const STEREO_RS: i32 = roar_layout_t::ck_roar_layout_stereo_rs as i32;
        const STEREO_TF: i32 = roar_layout_t::ck_roar_layout_stereo_tf as i32;
        const STEREO_TB: i32 = roar_layout_t::ck_roar_layout_stereo_tb as i32;
        const TOP4CH: i32 = roar_layout_t::ck_roar_layout_top_4ch as i32;
        const THREECH: i32 = roar_layout_t::ck_roar_layout_3ch as i32;
        const STEREO_F: i32 = roar_layout_t::ck_roar_layout_stereo_f as i32;
        const STEREO_SI: i32 = roar_layout_t::ck_roar_layout_stereo_si as i32;
        const STEREO_TPSI: i32 = roar_layout_t::ck_roar_layout_stereo_tpsi as i32;
        const TOP6CH: i32 = roar_layout_t::ck_roar_layout_top_6ch as i32;
        const LFE_PAIR: i32 = roar_layout_t::ck_roar_layout_lfe_pair as i32;
        const BOTTOM3CH: i32 = roar_layout_t::ck_roar_layout_bottom_3ch as i32;
        const BOTTOM4CH: i32 = roar_layout_t::ck_roar_layout_bottom_4ch as i32;
        const TOP1CH: i32 = roar_layout_t::ck_roar_layout_top_1ch as i32;
        const TOP5CH: i32 = roar_layout_t::ck_roar_layout_top_5ch as i32;

        match layout {
            MONO => Ok(InputLayout::Mono),
            STEREO => Ok(InputLayout::Stereo),
            L51 => Ok(InputLayout::Layout51),
            L512 => Ok(InputLayout::Layout512),
            L514 => Ok(InputLayout::Layout514),
            L71 => Ok(InputLayout::Layout71),
            L712 => Ok(InputLayout::Layout712),
            L714 => Ok(InputLayout::Layout714),
            L312 => Ok(InputLayout::Layout312),
            L916 => Ok(InputLayout::Layout916),
            LA293 => Ok(InputLayout::LayoutA293),
            L7154 => Ok(InputLayout::Layout7154),
            BINAURAL => Ok(InputLayout::Binaural),
            LFE => Ok(InputLayout::Lfe),
            STEREOS => Ok(InputLayout::StereoS),
            STEREOSS => Ok(InputLayout::StereoSs),
            STEREO_RS => Ok(InputLayout::StereoRs),
            STEREO_TF => Ok(InputLayout::StereoTf),
            STEREO_TB => Ok(InputLayout::StereoTb),
            TOP4CH => Ok(InputLayout::Top4ch),
            THREECH => Ok(InputLayout::ThreeCh),
            STEREO_F => Ok(InputLayout::StereoF),
            STEREO_SI => Ok(InputLayout::StereoSi),
            STEREO_TPSI => Ok(InputLayout::StereoTpsi),
            TOP6CH => Ok(InputLayout::Top6ch),
            LFE_PAIR => Ok(InputLayout::LfePair),
            BOTTOM3CH => Ok(InputLayout::Bottom3ch),
            BOTTOM4CH => Ok(InputLayout::Bottom4ch),
            TOP1CH => Ok(InputLayout::Top1ch),
            TOP5CH => Ok(InputLayout::Top5ch),
            _ => Err(OarError::InvalidParameter),
        }
    }
}

impl TryFrom<crate::c_types::roar_config_t> for crate::common::definitions::Config {
    type Error = crate::common::definitions::OarError;
    fn try_from(c: crate::c_types::roar_config_t) -> Result<Self, Self::Error> {
        use crate::common::definitions::{Config, OutputLayout, SampleRate, Samples};
        let target_layout = OutputLayout::try_from(c.target_layout)?;
        let spc = Samples::new(c.samples_per_channel)?;
        let sr = SampleRate::new(c.sampling_rate)?;
        Config::new(target_layout, spc, sr)
    }
}

impl TryFrom<i32> for crate::common::definitions::DownmixMode {
    type Error = crate::common::definitions::OarError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        use crate::common::definitions::{DownmixMode, OarError};
        match value {
            0 => Ok(DownmixMode::Mode1NegOffset),
            1 => Ok(DownmixMode::Mode2NegOffset),
            2 => Ok(DownmixMode::Mode3NegOffset),
            4 => Ok(DownmixMode::Mode1PosOffset),
            5 => Ok(DownmixMode::Mode2PosOffset),
            6 => Ok(DownmixMode::Mode3PosOffset),
            _ => Err(OarError::InvalidParameter),
        }
    }
}

impl TryFrom<crate::c_types::roar_downmix_info_t> for crate::common::definitions::DownmixInfo {
    type Error = crate::common::definitions::OarError;

    fn try_from(c: crate::c_types::roar_downmix_info_t) -> Result<Self, Self::Error> {
        use crate::common::definitions::{DownmixInfo, DownmixMode, WeightIndex};
        let mode = DownmixMode::try_from(c.mode)?;
        let weight_index = if (0..=10).contains(&c.weight_index) {
            Some(WeightIndex::new(c.weight_index)?)
        } else {
            None
        };
        Ok(DownmixInfo::new(mode, weight_index))
    }
}

impl TryFrom<u32> for crate::common::definitions::HeadphonesRenderingMode {
    type Error = crate::common::definitions::OarError;

    fn try_from(val: u32) -> Result<Self, Self::Error> {
        use crate::c_types::roar_headphones_rendering_mode_t;
        use crate::common::definitions::{HeadphonesRenderingMode, OarError};

        const WORLD_LOCKED_RESTRICTED: u32 =
            roar_headphones_rendering_mode_t::ck_roar_world_locked_restricted as u32;
        const WORLD_LOCKED: u32 = roar_headphones_rendering_mode_t::ck_roar_world_locked as u32;
        const HEAD_LOCKED: u32 = roar_headphones_rendering_mode_t::ck_roar_head_locked as u32;
        const RESERVED: u32 = roar_headphones_rendering_mode_t::ck_roar_reserved as u32;

        match val {
            WORLD_LOCKED_RESTRICTED => Ok(HeadphonesRenderingMode::WorldLockedRestricted),
            WORLD_LOCKED => Ok(HeadphonesRenderingMode::WorldLocked),
            HEAD_LOCKED => Ok(HeadphonesRenderingMode::HeadLocked),
            RESERVED => Ok(HeadphonesRenderingMode::Reserved),
            _ => Err(OarError::InvalidParameter),
        }
    }
}

impl TryFrom<u32> for crate::common::definitions::BinauralFilterProfile {
    type Error = crate::common::definitions::OarError;

    fn try_from(val: u32) -> Result<Self, Self::Error> {
        use crate::c_types::roar_binaural_filter_profile_t;
        use crate::common::definitions::{BinauralFilterProfile, OarError};

        const AMBIENT: u32 = roar_binaural_filter_profile_t::ck_roar_ambient as u32;
        const DIRECT: u32 = roar_binaural_filter_profile_t::ck_roar_direct as u32;
        const REVERBERANT: u32 = roar_binaural_filter_profile_t::ck_roar_reverberant as u32;

        match val {
            AMBIENT => Ok(BinauralFilterProfile::Ambient),
            DIRECT => Ok(BinauralFilterProfile::Direct),
            REVERBERANT => Ok(BinauralFilterProfile::Reverberant),
            _ => Err(OarError::InvalidParameter),
        }
    }
}

impl TryFrom<crate::c_types::roar_audio_element_rendering_config_t>
    for crate::common::definitions::ElementRenderingConfig
{
    type Error = crate::common::definitions::OarError;

    fn try_from(
        c: crate::c_types::roar_audio_element_rendering_config_t,
    ) -> Result<Self, Self::Error> {
        use crate::common::definitions::{
            BinauralFilterProfile, ElementRenderingConfig, HeadphonesRenderingMode,
        };
        let headphones_rendering_mode =
            HeadphonesRenderingMode::try_from(c.headphones_rendering_mode)?;
        let binaural_filter_profile = BinauralFilterProfile::try_from(c.binaural_filter_profile)?;
        Ok(ElementRenderingConfig { headphones_rendering_mode, binaural_filter_profile })
    }
}

impl TryFrom<crate::c_types::roar_animated_float32_t>
    for crate::common::definitions::AnimatedFloat32
{
    type Error = crate::common::definitions::OarError;

    fn try_from(c: crate::c_types::roar_animated_float32_t) -> Result<Self, Self::Error> {
        use crate::c_types::roar_animation_type_t;
        use crate::common::definitions::{Animated, OarError};

        const STEP: u32 = roar_animation_type_t::ck_roar_animation_type_step as u32;
        const LINEAR: u32 = roar_animation_type_t::ck_roar_animation_type_linear as u32;
        const BEZIER: u32 = roar_animation_type_t::ck_roar_animation_type_bezier as u32;

        let d = c.data;
        if !d.start.is_finite() {
            return Err(OarError::InvalidParameter);
        }

        match c.animation_type {
            STEP => Ok(Animated::Step { value: d.start }),
            LINEAR => {
                if !d.end.is_finite() {
                    return Err(OarError::InvalidParameter);
                }
                Ok(Animated::Linear { start: d.start, end: d.end })
            }
            BEZIER => {
                if !d.end.is_finite()
                    || !d.control.is_finite()
                    || !d.control_relative_time.is_finite()
                    || !(0.0..=1.0).contains(&d.control_relative_time)
                {
                    return Err(OarError::InvalidParameter);
                }
                Ok(Animated::Bezier {
                    start: d.start,
                    end: d.end,
                    control: d.control,
                    control_relative_time: d.control_relative_time,
                })
            }
            _ => Err(OarError::InvalidParameter),
        }
    }
}

impl TryFrom<crate::c_types::roar_animated_polar_t> for crate::common::definitions::AnimatedPolar {
    type Error = crate::common::definitions::OarError;

    fn try_from(c: crate::c_types::roar_animated_polar_t) -> Result<Self, Self::Error> {
        use crate::c_types::roar_animation_type_t;
        use crate::common::definitions::{
            AnimatedPolar, OarError, PolarControlRelativeTime, PolarCoordinate,
        };

        const STEP: u32 = roar_animation_type_t::ck_roar_animation_type_step as u32;
        const LINEAR: u32 = roar_animation_type_t::ck_roar_animation_type_linear as u32;
        const BEZIER: u32 = roar_animation_type_t::ck_roar_animation_type_bezier as u32;

        fn to_polar(az: f32, el: f32, dist: f32) -> Result<PolarCoordinate, OarError> {
            if !az.is_finite() || !el.is_finite() || !dist.is_finite() {
                return Err(OarError::InvalidParameter);
            }
            if !(-90.0..=90.0).contains(&el) || dist < 0.0 {
                return Err(OarError::InvalidParameter);
            }
            let mut norm_az = az % 360.0;
            if norm_az > 180.0 {
                norm_az -= 360.0;
            } else if norm_az < -180.0 {
                norm_az += 360.0;
            }
            PolarCoordinate::new_from_floats(norm_az, el, dist)
        }

        let start = to_polar(c.azimuth.start, c.elevation.start, c.distance.start)?;
        match c.animation_type {
            STEP => Ok(AnimatedPolar::Step { value: start }),
            LINEAR => {
                let end = to_polar(c.azimuth.end, c.elevation.end, c.distance.end)?;
                Ok(AnimatedPolar::Linear { start, end })
            }
            BEZIER => {
                let end = to_polar(c.azimuth.end, c.elevation.end, c.distance.end)?;
                let control = to_polar(c.azimuth.control, c.elevation.control, c.distance.control)?;
                let t_ctrl_az = c.azimuth.control_relative_time;
                let t_ctrl_el = c.elevation.control_relative_time;
                let t_ctrl_dist = c.distance.control_relative_time;
                if !t_ctrl_az.is_finite()
                    || !(0.0..=1.0).contains(&t_ctrl_az)
                    || !t_ctrl_el.is_finite()
                    || !(0.0..=1.0).contains(&t_ctrl_el)
                    || !t_ctrl_dist.is_finite()
                    || !(0.0..=1.0).contains(&t_ctrl_dist)
                {
                    return Err(OarError::InvalidParameter);
                }
                Ok(AnimatedPolar::Bezier {
                    start,
                    end,
                    control,
                    control_relative_time: PolarControlRelativeTime {
                        azimuth: t_ctrl_az,
                        elevation: t_ctrl_el,
                        distance: t_ctrl_dist,
                    },
                })
            }
            _ => Err(OarError::InvalidParameter),
        }
    }
}

impl TryFrom<crate::c_types::roar_animated_cartesian_t>
    for crate::common::definitions::AnimatedCartesian
{
    type Error = crate::common::definitions::OarError;

    fn try_from(c: crate::c_types::roar_animated_cartesian_t) -> Result<Self, Self::Error> {
        use crate::c_types::roar_animation_type_t;
        use crate::common::definitions::{
            AnimatedCartesian, CartesianControlRelativeTime, CartesianCoordinate, OarError,
        };

        const STEP: u32 = roar_animation_type_t::ck_roar_animation_type_step as u32;
        const LINEAR: u32 = roar_animation_type_t::ck_roar_animation_type_linear as u32;
        const BEZIER: u32 = roar_animation_type_t::ck_roar_animation_type_bezier as u32;

        let start = CartesianCoordinate::new(c.x.start, c.y.start, c.z.start)?;
        match c.animation_type {
            STEP => Ok(AnimatedCartesian::Step { value: start }),
            LINEAR => {
                let end = CartesianCoordinate::new(c.x.end, c.y.end, c.z.end)?;
                Ok(AnimatedCartesian::Linear { start, end })
            }
            BEZIER => {
                let end = CartesianCoordinate::new(c.x.end, c.y.end, c.z.end)?;
                let control = CartesianCoordinate::new(c.x.control, c.y.control, c.z.control)?;
                let t_ctrl_x = c.x.control_relative_time;
                let t_ctrl_y = c.y.control_relative_time;
                let t_ctrl_z = c.z.control_relative_time;
                if !t_ctrl_x.is_finite()
                    || !(0.0..=1.0).contains(&t_ctrl_x)
                    || !t_ctrl_y.is_finite()
                    || !(0.0..=1.0).contains(&t_ctrl_y)
                    || !t_ctrl_z.is_finite()
                    || !(0.0..=1.0).contains(&t_ctrl_z)
                {
                    return Err(OarError::InvalidParameter);
                }
                Ok(AnimatedCartesian::Bezier {
                    start,
                    end,
                    control,
                    control_relative_time: CartesianControlRelativeTime {
                        x: t_ctrl_x,
                        y: t_ctrl_y,
                        z: t_ctrl_z,
                    },
                })
            }
            _ => Err(OarError::InvalidParameter),
        }
    }
}

// ===== Tests =====

#[cfg(test)]
mod test {
    use crate::c_types::{
        roar_animated_cartesian_t, roar_animated_data_float32_t, roar_animated_float32_t,
        roar_animated_polar_t, roar_audio_element_config_t, roar_audio_element_type_t,
        roar_binaural_filter_profile_t, roar_coordinate_type_t, roar_headphones_rendering_mode_t,
        roar_layout_t, roar_metadata_gain_t, roar_metadata_gain_union_t,
        roar_metadata_object_positions_t, roar_metadata_t, roar_metadata_type_t,
        roar_metadata_union_t, roar_param_type_t, roar_polar_t, roar_quaternion_t,
        ROAR_PARAMETER_SET_FLAG_IAMF_ELEMENT_RENDERING_CONFIG,
    };
    use crate::common::definitions::{
        AnimatedCartesian, AnimatedFloat32, AnimatedPolar, AudioElementConfig, ChannelBasedConfig,
        Decibels, Gain, InputLayout, OarError, ObjectPosition, PolarCoordinate,
    };
    use googletest::prelude::*;
    use std::mem::MaybeUninit;

    #[gtest]
    fn test_metadata_layout() {
        let rotation = roar_quaternion_t { w: 0.0, x: 0.0, y: 0.0, z: 0.0 };
        let meta = roar_metadata_t {
            r#type: 0,
            value: roar_metadata_union_t { head_rotation: rotation },
            duration: 0,
        };
        let base = &meta as *const _ as usize;
        let duration = &meta.duration as *const _ as usize;
        let value = &meta.value as *const _ as usize;

        expect_that!(std::mem::size_of::<roar_metadata_t>(), eq(136));
        expect_that!(duration - base, eq(128));
        expect_that!(value - base, eq(8));
    }

    #[gtest]
    fn from_raw_with_zero_flags_ignores_uninitialized_parameter_set_substructs() {
        let mut raw_cfg = MaybeUninit::<roar_audio_element_config_t>::uninit();
        let cfg_ptr = raw_cfg.as_mut_ptr();
        // SAFETY: `cfg_ptr` points to a valid, properly aligned, exclusively owned
        // `MaybeUninit<roar_audio_element_config_t>` on the stack, so writing to its fields via
        // `&raw mut` is valid and does not create references to uninitialized memory.
        unsafe {
            (&raw mut (*cfg_ptr).r#type)
                .write(roar_audio_element_type_t::ck_roar_channel_based as i32);
            (&raw mut (*cfg_ptr).config.cbc.layout)
                .write(roar_layout_t::ck_roar_layout_stereo as i32);
            (&raw mut (*cfg_ptr).parameters.flags).write(0);
        }

        // SAFETY: `cfg_ptr` is non-null, properly aligned, and points to readable stack memory
        // for the duration of this call with `r#type`, `config.cbc`, and `parameters.flags` (`0`)
        // initialized above; unflagged parameter sub-structs are never read.
        let result = unsafe { roar_audio_element_config_t::from_raw(cfg_ptr) };

        expect_that!(
            result,
            ok(eq(AudioElementConfig::ChannelBased(ChannelBasedConfig {
                layout: InputLayout::Stereo,
                downmix_info: None,
                rendering_config: None,
            })))
        );
    }

    #[gtest]
    fn from_raw_with_out_of_range_rendering_config_enums_returns_invalid_parameter() {
        // SAFETY: `roar_audio_element_config_t` is a `#[repr(C)]` FFI struct of integers and
        // unions for which an all-zero bit pattern is a valid representation.
        let mut bad_headphones_cfg: roar_audio_element_config_t = unsafe { std::mem::zeroed() };
        bad_headphones_cfg.r#type = roar_audio_element_type_t::ck_roar_channel_based as i32;
        bad_headphones_cfg.config.cbc.layout = roar_layout_t::ck_roar_layout_stereo as i32;
        bad_headphones_cfg.parameters.flags = ROAR_PARAMETER_SET_FLAG_IAMF_ELEMENT_RENDERING_CONFIG;
        bad_headphones_cfg.parameters.element_rendering_config.headphones_rendering_mode = 99;
        bad_headphones_cfg.parameters.element_rendering_config.binaural_filter_profile =
            roar_binaural_filter_profile_t::ck_roar_ambient as u32;
        let mut bad_profile_cfg = bad_headphones_cfg;
        bad_profile_cfg.parameters.element_rendering_config.headphones_rendering_mode =
            roar_headphones_rendering_mode_t::ck_roar_world_locked as u32;
        bad_profile_cfg.parameters.element_rendering_config.binaural_filter_profile = 99;

        // SAFETY: `&bad_headphones_cfg` is a valid, aligned reference to a fully initialized
        // `roar_audio_element_config_t`.
        let bad_headphones_result =
            unsafe { roar_audio_element_config_t::from_raw(&bad_headphones_cfg) };
        // SAFETY: `&bad_profile_cfg` is a valid, aligned reference to a fully initialized
        // `roar_audio_element_config_t`.
        let bad_profile_result = unsafe { roar_audio_element_config_t::from_raw(&bad_profile_cfg) };

        expect_that!(bad_headphones_result, err(eq(OarError::InvalidParameter)));
        expect_that!(bad_profile_result, err(eq(OarError::InvalidParameter)));
    }

    #[gtest]
    fn metadata_conversions_with_out_of_range_enum_discriminants_return_invalid_parameter() {
        let bad_gain_meta = roar_metadata_t {
            r#type: roar_metadata_type_t::ck_roar_metadata_gain as u32,
            value: roar_metadata_union_t {
                gain: roar_metadata_gain_t {
                    id: 1,
                    param_type: 99,
                    value: roar_metadata_gain_union_t { constant_gain: 0.0 },
                },
            },
            duration: 64,
        };
        let bad_pos_param_meta = roar_metadata_t {
            r#type: roar_metadata_type_t::ck_roar_metadata_object_positions as u32,
            value: roar_metadata_union_t {
                object_positions: roar_metadata_object_positions_t {
                    param_type: 99,
                    position_type: roar_coordinate_type_t::ck_roar_polar as u32,
                    num_objects: 1,
                    // SAFETY: `roar_metadata_object_positions_union_t` is a `#[repr(C)]` union of
                    // `f32` arrays for which an all-zero bit pattern is valid.
                    positions: unsafe { std::mem::zeroed() },
                },
            },
            duration: 64,
        };
        let bad_pos_coord_meta = roar_metadata_t {
            r#type: roar_metadata_type_t::ck_roar_metadata_object_positions as u32,
            value: roar_metadata_union_t {
                object_positions: roar_metadata_object_positions_t {
                    param_type: roar_param_type_t::ck_roar_param_constant as u32,
                    position_type: 99,
                    num_objects: 1,
                    // SAFETY: `roar_metadata_object_positions_union_t` is a `#[repr(C)]` union of
                    // `f32` arrays for which an all-zero bit pattern is valid.
                    positions: unsafe { std::mem::zeroed() },
                },
            },
            duration: 64,
        };
        let anim_axis = roar_animated_data_float32_t {
            start: 0.0,
            end: 0.5,
            control: 0.25,
            control_relative_time: 0.5,
        };
        let bad_anim_float = roar_animated_float32_t { animation_type: 99, data: anim_axis };
        let bad_anim_polar = roar_animated_polar_t {
            animation_type: 99,
            azimuth: anim_axis,
            elevation: anim_axis,
            distance: anim_axis,
        };
        let bad_anim_cartesian = roar_animated_cartesian_t {
            animation_type: 99,
            x: anim_axis,
            y: anim_axis,
            z: anim_axis,
        };

        // SAFETY: `bad_gain_meta.value.gain.param_type` is initialized (`99`) and does not select
        // `ck_roar_param_multiple`.
        let gain_res = unsafe { bad_gain_meta.to_gain() };
        // SAFETY: `bad_pos_param_meta.value.object_positions` header fields are initialized and
        // `param_type` (`99`) is rejected before reading any entries of `positions`.
        let pos_param_res = unsafe { bad_pos_param_meta.to_positions() };
        // SAFETY: `bad_pos_coord_meta.value.object_positions` header fields are initialized and
        // `position_type` (`99`) is rejected before reading any entries of `positions`.
        let pos_coord_res = unsafe { bad_pos_coord_meta.to_positions() };
        let anim_float_res = AnimatedFloat32::try_from(bad_anim_float);
        let anim_polar_res = AnimatedPolar::try_from(bad_anim_polar);
        let anim_cart_res = AnimatedCartesian::try_from(bad_anim_cartesian);

        expect_that!(gain_res, err(eq(&OarError::InvalidParameter)));
        expect_that!(pos_param_res, err(eq(&OarError::InvalidParameter)));
        expect_that!(pos_coord_res, err(eq(&OarError::InvalidParameter)));
        expect_that!(anim_float_res, err(eq(OarError::InvalidParameter)));
        expect_that!(anim_polar_res, err(eq(OarError::InvalidParameter)));
        expect_that!(anim_cart_res, err(eq(OarError::InvalidParameter)));
    }

    #[gtest]
    fn to_gain_with_multiple_param_type_converts_gain_array() {
        let mut gains = [0.0f32, -6.0, 3.0];
        let meta = roar_metadata_t {
            r#type: roar_metadata_type_t::ck_roar_metadata_gain as u32,
            value: roar_metadata_union_t {
                gain: roar_metadata_gain_t {
                    id: 1,
                    param_type: roar_param_type_t::ck_roar_param_multiple as u32,
                    value: roar_metadata_gain_union_t { gain_array: gains.as_mut_ptr() },
                },
            },
            duration: 3,
        };

        // SAFETY: `gains` is a live array of 3 initialized `f32` values valid for reads for the
        // duration of `to_gain`.
        let result = unsafe { meta.to_gain() };

        let expected = Gain::new_multiple(vec![
            Decibels::new(0.0).unwrap(),
            Decibels::new(-6.0).unwrap(),
            Decibels::new(3.0).unwrap(),
        ])
        .unwrap();
        expect_that!(result, ok(eq(&expected)));
    }

    #[gtest]
    fn to_gain_with_invalid_multiple_gain_pointer_or_duration_returns_invalid_parameter() {
        let mut gains = [0.0f32, -6.0];
        let unaligned_ptr = gains.as_mut_ptr().cast::<u8>().wrapping_add(1).cast::<f32>();
        let null_ptr_meta = roar_metadata_t {
            r#type: roar_metadata_type_t::ck_roar_metadata_gain as u32,
            value: roar_metadata_union_t {
                gain: roar_metadata_gain_t {
                    id: 1,
                    param_type: roar_param_type_t::ck_roar_param_multiple as u32,
                    value: roar_metadata_gain_union_t { gain_array: std::ptr::null_mut() },
                },
            },
            duration: 2,
        };
        let unaligned_ptr_meta = roar_metadata_t {
            r#type: roar_metadata_type_t::ck_roar_metadata_gain as u32,
            value: roar_metadata_union_t {
                gain: roar_metadata_gain_t {
                    id: 1,
                    param_type: roar_param_type_t::ck_roar_param_multiple as u32,
                    value: roar_metadata_gain_union_t { gain_array: unaligned_ptr },
                },
            },
            duration: 2,
        };
        let zero_duration_meta = roar_metadata_t {
            r#type: roar_metadata_type_t::ck_roar_metadata_gain as u32,
            value: roar_metadata_union_t {
                gain: roar_metadata_gain_t {
                    id: 1,
                    param_type: roar_param_type_t::ck_roar_param_multiple as u32,
                    value: roar_metadata_gain_union_t { gain_array: gains.as_mut_ptr() },
                },
            },
            duration: 0,
        };

        // SAFETY: Each metadata struct has initialized union fields and triggers early parameter
        // validation errors before any dereference of `gain_array`.
        let null_res = unsafe { null_ptr_meta.to_gain() };
        // SAFETY: `unaligned_ptr` is rejected by alignment checks prior to dereferencing.
        let unaligned_res = unsafe { unaligned_ptr_meta.to_gain() };
        // SAFETY: `duration <= 0` is rejected prior to dereferencing `gain_array`.
        let zero_dur_res = unsafe { zero_duration_meta.to_gain() };

        expect_that!(null_res, err(eq(&OarError::InvalidParameter)));
        expect_that!(unaligned_res, err(eq(&OarError::InvalidParameter)));
        expect_that!(zero_dur_res, err(eq(&OarError::InvalidParameter)));
    }

    #[gtest]
    fn to_positions_with_single_object_ignores_uninitialized_trailing_slot() {
        let mut raw_meta = MaybeUninit::<roar_metadata_t>::uninit();
        let meta_ptr = raw_meta.as_mut_ptr();
        // SAFETY: `meta_ptr` points to a valid, properly aligned, exclusively owned
        // `MaybeUninit<roar_metadata_t>` on the stack, so writing to its fields via `&raw mut`
        // initializes the header and first polar slot without creating references to uninitialized
        // bytes.
        unsafe {
            (&raw mut (*meta_ptr).r#type)
                .write(roar_metadata_type_t::ck_roar_metadata_object_positions as u32);
            (&raw mut (*meta_ptr).duration).write(64);
            (&raw mut (*meta_ptr).value.object_positions.param_type)
                .write(roar_param_type_t::ck_roar_param_constant as u32);
            (&raw mut (*meta_ptr).value.object_positions.position_type)
                .write(roar_coordinate_type_t::ck_roar_polar as u32);
            (&raw mut (*meta_ptr).value.object_positions.num_objects).write(1);
            (&raw mut (*meta_ptr).value.object_positions.positions.polar_positions[0])
                .write(roar_polar_t { azimuth: 30.0, elevation: 15.0, distance: 0.75 });
        }

        // SAFETY: `meta_ptr` is properly aligned and points to readable stack memory for the
        // duration of this call, with `r#type`, `duration`, `object_positions` header fields,
        // and the first `num_objects` (`1`) entry of `polar_positions` initialized above.
        let result = unsafe { (&*meta_ptr).to_positions() };

        let expected = ObjectPosition::Polar(vec![PolarCoordinate::new_from_floats(
            30.0, 15.0, 0.75,
        )
        .unwrap()]);
        expect_that!(result, ok(eq(&expected)));
    }
}
