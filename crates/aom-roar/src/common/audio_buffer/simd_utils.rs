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

//! Safe slice-based vector utilities.

/// Rounds a number of frames up to the next aligned memory address
/// based on `memory_alignment_bytes`.
pub fn find_next_aligned_array_index(
    length: usize,
    type_size_bytes: usize,
    memory_alignment_bytes: usize,
) -> usize {
    let byte_length = type_size_bytes * length;
    let unaligned_bytes = byte_length % memory_alignment_bytes;
    let bytes_to_next_aligned =
        if unaligned_bytes == 0 { 0 } else { memory_alignment_bytes - unaligned_bytes };
    (byte_length + bytes_to_next_aligned) / type_size_bytes
}

/// In-place addition: `slice[i] += input_a[i]`.
pub fn add_pointwise_in_place(slice: &mut [f32], input_a: &[f32]) {
    let len = slice.len().min(input_a.len());
    for i in 0..len {
        slice[i] += input_a[i];
    }
}

/// In-place subtraction: `slice[i] -= input_a[i]`.
pub fn subtract_pointwise_in_place(slice: &mut [f32], input_a: &[f32]) {
    let len = slice.len().min(input_a.len());
    for i in 0..len {
        slice[i] -= input_a[i];
    }
}

/// In-place pointwise multiplication: `slice[i] *= input_a[i]`.
pub fn multiply_pointwise_in_place(slice: &mut [f32], input_a: &[f32]) {
    let len = slice.len().min(input_a.len());
    for i in 0..len {
        slice[i] *= input_a[i];
    }
}

/// Multiplies `input` by a scalar `gain` and adds onto `accumulator`.
pub fn scalar_multiply_and_accumulate(gain: f32, input: &[f32], accumulator: &mut [f32]) {
    let len = input.len().min(accumulator.len());
    for i in 0..len {
        accumulator[i] += input[i] * gain;
    }
}

/// Multiplies `input` by a linearly ramped gain from `start_gain` to `end_gain` and adds to
/// `accumulator`.
pub fn ramp_multiply_and_accumulate(
    // TODO(b/525080422): Use Linear gain type.
    start_gain: f32,
    end_gain: f32,
    input: &[f32],
    accumulator: &mut [f32],
) {
    let len = input.len().min(accumulator.len());
    if len == 0 {
        return;
    }
    if len == 1 || start_gain == end_gain {
        scalar_multiply_and_accumulate(end_gain, &input[..len], &mut accumulator[..len]);
        return;
    }
    let step = (end_gain - start_gain) / (len - 1) as f32;
    for (i, (&inp, acc)) in input[..len].iter().zip(accumulator[..len].iter_mut()).enumerate() {
        let gain = start_gain + (i as f32) * step;
        *acc += inp * gain;
    }
}

/// Converts 16-bit int input to float output (`input * 1.0 / 32767.0`).
pub fn float_from_int16(input: &[i16], output: &mut [f32]) {
    const FLOAT_FROM_INT16: f32 = 1.0 / (0x7FFF as f32);
    let len = input.len().min(output.len());
    for i in 0..len {
        output[i] = (input[i] as f32) * FLOAT_FROM_INT16;
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use googletest::prelude::*;

    const EPSILON: f32 = 1e-6;

    #[gtest]
    fn test_find_next_aligned_array_index() {
        expect_eq!(find_next_aligned_array_index(0, 4, 64), 0);
        expect_eq!(find_next_aligned_array_index(16, 4, 64), 16);
        expect_eq!(find_next_aligned_array_index(32, 4, 64), 32);
        expect_eq!(find_next_aligned_array_index(1, 4, 64), 16);
        expect_eq!(find_next_aligned_array_index(15, 4, 64), 16);
        expect_eq!(find_next_aligned_array_index(17, 4, 64), 32);
    }

    #[gtest]
    fn test_add_pointwise_in_place() {
        let mut slice = [1.0, 2.0, 3.0];
        let input = [4.0, 5.0, 6.0];
        add_pointwise_in_place(&mut slice, &input);
        expect_eq!(slice, [5.0, 7.0, 9.0]);
    }

    #[gtest]
    fn test_subtract_pointwise_in_place() {
        let mut slice = [5.0, 7.0, 9.0];
        let input = [4.0, 5.0, 6.0];
        subtract_pointwise_in_place(&mut slice, &input);
        expect_eq!(slice, [1.0, 2.0, 3.0]);
    }

    #[gtest]
    fn test_multiply_pointwise_in_place() {
        let mut slice = [2.0, 3.0, 4.0];
        let input = [5.0, 6.0, 7.0];
        multiply_pointwise_in_place(&mut slice, &input);
        expect_eq!(slice, [10.0, 18.0, 28.0]);
    }

    #[gtest]
    fn test_scalar_multiply_and_accumulate() {
        let input = [1.0, 2.0, 3.0];
        let mut accumulator = [1.0, 1.0, 1.0];
        scalar_multiply_and_accumulate(2.0, &input, &mut accumulator);
        expect_eq!(accumulator, [3.0, 5.0, 7.0]);
    }

    #[gtest]
    fn test_ramp_multiply_and_accumulate() {
        let input = [1.0, 1.0, 1.0];
        let mut accumulator = [0.0, 0.0, 0.0];
        ramp_multiply_and_accumulate(1.0, 3.0, &input, &mut accumulator);
        expect_near!(accumulator[0], 1.0, EPSILON);
        expect_near!(accumulator[1], 2.0, EPSILON);
        expect_near!(accumulator[2], 3.0, EPSILON);
    }

    #[gtest]
    fn test_float_from_int16() {
        let input = [0, 32767, -32767];
        let mut output = [0.0; 3];
        float_from_int16(&input, &mut output);
        expect_near!(output[0], 0.0, EPSILON);
        expect_near!(output[1], 1.0, EPSILON);
        expect_near!(output[2], -1.0, EPSILON);
    }
}
