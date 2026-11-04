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

//! Look-ahead dynamics peak limiter implementation.
//!
//! This module implements a look-ahead peak limiter with quadratic attack and release
//! curves, delay-line buffering, and a hard-clipping safety stage. It mirrors
//! `audio_effect_peak_limiter_t` and `oar_limiter_t` in `liboar/src/limiter/` and
//! performs zero heap allocations during real-time block processing.

use crate::common::definitions::ChannelCount;
use crate::common::definitions::{
    Decibels, LinearGain, Milliseconds, OarError, PlanarBufferMut, SampleRate, Samples,
};

const THRESHOLD_DB_MIN: f32 = -60.0;
const THRESHOLD_DB_MAX: f32 = 0.0;

// In OBR, these values are `#define` values in `oar.c`, but are still clamped within ranges in
// `audio_effect_peak_limiter.c`.  Since these values are hard-coded here, we don't bother clamping
// them.
/// Default limiter ceiling threshold (-1.0 dBFS).
pub const LIMITER_THRESHOLD_DBFS: Decibels = Decibels::from_const(-1.0);
/// Default limiter attack duration (1 ms).
pub const LIMITER_ATTACK: Milliseconds = Milliseconds::from_const(1.0);
/// Default limiter release duration (200 ms).
pub const LIMITER_RELEASE: Milliseconds = Milliseconds::from_const(200.0);
/// Default limiter look-ahead duration (5 ms).
pub const LIMITER_LOOK_AHEAD: Milliseconds = Milliseconds::from_const(5.0);

/// Evaluates the quadratic acceleration curve `1.0 - (x - 1.0)^2`, clamped to `[0.0, 1.0]`.
#[inline]
fn curve_accel(x: f32) -> f32 {
    let d = x.clamp(0.0, 1.0) - 1.0;
    1.0 - d * d
}

/// Converts a dBFS threshold to a linear amplitude ceiling after clamping to
/// `[THRESHOLD_DB_MIN, THRESHOLD_DB_MAX]`.
#[inline]
fn threshold_db_to_linear(threshold_db: Decibels) -> LinearGain {
    let clamped_db = threshold_db.value().clamp(THRESHOLD_DB_MIN, THRESHOLD_DB_MAX);
    LinearGain::from(Decibels::from_const(clamped_db))
}

/// Look-ahead dynamics peak limiter with zero dynamic allocation inside real-time block loops.
///
/// Mirrors `oar_limiter_t` and `audio_effect_peak_limiter_t` in `liboar/src/limiter/`.
#[derive(Debug, Clone, PartialEq)]
pub struct OarLimiter {
    /// Linear-domain amplitude ceiling derived from the dBFS threshold.
    linear_threshold: LinearGain,
    /// Attack duration in samples.
    attack_samples: Samples,
    /// Release duration in samples.
    release_samples: Samples,
    /// Precomputed `1.0 / attack_samples` to avoid per-sample division during attack.
    inv_attack_samples: f32,
    /// Precomputed `1.0 / release_samples` to avoid per-sample division during release.
    inv_release_samples: f32,
    /// Number of audio channels processed by the limiter.
    num_channels: ChannelCount,
    /// Look-ahead delay length in samples per channel (`None` indicates no-delay mode).
    delay_size: Option<Samples>,
    /// Currently applied linear gain envelope value.
    current_gain: LinearGain,
    /// Linear gain value at the start of the active attack transition.
    target_start_gain: LinearGain,
    /// Target linear gain value at the end of the active attack transition.
    target_end_gain: LinearGain,
    /// Elapsed sample counter within the active attack/release envelope (`None` when idle).
    current_sample: Option<u32>,
    /// Circular look-ahead audio delay line of length `delay_size * num_channels`.
    /// Contains the buffered multi-channel input frames waiting to be emitted, stored in
    /// frame-interleaved layout (`frame * num_channels + ch`) and indexed by `entry_index`.
    delay_data: Vec<f32>,
    /// Circular buffer of length `delay_size` containing the cross-channel peak amplitude
    /// (`max_ch |sample|`) for each frame buffered in `delay_data`, indexed by `entry_index`.
    peak_data: Vec<f32>,
    /// Circular monotonic deque of capacity `delay_size` containing frame indices into `peak_data`
    /// ordered strictly decreasing by peak value from front (`deque_head`, window maximum) to
    /// back.
    peak_deque: Vec<usize>,
    /// Front index of `peak_deque`.
    deque_head: usize,
    /// Number of active indices in `peak_deque`.
    deque_len: usize,
    /// Current write cursor into the circular delay and peak buffers.
    entry_index: usize,
}

