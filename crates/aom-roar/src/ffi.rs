// Copyright (c) 2026, Alliance for Open Media. All rights reserved
//
// This source code is subject to the terms of the BSD 3-Clause Clear License
// and the Alliance for Open Media Patent License 1.0. If the BSD 3-Clause Clear
// License was not distributed with this source code in the LICENSE file, you
// can obtain it at www.aomedia.org/license/software-license/bsd-3-c-c. If the
// Alliance for Open Media Patent License 1.0 was not distributed with this
// source code in the PATENTS file, you can obtain it at
// www.aomedia.org/license/patent.

//! C API for Rust Open Audio Renderer (ROAR)
//!
//! All `#[unsafe(no_mangle)] extern "C"` functions live here and nowhere else.
//! Every function is a thin dispatcher that validates raw pointer arguments,
//! delegates to pure-safe functions, and converts `Result` back to C-style error codes.
//! All functions wrap their execution inside `std::panic::catch_unwind`.

use crate::c_types::{
    roar_audio_block_t, roar_audio_element_config_t, roar_config_t, roar_metadata_t,
    roar_metadata_type_t, roar_t, FfiWrapper,
};
use crate::common::definitions::{
    Decibels, GroupId, OarError, PlanarBufferMut, PlanarBufferRef, Samples,
    MAX_OUTPUT_CHANNEL_COUNT, ROAR_STATUS_OK,
};
use std::os::raw::c_int;

/// Converts a `roar_audio_block_t` into a validated `PlanarBufferMut`.
///
/// # Safety
/// `output_block.data` must be non-null and the caller must guarantee that it is properly aligned,
/// exclusively accessed for `'a`, and points to an allocated buffer of at least
/// `(output_block.channels as usize) * samples.value()` `f32` elements (which may be
/// uninitialized on entry).
unsafe fn block_to_planar_buffer_mut<'a, 'b>(
    output_block: &'a mut roar_audio_block_t,
    slices_array: &'b mut [&'a mut [f32]; MAX_OUTPUT_CHANNEL_COUNT],
    samples: Samples,
) -> Result<PlanarBufferMut<'a, 'b>, OarError> {
    let out_channels = output_block.channels as usize;
    if output_block.data.is_null()
        || !output_block.data.is_aligned()
        || out_channels == 0
        || out_channels > MAX_OUTPUT_CHANNEL_COUNT
    {
        return Err(OarError::InvalidParameter);
    }
    output_block.samples_per_channel = samples.value();
    let samples_usize = samples.value() as usize;
    let total_samples = out_channels
        .checked_mul(samples_usize)
        .filter(|&total| total <= isize::MAX as usize / std::mem::size_of::<f32>())
        .ok_or(OarError::InvalidParameter)?;
    // SAFETY: `output_block.data` is verified non-null above, and the caller guarantees exclusive
    // access for `'a` to at least `total_samples` aligned `f32` elements. Zero-writing initializes
    // all elements to `0.0f32` prior to forming `&mut [f32]` in case they were uninitialized.
    let full_slice = unsafe {
        std::ptr::write_bytes(output_block.data, 0, total_samples);
        std::slice::from_raw_parts_mut(output_block.data, total_samples)
    };
    for (slot, chunk) in slices_array.iter_mut().zip(full_slice.chunks_exact_mut(samples_usize)) {
        *slot = chunk;
    }
    PlanarBufferMut::new(&mut slices_array[..out_channels], out_channels, samples)
}

/// Creates a new ROAR renderer object.
///
/// Returns a pointer to the created `roar_t` object on success, or `null_mut()` on failure.
///
/// # Safety
///
/// The caller must guarantee that:
/// - `config` points to a valid, initialized `roar_config_t` structure.
/// - The pointer is safe to read for the duration of this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn roar_create(config: *const roar_config_t) -> *mut roar_t {
    std::panic::catch_unwind(|| {
        if config.is_null() {
            return std::ptr::null_mut();
        }
        // SAFETY: We checked that `config` is not null. The caller guarantees that `config`
        // points to a valid, initialized `roar_config_t` structure.
        let config_ref = unsafe { &*config };
        let oar_config = match crate::common::definitions::Config::try_from(*config_ref) {
            Ok(c) => c,
            Err(_) => return std::ptr::null_mut(),
        };
        let renderer = match crate::renderer::RoarRenderer::create(&oar_config) {
            Ok(r) => r,
            Err(_) => return std::ptr::null_mut(),
        };
        let wrapper = FfiWrapper { renderer, inputs: std::collections::HashMap::new() };
        // Box the wrapper and return it as a `roar_t` pointer.
        Box::into_raw(Box::new(wrapper)) as *mut roar_t
    })
    .unwrap_or(std::ptr::null_mut())
}

/// Destroys a ROAR renderer object.
///
/// # Safety
///
/// The caller must guarantee that:
/// - `roar` points to a valid ROAR object returned by `roar_create`, or is null.
/// - The object has not been destroyed already and is not accessed concurrently.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn roar_destroy(roar: *mut roar_t) {
    let _ = std::panic::catch_unwind(|| {
        if roar.is_null() {
            return;
        }
        // SAFETY: The caller guarantees `roar` is a valid pointer returned by `roar_create`
        // and has not been destroyed yet.
        unsafe {
            // Cast `roar_t` to its real type, `FfiWrapper`, and reconstruct the Box for
            // deallocation.
            let _ = Box::from_raw(roar as *mut FfiWrapper);
        }
    });
}

