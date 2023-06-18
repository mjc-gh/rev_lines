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
//!      let file = File::open("README.md").unwrap();
//!      let rev_lines = RevLines::new(file);
//!
//!      for line in rev_lines {
//!          println!("{:?}", line);
//!      }
//!  }
//! ```
//!
//! If a line with invalid UTF-8 is encountered, the iterator will return `None` next, and stop iterating.
//!
//! This method uses logic borrowed from [uutils/coreutils tail](https://github.com/uutils/coreutils/blob/f2166fed0ad055d363aedff6223701001af090d3/src/tail/tail.rs#L399-L402)

use std::cmp::min;
use std::io::{self, BufReader, Read, Seek, SeekFrom};

extern crate thiserror;
use thiserror::Error;

static DEFAULT_SIZE: usize = 4096;

static LF_BYTE: u8 = b'\n';
static CR_BYTE: u8 = b'\r';

/// `RevLines` struct
pub struct RawRevLines<R> {
    reader: BufReader<R>,
    reader_cursor: u64,
    buffer: Vec<u8>,
    buffer_end: usize,
    read_len: u64,
    was_last_byte_line_feed: bool,
}

impl<R: Seek + Read> RawRevLines<R> {
    /// Create a new `RawRevLines` struct from a Reader.
    /// Internal buffering for iteration will default to 4096 bytes at a time.
    pub fn new(reader: R) -> RawRevLines<R> {
        RawRevLines::with_capacity(DEFAULT_SIZE, reader)
    }

    /// Create a new `RawRevLines` struct from a Reader`.
    /// Internal buffering for iteration will use `cap` bytes at a time.
    pub fn with_capacity(cap: usize, reader: R) -> RawRevLines<R> {
        RawRevLines {
            reader: BufReader::new(reader),
            reader_cursor: u64::MAX,
            buffer: vec![0; cap],
            buffer_end: 0,
            read_len: 0,
            was_last_byte_line_feed: false,
        }
    }

    fn init_reader(&mut self) -> io::Result<()> {
        // Move cursor to the end of the file and store the cursor position
        self.reader_cursor = self.reader.seek(SeekFrom::End(0))?;
        // Next read will be the full buffer size or the remaining bytes in the file
        self.read_len = min(self.buffer.len() as u64, self.reader_cursor);
        // Move cursor just before the next bytes to read
        self.reader.seek_relative(-(self.read_len as i64))?;
        // Update the cursor position
        self.reader_cursor -= self.read_len;

        self.read_to_buffer()?;

        // Handle any trailing new line characters for the reader
        // so the first next call does not return Some("")
        if self.buffer_end > 0 {
            if let Some(last_byte) = self.buffer.get(self.buffer_end - 1) {
                if *last_byte == LF_BYTE {
                    self.buffer_end -= 1;
                    self.was_last_byte_line_feed = true;
                }
            }
        }

        Ok(())
    }

    fn read_to_buffer(&mut self) -> io::Result<()> {
        // Read the next bytes into the buffer, self.read_len was already prepared for that
        self.reader
            .read_exact(&mut self.buffer[0..(self.read_len as usize)])?;
        // Specify which part of the buffer is valid
        self.buffer_end = self.read_len as usize;

        // Determine what the next read length will be
        let next_read_len = min(self.buffer.len() as u64, self.reader_cursor);
        // Move the cursor just in front of the next read
        self.reader
            .seek_relative(-(self.read_len as i64 + next_read_len as i64))?;
        // Update cursor position
        self.reader_cursor -= next_read_len;

        // Store the next read length, it'll be used in the next call
        self.read_len = next_read_len;

        Ok(())
    }

    fn next_line(&mut self) -> io::Result<Option<Vec<u8>>> {
        // TODO: make self.reader_pos an Option, handle None in a helper method
        if self.reader_cursor == u64::MAX {
            self.init_reader()?;
        }

        let mut result: Vec<u8> = Vec::new();

        'outer: loop {
            // Current buffer was read to completion, read new contents
            if self.buffer_end == 0 {
                // Read the of minimum between the desired
                // buffer size or remaining length of the reader
                self.read_to_buffer()?;
            }

            // If buffer_end is still 0, it means the reader is empty
            if self.buffer_end == 0 {
                if result.is_empty() {
                    return Ok(None);
                } else {
                    break;
                }
            }

            for ch in self.buffer[..self.buffer_end].iter().rev() {
                self.buffer_end -= 1;
                // Found a new line character to break on
                if *ch == LF_BYTE {
                    self.was_last_byte_line_feed = true;
                    break 'outer;
                }
                // If previous byte was line feed, skip carriage return
                if *ch != CR_BYTE || !self.was_last_byte_line_feed {
                    result.push(*ch);
                }
                self.was_last_byte_line_feed = false;
            }
        }

        // Reverse the results since they were written backwards
        result.reverse();

