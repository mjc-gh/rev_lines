//! ### RevLines
//!
//! This library provides a small Rust Iterator for reading files or
//! any `BufReader` line by line with buffering in reverse.
//!
//! #### Example
//!
//! ```
//!  extern crate rev_lines;
//!
//!  use rev_lines::RevLines;
//!  use std::io::BufReader;
//!  use std::fs::File;
//!
//!  fn main() {
//!      let file = File::open("tests/multi_line_file").unwrap();
//!      let rev_lines = RevLines::new(BufReader::new(file)).unwrap();
//!
//!      for line in rev_lines {
//!          println!("{}", line);
//!      }
//!  }
//! ```
//!
//! If a line with invalid UTF-8 is encountered, the iterator will return `None` next, and stop iterating.
//!
//! This method uses logic borrowed from [uutils/coreutils
//! tail](https://github.com/uutils/coreutils/blob/f2166fed0ad055d363aedff6223701001af090d3/src/tail/tail.rs#L399-L402)

use std::cmp::min;
use std::io::{Seek, SeekFrom, Read, Result};
use std::io::BufReader;

static DEFAULT_SIZE: usize = 4096;

static LF_BYTE: u8 = '\n' as u8;
static CR_BYTE: u8 = '\r' as u8;

/// `RevLines` struct
pub struct RevLines<R> {
    reader: BufReader<R>,
    reader_pos: u64,
    buf_size: u64
}

impl<R:Seek+Read> RevLines<R> {
    /// Create a new `RevLines` struct from a `BufReader<R>`. Internal
    /// buffering for iteration will default to 4096 bytes at a time.
    pub fn new(reader: BufReader<R>) -> Result<RevLines<R>> {
        RevLines::with_capacity(DEFAULT_SIZE, reader)
    }

    /// Create a new `RevLines` struct from a `BufReader<R>`. Interal
    /// buffering for iteration will use `cap` bytes at a time.
    pub fn with_capacity(cap: usize, mut reader: BufReader<R>) -> Result<RevLines<R>> {
        // Seek to end of reader now
        let reader_size = reader.seek(SeekFrom::End(0))?;

        let mut rev_lines = RevLines {
            reader: reader,
            reader_pos: reader_size,
            buf_size: cap as u64,
        };

        // Handle any trailing new line characters for the reader
        // so the first next call does not return Some("")

        // Read at most 2 bytes
        let end_size = min(reader_size, 2);
        let end_buf = rev_lines.read_to_buffer(end_size)?;

        if end_size == 1 {
            if end_buf[0] != LF_BYTE {
                rev_lines.move_reader_position(1)?;
            }
        } else if end_size == 2 {
            if end_buf[0] != CR_BYTE {
                rev_lines.move_reader_position(1)?;
            }

            if end_buf[1] != LF_BYTE {
                rev_lines.move_reader_position(1)?;
            }
        }

        Ok(rev_lines)
    }

    fn read_to_buffer(&mut self, size: u64) -> Result<Vec<u8>> {
        let mut buf = vec![0; size as usize];
        let offset = -(size as i64);

        self.reader.seek(SeekFrom::Current(offset))?;
        self.reader.read_exact(&mut buf[0..(size as usize)])?;
        self.reader.seek(SeekFrom::Current(offset))?;

        self.reader_pos -= size;

        Ok(buf)
    }

    fn move_reader_position(&mut self, offset: u64) -> Result<()> {
        self.reader.seek(SeekFrom::Current(offset as i64))?;
        self.reader_pos += offset;

        Ok(())
    }
}

impl<R:Read+Seek> Iterator for RevLines<R> {
    type Item = String;