impl OarLimiter {
    /// Creates a new `OarLimiter`.
    ///
    /// # Arguments
    /// * `sample_rate` - Audio sample rate in Hz.
    /// * `num_channels` - Number of audio channels.
    pub fn new(sample_rate: SampleRate, num_channels: ChannelCount) -> Result<Self, OarError> {
        let raw_delay = sample_rate * LIMITER_LOOK_AHEAD;
        let delay_size = if raw_delay.value() > 0 { Some(raw_delay) } else { None };
        let delay_samples_len = delay_size.map_or(0, usize::from);
        let attack_samples = sample_rate * LIMITER_ATTACK;
        let release_samples = sample_rate * LIMITER_RELEASE;
        Ok(Self {
            linear_threshold: threshold_db_to_linear(LIMITER_THRESHOLD_DBFS),
            attack_samples,
            release_samples,
            inv_attack_samples: 1.0 / attack_samples.value() as f32,
            inv_release_samples: 1.0 / release_samples.value() as f32,
            num_channels,
            delay_size,
            current_gain: LinearGain::ONE,
            target_start_gain: LinearGain::ZERO,
            target_end_gain: LinearGain::ZERO,
            current_sample: None,
            delay_data: vec![0.0; delay_samples_len * num_channels.get()],
            peak_data: vec![0.0; delay_samples_len],
            peak_deque: vec![0; delay_samples_len],
            deque_head: 0,
            deque_len: 0,
            entry_index: 0,
        })
    }

    /// Processes a multi-channel planar audio buffer in-place through the look-ahead limiter.
    ///
    /// During the initial `delay_size` samples after creation or reset, the delay line emits
    /// priming zeros while buffering the incoming audio.
    ///
    /// # Arguments
    /// * `buffer` - Planar audio buffer whose channel count must match `num_channels` and whose
    ///   sample count must be greater than 0.
    pub fn process_block(&mut self, buffer: &mut PlanarBufferMut<'_, '_>) -> Result<(), OarError> {
        let num_channels = buffer.num_channels();
        let frame_size = buffer.num_samples();
        if num_channels != self.num_channels.get() || frame_size == 0 {
            return Err(OarError::InvalidParameter);
        }
        self.process_core(buffer, frame_size, false);
        Ok(())
    }

    /// Drains the look-ahead delay buffer into `output` by feeding `delay_size` zero samples, then
    /// resets the limiter state to its post-creation initial values.
    ///
    /// # Arguments
    /// * `output` - Destination planar buffer with `num_channels` channels and at least
    ///   `delay_size` samples per channel.
    ///
    /// # Returns
    /// The number of flushed samples per channel (`delay_size`).
    pub fn flush(&mut self, output: &mut PlanarBufferMut<'_, '_>) -> Result<usize, OarError> {
        if output.num_channels() != self.num_channels.get() {
            return Err(OarError::InvalidParameter);
        }
        let Some(delay_samples) = self.delay_size else {
            self.reset();
            return Ok(0);
        };
        let delay_len = usize::from(delay_samples);
        if output.num_samples() < delay_len {
            return Err(OarError::InvalidParameter);
        }
        self.process_core(output, delay_len, true);
        self.reset();
        Ok(delay_len)
    }

    /// Resets the limiter gain envelope and delay buffers to their post-creation state.
    pub fn reset(&mut self) {
        self.entry_index = 0;
        self.deque_head = 0;
        self.deque_len = 0;
        self.current_gain = LinearGain::ONE;
        self.target_start_gain = LinearGain::ZERO;
        self.target_end_gain = LinearGain::ZERO;
        self.current_sample = None;
        self.delay_data.fill(0.0);
        self.peak_data.fill(0.0);
    }

