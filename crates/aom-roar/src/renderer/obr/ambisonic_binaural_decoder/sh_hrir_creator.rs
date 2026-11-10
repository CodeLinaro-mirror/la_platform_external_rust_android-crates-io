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

//! Spherical harmonic HRIR loading and creation.
//!
//! Provides routines to synthesize and load spherical harmonic Head-Related Impulse Response (HRIR)
//! filters from embedded Rice-encoded static data.

use super::binaural_filters::BinauralFiltersWrapper;
use super::resampler::Resampler;
use super::rice_coding::RiceFilterDecoder;
use crate::common::audio_buffer::AudioBuffer;
use crate::renderer::obr::common::ambisonic_utils::is_valid_ambisonic_order;

use crate::common::definitions::SampleRate;

/// Native sampling rate (in Hz) of the embedded spherical harmonic HRIR filter assets.
const HRIR_NATIVE_SAMPLE_RATE_HZ: i32 = 48000;

/// Synthesizes an `AudioBuffer` of planar SH-HRIR filters from Rice-encoded planar filter bytes.
///
/// # Arguments
/// * `encoded_bytes` - Rice-encoded planar filter asset starting with `[num_channels,
///   blocks_per_channel]`.
/// * `target_sample_rate` - Desired output sampling rate; resamples if different from 48 kHz.
/// * `resampler` - Reusable resampler instance used when `target_sample_rate` differs from 48 kHz.
pub fn create_sh_hrirs_from_encoded_bytes(
    encoded_bytes: &[u8],
    target_sample_rate: SampleRate,
    resampler: &mut Resampler,
) -> Option<AudioBuffer> {
    let sh_hrirs = RiceFilterDecoder::decode_to_audio_buffer(encoded_bytes)?;
    let num_channels = sh_hrirs.num_channels();
    if !is_valid_ambisonic_order(num_channels) {
        return None;
    }

    let sh_hrir_length = sh_hrirs.num_frames();
    let target_sample_rate_hz = target_sample_rate.value() as i32;
    assert!(target_sample_rate_hz > 0);
    if HRIR_NATIVE_SAMPLE_RATE_HZ != target_sample_rate_hz {
        assert!(
            Resampler::are_sample_rates_supported(
                HRIR_NATIVE_SAMPLE_RATE_HZ,
                target_sample_rate_hz
            ),
            "Unsupported sampling rates for loading HRIRs: {}, {}",
            HRIR_NATIVE_SAMPLE_RATE_HZ,
            target_sample_rate_hz
        );
        resampler.reset_state();
        resampler.set_rate_and_num_channels(
            HRIR_NATIVE_SAMPLE_RATE_HZ,
            target_sample_rate_hz,
            num_channels,
        );
        let mut resampled =
            AudioBuffer::new(num_channels, resampler.get_next_output_length(sh_hrir_length));
        resampler.process(&sh_hrirs, &mut resampled);
        // The resampler preserves the waveform of the impulse responses, but the gain of the filter
        // realized by discrete convolution is proportional to the rate at which that waveform is
        // sampled: convolving at the target rate accumulates target/wav times as many taps of the
        // same continuous curve.  Rescale so the realized frequency response, and thus the rendered
        // loudness, is invariant to the target rate.
        let filter_gain_compensation =
            (HRIR_NATIVE_SAMPLE_RATE_HZ as f32) / (target_sample_rate_hz as f32);
        for channel in 0..num_channels {
            for sample in resampled.channel_mut(channel).as_mut_slice() {
                *sample *= filter_gain_compensation;
            }
        }
        return Some(resampled);
    }
    Some(sh_hrirs)
}

/// Synthesizes an `AudioBuffer` of planar SH-HRIR filters from embedded Rice-encoded assets.
pub fn create_sh_hrirs_from_assets(
    filename: &str,
    target_sample_rate: SampleRate,
    resampler: &mut Resampler,
) -> Option<AudioBuffer> {
    let wrapper = BinauralFiltersWrapper::new();
    let data = wrapper.get_file(filename)?;
    create_sh_hrirs_from_encoded_bytes(data, target_sample_rate, resampler)
}