    fn next(&mut self) -> Option<String> {
        let mut result: Vec<u8> = Vec::new();

        'outer: loop {
            if self.reader_pos < 1 {
                if result.len() > 0 {
                    break;
                }

                return None;
            }

            // Read the of minimum between the desired
            // buffer size or remaining length of the reader
            let size = min(self.buf_size, self.reader_pos);

            match self.read_to_buffer(size) {
                Ok(buf) => {
                    for (idx, ch) in (&buf).iter().enumerate().rev() {
                        // Found a new line character to break on
                        if *ch == LF_BYTE {
                            let mut offset = idx as u64;

                            // Add an extra byte cause of CR character
                            if idx > 1 && buf[idx - 1] == CR_BYTE {
                                offset -= 1;
                            }

                            match self.reader.seek(SeekFrom::Current(offset as i64)) {
                                Ok(_)  => {
                                    self.reader_pos += offset;

                                    break 'outer;
                                },

                                Err(_) => return None,
                            }
                        } else {
                            result.push(ch.clone());
                        }
                    }
                }

                Err(_) => return None
            }
        }

        // Reverse the results since they were written backwards
        result.reverse();

        // Convert to a String
        String::from_utf8(result).ok()
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use RevLines;
    use std::io::BufReader;

    #[test]
    fn it_handles_empty_files() {
        let file = File::open("tests/empty_file").unwrap();
        let mut rev_lines = RevLines::new(BufReader::new(file)).unwrap();

        assert_eq!(rev_lines.next(), None);
    }

    #[test]
    fn it_handles_file_with_one_line() {
        let file = File::open("tests/one_line_file", ).unwrap();
        let mut rev_lines = RevLines::new(BufReader::new(file)).unwrap();

        assert_eq!(rev_lines.next(), Some("ABCD".to_string()));
        assert_eq!(rev_lines.next(), None);
    }

    #[test]
    fn it_handles_file_with_multi_lines() {
        let file = File::open("tests/multi_line_file").unwrap();
        let mut rev_lines = RevLines::new(BufReader::new(file)).unwrap();

        assert_eq!(rev_lines.next(), Some("UVWXYZ".to_string()));
        assert_eq!(rev_lines.next(), Some("LMNOPQRST".to_string()));
        assert_eq!(rev_lines.next(), Some("GHIJK".to_string()));
        assert_eq!(rev_lines.next(), Some("ABCDEF".to_string()));
        assert_eq!(rev_lines.next(), None);
    }

    #[test]
    fn it_handles_file_with_blank_lines() {
        let file = File::open("tests/blank_line_file").unwrap();
        let mut rev_lines = RevLines::new(BufReader::new(file)).unwrap();

        assert_eq!(rev_lines.next(), Some("".to_string()));
        assert_eq!(rev_lines.next(), Some("".to_string()));
        assert_eq!(rev_lines.next(), Some("XYZ".to_string()));
        assert_eq!(rev_lines.next(), Some("".to_string()));
        assert_eq!(rev_lines.next(), Some("ABCD".to_string()));
        assert_eq!(rev_lines.next(), None);
    }

    #[test]
    fn it_handles_file_with_multi_lines_and_with_capacity() {
        let file = File::open("tests/multi_line_file").unwrap();
        let mut rev_lines = RevLines::with_capacity(5, BufReader::new(file)).unwrap();

        assert_eq!(rev_lines.next(), Some("UVWXYZ".to_string()));
        assert_eq!(rev_lines.next(), Some("LMNOPQRST".to_string()));
        assert_eq!(rev_lines.next(), Some("GHIJK".to_string()));
        assert_eq!(rev_lines.next(), Some("ABCDEF".to_string()));
        assert_eq!(rev_lines.next(), None);
    }

    #[test]
    fn it_stops_iteration_on_invalid_utf8() {
        let file = File::open("tests/invalid_utf8").unwrap();
        let mut rev_lines = RevLines::with_capacity(5, BufReader::new(file)).unwrap();

        assert_eq!(rev_lines.next(), Some("Valid UTF8".to_string()));
        assert_eq!(rev_lines.next(), None);
        assert_eq!(rev_lines.next(), None);
    }
}
