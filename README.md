# rev_lines

[![crates.io](https://img.shields.io/crates/v/rev-lines.svg)](https://crates.io/crates/rev_lines)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)

This library provides a small Rust Iterator for reading files line by
line with a buffer in reverse

### Documentation

Documentation is available on [Docs.rs](https://docs.rs/rev_lines).

### Example

```rust
extern crate rev_lines;

use rev_lines::RevLines;

let file = File::open("/path/to/file").unwrap();
let mut rev_lines = RevLines::new(file).unwrap();

for line in rev_lines {
    println!("{}", line);
}
```