    /// Updates the limiter threshold ceiling in-place without resetting delay buffers or envelope
    /// state.
    ///
    /// # Arguments
    /// * `threshold_db` - New threshold in dBFS; clamped to `[-60.0, 0.0]`.
    pub fn set_threshold(&mut self, threshold_db: Decibels) {
        self.linear_threshold = threshold_db_to_linear(threshold_db);
    }

    /// Returns the look-ahead delay in samples per channel.
    pub fn delay(&self) -> usize {
        self.delay_size.map_or(0, usize::from)
    }

    /// Core per-sample processing loop shared between `process_block` and `flush`.
    fn process_core(
        &mut self,
        buffer: &mut PlanarBufferMut<'_, '_>,
        frame_size: usize,
        is_flush: bool,
    ) {
        let ceiling = self.linear_threshold.value();
        let num_channels = self.num_channels.get();
        let delay_len = self.delay();
        let mut idx = self.entry_index;
        for k in 0..frame_size {
            // Advance the active envelope first using the current look-ahead peak so attenuation
            // reaches target gain right as the buffered peak arrives at the output, then check if
            // a new attack ramp must begin for upcoming samples.
            let peak = self.get_lookahead_peak();
            let gain = self.update_gain_envelope().value();
            self.detect_peak(peak);
            let mut peak_max = 0.0_f32;
            if delay_len > 0 {
                let frame_start = idx * num_channels;
                for ch in 0..num_channels {
                    let channel_slice = buffer.channel_mut(ch);
                    // Feed silence when draining the delay line during flush; otherwise read input.
                    let input_sample = if is_flush { 0.0 } else { channel_slice[k] };
                    // Swap the oldest delayed sample out to the output and store the new sample.
                    let delay_slot = &mut self.delay_data[frame_start + ch];
                    let out = *delay_slot * gain;
                    *delay_slot = input_sample;
                    channel_slice[k] = out.clamp(-ceiling, ceiling);
                    peak_max = peak_max.max(input_sample.abs());
                }
                // Record the cross-channel peak for this frame and advance the ring cursor.
                self.update_peak_data(idx, peak_max, delay_len);
                idx += 1;
                if idx == delay_len {
                    idx = 0;
                }
            } else {
                // Zero look-ahead delay: apply envelope gain and ceiling clamp directly in place.
                for ch in 0..num_channels {
                    let channel_slice = buffer.channel_mut(ch);
                    let input_sample = if is_flush { 0.0 } else { channel_slice[k] };
                    let out = input_sample * gain;
                    channel_slice[k] = out.clamp(-ceiling, ceiling);
                }
            }
        }
        self.entry_index = idx;
    }

    /// Returns the maximum sample peak in the pre-found monotonic peak deque, or `0.0` if empty.
    #[inline]
    fn get_lookahead_peak(&self) -> f32 {
        if self.deque_len == 0 {
            0.0
        } else {
            self.peak_data[self.peak_deque[self.deque_head]]
        }
    }

    /// Advances the attack/release envelope state machine by one sample and returns the resulting
    /// linear gain.
    #[inline]
    fn update_gain_envelope(&mut self) -> LinearGain {
        if let Some(ref mut sample_count) = self.current_sample {
            let attack_len = self.attack_samples.value();
            let release_len = self.release_samples.value();
            if *sample_count < attack_len {
                *sample_count += 1;
                let acc_ratio = curve_accel(*sample_count as f32 * self.inv_attack_samples);
                self.current_gain = self.target_start_gain
                    - (self.target_start_gain - self.target_end_gain) * acc_ratio;
            } else if *sample_count < release_len + attack_len {
                *sample_count += 1;
                let acc_ratio =
                    curve_accel((*sample_count - attack_len) as f32 * self.inv_release_samples);
                self.current_gain =
                    self.target_end_gain + (LinearGain::ONE - self.target_end_gain) * acc_ratio;
            } else {
                self.current_sample = None;
                self.current_gain = LinearGain::ONE;
            }
        } else {
            // No active attack/release transition; pass audio through at unity gain.
            self.current_gain = LinearGain::ONE;
        }
        self.current_gain
    }

    /// Starts or restarts an attack transition if the look-ahead `peak` scaled by `current_gain`
    /// exceeds `linear_threshold`.
    #[inline]
    fn detect_peak(&mut self, peak: f32) {
        let ceiling = self.linear_threshold.value();
        if peak * self.current_gain.value() > ceiling {
            self.target_start_gain = self.current_gain;
            self.target_end_gain = LinearGain::new(ceiling / peak).unwrap_or(LinearGain::ONE);
            self.current_sample = Some(0);
        }
    }