/// Adds a new audio group to the renderer.
///
/// Returns the group ID (0 or 1) on success, or a negative error code (e.g. `-22` for invalid
/// param, `-12` for out of memory, `-16` if group limit reached) on failure.
///
/// # Safety
///
/// The caller must guarantee that:
/// - `roar` points to a valid, initialized ROAR object.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn roar_add_audio_group(roar: *mut roar_t) -> c_int {
    std::panic::catch_unwind(|| {
        if roar.is_null() {
            return OarError::InvalidParameter.into();
        }
        // SAFETY: The caller guarantees `roar` points to a valid FfiWrapper.
        let wrapper = unsafe { &mut *(roar as *mut FfiWrapper) };
        match wrapper.renderer.add_audio_group() {
            Ok(new_id) => new_id as c_int,
            Err(e) => e.into(),
        }
    })
    .unwrap_or(OarError::InvalidParameter.into())
}

/// Adds a new audio element to the renderer.
///
/// Returns `ROAR_STATUS_OK` (0) on success, or a negative error code (e.g. `-22` for invalid param,
/// `-12` for out of memory) on failure.
///
/// # Safety
///
/// The caller must guarantee that:
/// - `roar` points to a valid, initialized ROAR object.
/// - `config` points to a readable, aligned `roar_audio_element_config_t` structure whose `r#type`,
///   `parameters.flags`, active `config` union variant, and any `parameters` sub-structs enabled in
///   `parameters.flags` are initialized.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn roar_add_audio_element(
    roar: *mut roar_t,
    gid: u32,
    id: u32,
    config: *const roar_audio_element_config_t,
) -> c_int {
    std::panic::catch_unwind(|| {
        if roar.is_null() || config.is_null() {
            return OarError::InvalidParameter.into();
        }
        // SAFETY: The caller guarantees `roar` points to a valid FfiWrapper.
        let wrapper = unsafe { &mut *(roar as *mut FfiWrapper) };
        // SAFETY: We checked that `config` is not null. The caller guarantees that `config` points
        // to an aligned `roar_audio_element_config_t` whose active fields are initialized.
        let safe_config = match unsafe { roar_audio_element_config_t::from_raw(config) } {
            Ok(c) => c,
            Err(e) => return e.into(),
        };

        let group_id = match GroupId::try_from(gid) {
            Ok(id) => id,
            Err(e) => return e.into(),
        };

        match wrapper.renderer.add_element(group_id, id, &safe_config) {
            Ok(()) => ROAR_STATUS_OK,
            Err(e) => e.into(),
        }
    })
    .unwrap_or(OarError::InvalidParameter.into())
}

/// Removes an audio element from the renderer.
///
/// Returns `ROAR_STATUS_OK` (0) on success, or a negative error code (e.g. `-22` if element not
/// found or invalid param) on failure.
///
/// # Safety
///
/// The caller must guarantee that:
/// - `roar` points to a valid, initialized ROAR object.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn roar_remove_audio_element(roar: *mut roar_t, id: u32) -> c_int {
    std::panic::catch_unwind(|| {
        if roar.is_null() {
            return OarError::InvalidParameter.into();
        }
        // SAFETY: The caller guarantees `roar` points to a valid FfiWrapper.
        let wrapper = unsafe { &mut *(roar as *mut FfiWrapper) };
        match wrapper.renderer.remove_element(id) {
            Ok(()) => {
                wrapper.inputs.remove(&id);
                ROAR_STATUS_OK
            }
            Err(e) => e.into(),
        }
    })
    .unwrap_or(OarError::InvalidParameter.into())
}

