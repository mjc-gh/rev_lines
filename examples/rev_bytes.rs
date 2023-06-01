use std::io::Cursor;

extern crate rev_lines;
use rev_lines::RevLines;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = Cursor::new(vec![
        b'A', b'B', b'C', b'D', b'E', b'F', b'\n', // some valid UTF-8 in this line
        b'X', 252, 253, 254, b'Y', b'\n', // invalid UTF-8 in this line
        b'G', b'H', b'I', b'J', b'K', b'\n', // some more valid UTF-8 at the end
    ]);
    let rev_lines = RevLines::new(file);

    for line in rev_lines {
        // String::from_utf8_lossy would be another use case
        match String::from_utf8(line?) {
            Ok(line) => println!("{}", line),
            Err(e) => println!("Error: {}", e),
        }
    }

    Ok(())
}
