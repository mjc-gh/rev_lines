# rev_lines

[![Travis Build Status](https://travis-ci.org/mikeycgto/message_verifier.svg)](https://travis-ci.org/mikeycgto/message_verifier)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)

This library provides a small Rust Iterator for reading files line by
line with a buffer in reverse

### Example

```rust
let file = File::open("/path/to/file").unwrap();
let mut rev_lines = RevLines::new(file).unwrap();

for line in rev_lines {
    println!("{}", line);
}
```