/// Updates metadata parameters for an audio element.
///
/// Supported metadata types:
/// - **Object positions** (`roar_metadata_object_positions_t`): 3D spatial coordinates supported
///   only on object-based audio elements.
///   - Duration must be specified. A value `<= 0` means the update will be silently dropped.
/// - **Gain** (`roar_metadata_gain_t`): Dynamic gain adjustment supported on all audio element
///   types.
///   - Duration must be specified. A value `<= 0` means the update will be silently dropped.
/// - **IAMF downmix mode** (`roar_metadata_iamf_downmix_mode_t`): Advisory metadata hint that takes
///   effect only when the element's current rendering path has a downmix consumer
///   (`DownmixRenderer`). When the element's current rendering path has no downmix consumer (such
///   as EAR fallback, OLR, or binaural OBR), the update is accepted and ignored (`ROAR_STATUS_OK`).
///   - Duration is optional. A value `<= 0` means the update will persist indefinitely (until a new
///     update is provided).
///
/// To update head rotation (which is always applied globally), use `roar_update_metadata` instead.
///
/// # Arguments
/// * `roar` - Pointer to the ROAR object.
/// * `id` - Unique element identifier (searched across all audio groups).
/// * `metadata` - Metadata payload to apply to the element.
///
/// # Returns
/// - `ROAR_STATUS_OK` (`0`) on success. For advisory metadata
///   (`ck_roar_metadata_iamf_downmix_mode`), `0` means "accepted" and also covers updates ignored
///   because the element's current rendering path has no downmix consumer.
/// - `-22` (`OarError::InvalidParameter`) for null pointers, invalid parameters, a non-existent
///   element `id`, or an invalid `mode` value (outside `{0, 1, 2, 4, 5, 6}`) when the element's
///   current rendering path has a downmix consumer.
/// - `-95` (`OarError::NotSupported`) for unknown metadata types, or non-advisory metadata
///   unsupported by the target element (e.g., object positions on a non-object element).
///
/// # Safety
///
/// The caller must guarantee that:
/// - `roar` points to a valid, initialized ROAR object.
/// - `metadata` points to a readable, aligned `roar_metadata_t` structure for the duration of this
///   call whose `r#type`, `duration`, and active `value` union payload (and active nested union
///   fields) are initialized.
/// - When `metadata.r#type == ck_roar_metadata_gain`, `metadata.value.gain.param_type ==
///   ck_roar_param_multiple`, `metadata.duration > 0`, and `metadata.value.gain.value.gain_array`
///   is non-null and aligned for `f32`, `gain_array` points to a single live allocated object
///   containing at least `metadata.duration as usize` consecutive, properly initialized `f32`
///   elements (`(metadata.duration as usize) * size_of::<f32>()` bytes) without address-space
///   wraparound, valid for reads and not mutated concurrently for the duration of this call.
/// - When `metadata.r#type == ck_roar_metadata_object_positions`, `metadata.duration > 0`,
///   `metadata.value.object_positions.num_objects <= ROAR_MAX_NUMBER_OF_OBJECTS`, and `(param_type,
///   position_type)` selects a supported variant (`ck_roar_param_constant` or
///   `ck_roar_param_animated` paired with `ck_roar_polar` or `ck_roar_cartesian`), the first
///   `num_objects` entries of the active `positions` union array are initialized and valid for
///   reads for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn roar_update_audio_element_metadata(
    roar: *mut roar_t,
    id: u32,
    metadata: *const roar_metadata_t,
) -> c_int {
    std::panic::catch_unwind(|| {
        if roar.is_null() || metadata.is_null() {
            return OarError::InvalidParameter.into();
        }
        // SAFETY: The caller guarantees `roar` points to a valid FfiWrapper.
        let wrapper = unsafe { &mut *(roar as *mut FfiWrapper) };
        if !wrapper.renderer.active_elements().contains_key(&id) {
            return OarError::InvalidParameter.into();
        }
        // SAFETY: We checked that `metadata` is not null. The caller guarantees that `metadata`
        // points to a readable, aligned `roar_metadata_t` structure with initialized active fields
        // for the duration of this call.
        let meta_ref = unsafe { &*metadata };
        let meta_type = meta_ref.r#type;

        if meta_type == (roar_metadata_type_t::ck_roar_metadata_gain as u32) {
            // C liboar ignores gain updates with duration <= 0.
            if meta_ref.duration <= 0 {
                return ROAR_STATUS_OK;
            }
            let duration = match Samples::new(meta_ref.duration as u32) {
                Ok(s) => s,
                Err(e) => return e.into(),
            };
            // SAFETY: `meta_type` was verified to be `ck_roar_metadata_gain` and `duration > 0`.
            // The caller guarantees that `meta_ref.value.gain` has initialized active union fields
            // and, when `param_type == ck_roar_param_multiple` and `gain_array` is non-null and
            // aligned, `gain_array` points to a single live allocation of at least `duration`
            // consecutive, initialized `f32` elements valid for reads and not mutated concurrently
            // for the duration of this call.
            let gain = match unsafe { meta_ref.to_gain() } {
                Ok(g) => g,
                Err(e) => return e.into(),
            };
            // SAFETY: `meta_type` was verified to be `ck_roar_metadata_gain`, so `value.gain.id`
            // is initialized. Reading via pointer projection avoids copying inactive union padding.
            let gain_id = unsafe { (&raw const meta_ref.value.gain.id).read() };
            match wrapper.renderer.update_element_gain(id, gain_id, &gain, duration) {
                Ok(()) => ROAR_STATUS_OK,
                Err(e) => e.into(),
            }
        } else if meta_type == (roar_metadata_type_t::ck_roar_metadata_object_positions as u32) {
            // C liboar ignores position updates with duration <= 0.
            if meta_ref.duration <= 0 {
                return ROAR_STATUS_OK;
            }
            let duration = match Samples::new(meta_ref.duration as u32) {
                Ok(s) => s,
                Err(e) => return e.into(),
            };
            // SAFETY: `meta_type` was verified to be `ck_roar_metadata_object_positions` and
            // `duration > 0`. The caller guarantees that `meta_ref.value.object_positions` has
            // initialized header fields and that the first `num_objects` entries of the active
            // `positions` union array are initialized and valid for reads for the duration of this
            // call.
            let positions = match unsafe { meta_ref.to_positions() } {
                Ok(p) => p,
                Err(e) => return e.into(),
            };
            match wrapper.renderer.update_element_positions(id, &positions, duration) {
                Ok(()) => ROAR_STATUS_OK,
                Err(e) => e.into(),
            }
        } else if meta_type == (roar_metadata_type_t::ck_roar_metadata_iamf_downmix_mode as u32) {
            // Downmix mode metadata is advisory: when the element's active rendering path has no
            // downmix consumer, accept and ignore the update without validating `mode`.
            match wrapper.renderer.element_has_downmix_consumer(id) {
                Ok(true) => {}
                Ok(false) => return ROAR_STATUS_OK,
                Err(e) => return e.into(),
            }
            // C liboar treats downmix mode updates with duration <= 0 as infinite
            // (block-persisting).
            let duration = if meta_ref.duration <= 0 {
                None
            } else {
                match Samples::new(meta_ref.duration as u32) {
                    Ok(s) => Some(s),
                    Err(e) => return e.into(),
                }
            };
            let mode = match meta_ref.to_downmix_mode() {
                Ok(m) => m,
                Err(e) => return e.into(),
            };
            match wrapper.renderer.update_element_downmix_mode(id, mode, duration) {
                Ok(()) => ROAR_STATUS_OK,
                Err(e) => e.into(),
            }
        } else {
            OarError::NotSupported.into()
        }
    })
    .unwrap_or(OarError::InvalidParameter.into())
}

