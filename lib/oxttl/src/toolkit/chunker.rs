// Adapted from: https://github.com/pola-rs/polars/blob/main/crates/polars-io/src/csv/read/utils.rs#L10
// and https://github.com/pola-rs/polars/blob/main/crates/polars-io/src/csv/read/parser.rs#L124
// and https://github.com/pola-rs/polars/blob/main/crates/polars-io/src/csv/read/parser.rs#L310
// Which has the following license:
// Copyright (c) 2020 Ritchie Vink
// Some portions Copyright (c) 2024 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use crate::TurtleParser;

const EOT_CHAR: u8 = b'.';

// Given a number of desired chunks, corresponding to threads find offsets that break the file into chunks that can be read in parallel.
// Parser should not be reused, hence it is passed by value.
#[allow(clippy::needless_pass_by_value)]
pub fn get_turtle_file_chunks(
    bytes: &[u8],
    n_chunks: usize,
    parser: TurtleParser,
) -> Option<Vec<(usize, usize)>> {
    let mut last_pos = 0;
    let total_len = bytes.len();
    let chunk_size = total_len / n_chunks;
    let mut offsets = Vec::with_capacity(n_chunks);
    for _ in 0..n_chunks {
        let search_pos = last_pos + chunk_size;

        if search_pos >= bytes.len() {
            break;
        }

        let end_pos = match next_terminating_period(parser.clone(), &bytes[search_pos..]) {
            Some(pos) => search_pos + pos,
            None => {
                return None;
            }
        };
        offsets.push((last_pos, end_pos));
        last_pos = end_pos;
    }
    offsets.push((last_pos, total_len));
    Some(offsets)
}

// Heuristically, we assume that a period is terminating (a triple) if we can start immediately after it and parse three triples.
// Parser should not be reused, hence it is passed by value.
#[allow(clippy::needless_pass_by_value)]
fn next_terminating_period(parser: TurtleParser, mut input: &[u8]) -> Option<usize> {
    fn accept(parser: TurtleParser, input: &[u8]) -> bool {
        let mut f = parser.parse_slice(input);
        for _ in 0..3 {
            if let Some(r) = f.next() {
                if r.is_err() {
                    return false;
                }
            } else {
                return false;
            }
        }
        true
    }

    let mut rejected_periods = 0_u16;

    let mut total_pos = 0;
    if input.is_empty() {
        return None;
    }
    loop {
        if rejected_periods >= 1_000 {
            return None;
        }

        let pos = memchr::memchr(EOT_CHAR, input)? + 1;
        if input.len() - pos == 0 {
            return None;
        }
        let new_input = &input[pos..];
        let p = parser.clone();
        let accepted = accept(p, new_input);
        if accepted {
            return Some(total_pos + pos);
        }
        input = &input[pos + 1..];
        total_pos += pos + 1;
        rejected_periods += 1;
    }
}
