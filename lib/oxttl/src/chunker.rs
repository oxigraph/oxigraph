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
use memchr::memchr;
use std::fs::File;
use std::io;
use std::io::{Read, Seek, SeekFrom};

// Given a number of desired chunks, corresponding to threads, find offsets that break the file into chunks that can be read in parallel.
// For NTriples, this can be done simply on newlines.
pub fn get_ntriples_slice_chunks(bytes: &[u8], n_chunks: usize) -> Vec<(usize, usize)> {
    let mut last_pos = 0;
    let total_len = bytes.len();
    let chunk_size = total_len / n_chunks;
    let mut offsets = Vec::with_capacity(n_chunks);
    for _ in 0..n_chunks {
        let search_pos = last_pos + chunk_size;

        if search_pos >= bytes.len() {
            break;
        }

        let Some(pos) = next_newline_position(&bytes[search_pos..]) else {
            // We keep the valid chunks we found, and add (outside the loop) the rest of the bytes as a chunk.
            break;
        };
        let end_pos = search_pos + pos;
        offsets.push((last_pos, end_pos));
        last_pos = end_pos;
    }
    if last_pos < total_len {
        offsets.push((last_pos, total_len));
    }
    offsets
}

// Finds the first newline in input that is preceded by something that is not an escape char.
// Such newlines split the triples in the NTriples format.
fn next_newline_position(input: &[u8]) -> Option<usize> {
    Some(memchr(b'\n', input)? + 1)
}

// Given a number of desired chunks, corresponding to threads, find offsets that break the file into chunks that can be read in parallel.
// For NTriples, this can be done simply on newlines.
pub fn get_ntriples_file_chunks(
    file: &mut File,
    file_size: u64,
    n_chunks: usize,
) -> io::Result<Vec<(u64, u64)>> {
    let mut last_pos = 0;
    let chunk_size = file_size / u64::try_from(n_chunks).unwrap();
    let mut offsets = Vec::with_capacity(n_chunks);
    let mut buffer = [0; 4096];
    for _ in 0..n_chunks {
        let search_pos = last_pos + chunk_size;

        if search_pos >= file_size {
            break;
        }

        let Some(end_pos) = ({
            file.seek(SeekFrom::Start(search_pos))?;
            let mut pos = search_pos;
            loop {
                let extra = file.read(&mut buffer)?;
                if extra == 0 {
                    break None;
                }
                if let Some(extra) = next_newline_position(&buffer[..extra]) {
                    pos += u64::try_from(extra).unwrap();
                    break Some(pos);
                }
                pos += u64::try_from(extra).unwrap();
            }
        }) else {
            // We keep the valid chunks we found, and add (outside the loop) the rest of the bytes as a chunk.
            break;
        };
        offsets.push((last_pos, end_pos));
        last_pos = end_pos;
    }
    if last_pos < file_size {
        offsets.push((last_pos, file_size));
    }
    Ok(offsets)
}

// Given a number of desired chunks, corresponding to threads find offsets that break the file into chunks that can be read in parallel.
// A Turtle parser will be used to check (heuristically) if an offset starting with a period actually splits the file properly.
// The parser should not be reused, hence it is passed by value.
pub fn get_turtle_slice_chunks(
    bytes: &[u8],
    n_chunks: usize,
    parser: &TurtleParser,
) -> Vec<(usize, usize)> {
    let mut last_pos = 0;
    let total_len = bytes.len();
    let chunk_size = total_len / n_chunks;
    let mut offsets = Vec::with_capacity(n_chunks);
    for _ in 0..n_chunks {
        let search_pos = last_pos + chunk_size;

        if search_pos >= bytes.len() {
            break;
        }

        let Some(pos) = next_terminating_char(parser, &bytes[search_pos..]) else {
            // We keep the valid chunks we found,
            // and add (outside the loop) the rest of the bytes as a chunk.
            break;
        };
        let end_pos = search_pos + pos;
        offsets.push((last_pos, end_pos));
        last_pos = end_pos;
    }
    if last_pos < total_len {
        offsets.push((last_pos, total_len));
    }
    offsets
}

// Heuristically, we assume that a period is terminating (a triple) if we can start immediately after it and parse three triples.
// Parser should not be reused, hence it is passed by value.
// If no such period can be found, looking at 1000 consecutive periods, we give up.
// Important to keep this number this high, as some TTL files can have a lot of periods.
fn next_terminating_char(parser: &TurtleParser, mut input: &[u8]) -> Option<usize> {
    fn accept(parser: TurtleParser, input: &[u8]) -> bool {
        let mut f = parser.for_slice(input);
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

    let mut total_pos = 0;
    for _ in 0..1_000 {
        let pos = memchr(b'.', input)? + 1;
        if pos >= input.len() {
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
    }
    None
}