        Ok(Some(result))
    }
}

impl<R: Read + Seek> Iterator for RawRevLines<R> {
    type Item = io::Result<Vec<u8>>;

    fn next(&mut self) -> Option<io::Result<Vec<u8>>> {
        self.next_line().transpose()
    }
}

#[derive(Debug, Error)]
pub enum RevLinesError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    InvalidUtf8(#[from] std::string::FromUtf8Error),
}

pub struct RevLines<R>(RawRevLines<R>);

impl<R: Read + Seek> RevLines<R> {
    /// Create a new `RawRevLines` struct from a Reader.
    /// Internal buffering for iteration will default to 4096 bytes at a time.
    pub fn new(reader: R) -> RevLines<R> {
        RevLines(RawRevLines::new(reader))
    }

    /// Create a new `RawRevLines` struct from a Reader`.
    /// Internal buffering for iteration will use `cap` bytes at a time.
    pub fn with_capacity(cap: usize, reader: R) -> RevLines<R> {
        RevLines(RawRevLines::with_capacity(cap, reader))
    }
}

impl<R: Read + Seek> Iterator for RevLines<R> {
    type Item = Result<String, RevLinesError>;

    fn next(&mut self) -> Option<Result<String, RevLinesError>> {
        let line = match self.0.next_line().transpose()? {
            Ok(line) => line,
            Err(error) => return Some(Err(RevLinesError::Io(error))),
        };

        Some(String::from_utf8(line).map_err(RevLinesError::InvalidUtf8))
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Cursor};

    use crate::{RawRevLines, RevLines};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn raw_handles_empty_files() -> TestResult {
        let file = Cursor::new(Vec::new());
        let mut rev_lines = RawRevLines::new(file);

        assert!(rev_lines.next().transpose()?.is_none());

        Ok(())
    }

    #[test]
    fn raw_handles_file_with_one_line() -> TestResult {
        let text = b"ABCD\n".to_vec();
        for cap in 1..(text.len() + 1) {
            let file = Cursor::new(&text);
            let mut rev_lines = RawRevLines::with_capacity(cap, file);

            assert_eq!(rev_lines.next().transpose()?, Some(b"ABCD".to_vec()));
            assert_eq!(rev_lines.next().transpose()?, None);
        }

        Ok(())
    }

    #[test]
    fn raw_handles_file_with_multi_lines() -> TestResult {
        let text = b"ABCDEF\nGHIJK\nLMNOPQRST\nUVWXYZ\n".to_vec();
        for cap in 1..(text.len() + 1) {
            let file = Cursor::new(b"ABCDEF\nGHIJK\nLMNOPQRST\nUVWXYZ\n".to_vec());
            let mut rev_lines = RawRevLines::with_capacity(cap, file);

            assert_eq!(rev_lines.next().transpose()?, Some(b"UVWXYZ".to_vec()));
            assert_eq!(rev_lines.next().transpose()?, Some(b"LMNOPQRST".to_vec()));
            assert_eq!(rev_lines.next().transpose()?, Some(b"GHIJK".to_vec()));
            assert_eq!(rev_lines.next().transpose()?, Some(b"ABCDEF".to_vec()));
            assert_eq!(rev_lines.next().transpose()?, None);
        }

        Ok(())
    }

    #[test]
    fn raw_handles_windows_file_with_multi_lines() -> TestResult {
        let text = b"ABCDEF\r\nGHIJK\r\nLMNOP\rQRST\r\nUVWXYZ\r\n".to_vec();
        for cap in 1..(text.len() + 1) {
            let file = Cursor::new(&text);
            let mut rev_lines = RawRevLines::with_capacity(cap, file);

            assert_eq!(rev_lines.next().transpose()?, Some(b"UVWXYZ".to_vec()));
            assert_eq!(rev_lines.next().transpose()?, Some(b"LMNOP\rQRST".to_vec())); // bare CR not stripped
            assert_eq!(rev_lines.next().transpose()?, Some(b"GHIJK".to_vec()));
            assert_eq!(rev_lines.next().transpose()?, Some(b"ABCDEF".to_vec()));
            assert_eq!(rev_lines.next().transpose()?, None);
        }

        Ok(())
    }

    #[test]
    fn raw_handles_file_with_blank_lines() -> TestResult {
        let file = Cursor::new(b"ABCD\n\nXYZ\n\n\n".to_vec());
        let mut rev_lines = RawRevLines::new(file);

        assert_eq!(rev_lines.next().transpose()?, Some(b"".to_vec()));
        assert_eq!(rev_lines.next().transpose()?, Some(b"".to_vec()));
        assert_eq!(rev_lines.next().transpose()?, Some(b"XYZ".to_vec()));
        assert_eq!(rev_lines.next().transpose()?, Some(b"".to_vec()));
        assert_eq!(rev_lines.next().transpose()?, Some(b"ABCD".to_vec()));
        assert_eq!(rev_lines.next().transpose()?, None);

        Ok(())
    }