    /// Writes `peak_max` into the circular `peak_data` buffer at `idx` and updates the monotonic
    /// `peak_deque` in amortized constant time.
    ///
    /// # Arguments
    /// * `idx` - Current write cursor position in the circular `peak_data` buffer.
    /// * `peak_max` - Maximum absolute sample value across all channels for the current frame.
    /// * `delay_len` - Capacity of the circular look-ahead buffers.
    #[inline]
    fn update_peak_data(&mut self, idx: usize, peak_max: f32, delay_len: usize) {
        // If the window maximum at the front of the deque resides at `idx`, that sample just
        // exited the delay line and is being overwritten, so pop it from the front.
        if self.deque_len > 0 && self.peak_deque[self.deque_head] == idx {
            self.deque_head += 1;
            if self.deque_head == delay_len {
                self.deque_head = 0;
            }
            self.deque_len -= 1;
        }
        // Discard any older tail entries whose peaks are `<= peak_max`: they expire sooner than
        // `idx` and can never become the window maximum ahead of `peak_max`.
        while self.deque_len > 0 {
            let back_pos = (self.deque_head + self.deque_len - 1) % delay_len;
            if self.peak_data[self.peak_deque[back_pos]] <= peak_max {
                self.deque_len -= 1;
            } else {
                break;
            }
        }
        // Only track positive peaks so an all-silent window leaves `deque_len == 0`.
        if peak_max > 0.0 {
            let push_pos = (self.deque_head + self.deque_len) % delay_len;
            self.peak_deque[push_pos] = idx;
            self.deque_len += 1;
        }
        // Update `peak_data[idx]` last so tail comparisons above inspect the prior values.
        self.peak_data[idx] = peak_max;
    }
}

// ===== Tests =====

#[cfg(test)]
mod test {

    use super::*;
    use crate::common::definitions::{AudioBuffer, Samples};
    use googletest::prelude::*;

    const BLOCK_SIZE: usize = 480;
    const NUM_CHANNELS: usize = 2;
    const SAMPLE_RATE_HZ: u32 = 48000;
    // 5.0 ms * 48000 Hz = 240 samples.
    const EXPECTED_DELAY_SAMPLES: usize = 240;
    const EPSILON: f32 = 1e-5;

