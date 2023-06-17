extern crate iai;
extern crate rev_lines;

use std::io::Cursor;

use rev_lines::RawRevLines;

const KB: usize = 1024;
const FILE_LENGTH: usize = 20 * KB;

fn input(file_length: usize, lines_length: u32) -> Vec<u8> {
    let mut count = 0;
    std::iter::from_fn(move || {
        count += 1;

        if count % lines_length == 0 {
            Some(b'\n')
        } else {
            Some(b'a')
        }
    })
    .take(file_length)
    .collect()
}

fn raw_rev_lines_next_line_length_20_buffer_capacity_4096() {
    let reader = Cursor::new(input(FILE_LENGTH, 20));
    let mut rev_lines = RawRevLines::with_capacity(4096, reader);
    while let Some(_) = rev_lines.next() {}
}

fn raw_rev_lines_next_line_length_160_buffer_capacity_4096() {
    let reader = Cursor::new(input(FILE_LENGTH, 160));
    let mut rev_lines = RawRevLines::with_capacity(4096, reader);
    while let Some(_) = rev_lines.next() {}
}

iai::main!(
    raw_rev_lines_next_line_length_20_buffer_capacity_4096,
    raw_rev_lines_next_line_length_160_buffer_capacity_4096
);
