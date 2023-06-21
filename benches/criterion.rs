extern crate criterion;
use std::io::Cursor;

use criterion::{black_box, criterion_group, criterion_main, Criterion};

extern crate rev_lines;
use rev_lines::RawRevLines;

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

pub fn criterion_benchmark(c: &mut Criterion) {
    for (file_length, line_length, buffer_capacity) in [
        (1000000, 100, 20),
        (1000000, 100, 50),
        (1000000, 100, 100),
        (1000000, 5, 4096),
        (1000000, 20, 4096),
        (1000000, 50, 4096),
        (1000000, 80, 4096),
        (1000000, 1000, 4096),
    ] {
        c.bench_function(
            &format!("RawRevLines file_length={file_length} line_length={line_length}, buffer_capacity={buffer_capacity}"),
            |b| {
                b.iter(|| {
                    let reader = Cursor::new(input(black_box(file_length), black_box(line_length)));
                    let mut rev_lines = RawRevLines::with_capacity(buffer_capacity, reader);
                    while let Some(_) = rev_lines.next() {}
                })
            },
        );
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