    /// Wraps an `AudioBuffer` in a `PlanarBufferMut` for test invocations.
    fn wrap_audio_buffer<'a, 'b>(
        buf: &'a mut AudioBuffer,
        slices: &'b mut [&'a mut [f32]],
    ) -> PlanarBufferMut<'a, 'b> {
        let chans = buf.num_channels();
        let samples = buf.num_frames();
        buf.as_slices_mut(slices);
        PlanarBufferMut::new(&mut slices[..chans], chans, Samples::new(samples as u32).unwrap())
            .unwrap()
    }

    /// Creates a stereo 48 kHz limiter with 0 dBFS threshold and 5 ms (240-sample) look-ahead.
    fn make_limiter() -> OarLimiter {
        let mut limiter = OarLimiter::new(
            SampleRate::new(SAMPLE_RATE_HZ).unwrap(),
            ChannelCount::new(NUM_CHANNELS).unwrap(),
        )
        .unwrap();
        limiter.set_threshold(Decibels::new(0.0).unwrap());
        limiter
    }

    #[gtest]
    fn delay_returns_configured_look_ahead_samples() {
        let limiter = make_limiter();

        let delay = limiter.delay();

        expect_eq!(delay, EXPECTED_DELAY_SAMPLES);
    }

    #[gtest]
    fn process_block_emits_priming_zeros_then_attenuates_peak_above_ceiling() {
        let mut limiter = make_limiter();
        let mut signal = AudioBuffer::new(NUM_CHANNELS, BLOCK_SIZE);
        signal.fill(2.0);
        let mut slices: [&mut [f32]; NUM_CHANNELS] =
            std::array::from_fn(|_| &mut [] as &mut [f32]);
        let mut buf = wrap_audio_buffer(&mut signal, &mut slices);

        limiter.process_block(&mut buf).unwrap();

        for ch_idx in 0..NUM_CHANNELS {
            let ch = signal.channel(ch_idx).as_slice();
            expect_eq!(&ch[..EXPECTED_DELAY_SAMPLES], &[0.0; EXPECTED_DELAY_SAMPLES]);
            for &sample in &ch[EXPECTED_DELAY_SAMPLES..] {
                expect_that!(sample, gt(0.0));
                expect_that!(sample, le(1.00001));
            }
        }
    }

    #[gtest]
    fn process_block_recovers_gain_gradually_on_quiet_signal() {
        let mut limiter = make_limiter();
        let mut loud_block = AudioBuffer::new(NUM_CHANNELS, BLOCK_SIZE);
        loud_block.fill(2.0);
        let mut quiet_block = AudioBuffer::new(NUM_CHANNELS, BLOCK_SIZE);
        quiet_block.fill(0.1);

        {
            let mut slices: [&mut [f32]; NUM_CHANNELS] =
                std::array::from_fn(|_| &mut [] as &mut [f32]);
            let mut buf = wrap_audio_buffer(&mut loud_block, &mut slices);
            limiter.process_block(&mut buf).unwrap();
        }
        {
            let mut slices: [&mut [f32]; NUM_CHANNELS] =
                std::array::from_fn(|_| &mut [] as &mut [f32]);
            let mut buf = wrap_audio_buffer(&mut quiet_block, &mut slices);
            limiter.process_block(&mut buf).unwrap();
        }
        let attenuated_sample = quiet_block.channel(0).as_slice()[EXPECTED_DELAY_SAMPLES];
        for _ in 0..25 {
            quiet_block.fill(0.1);
            let mut slices: [&mut [f32]; NUM_CHANNELS] =
                std::array::from_fn(|_| &mut [] as &mut [f32]);
            let mut buf = wrap_audio_buffer(&mut quiet_block, &mut slices);
            limiter.process_block(&mut buf).unwrap();
        }
        let recovered_sample = quiet_block.channel(0).as_slice()[BLOCK_SIZE - 1];

        expect_that!(attenuated_sample, lt(0.06));
        expect_that!(recovered_sample, near(0.1, EPSILON));
    }

    #[gtest]
    fn process_block_and_flush_preserve_sub_threshold_signal_shifted_by_delay() {
        let mut limiter = make_limiter();
        let mut signal = AudioBuffer::new(NUM_CHANNELS, BLOCK_SIZE);
        signal.fill(0.5);
        let mut flush_out = AudioBuffer::new(NUM_CHANNELS, EXPECTED_DELAY_SAMPLES);

        {
            let mut slices: [&mut [f32]; NUM_CHANNELS] =
                std::array::from_fn(|_| &mut [] as &mut [f32]);
            let mut buf = wrap_audio_buffer(&mut signal, &mut slices);
            limiter.process_block(&mut buf).unwrap();
        }
        let flushed = {
            let mut slices: [&mut [f32]; NUM_CHANNELS] =
                std::array::from_fn(|_| &mut [] as &mut [f32]);
            let mut buf = wrap_audio_buffer(&mut flush_out, &mut slices);
            limiter.flush(&mut buf).unwrap()
        };

        expect_eq!(flushed, EXPECTED_DELAY_SAMPLES);
        for ch_idx in 0..NUM_CHANNELS {
            let ch = signal.channel(ch_idx).as_slice();
            expect_eq!(&ch[..EXPECTED_DELAY_SAMPLES], &[0.0; EXPECTED_DELAY_SAMPLES]);
            expect_eq!(&ch[EXPECTED_DELAY_SAMPLES..], &[0.5; EXPECTED_DELAY_SAMPLES]);
            expect_eq!(flush_out.channel(ch_idx).as_slice(), &[0.5; EXPECTED_DELAY_SAMPLES]);
        }
    }

    #[gtest]
    fn set_threshold_updates_ceiling_for_subsequent_blocks() {
        let mut limiter = make_limiter();
        let mut signal = AudioBuffer::new(NUM_CHANNELS, BLOCK_SIZE);
        signal.fill(0.8);

        limiter.set_threshold(Decibels::new(-6.0).unwrap());
        let mut slices: [&mut [f32]; NUM_CHANNELS] =
            std::array::from_fn(|_| &mut [] as &mut [f32]);
        let mut buf = wrap_audio_buffer(&mut signal, &mut slices);
        limiter.process_block(&mut buf).unwrap();

        // -6.0 dBFS corresponds to a linear ceiling of ~0.501187.
        for &sample in &signal.channel(0).as_slice()[EXPECTED_DELAY_SAMPLES..] {
            expect_that!(sample, le(0.5012));
        }
    }

    #[gtest]
    fn process_block_returns_invalid_parameter_for_channel_count_mismatch() {
        let mut limiter = make_limiter();
        let mut wrong_channels_buffer = AudioBuffer::new(1, BLOCK_SIZE);
        let mut slices: [&mut [f32]; 1] = std::array::from_fn(|_| &mut [] as &mut [f32]);
        let mut buf = wrap_audio_buffer(&mut wrong_channels_buffer, &mut slices);

        let result = limiter.process_block(&mut buf);

        expect_eq!(result.unwrap_err(), OarError::InvalidParameter);
    }

    #[gtest]
    fn circular_peak_cache_rescans_when_impulse_ages_out_of_window() {
        let mut limiter = make_limiter();
        let mut block_with_impulse = AudioBuffer::new(NUM_CHANNELS, BLOCK_SIZE);
        block_with_impulse.fill(0.2);
        // Place a single large spike at sample 0 followed by a moderate spike at sample 100.
        block_with_impulse.channel_mut(0).as_mut_slice()[0] = 3.0;
        block_with_impulse.channel_mut(0).as_mut_slice()[100] = 1.5;

        let mut slices: [&mut [f32]; NUM_CHANNELS] =
            std::array::from_fn(|_| &mut [] as &mut [f32]);
        let mut buf = wrap_audio_buffer(&mut block_with_impulse, &mut slices);
        limiter.process_block(&mut buf).unwrap();

        let out_ch0 = block_with_impulse.channel(0).as_slice();
        expect_eq!(&out_ch0[..EXPECTED_DELAY_SAMPLES], &[0.0; EXPECTED_DELAY_SAMPLES]);
        for &sample in &out_ch0[EXPECTED_DELAY_SAMPLES..] {
            expect_that!(sample, le(1.00001));
        }
    }

    #[gtest]
    fn reset_and_flush_re_prime_delay_line_and_validate_flush_buffer() {
        let mut limiter = make_limiter();
        let mut signal = AudioBuffer::new(NUM_CHANNELS, BLOCK_SIZE);
        signal.fill(0.5);
        let mut too_small_flush = AudioBuffer::new(NUM_CHANNELS, EXPECTED_DELAY_SAMPLES - 1);
        let mut post_reset_block = AudioBuffer::new(NUM_CHANNELS, BLOCK_SIZE);
        post_reset_block.fill(0.5);

        let small_flush_err = {
            let mut slices: [&mut [f32]; NUM_CHANNELS] =
                std::array::from_fn(|_| &mut [] as &mut [f32]);
            let mut buf = wrap_audio_buffer(&mut too_small_flush, &mut slices);
            limiter.flush(&mut buf).unwrap_err()
        };
        {
            let mut slices: [&mut [f32]; NUM_CHANNELS] =
                std::array::from_fn(|_| &mut [] as &mut [f32]);
            let mut buf = wrap_audio_buffer(&mut signal, &mut slices);
            limiter.process_block(&mut buf).unwrap();
        }
        limiter.reset();
        {
            let mut slices: [&mut [f32]; NUM_CHANNELS] =
                std::array::from_fn(|_| &mut [] as &mut [f32]);
            let mut buf = wrap_audio_buffer(&mut post_reset_block, &mut slices);
            limiter.process_block(&mut buf).unwrap();
        }

        expect_eq!(small_flush_err, OarError::InvalidParameter);
        expect_eq!(
            &post_reset_block.channel(0).as_slice()[..EXPECTED_DELAY_SAMPLES],
            &[0.0; EXPECTED_DELAY_SAMPLES]
        );
        expect_eq!(
            &post_reset_block.channel(0).as_slice()[EXPECTED_DELAY_SAMPLES..],
            &[0.5; EXPECTED_DELAY_SAMPLES]
        );
    }
}
