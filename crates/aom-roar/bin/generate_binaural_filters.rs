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

//! CLI utility that reads 16-bit PCM WAV binaural filter files, compresses their planar channels
//! using block-variable Rice coding (256 samples per block), verifies bit-exact round-trip
//! decoding, and writes the generated Rust source files for `binaural_filters`.

use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

// Use the RiceBlockDecoder from inside OBR without exposing it as part of the aom-roar API.
#[path = "../src/renderer/obr/ambisonic_binaural_decoder/rice_block_decoder.rs"]
mod rice_block_decoder;

/// Number of audio samples per Rice-encoded block.
const BLOCK_SIZE: usize = 256;

/// Size in bytes of the stream header (`[num_channels, blocks_per_channel]`).
const HEADER_SIZE: usize = 2;

/// Maximum Rice exponent `k` (`M = 1 << k`) evaluated during block encoding.
const MAX_RICE_EXPONENT: u8 = 14;

/// Expected native sampling rate (in Hz) of the source binaural filter WAV files.
const EXPECTED_SAMPLE_RATE_HZ: u32 = 48000;

/// Number of hex byte literals formatted onto a single line in generated `.rs` files.
const BYTES_PER_SOURCE_LINE: usize = 15;

/// Parsed 16-bit PCM WAV file with samples de-interleaved into planar channels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WavAudio {
    sample_rate: u32,
    channels: Vec<Vec<i16>>,
}

impl WavAudio {
    /// Parses a 16-bit little-endian PCM RIFF WAVE byte slice into planar channels.
    ///
    /// # Arguments
    /// * `bytes` - Raw bytes of a `.wav` file. Returns `None` if not a valid 16-bit PCM WAV.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        const MIN_WAV_HEADER_SIZE: usize = 44;
        const RIFF_HEADER_SIZE: usize = 12;
        const SUBCHUNK_HEADER_SIZE: usize = 8;
        const MIN_FMT_CHUNK_SIZE: usize = 16;
        const PCM_FORMAT_TAG: u16 = 1;
        const WAVE_FORMAT_EXTENSIBLE_TAG: u16 = 0xFFFE;
        const BYTES_PER_SAMPLE_16BIT: usize = 2;

        if bytes.len() < MIN_WAV_HEADER_SIZE || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE"
        {
            return None;
        }

        let mut idx = RIFF_HEADER_SIZE;
        let mut num_channels = 0_usize;
        let mut sample_rate = 0_u32;
        let mut bits_per_sample = 0_u16;
        let mut channels: Option<Vec<Vec<i16>>> = None;

        while idx + SUBCHUNK_HEADER_SIZE <= bytes.len() {
            let chunk_id = &bytes[idx..idx + 4];
            let chunk_size = u32::from_le_bytes([
                bytes[idx + 4],
                bytes[idx + 5],
                bytes[idx + 6],
                bytes[idx + 7],
            ]) as usize;
            idx += SUBCHUNK_HEADER_SIZE;

            if idx + chunk_size > bytes.len() {
                break;
            }

            if chunk_id == b"fmt " {
                if chunk_size < MIN_FMT_CHUNK_SIZE {
                    return None;
                }
                let format_tag = u16::from_le_bytes([bytes[idx], bytes[idx + 1]]);
                if format_tag != PCM_FORMAT_TAG && format_tag != WAVE_FORMAT_EXTENSIBLE_TAG {
                    return None;
                }
                num_channels = usize::from(u16::from_le_bytes([bytes[idx + 2], bytes[idx + 3]]));
                sample_rate = u32::from_le_bytes([
                    bytes[idx + 4],
                    bytes[idx + 5],
                    bytes[idx + 6],
                    bytes[idx + 7],
                ]);
                bits_per_sample = u16::from_le_bytes([bytes[idx + 14], bytes[idx + 15]]);
                if bits_per_sample != 16 || num_channels == 0 || sample_rate == 0 {
                    return None;
                }
            } else if chunk_id == b"data" {
                if num_channels == 0 || bits_per_sample != 16 {
                    return None;
                }
                if !chunk_size.is_multiple_of(BYTES_PER_SAMPLE_16BIT * num_channels) {
                    return None;
                }
                let num_frames = chunk_size / (BYTES_PER_SAMPLE_16BIT * num_channels);
                let mut planar = vec![Vec::with_capacity(num_frames); num_channels];
                let data_slice = &bytes[idx..idx + chunk_size];
                for (sample_idx, sample_bytes) in
                    data_slice.chunks_exact(BYTES_PER_SAMPLE_16BIT).enumerate()
                {
                    let sample = i16::from_le_bytes([sample_bytes[0], sample_bytes[1]]);
                    planar[sample_idx % num_channels].push(sample);
                }
                channels = Some(planar);
                break;
            }

            idx += chunk_size + (chunk_size % 2);
        }