/// Updates audio element data blocks for rendering.
///
/// Returns `ROAR_STATUS_OK` (0) on success, or a negative error code (e.g. `-22` for invalid param
/// / channels count) on failure.
///
/// # Safety
///
/// The caller must guarantee that:
/// - `roar` points to a valid, initialized ROAR object.
/// - `data` points to a valid `roar_audio_block_t` structure containing active planar f32 input
///   buffers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn roar_update_audio_element_data(
    roar: *mut roar_t,
    id: u32,
    data: *mut roar_audio_block_t,
) -> c_int {
    std::panic::catch_unwind(|| {
        if roar.is_null() || data.is_null() {
            return OarError::InvalidParameter.into();
        }
        // SAFETY: The caller guarantees `roar` points to a valid FfiWrapper.
        let wrapper = unsafe { &mut *(roar as *mut FfiWrapper) };

        let Some(expected_channels) =
            wrapper.renderer.active_elements().get(&id).map(|c| c.channels())
        else {
            return OarError::InvalidParameter.into();
        };

        // SAFETY: We checked that `data` is not null. The caller guarantees that `data`
        // points to a valid `roar_audio_block_t` structure containing active planar f32 input
        // buffers, which ensures `as_slices()` can safely dereference the inner data pointers.
        let data_slices = unsafe { (*data).as_slices() };
        if data_slices.len() != expected_channels.get() {
            return OarError::InvalidParameter.into();
        }

        // TODO(b/525080422): Could we reuse the previously allocated memory (if applicable) rather
        // than reallocating?
        let copied_data: Vec<Vec<f32>> = data_slices.iter().map(|slice| slice.to_vec()).collect();

        wrapper.inputs.insert(id, copied_data);
        ROAR_STATUS_OK
    })
    .unwrap_or(OarError::InvalidParameter.into())
}

/// Sets the metadata unit processing size in samples.
///
/// Returns `ROAR_STATUS_OK` (0) on success, or a negative error code (e.g. `-22` for invalid param,
/// `-95` for unsupported metadata type) on failure.
///
/// Matching liboar, only object positions metadata type is supported.
///
/// # Arguments
/// * `roar` - Pointer to the ROAR object.
/// * `r#type` - Raw `roar_metadata_type_t` discriminant (`u32`) accepted as an integer across FFI
///   so out-of-range C enum values return `ck_roar_error_notsup` without undefined behavior.
/// * `samples` - Sub-block processing unit in samples; must be in `1..=samples_per_channel`.
///
/// # Safety
///
/// The caller must guarantee that:
/// - `roar` points to a properly aligned, validly initialized object and that no aliasing accesses
///   occur for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn roar_set_metadata_unit_to_process(
    roar: *mut roar_t,
    r#type: u32,
    samples: u32,
) -> c_int {
    std::panic::catch_unwind(|| {
        if roar.is_null() {
            return OarError::InvalidParameter.into();
        }
        if r#type != (roar_metadata_type_t::ck_roar_metadata_object_positions as u32) {
            return OarError::NotSupported.into();
        }
        let Ok(samples) = Samples::new(samples) else {
            return OarError::InvalidParameter.into();
        };
        // SAFETY: `roar` was verified non-null above. The caller guarantees that `roar` points to a
        // properly aligned, validly initialized `FfiWrapper` and that no aliasing accesses occur
        // for the duration of this call, satisfying `&mut` validity and exclusivity.
        let wrapper = unsafe { &mut *(roar as *mut FfiWrapper) };
        match wrapper.renderer.set_object_position_metadata_unit_to_process(samples) {
            Ok(()) => ROAR_STATUS_OK,
            Err(e) => e.into(),
        }
    })
    .unwrap_or(OarError::InvalidParameter.into())
}