#[cfg(test)]
mod test {
    use super::*;
    use googletest::prelude::*;
    use rstest::rstest;

    /// The bundled HRIR assets are recorded at this rate; loading at it performs no resampling,
    /// making it the reference for the invariance tests.
    const HRIR_NATIVE_RATE_HZ: u32 = 48000;

    /// Computes the magnitude of the filter's discrete-time Fourier transform (DTFT) at
    /// `frequency_hz`.
    fn filter_magnitude_at(
        filter: &AudioBuffer,
        channel: usize,
        frequency_hz: f64,
        sample_rate_hz: u32,
    ) -> f64 {
        let mut real = 0.0f64;
        let mut imag = 0.0f64;
        let samples = filter.channel(channel).as_slice();
        for (n, &sample) in samples.iter().enumerate() {
            let phase =
                -std::f64::consts::TAU * frequency_hz * (n as f64) / (sample_rate_hz as f64);
            real += (sample as f64) * phase.cos();
            imag += (sample as f64) * phase.sin();
        }
        real.hypot(imag)
    }

    /// Computes the L2 norm of the frequency response magnitudes across all channels.
    fn filter_magnitude_norm_at(
        filter: &AudioBuffer,
        frequency_hz: f64,
        sample_rate_hz: u32,
    ) -> f64 {
        let sum_squares: f64 = (0..filter.num_channels())
            .map(|ch| {
                let mag = filter_magnitude_at(filter, ch, frequency_hz, sample_rate_hz);
                mag * mag
            })
            .sum();
        sum_squares.sqrt()
    }

    fn load_hrirs(asset_name: &str, target_sample_rate_hz: u32) -> AudioBuffer {
        let mut resampler = Resampler::new();
        create_sh_hrirs_from_assets(
            asset_name,
            SampleRate::new(target_sample_rate_hz).unwrap(),
            &mut resampler,
        )
        .unwrap()
    }

    /// Verifies that loading the binaural spatialization filters (HRIRs) at a non-native sample
    /// rate preserves their overall loudness across audible frequencies. Because resampling
    /// changes the number of filter taps per unit time, the filter coefficients must be scaled
    /// by `native_rate / target_rate` so convolution produces the same output level as at 48
    /// kHz.
    #[rstest]
    #[case(44100)]
    #[case(88200)]
    #[case(96000)]
    #[googletest::test]
    fn filter_response_is_invariant_to_target_rate(#[case] target_rate: u32) {
        let reference = load_hrirs("3OAAmbientL", HRIR_NATIVE_RATE_HZ);
        let probe_frequencies_hz = [500.0, 1000.0, 2000.0, 4000.0, 8000.0];

        let resampled = load_hrirs("3OAAmbientL", target_rate);

        assert_eq!(reference.num_channels(), resampled.num_channels());
        for frequency_hz in probe_frequencies_hz {
            let reference_norm =
                filter_magnitude_norm_at(&reference, frequency_hz, HRIR_NATIVE_RATE_HZ);
            let resampled_norm = filter_magnitude_norm_at(&resampled, frequency_hz, target_rate);
            assert!(reference_norm > 0.0);
            // 0.02 is roughly 0.17 dB, far below the uncompensated errors (+6 dB at 96k)
            // while leaving room for resampling filter ripple.
            expect_that!(resampled_norm / reference_norm, near(1.0, 0.02));
        }
    }