        channels.map(|planar_channels| Self { sample_rate, channels: planar_channels })
    }

    /// Returns the audio sampling rate in Hz.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Returns the de-interleaved planar audio channels.
    pub fn channels(&self) -> &[Vec<i16>] {
        &self.channels
    }
}

/// Sequential MSB-first bit writer that packs bits into a byte vector.
#[derive(Debug, Default, Clone)]
pub struct BitWriter {
    bytes: Vec<u8>,
    current_byte: u8,
    bit_offset: u8,
}

impl BitWriter {
    /// Creates an empty `BitWriter`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a single bit (`0` or `1`) to the stream.
    ///
    /// # Arguments
    /// * `bit` - Low bit (`bit & 1`) is written at the next MSB-first position.
    pub fn write_bit(&mut self, bit: u32) {
        let masked = (bit & 1) as u8;
        self.current_byte |= masked << (7 - self.bit_offset);
        self.bit_offset += 1;
        if self.bit_offset == 8 {
            self.bytes.push(self.current_byte);
            self.current_byte = 0;
            self.bit_offset = 0;
        }
    }

    /// Appends the lowest `num_bits` bits of `value` to the stream, MSB-first.
    ///
    /// # Arguments
    /// * `value` - Unsigned value whose lowest `num_bits` bits will be written.
    /// * `num_bits` - Number of bits to write (`0..=16`).
    pub fn write_bits(&mut self, value: u32, num_bits: u8) {
        for shift in (0..num_bits).rev() {
            self.write_bit((value >> shift) & 1);
        }
    }

    /// Flushes any partially filled byte (padding remaining low bits with zeros) and returns the
    /// packed byte vector.
    pub fn into_bytes(mut self) -> Vec<u8> {
        if self.bit_offset > 0 {
            self.bytes.push(self.current_byte);
        }
        self.bytes
    }
}

/// Encodes and decodes 256-sample blocks using Rice coding with an adaptive power-of-two divisor.
pub struct RiceBlockCodec;

impl RiceBlockCodec {
    /// Maps a signed 16-bit integer to an unsigned integer where small magnitudes stay small
    /// (`0 -> 0, -1 -> 1, 1 -> 2, -2 -> 3, 2 -> 4, ...`).
    pub fn map_to_unsigned(value: i16) -> u32 {
        let wide = i32::from(value);
        if wide >= 0 {
            (wide as u32) * 2
        } else {
            ((-wide) as u32) * 2 - 1
        }
    }

    /// Reverses [`Self::map_to_unsigned`], converting `0..=65535` back into `i16`.
    pub fn unmap_to_signed(mapped: u32) -> Option<i16> {
        if mapped > u32::from(u16::MAX) {
            return None;
        }
        let signed_wide = if mapped.is_multiple_of(2) {
            (mapped / 2) as i32
        } else {
            -(((mapped + 1) / 2) as i32)
        };
        Some(signed_wide as i16)
    }

    /// Computes the total number of bits required to Rice-encode `block` using exponent `k`
    /// (`M = 1 << k`).
    pub fn bit_cost_for_exponent(block: &[i16; BLOCK_SIZE], k: u8) -> usize {
        let mut num_bits = 0_usize;
        for &value in block {
            let mapped = Self::map_to_unsigned(value);
            let quotient = (mapped >> k) as usize;
            num_bits += quotient + 1 + usize::from(k);
        }
        num_bits
    }