/// Updates metadata parameters for an audio group.
///
/// Supported metadata and value constraints:
/// - **Head rotation** (`ck_roar_metadata_head_rotation`):
///   - Quaternion `w, x, y, z` representing head orientation.
///   - Expected to be a unit quaternion (sum of squares is 1.0).
///   - Uses the ADM object coordinate system to orient the listener's head.
///   - Note: Duration is not used.
/// - **Gain** (`ck_roar_metadata_gain`):
///   - Gain in dB, applied to all channels in the group.
///   - Duration must be specified. A value <= 0 means the update will be silently dropped!
///
/// Returns `ROAR_STATUS_OK` (0) on success, or a negative error code (e.g. `-22` for invalid param,
/// `-16` if head tracking is disabled, `-95` for unsupported metadata type) on failure.
///
/// # Safety
///
/// The caller must guarantee that:
/// - `roar` points to a valid, initialized ROAR object.
/// - `metadata` points to a readable, aligned `roar_metadata_t` structure for the duration of this
///   call whose `r#type`, `duration`, and active `value` union payload (and active nested union
///   fields) are initialized.
/// - When `metadata.r#type == ck_roar_metadata_gain`, `metadata.value.gain.param_type ==
///   ck_roar_param_multiple`, `metadata.duration > 0`, and `metadata.value.gain.value.gain_array`
///   is non-null and aligned for `f32`, `gain_array` points to a single live allocated object
///   containing at least `metadata.duration as usize` consecutive, properly initialized `f32`
///   elements (`(metadata.duration as usize) * size_of::<f32>()` bytes) without address-space
///   wraparound, valid for reads and not mutated concurrently for the duration of this call.
/// - When `metadata.r#type == ck_roar_metadata_object_positions`, `metadata.duration > 0`,
///   `metadata.value.object_positions.num_objects <= ROAR_MAX_NUMBER_OF_OBJECTS`, and `(param_type,
///   position_type)` selects a supported variant (`ck_roar_param_constant` or
///   `ck_roar_param_animated` paired with `ck_roar_polar` or `ck_roar_cartesian`), the first
///   `num_objects` entries of the active `positions` union array are initialized and valid for
///   reads for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn roar_update_metadata(
    roar: *mut roar_t,
    gid: u32,
    metadata: *const roar_metadata_t,
) -> c_int {
    std::panic::catch_unwind(|| {
        if roar.is_null() || metadata.is_null() {
            return OarError::InvalidParameter.into();
        }
        // SAFETY: The caller guarantees `roar` points to a valid FfiWrapper.
        let wrapper = unsafe { &mut *(roar as *mut FfiWrapper) };
        // SAFETY: We checked that `metadata` is not null. The caller guarantees that `metadata`
        // points to a readable, aligned `roar_metadata_t` structure with initialized active fields
        // for the duration of this call.
        let meta_ref = unsafe { &*metadata };

        let group_id = match GroupId::try_from(gid) {
            Ok(id) => id,
            Err(e) => return e.into(),
        };

        if !wrapper.renderer.has_group(group_id) {
            return OarError::InvalidParameter.into();
        }

        if meta_ref.r#type == (roar_metadata_type_t::ck_roar_metadata_gain as u32) {
            // C liboar ignores group gain updates with duration <= 0.
            if meta_ref.duration <= 0 {
                return ROAR_STATUS_OK;
            }
            // SAFETY: `meta_ref.r#type` was verified to be `ck_roar_metadata_gain` and
            // `duration > 0`. The caller guarantees that `meta_ref.value.gain` has initialized
            // active union fields and, when `param_type == ck_roar_param_multiple` and
            // `gain_array` is non-null and aligned, `gain_array` points to a single live allocation
            // of at least `duration` consecutive, initialized `f32` elements valid for reads and
            // not mutated concurrently for the duration of this call.
            let gain = match unsafe { meta_ref.to_gain() } {
                Ok(g) => g,
                Err(e) => return e.into(),
            };
            let duration = match Samples::new(meta_ref.duration as u32) {
                Ok(s) => s,
                Err(e) => return e.into(),
            };
            match wrapper.renderer.update_group_gain(group_id, &gain, duration) {
                Ok(()) => ROAR_STATUS_OK,
                Err(e) => e.into(),
            }
        } else if meta_ref.r#type == (roar_metadata_type_t::ck_roar_metadata_head_rotation as u32) {
            if !wrapper.renderer.head_tracking_enabled {
                // Returns `ck_roar_error_busy` if head tracking is disabled.
                return OarError::Busy.into();
            }
            match meta_ref.to_head_rotation() {
                Ok(q) => {
                    if let Err(e) = wrapper.renderer.set_head_rotation(q) {
                        return e.into();
                    }
                }
                Err(e) => return e.into(),
            }
            ROAR_STATUS_OK
        } else {
            OarError::NotSupported.into()
        }
    })
    .unwrap_or(OarError::InvalidParameter.into())
}

