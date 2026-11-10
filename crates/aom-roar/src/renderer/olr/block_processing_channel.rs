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

//! Channel-specific block-based audio rendering for OLR.
//!
//! This module defines the `BlockProcessingChannel`, which handles the rendering
//! lifecycle of a single audio object. It receives dynamic metadata updates and
//! queues them, translating metadata into spatial gains, and applies those gains
//! over sequential audio processing blocks.

use crate::common::definitions::{
    OarError, PolarCoordinate, SampleRate, Samples, METADATA_QUEUE_CAPACITY,
};
use crate::renderer::olr::gain_calculator::GainCalculator;
use crate::renderer::olr::interpret_object_metadata::InterpretObjectMetadata;
use crate::renderer::olr::processing_block::ProcessingBlockVariant;
use std::collections::VecDeque;
use std::sync::Arc;

// TODO(b/525080422): Document.
#[derive(Debug, Clone)]
struct StoredMetadataBlock {
    sample_rate: SampleRate,
    coordinate: PolarCoordinate,
    start_sample: u64,
    duration: Samples,
}

/// Manages the rendering state and processing of a single audio object channel.
///
/// This struct acts as a buffer and coordinator, queuing metadata updates and
/// executing spatial panning algorithms over subsequent audio processing blocks.
#[derive(Debug)]
pub struct BlockProcessingChannel {
    interpreter: InterpretObjectMetadata,
    metadata_blocks: VecDeque<StoredMetadataBlock>,
    processing_queue: VecDeque<ProcessingBlockVariant>,
    accumulated_duration: u64,
    current_render_offset: u64,
}

impl BlockProcessingChannel {
    /// Creates a new `BlockProcessingChannel` with the given spatial gain calculator.
    pub fn new(gain_calculator: Arc<dyn GainCalculator>) -> Self {
        BlockProcessingChannel {
            interpreter: InterpretObjectMetadata::new(gain_calculator),
            metadata_blocks: VecDeque::with_capacity(METADATA_QUEUE_CAPACITY),
            processing_queue: VecDeque::with_capacity(METADATA_QUEUE_CAPACITY),
            accumulated_duration: 0,
            current_render_offset: 0,
        }
    }

    /// Queues a new dynamic metadata update block for this channel.
    ///
    /// # Parameters
    ///
    /// * `sample_rate` - The sampling rate of the audio engine.
    /// * `coordinate` - The 3D polar coordinate of the audio object.
    /// * `duration` - The duration in samples for which this metadata is valid.
    ///
    /// # Errors
    ///
    /// Returns `OarError::InvalidParameter` if a metadata underrun occurs (i.e., the added
    /// metadata is for a timeframe that has already been rendered).
    pub fn add_metadata(
        &mut self,
        sample_rate: SampleRate,
        coordinate: PolarCoordinate,
        duration: Samples,
    ) -> Result<(), OarError> {
        if self.accumulated_duration < self.current_render_offset {
            // Metadata underrun: metadata is for the past.
            return Err(OarError::InvalidParameter);
        }

        self.metadata_blocks.push_back(StoredMetadataBlock {
            sample_rate,
            coordinate,
            start_sample: self.accumulated_duration,
            duration,
        });

        self.accumulated_duration += u32::from(duration) as u64;
        Ok(())
    }

    fn refill_processing_queue(&mut self, start_sample: u64) -> Result<(), OarError> {
        while self.processing_queue.is_empty() {
            let Some(stored) = self.metadata_blocks.pop_front() else {
                break;
            };

            if stored.start_sample + (u32::from(stored.duration) as u64) < start_sample {
                // Expired historical block
                continue;
            }

            if let Some(block) = self.interpreter.process(
                stored.sample_rate,
                &stored.coordinate,
                stored.start_sample,
                stored.duration,
            )? {
                self.processing_queue.push_back(block);
            }
        }
        Ok(())
    }

    /// Processes a block of input audio and accumulates panned audio into the output.
    ///
    /// Spatial gains are calculated and applied to the input buffer, writing the
    /// resulting multi-channel audio to the output buffer.
    ///
    /// # Arguments
    ///
    /// * `sample_rate` - The audio sample rate in Hz.
    /// * `offset` - The absolute sample offset of this audio block.
    /// * `input` - The mono input audio buffer for this object channel.
    /// * `output` - The multi-channel output buffer to accumulate the panned audio into.
    ///
    /// # Errors
    ///
    /// Returns `OarError` if gain application or metadata interpretation fails.
    pub fn process(
        &mut self,
        _sample_rate: SampleRate,
        offset: u64,
        input: &[f32],
        output: &mut [f32],
    ) -> Result<(), OarError> {
        let start_sample = offset;
        let num_samples = input.len();
        let end_sample = start_sample + num_samples as u64;

        self.current_render_offset = end_sample;

        self.refill_processing_queue(start_sample)?;

        // TODO(b/525080422): Replace with a `while let` expression.
        while !self.processing_queue.is_empty() {
            let last_sample = self.processing_queue.front().unwrap().last_sample();
            let block = self.processing_queue.front_mut().unwrap();

            block.process(start_sample, input, num_samples, 1, output)?;

            if last_sample < end_sample {
                self.processing_queue.pop_front();
                let _ = self.refill_processing_queue(start_sample);
            } else if last_sample == end_sample {
                self.processing_queue.pop_front();
                break;
            } else {
                break;
            }
        }

        Ok(())
    }
}

// ===== Tests =====

#[cfg(test)]
mod test {

    use super::*;
    use crate::common::definitions::PolarCoordinate;
    use crate::renderer::olr::gain_calculator::VogPanner;
    use googletest::prelude::*;

    const EPSILON: f32 = 1e-4;

    #[gtest]
    fn test_block_processing_channel_add_metadata_advances_duration() {
        let mut chan = BlockProcessingChannel::new(Arc::new(VogPanner));
        let coord = PolarCoordinate::new_from_floats(0.0, 0.0, 1.0).unwrap();

        let result =
            chan.add_metadata(SampleRate::new(48000).unwrap(), coord, Samples::new(10).unwrap());

        assert_ok!(result);
        expect_eq!(chan.accumulated_duration, 10);
    }

    #[gtest]
    fn test_block_processing_channel_add_metadata_past_offset_returns_error() {
        let mut chan = BlockProcessingChannel::new(Arc::new(VogPanner));
        let coord = PolarCoordinate::new_from_floats(0.0, 0.0, 1.0).unwrap();
        let input = [1.0; 10];
        let mut output = [0.0; 10];
        chan.process(SampleRate::new(48000).unwrap(), 0, &input, &mut output).unwrap();

        let result =
            chan.add_metadata(SampleRate::new(48000).unwrap(), coord, Samples::new(5).unwrap());

        expect_that!(result, eq(Err(OarError::InvalidParameter)));
    }

    #[gtest]
    fn test_block_processing_channel_process_applies_gain() {
        let mut chan = BlockProcessingChannel::new(Arc::new(VogPanner));
        let coord = PolarCoordinate::new_from_floats(0.0, 0.0, 1.0).unwrap();
        chan.add_metadata(SampleRate::new(48000).unwrap(), coord, Samples::new(10).unwrap())
            .unwrap();
        let input = [2.0; 10];
        let mut output = [0.0; 10];

        let result = chan.process(SampleRate::new(48000).unwrap(), 0, &input, &mut output);

        assert_ok!(result);
        for val in output {
            expect_that!(val, near(2.0, EPSILON));
        }
    }
}