    /// Selects the exponent `k` (`0..=MAX_RICE_EXPONENT`) that minimizes the encoded bit count for
    /// `block`.
    pub fn select_optimal_exponent(block: &[i16; BLOCK_SIZE]) -> u8 {
        let mut best_k = 0_u8;
        let mut min_bits = usize::MAX;
        for k in 0..=MAX_RICE_EXPONENT {
            let bits = Self::bit_cost_for_exponent(block, k);
            if bits < min_bits {
                min_bits = bits;
                best_k = k;
            }
        }
        best_k
    }

    /// Encodes a 256-sample `block` into `[k, ...rice_encoded_bytes]` using the optimal `k`.
    pub fn encode_block(block: &[i16; BLOCK_SIZE]) -> Vec<u8> {
        let k = Self::select_optimal_exponent(block);
        let mask = (1_u32 << k) - 1;
        let mut writer = BitWriter::new();
        for &value in block {
            let mapped = Self::map_to_unsigned(value);
            let quotient = mapped >> k;
            let remainder = mapped & mask;
            for _ in 0..quotient {
                writer.write_bit(0);
            }
            writer.write_bit(1);
            writer.write_bits(remainder, k);
        }
        let payload = writer.into_bytes();
        let mut out = Vec::with_capacity(1 + payload.len());
        out.push(k);
        out.extend_from_slice(&payload);
        out
    }

    /// Decodes a single 256-sample block starting at `bytes`, returning the number of bytes
    /// consumed.
    pub fn decode_block(bytes: &[u8], output: &mut [i16; BLOCK_SIZE]) -> Option<usize> {
        rice_block_decoder::RiceBlockDecoder::decode_block(bytes, output)
    }
}

/// Encodes and decodes multi-channel planar 16-bit audio streams.
pub struct RiceFilterCodec;

impl RiceFilterCodec {
    /// Encodes planar 16-bit channels into `[num_channels, blocks_per_channel, ...blocks]`.
    ///
    /// # Arguments
    /// * `channels` - Non-empty slice of planar channels; each channel must have the same length,
    ///   and that length must be a positive multiple of [`BLOCK_SIZE`] (256) with at most 255
    ///   blocks per channel.
    pub fn encode_planar_channels(channels: &[Vec<i16>]) -> Option<Vec<u8>> {
        let num_channels = channels.len();
        if num_channels == 0 || num_channels > usize::from(u8::MAX) {
            return None;
        }
        let num_frames = channels[0].len();
        if num_frames == 0 || !num_frames.is_multiple_of(BLOCK_SIZE) {
            return None;
        }
        let blocks_per_channel = num_frames / BLOCK_SIZE;
        if blocks_per_channel > usize::from(u8::MAX) {
            return None;
        }
        if channels.iter().any(|ch| ch.len() != num_frames) {
            return None;
        }

        let mut encoded = vec![num_channels as u8, blocks_per_channel as u8];
        for channel in channels {
            for chunk in channel.chunks_exact(BLOCK_SIZE) {
                let block: &[i16; BLOCK_SIZE] = chunk.try_into().ok()?;
                encoded.extend_from_slice(&RiceBlockCodec::encode_block(block));
            }
        }
        Some(encoded)
    }

    /// Decodes a planar Rice-encoded byte slice back into planar `Vec<Vec<i16>>` channels.
    pub fn decode_planar_channels(bytes: &[u8]) -> Option<Vec<Vec<i16>>> {
        if bytes.len() < HEADER_SIZE {
            return None;
        }
        let num_channels = usize::from(bytes[0]);
        let blocks_per_channel = usize::from(bytes[1]);
        if num_channels == 0 || blocks_per_channel == 0 {
            return None;
        }

        let mut channels = Vec::with_capacity(num_channels);
        let mut cursor = HEADER_SIZE;
        let mut block = [0_i16; BLOCK_SIZE];

        for _ in 0..num_channels {
            let mut channel = Vec::with_capacity(blocks_per_channel * BLOCK_SIZE);
            for _ in 0..blocks_per_channel {
                let consumed = RiceBlockCodec::decode_block(&bytes[cursor..], &mut block)?;
                cursor += consumed;
                channel.extend_from_slice(&block);
            }
            channels.push(channel);
        }

        if cursor != bytes.len() {
            return None;
        }
        Some(channels)
    }
}