/// Renders all active audio elements and writes results to planar outputs.
///
/// Returns `ROAR_STATUS_OK` (0) on success, or a negative error code (e.g. `-22` for invalid
/// parameters, or sub-renderer errors) on failure.
///
/// # Safety
///
/// The caller must guarantee that:
/// - `roar` points to a valid, initialized ROAR object.
/// - `output` points to a valid `roar_audio_block_t` with correct `channels` and
///   `samples_per_channel` and allocated planar float memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn roar_render(roar: *mut roar_t, output: *mut roar_audio_block_t) -> c_int {
    std::panic::catch_unwind(|| {
        if roar.is_null() || output.is_null() {
            return OarError::InvalidParameter.into();
        }
        // SAFETY: The caller guarantees `roar` points to a valid FfiWrapper.
        let wrapper = unsafe { &mut *(roar as *mut FfiWrapper) };

        // SAFETY: We checked that `output` is not null. The caller guarantees that `output`
        // points to a valid `roar_audio_block_t` structure.
        let output_block = unsafe { &mut *output };
        let expected_samples = wrapper.renderer.config().samples_per_channel().value();
        let expected_channels = wrapper.renderer.config().output_layout().channels().get() as u32;
        if output_block.data.is_null()
            || output_block.channels != expected_channels
            || output_block.samples_per_channel < expected_samples
        {
            return OarError::InvalidParameter.into();
        }
        output_block.samples_per_channel = expected_samples;

        // TODO(b/525080422): Could we simplify the preparation of memory for rendering (i.e. the
        // creation of PlanarBufferRef), maybe by changing the way we store inside `wrapper.inputs`?
        let collected_inputs: Vec<(&u32, &Vec<Vec<f32>>)> = wrapper.inputs.iter().collect();

        let mut element_slices: Vec<Vec<&[f32]>> = Vec::new();
        for (_, chans) in &collected_inputs {
            let slices: Vec<&[f32]> = chans.iter().map(|c| c.as_slice()).collect();
            element_slices.push(slices);
        }

        let mut input_refs: Vec<(u32, PlanarBufferRef<'_, '_>)> = Vec::new();
        for (i, (&id, chans)) in collected_inputs.into_iter().enumerate() {
            let slices_ref = match PlanarBufferRef::new(
                element_slices[i].as_slice(),
                chans.len(),
                wrapper.renderer.config().samples_per_channel(),
            ) {
                Ok(r) => r,
                Err(e) => return e.into(),
            };
            input_refs.push((id, slices_ref));
        }

        if wrapper.renderer.active_groups().is_empty() {
            return OarError::InvalidParameter.into();
        }

        let mut slices_array: [&mut [f32]; MAX_OUTPUT_CHANNEL_COUNT] =
            std::array::from_fn(|_| &mut [] as &mut [f32]);
        // SAFETY: If non-null, the caller guarantees `output_block.data` points to aligned,
        // exclusively accessed storage for at least `output_block.channels * samples` floats.
        let mut out_buf = match unsafe {
            block_to_planar_buffer_mut(
                output_block,
                &mut slices_array,
                wrapper.renderer.config().samples_per_channel(),
            )
        } {
            Ok(r) => r,
            Err(e) => return e.into(),
        };

        let render_result = wrapper.renderer.render(&input_refs, &mut out_buf);
        drop(input_refs);
        drop(element_slices);
        wrapper.inputs.clear();

        match render_result {
            Ok(()) => ROAR_STATUS_OK,
            Err(e) => e.into(),
        }
    })
    .unwrap_or(OarError::InvalidParameter.into())
}

/// Enables/disables the loudness processor.
///
/// Returns `ROAR_STATUS_OK` (0) on success, or a negative error code (e.g. `-22` for invalid param)
/// on failure.
///
/// # Safety
///
/// The caller must guarantee that:
/// - `roar` points to a valid, initialized ROAR object.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn roar_enable_loudness_processor(roar: *mut roar_t, enable: c_int) -> c_int {
    std::panic::catch_unwind(|| {
        if roar.is_null() {
            return OarError::InvalidParameter.into();
        }
        // SAFETY: The caller guarantees `roar` points to a valid FfiWrapper.
        let wrapper = unsafe { &mut *(roar as *mut FfiWrapper) };
        let enable_bool = enable != 0;
        match wrapper.renderer.enable_loudness_processor(enable_bool) {
            Ok(()) => ROAR_STATUS_OK,
            Err(e) => e.into(),
        }
    })
    .unwrap_or(OarError::InvalidParameter.into())
}

/// Configures loudness adjustments for a group.
///
/// Returns `ROAR_STATUS_OK` (0) on success, or a negative error code (e.g. `-22` for invalid param/
/// group ID) on failure.
///
/// # Arguments
/// * `roar` - Pointer to the ROAR object.
/// * `gid` - The group ID to configure.
/// * `loudness` - The current loudness of the group, in dB.
/// * `target_loudness` - The target loudness for the group, in dB.
///
/// # Safety
///
/// The caller must guarantee that:
/// - `roar` points to a valid, initialized ROAR object.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn roar_set_loudness(
    roar: *mut roar_t,
    gid: u32,
    loudness: f32,
    target_loudness: f32,
) -> c_int {
    std::panic::catch_unwind(|| {
        if roar.is_null() {
            return OarError::InvalidParameter.into();
        }
        // SAFETY: The caller guarantees `roar` points to a valid FfiWrapper.
        let wrapper = unsafe { &mut *(roar as *mut FfiWrapper) };
        let group_id = match GroupId::try_from(gid) {
            Ok(id) => id,
            Err(e) => return e.into(),
        };
        let loudness = match Decibels::new(loudness) {
            Ok(l) => l,
            Err(e) => return e.into(),
        };
        let target_loudness = match Decibels::new(target_loudness) {
            Ok(l) => l,
            Err(e) => return e.into(),
        };
        match wrapper.renderer.set_loudness(group_id, loudness, target_loudness) {
            Ok(()) => ROAR_STATUS_OK,
            Err(e) => e.into(),
        }
    })
    .unwrap_or(OarError::InvalidParameter.into())
}

/// Enables/disables the dynamics limiter.
///
/// Returns `ROAR_STATUS_OK` (0) on success, or a negative error code (e.g. `-22` for invalid param)
/// on failure.
///
/// # Safety
///
/// The caller must guarantee that:
/// - `roar` points to a valid, initialized ROAR object.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn roar_enable_limiter(roar: *mut roar_t, enable: c_int) -> c_int {
    std::panic::catch_unwind(|| {
        if roar.is_null() {
            return OarError::InvalidParameter.into();
        }
        // SAFETY: The caller guarantees `roar` points to a valid FfiWrapper.
        let wrapper = unsafe { &mut *(roar as *mut FfiWrapper) };
        let enable_bool = enable != 0;
        match wrapper.renderer.enable_limiter(enable_bool) {
            Ok(()) => ROAR_STATUS_OK,
            Err(e) => e.into(),
        }
    })
    .unwrap_or(OarError::InvalidParameter.into())
}

