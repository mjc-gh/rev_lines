use std::io::{Cursor, BufReader};

extern crate rev_lines;
use rev_lines::RevByteLines;

fn main() -> Result<(), Box<dyn std::error::Error>> {
let file = BufReader::new(Cursor::new(vec![
    b'G', b'H', b'I', b'J', b'K', b'\n',
    b'X', 252, 253, 254, b'Y', b'\n',
    b'A', b'B', b'C', b'D', b'E', b'F']));
let rev_byte_lines = RevByteLines::new(file)?;

for line in rev_byte_lines {
    // String::from_utf8_lossy would be another use case
    match String::from_utf8(line) {
        Ok(line) => println!("{}", line),
        Err(e) => println!("Error: {}", e),
    }
}

    Ok(())
}