    #[test]
    fn raw_handles_file_with_invalid_utf8() -> TestResult {
        let file = BufReader::new(Cursor::new(vec![
            b'A', b'B', b'C', b'D', b'E', b'F', b'\n', // some valid UTF-8 in this line
            b'X', 252, 253, 254, b'Y', b'\n', // invalid UTF-8 in this line
            b'G', b'H', b'I', b'J', b'K', b'\n', // some more valid UTF-8 at the end
        ]));
        let mut rev_lines = RawRevLines::new(file);
        assert_eq!(rev_lines.next().transpose()?, Some(b"GHIJK".to_vec()));
        assert_eq!(
            rev_lines.next().transpose()?,
            Some(vec![b'X', 252, 253, 254, b'Y'])
        );
        assert_eq!(rev_lines.next().transpose()?, Some(b"ABCDEF".to_vec()));
        assert_eq!(rev_lines.next().transpose()?, None);

        Ok(())
    }

    #[test]
    fn it_handles_empty_files() -> TestResult {
        let file = Cursor::new(Vec::new());
        let mut rev_lines = RevLines::new(file);

        assert!(rev_lines.next().transpose()?.is_none());

        Ok(())
    }

    #[test]
    fn it_handles_file_with_one_line() -> TestResult {
        let file = Cursor::new(b"ABCD\n".to_vec());
        let mut rev_lines = RevLines::new(file);

        assert_eq!(rev_lines.next().transpose()?, Some("ABCD".to_string()));
        assert_eq!(rev_lines.next().transpose()?, None);

        Ok(())
    }

    #[test]
    fn it_handles_file_with_multi_lines() -> TestResult {
        let file = Cursor::new(b"ABCDEF\nGHIJK\nLMNOPQRST\nUVWXYZ\n".to_vec());
        let mut rev_lines = RevLines::new(file);

        assert_eq!(rev_lines.next().transpose()?, Some("UVWXYZ".to_string()));
        assert_eq!(rev_lines.next().transpose()?, Some("LMNOPQRST".to_string()));
        assert_eq!(rev_lines.next().transpose()?, Some("GHIJK".to_string()));
        assert_eq!(rev_lines.next().transpose()?, Some("ABCDEF".to_string()));
        assert_eq!(rev_lines.next().transpose()?, None);

        Ok(())
    }

    #[test]
    fn it_handles_file_with_blank_lines() -> TestResult {
        let file = Cursor::new(b"ABCD\n\nXYZ\n\n\n".to_vec());
        let mut rev_lines = RevLines::new(file);

        assert_eq!(rev_lines.next().transpose()?, Some("".to_string()));
        assert_eq!(rev_lines.next().transpose()?, Some("".to_string()));
        assert_eq!(rev_lines.next().transpose()?, Some("XYZ".to_string()));
        assert_eq!(rev_lines.next().transpose()?, Some("".to_string()));
        assert_eq!(rev_lines.next().transpose()?, Some("ABCD".to_string()));
        assert_eq!(rev_lines.next().transpose()?, None);

        Ok(())
    }

    #[test]
    fn it_handles_file_with_multi_lines_and_with_capacity() -> TestResult {
        let file = Cursor::new(b"ABCDEF\nGHIJK\nLMNOPQRST\nUVWXYZ\n".to_vec());
        let mut rev_lines = RevLines::with_capacity(5, file);

        assert_eq!(rev_lines.next().transpose()?, Some("UVWXYZ".to_string()));
        assert_eq!(rev_lines.next().transpose()?, Some("LMNOPQRST".to_string()));
        assert_eq!(rev_lines.next().transpose()?, Some("GHIJK".to_string()));
        assert_eq!(rev_lines.next().transpose()?, Some("ABCDEF".to_string()));
        assert_eq!(rev_lines.next().transpose()?, None);

        Ok(())
    }

    #[test]
    fn it_handles_file_with_invalid_utf8() -> TestResult {
        let file = BufReader::new(Cursor::new(vec![
            b'A', b'B', b'C', b'D', b'E', b'F', b'\n', // some valid UTF-8 in this line
            b'X', 252, 253, 254, b'Y', b'\n', // invalid UTF-8 in this line
            b'G', b'H', b'I', b'J', b'K', b'\n', // some more valid UTF-8 at the end
        ]));
        let mut rev_lines = RevLines::new(file);
        assert_eq!(rev_lines.next().transpose()?, Some("GHIJK".to_string()));
        assert!(rev_lines.next().transpose().is_err());
        assert_eq!(rev_lines.next().transpose()?, Some("ABCDEF".to_string()));
        assert_eq!(rev_lines.next().transpose()?, None);

        Ok(())
    }
}