/// Flushes the limiter's look-ahead delay buffer and resets limiter state.
///
/// On return, `output.samples_per_channel` is set to the number of flushed samples (`0` if the
/// limiter is disabled or has zero delay).
///
/// Returns `ROAR_STATUS_OK` (0) on success, or a negative error code (e.g. `-22` for invalid param)
/// on failure.
///
/// # Arguments
/// * `roar` - Pointer to the ROAR object.
/// * `output` - Output audio block whose `data` buffer must hold at least `output_channels *
///   roar_get_limiter_delay(roar)` floats when the limiter is enabled.
///
/// # Safety
///
/// The caller must guarantee that:
/// - `roar` is either null or points to a valid, initialized ROAR object created by `roar_create`.
/// - `output` is either null or points to a valid, initialized `roar_audio_block_t` (`output.data`
///   may also be null).
/// - If `roar`, `output`, and `output.data` are non-null and the limiter is enabled with non-zero
///   delay (`roar_get_limiter_delay(roar) > 0`), `output.data` points to an allocated, properly
///   aligned (`f32`), and exclusively accessed buffer of at least `output.channels as usize *
///   roar_get_limiter_delay(roar) as usize` floats (note that `output.samples_per_channel` is
///   ignored on entry).
/// - Neither `roar` nor `output` (nor its `data` buffer) is accessed or modified concurrently by
///   any other thread for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn roar_flush(roar: *mut roar_t, output: *mut roar_audio_block_t) -> c_int {
    std::panic::catch_unwind(|| {
        if roar.is_null() || output.is_null() {
            return OarError::InvalidParameter.into();
        }
        // SAFETY: We checked that `roar` is not null. The caller guarantees `roar` points to a
        // valid, initialized ROAR object with exclusive access for the duration of this call.
        let wrapper = unsafe { &mut *(roar as *mut FfiWrapper) };
        // SAFETY: We checked that `output` is not null. The caller guarantees `output` points to a
        // valid, initialized `roar_audio_block_t` with exclusive access for the duration of this
        // call.
        let output_block = unsafe { &mut *output };
        let out_channels = wrapper.renderer.config().output_layout().channels().get();

        if output_block.data.is_null()
            || output_block.channels as usize != out_channels
            || out_channels > MAX_OUTPUT_CHANNEL_COUNT
        {
            return OarError::InvalidParameter.into();
        }

        let delay = wrapper.renderer.limiter_delay();
        if !wrapper.renderer.is_limiter_enabled() || delay == 0 {
            output_block.samples_per_channel = 0;
            return ROAR_STATUS_OK;
        }

        let delay_samples = match Samples::new(delay as u32) {
            Ok(s) => s,
            Err(e) => return e.into(),
        };

        let mut slices_array: [&mut [f32]; MAX_OUTPUT_CHANNEL_COUNT] =
            std::array::from_fn(|_| &mut [] as &mut [f32]);
        // SAFETY: `output_block.data` was checked non-null above, and the limiter is enabled with
        // `delay > 0`. The caller guarantees `output_block.data` points to an allocated, aligned
        // (`f32`), exclusively accessed buffer of at least `output_block.channels as usize * delay`
        // floats for the duration of this call.
        let mut out_buf = match unsafe {
            block_to_planar_buffer_mut(output_block, &mut slices_array, delay_samples)
        } {
            Ok(r) => r,
            Err(e) => return e.into(),
        };

        match wrapper.renderer.flush(&mut out_buf) {
            Ok(flushed) => {
                output_block.samples_per_channel = flushed as u32;
                ROAR_STATUS_OK
            }
            Err(e) => {
                output_block.samples_per_channel = 0;
                e.into()
            }
        }
    })
    .unwrap_or(OarError::InvalidParameter.into())
}

/// Sets the peak limiter threshold in dBFS.
///
/// Values outside `[-60.0, 0.0]` dBFS are clamped to that range. Can be called before or after
/// `roar_enable_limiter`, and between `roar_render` calls without resetting delay buffers.
///
/// Returns `ROAR_STATUS_OK` (0) on success, or a negative error code (e.g. `-22` for invalid param)
/// on failure.
///
/// # Arguments
/// * `roar` - Pointer to the ROAR object.
/// * `threshold_db` - Target threshold in dBFS; must be finite.
///
/// # Safety
///
/// The caller must guarantee that:
/// - `roar` is either null or points to a valid, initialized ROAR object created by `roar_create`
///   that is not accessed or modified concurrently for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn roar_set_limiter_threshold(roar: *mut roar_t, threshold_db: f32) -> c_int {
    std::panic::catch_unwind(|| {
        if roar.is_null() {
            return OarError::InvalidParameter.into();
        }
        // SAFETY: We checked that `roar` is not null. The caller guarantees `roar` points to a
        // valid, initialized ROAR object with exclusive access for the duration of this call.
        let wrapper = unsafe { &mut *(roar as *mut FfiWrapper) };
        let threshold = match Decibels::new(threshold_db) {
            Ok(t) => t,
            Err(e) => return e.into(),
        };
        match wrapper.renderer.set_limiter_threshold(threshold) {
            Ok(()) => ROAR_STATUS_OK,
            Err(e) => e.into(),
        }
    })
    .unwrap_or(OarError::InvalidParameter.into())
}