/// Formats compressed filter bytes into a Rust source module for `binaural_filters`.
pub struct RustSourceGenerator;

impl RustSourceGenerator {
    /// Generates the `.rs` file contents for a filter module.
    ///
    /// # Arguments
    /// * `module_stem` - File stem such as `"binaural_filters_1_oa_ambient_l"`.
    /// * `encoded_bytes` - Compressed filter payload to embed as `static DATA: &[u8]`.
    pub fn format_filter_module(module_stem: &str, encoded_bytes: &[u8]) -> String {
        let fn_suffix = module_stem.replace('_', "");
        let mut out = String::new();
        out.push_str(
            "// Copyright (c) 2026, Alliance for Open Media. All rights reserved\n\
             //\n\
             // This source code is subject to the terms of the BSD 3-Clause Clear License\n\
             // and the Alliance for Open Media Patent License 1.0. If the BSD 3-Clause Clear\n\
             // License was not distributed with this source code in the LICENSE file, you\n\
             // can obtain it at www.aomedia.org/license/software-license/bsd-3-c-c. If the\n\
             // Alliance for Open Media Patent License 1.0 was not distributed with this\n\
             // source code in the PATENTS file, you can obtain it at\n\
             // www.aomedia.org/license/patent.\n\
             \n\
             //! Auto-generated Rice-encoded planar binaural filter data.\n\
             #![forbid(unsafe_code)]\n\
             \n",
        );
        let _ = writeln!(out, "pub fn get_{fn_suffix}() -> &'static [u8] {{");
        out.push_str("    static DATA: &[u8] = &[\n");

        for line_bytes in encoded_bytes.chunks(BYTES_PER_SOURCE_LINE) {
            out.push_str("        ");
            for (i, &byte) in line_bytes.iter().enumerate() {
                if i > 0 {
                    out.push(' ');
                }
                let _ = write!(out, "0x{byte:02x},");
            }
            out.push('\n');
        }

        out.push_str("    ];\n    DATA\n}\n");
        out
    }
}

/// Parsed command-line arguments for filter generation.
struct CliArgs {
    input_wav_dir: PathBuf,
    output_rs_dir: PathBuf,
}

/// Parses command-line arguments into [`CliArgs`].
fn parse_cli_args(args: &[String]) -> Option<CliArgs> {
    let mut input_dir: Option<PathBuf> = None;
    let mut output_dir: Option<PathBuf> = None;
    let mut positional = Vec::new();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--input_dir" | "-i" => {
                i += 1;
                input_dir = Some(PathBuf::from(args.get(i)?));
            }
            "--output_dir" | "-o" => {
                i += 1;
                output_dir = Some(PathBuf::from(args.get(i)?));
            }
            arg if !arg.starts_with('-') => {
                positional.push(PathBuf::from(arg));
            }
            _ => return None,
        }
        i += 1;
    }

    if let (Some(input_wav_dir), Some(output_rs_dir)) = (input_dir, output_dir) {
        return Some(CliArgs { input_wav_dir, output_rs_dir });
    }
    if positional.len() == 2 {
        return Some(CliArgs {
            input_wav_dir: positional[0].clone(),
            output_rs_dir: positional[1].clone(),
        });
    }
    None
}