    /// Verifies that each individual spatial channel within an HRIR filter bank retains its gain
    /// when resampled to 96 kHz across multiple filter assets (different ambisonic orders,
    /// rendering profiles, and left/right ears).
    #[rstest]
    #[case("1OADirectL")]
    #[case("1OAReverberantR")]
    #[case("2OAAmbientR")]
    #[case("3OADirectR")]
    #[googletest::test]
    fn filter_response_is_invariant_per_channel_across_assets(#[case] asset: &str) {
        let target_rate = 96000u32;
        let probe_frequency_hz = 1000.0;
        let reference = load_hrirs(asset, HRIR_NATIVE_RATE_HZ);

        let resampled = load_hrirs(asset, target_rate);

        assert_eq!(reference.num_channels(), resampled.num_channels());
        let max_magnitude = (0..reference.num_channels())
            .map(|ch| filter_magnitude_at(&reference, ch, probe_frequency_hz, HRIR_NATIVE_RATE_HZ))
            .fold(0.0f64, f64::max);
        assert!(max_magnitude > 0.0);
        for channel in 0..reference.num_channels() {
            let reference_magnitude =
                filter_magnitude_at(&reference, channel, probe_frequency_hz, HRIR_NATIVE_RATE_HZ);
            // Channels with a null at the probe frequency are noise-limited.
            if reference_magnitude < 0.02 * max_magnitude {
                continue;
            }
            let resampled_magnitude =
                filter_magnitude_at(&resampled, channel, probe_frequency_hz, target_rate);
            expect_that!(resampled_magnitude / reference_magnitude, near(1.0, 0.03));
        }
    }

    /// Verifies that resampling the 8192-sample (48 kHz) HRIR filter to another sample rate scales
    /// the filter's length in samples proportionally (`8192 * target_rate / 48000`) so the filter
    /// spans the same physical duration in seconds.
    #[rstest]
    #[case(44100, 7526.4)]
    #[case(88200, 15052.8)]
    #[case(96000, 16384.0)]
    #[googletest::test]
    fn resampled_length_matches_rate_ratio(#[case] target_rate: u32, #[case] expected_frames: f64) {
        let resampled = load_hrirs("3OAAmbientL", target_rate);

        expect_that!(resampled.num_frames() as f64, near(expected_frames, 1.0));
    }

    #[googletest::test]
    fn create_sh_hrirs_from_encoded_bytes_returns_none_on_invalid_bytes() {
        let mut resampler = Resampler::new();
        let target_rate = SampleRate::new(48000).unwrap();

        expect_true!(create_sh_hrirs_from_encoded_bytes(&[], target_rate, &mut resampler).is_none());
        expect_true!(
            create_sh_hrirs_from_encoded_bytes(&[1, 2, 3], target_rate, &mut resampler).is_none()
        );
    }

    #[googletest::test]
    fn create_sh_hrirs_from_encoded_bytes_returns_none_on_non_ambisonic_channel_count() {
        let mut resampler = Resampler::new();
        let target_rate = SampleRate::new(48000).unwrap();

        // 2 channels, 1 block each: 2 header bytes + two 33-byte blocks of zeros = 68 bytes.
        let mut two_channel_bytes = vec![2_u8, 1_u8];
        two_channel_bytes.push(0_u8);
        two_channel_bytes.extend_from_slice(&[0xFF; 32]);
        two_channel_bytes.push(0_u8);
        two_channel_bytes.extend_from_slice(&[0xFF; 32]);

        expect_true!(create_sh_hrirs_from_encoded_bytes(
            &two_channel_bytes,
            target_rate,
            &mut resampler
        )
        .is_none());
    }

    #[googletest::test]
    #[should_panic(expected = "Unsupported sampling rates for loading HRIRs")]
    fn create_sh_hrirs_from_encoded_bytes_panics_on_unsupported_sample_rate() {
        let mut resampler = Resampler::new();
        let unsupported_rate = SampleRate::new(44101).unwrap();

        // 1 channel (0OA, order 0 -> (0+1)^2 = 1 channel), 1 block of zeros.
        let mut one_channel_bytes = vec![1_u8, 1_u8, 0_u8];
        one_channel_bytes.extend_from_slice(&[0xFF; 32]);

        let _ = create_sh_hrirs_from_encoded_bytes(
            &one_channel_bytes,
            unsupported_rate,
            &mut resampler,
        );
    }

    #[googletest::test]
    fn create_sh_hrirs_from_assets_returns_none_for_missing_asset() {
        let mut resampler = Resampler::new();
        let target_rate = SampleRate::new(48000).unwrap();

        expect_true!(create_sh_hrirs_from_assets(
            "NonexistentFilterAsset",
            target_rate,
            &mut resampler
        )
        .is_none());
    }
}