/// Returns the limiter look-ahead delay in samples per channel, or `0` if `roar` is null.
///
/// # Safety
///
/// The caller must guarantee that:
/// - `roar` is either null or points to a valid, initialized ROAR object created by `roar_create`
///   that is not modified concurrently for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn roar_get_limiter_delay(roar: *const roar_t) -> c_int {
    std::panic::catch_unwind(|| {
        if roar.is_null() {
            return 0;
        }
        // SAFETY: We checked that `roar` is not null. The caller guarantees `roar` points to a
        // valid, initialized ROAR object not modified concurrently for the duration of this call.
        let wrapper = unsafe { &*(roar as *const FfiWrapper) };
        wrapper.renderer.limiter_delay() as c_int
    })
    .unwrap_or(0)
}

/// Enables/disables listener head tracking.
///
/// Returns `ROAR_STATUS_OK` (0) on success, or a negative error code (e.g. `-22` for invalid param,
/// `-95` if layout not Binaural) on failure.
///
/// # Safety
///
/// The caller must guarantee that:
/// - `roar` points to a valid, initialized ROAR object.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn roar_enable_head_tracking(roar: *mut roar_t, enable: c_int) -> c_int {
    std::panic::catch_unwind(|| {
        if roar.is_null() {
            return OarError::InvalidParameter.into();
        }
        // SAFETY: The caller guarantees `roar` points to a valid FfiWrapper.
        let wrapper = unsafe { &mut *(roar as *mut FfiWrapper) };
        let enable_bool = enable != 0;
        match wrapper.renderer.enable_head_tracking(enable_bool) {
            Ok(()) => ROAR_STATUS_OK,
            Err(e) => e.into(),
        }
    })
    .unwrap_or(OarError::InvalidParameter.into())
}

/// Returns the samples per channel for the renderer.
///
/// Returns the samples per channel value on success, or 0 if `roar` is invalid/null.
///
/// # Safety
///
/// The caller must guarantee that:
/// - `roar` points to a valid, initialized ROAR object.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn roar_get_samples_per_channel(roar: *const roar_t) -> u32 {
    std::panic::catch_unwind(|| {
        if roar.is_null() {
            return 0;
        }
        // SAFETY: The caller guarantees `roar` points to a valid FfiWrapper.
        let wrapper = unsafe { &*(roar as *const FfiWrapper) };
        wrapper.renderer.config().samples_per_channel().value()
    })
    .unwrap_or(0)
}

/// Returns the sampling rate for the renderer.
///
/// Returns the sampling rate value on success, or 0 if `roar` is invalid/null.
///
/// # Safety
///
/// The caller must guarantee that:
/// - `roar` points to a valid, initialized ROAR object.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn roar_get_sampling_rate(roar: *const roar_t) -> u32 {
    std::panic::catch_unwind(|| {
        if roar.is_null() {
            return 0;
        }
        // SAFETY: The caller guarantees `roar` points to a valid FfiWrapper.
        let wrapper = unsafe { &*(roar as *const FfiWrapper) };
        wrapper.renderer.config().sample_rate().value()
    })
    .unwrap_or(0)
}

/// Returns the channel count for a given audio element ID.
///
/// Returns the channel count on success, or 0 if `roar` is invalid/null or element not found.
///
/// # Safety
///
/// The caller must guarantee that:
/// - `roar` points to a valid, initialized ROAR object.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn roar_get_number_of_audio_element_channels(
    roar: *const roar_t,
    id: u32,
) -> u32 {
    std::panic::catch_unwind(|| {
        if roar.is_null() {
            return 0;
        }
        // SAFETY: The caller guarantees `roar` points to a valid FfiWrapper.
        let wrapper = unsafe { &*(roar as *const FfiWrapper) };
        wrapper.renderer.element_channels(id).map_or(0, |c| c.get() as u32)
    })
    .unwrap_or(0)
}

/// Returns the output channel count for the renderer.
///
/// Returns the output channel count on success, or 0 if `roar` is invalid/null.
///
/// # Safety
///
/// The caller must guarantee that:
/// - `roar` points to a valid, initialized ROAR object.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn roar_get_number_of_output_channels(roar: *const roar_t) -> u32 {
    std::panic::catch_unwind(|| {
        if roar.is_null() {
            return 0;
        }
        // SAFETY: The caller guarantees `roar` points to a valid FfiWrapper.
        let wrapper = unsafe { &*(roar as *const FfiWrapper) };
        wrapper.renderer.config().output_layout().channels().get() as u32
    })
    .unwrap_or(0)
}

/// Returns the total count of registered audio elements.
///
/// Returns the total element count on success, or 0 if `roar` is invalid/null.
///
/// # Safety
///
/// The caller must guarantee that:
/// - `roar` points to a valid, initialized ROAR object.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn roar_get_number_of_audio_elements(roar: *const roar_t) -> u32 {
    std::panic::catch_unwind(|| {
        if roar.is_null() {
            return 0;
        }
        // SAFETY: The caller guarantees `roar` points to a valid FfiWrapper.
        let wrapper = unsafe { &*(roar as *const FfiWrapper) };
        wrapper.renderer.number_of_audio_elements() as u32
    })
    .unwrap_or(0)
}
