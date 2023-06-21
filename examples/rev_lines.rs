use std::io::Cursor;

use rev_lines::RevLines;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = Cursor::new("Just\na\nfew\nlines\n");
    let rev_lines = RevLines::new(file);

    for line in rev_lines {
        println!("{:?}", line);
    }

    Ok(())
}