/// Processes all `.wav` files in `input_dir` and writes Rice-encoded `.rs` modules into
/// `output_dir`.
fn generate_all_filters(input_dir: &Path, output_dir: &Path) -> Result<(), String> {
    let mut wav_paths: Vec<PathBuf> = fs::read_dir(input_dir)
        .map_err(|e| format!("Failed to read input directory {}: {e}", input_dir.display()))?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("wav"))
        .collect();
    wav_paths.sort();

    if wav_paths.is_empty() {
        return Err(format!("No .wav files found in {}", input_dir.display()));
    }

    fs::create_dir_all(output_dir)
        .map_err(|e| format!("Failed to create output directory {}: {e}", output_dir.display()))?;

    let mut total_wav_bytes = 0_usize;
    let mut total_encoded_bytes = 0_usize;

    for wav_path in &wav_paths {
        let stem = wav_path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| format!("Invalid filename: {}", wav_path.display()))?;
        let wav_bytes = fs::read(wav_path)
            .map_err(|e| format!("Failed to read {}: {e}", wav_path.display()))?;
        let wav = WavAudio::from_bytes(&wav_bytes)
            .ok_or_else(|| format!("Failed to parse 16-bit PCM WAV: {}", wav_path.display()))?;

        if wav.sample_rate() != EXPECTED_SAMPLE_RATE_HZ {
            return Err(format!(
                "Unexpected sample rate {} Hz in {} (expected {} Hz)",
                wav.sample_rate(),
                wav_path.display(),
                EXPECTED_SAMPLE_RATE_HZ
            ));
        }

        let encoded = RiceFilterCodec::encode_planar_channels(wav.channels())
            .ok_or_else(|| format!("Failed to Rice-encode {}", wav_path.display()))?;

        // Verify bit-exact round-trip before writing the output source file.
        let decoded = RiceFilterCodec::decode_planar_channels(&encoded)
            .ok_or_else(|| format!("Round-trip decode failed for {}", wav_path.display()))?;
        if decoded != wav.channels() {
            return Err(format!("Round-trip mismatch for {}", wav_path.display()));
        }

        let rs_source = RustSourceGenerator::format_filter_module(stem, &encoded);
        let out_path = output_dir.join(format!("{stem}.rs"));
        fs::write(&out_path, rs_source)
            .map_err(|e| format!("Failed to write {}: {e}", out_path.display()))?;

        total_wav_bytes += wav_bytes.len();
        total_encoded_bytes += encoded.len();
        println!(
            "{stem}: {} WAV bytes -> {} Rice bytes ({:.1}% of original)",
            wav_bytes.len(),
            encoded.len(),
            (encoded.len() as f64) * 100.0 / (wav_bytes.len() as f64)
        );
    }

    println!(
        "Processed {} files: {} total WAV bytes -> {} total Rice bytes ({:.1}% of original)",
        wav_paths.len(),
        total_wav_bytes,
        total_encoded_bytes,
        (total_encoded_bytes as f64) * 100.0 / (total_wav_bytes as f64)
    );
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let Some(cli_args) = parse_cli_args(&args) else {
        eprintln!(
            "Usage: {} --input_dir <wav_dir> --output_dir <binaural_filters_dir>",
            args.first().map(String::as_str).unwrap_or("generate_binaural_filters")
        );
        return ExitCode::FAILURE;
    };

    if let Err(err) = generate_all_filters(&cli_args.input_wav_dir, &cli_args.output_rs_dir) {
        eprintln!("Error: {err}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod test {
    use super::*;
    use googletest::prelude::*;

    #[gtest]
    fn map_and_unmap_round_trip_signed_16bit_boundary_values() {
        expect_that!(RiceBlockCodec::map_to_unsigned(0), eq(0));
        expect_that!(RiceBlockCodec::map_to_unsigned(-1), eq(1));
        expect_that!(RiceBlockCodec::map_to_unsigned(1), eq(2));
        expect_that!(RiceBlockCodec::map_to_unsigned(-2), eq(3));
        expect_that!(RiceBlockCodec::map_to_unsigned(2), eq(4));
        expect_that!(RiceBlockCodec::map_to_unsigned(i16::MAX), eq(65534));
        expect_that!(RiceBlockCodec::map_to_unsigned(i16::MIN), eq(65535));

        expect_that!(RiceBlockCodec::unmap_to_signed(0), eq(Some(0)));
        expect_that!(RiceBlockCodec::unmap_to_signed(1), eq(Some(-1)));
        expect_that!(RiceBlockCodec::unmap_to_signed(2), eq(Some(1)));
        expect_that!(RiceBlockCodec::unmap_to_signed(65534), eq(Some(i16::MAX)));
        expect_that!(RiceBlockCodec::unmap_to_signed(65535), eq(Some(i16::MIN)));
        expect_that!(RiceBlockCodec::unmap_to_signed(65536), eq(None));
    }

    #[gtest]
    fn encode_block_of_zeros_selects_k_zero_and_uses_33_bytes() {
        let zero_block = [0_i16; BLOCK_SIZE];

        let encoded = RiceBlockCodec::encode_block(&zero_block);
        let mut decoded = [42_i16; BLOCK_SIZE];
        let consumed = RiceBlockCodec::decode_block(&encoded, &mut decoded);

        expect_that!(encoded.len(), eq(33));
        expect_that!(encoded[0], eq(0));
        expect_that!(&encoded[1..], eq(&[0xFF_u8; 32]));
        expect_that!(consumed, eq(Some(33)));
        expect_that!(decoded.as_slice(), eq(&zero_block));
    }

    #[gtest]
    fn encode_and_decode_planar_channels_preserves_exact_samples() {
        let mut channel_0 = [0_i16; BLOCK_SIZE].to_vec();
        channel_0[0] = i16::MIN;
        channel_0[1] = -12345;
        channel_0[2] = -1;
        channel_0[3] = 0;
        channel_0[4] = 1;
        channel_0[5] = 12345;
        channel_0[255] = i16::MAX;

        let mut channel_1 = [-3_i16; BLOCK_SIZE].to_vec();
        channel_1[0] = 4;
        channel_1[128] = -5;
        let channels = vec![channel_0, channel_1];

        let encoded = RiceFilterCodec::encode_planar_channels(&channels).unwrap();
        let decoded = RiceFilterCodec::decode_planar_channels(&encoded);

        expect_that!(encoded[0], eq(2));
        expect_that!(encoded[1], eq(1));
        expect_that!(&decoded, eq(&Some(channels)));
    }

    #[gtest]
    fn wav_audio_deinterleaves_stereo_frames_into_planar_channels() {
        let mut wav_bytes = Vec::new();
        wav_bytes.extend_from_slice(b"RIFF");
        wav_bytes.extend_from_slice(&(36 + 8_u32).to_le_bytes());
        wav_bytes.extend_from_slice(b"WAVE");
        wav_bytes.extend_from_slice(b"fmt ");
        wav_bytes.extend_from_slice(&16_u32.to_le_bytes());
        wav_bytes.extend_from_slice(&1_u16.to_le_bytes());
        wav_bytes.extend_from_slice(&2_u16.to_le_bytes());
        wav_bytes.extend_from_slice(&48000_u32.to_le_bytes());
        wav_bytes.extend_from_slice(&(48000_u32 * 4).to_le_bytes());
        wav_bytes.extend_from_slice(&4_u16.to_le_bytes());
        wav_bytes.extend_from_slice(&16_u16.to_le_bytes());
        wav_bytes.extend_from_slice(b"data");
        wav_bytes.extend_from_slice(&8_u32.to_le_bytes());
        // Interleaved stereo: frame 0 = (10, -20), frame 1 = (30, -40)
        for sample in [10_i16, -20_i16, 30_i16, -40_i16] {
            wav_bytes.extend_from_slice(&sample.to_le_bytes());
        }

        let parsed = WavAudio::from_bytes(&wav_bytes).unwrap();

        expect_that!(parsed.sample_rate(), eq(48000));
        expect_that!(parsed.channels(), eq(&[vec![10_i16, 30_i16], vec![-20_i16, -40_i16]]));
    }

    #[gtest]
    fn format_filter_module_emits_expected_function_signature_and_hex_table() {
        let encoded = [0x04_u8, 0x20_u8, 0xFF_u8];

        let source =
            RustSourceGenerator::format_filter_module("binaural_filters_1_oa_ambient_l", &encoded);

        expect_true!(source.contains("pub fn get_binauralfilters1oaambientl() -> &'static [u8] {"));
        expect_true!(source.contains("        0x04, 0x20, 0xff,\n"));
    }
}
