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
    reader_pos: u64,
    buf_size: u64,
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
            reader_pos: u64::MAX,
            buf_size: cap as u64,
        }
    }

    fn init_reader(&mut self) -> io::Result<()> {
        // Seek to end of reader now
        let reader_size = self.reader.seek(SeekFrom::End(0))?;
        self.reader_pos = reader_size;

        // Handle any trailing new line characters for the reader
        // so the first next call does not return Some("")

        // Read at most 2 bytes
        let end_size = min(reader_size, 2);
        let end_buf = self.read_to_buffer(end_size)?;

        if end_size == 1 {
            if end_buf[0] != LF_BYTE {
                self.move_reader_position(1)?;
            }
        } else if end_size == 2 {
            if end_buf[0] != CR_BYTE {
                self.move_reader_position(1)?;
            }

            if end_buf[1] != LF_BYTE {
                self.move_reader_position(1)?;
            }
        }

        Ok(())
    }

    fn read_to_buffer(&mut self, size: u64) -> io::Result<Vec<u8>> {
        let mut buf = vec![0; size as usize];
        let offset = -(size as i64);

        self.reader.seek(SeekFrom::Current(offset))?;
        self.reader.read_exact(&mut buf[0..(size as usize)])?;
        self.reader.seek(SeekFrom::Current(offset))?;

        self.reader_pos -= size;

        Ok(buf)
    }

    fn move_reader_position(&mut self, offset: u64) -> io::Result<()> {
        self.reader.seek(SeekFrom::Current(offset as i64))?;
        self.reader_pos += offset;

        Ok(())
    }

    fn next_line(&mut self) -> io::Result<Option<Vec<u8>>> {
        if self.reader_pos == u64::MAX {
            self.init_reader()?;
        }

        let mut result: Vec<u8> = Vec::new();

        'outer: loop {
            if self.reader_pos < 1 {
                if !result.is_empty() {
                    break;
                }

                return Ok(None);
            }

            // Read the of minimum between the desired
            // buffer size or remaining length of the reader
            let size = min(self.buf_size, self.reader_pos);

            let buf = self.read_to_buffer(size)?;
            for (idx, ch) in buf.iter().enumerate().rev() {
                // Found a new line character to break on
                if *ch == LF_BYTE {
                    let mut offset = idx as u64;

                    // Add an extra byte cause of CR character
                    if idx > 1 && buf[idx - 1] == CR_BYTE {
                        offset -= 1;
                    }

                    self.reader.seek(SeekFrom::Current(offset as i64))?;
                    self.reader_pos += offset;

                    break 'outer;
                } else {
                    result.push(*ch);
                }
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
        let file = Cursor::new(b"ABCD\n".to_vec());
        let mut rev_lines = RawRevLines::new(file);

        assert_eq!(rev_lines.next().transpose()?, Some(b"ABCD".to_vec()));
        assert_eq!(rev_lines.next().transpose()?, None);

        Ok(())
    }

    #[test]
    fn raw_handles_file_with_multi_lines() -> TestResult {
        let file = Cursor::new(b"ABCDEF\nGHIJK\nLMNOPQRST\nUVWXYZ\n".to_vec());
        let mut rev_lines = RawRevLines::new(file);

        assert_eq!(rev_lines.next().transpose()?, Some(b"UVWXYZ".to_vec()));
        assert_eq!(rev_lines.next().transpose()?, Some(b"LMNOPQRST".to_vec()));
        assert_eq!(rev_lines.next().transpose()?, Some(b"GHIJK".to_vec()));
        assert_eq!(rev_lines.next().transpose()?, Some(b"ABCDEF".to_vec()));
        assert_eq!(rev_lines.next().transpose()?, None);

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
    fn raw_handles_file_with_multi_lines_and_with_capacity() -> TestResult {
        let file = Cursor::new(b"ABCDEF\nGHIJK\nLMNOPQRST\nUVWXYZ\n".to_vec());
        let mut rev_lines = RawRevLines::with_capacity(5, file);

        assert_eq!(rev_lines.next().transpose()?, Some(b"UVWXYZ".to_vec()));
        assert_eq!(rev_lines.next().transpose()?, Some(b"LMNOPQRST".to_vec()));
        assert_eq!(rev_lines.next().transpose()?, Some(b"GHIJK".to_vec()));
        assert_eq!(rev_lines.next().transpose()?, Some(b"ABCDEF".to_vec()));
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
