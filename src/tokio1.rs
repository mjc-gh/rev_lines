//! #### Tokio Example
//!
//! ```rust
//!  extern crate rev_lines;
//!
//!  use rev_lines::tokio1::RevLines;
//!  use tokio::io::BufReader;
//!  use tokio::fs::File;
//!  use futures::StreamExt;
//!
//!  #[tokio::main]
//!  async fn main() {
//!      let file = File::open("tests/multi_line_file").await.unwrap();
//!      let mut rev_lines = RevLines::new(BufReader::new(file)).await.unwrap();
//!
//!      while let Some(line) = rev_lines.lines().next().await {
//!          println!("{}", line);
//!      }
//!  }
//! ```

use async_stream::stream;
use futures::{Stream, StreamExt};
use std::cmp::min;
use std::io::{Result, SeekFrom};
use std::pin::Pin;
use tokio::io::BufReader;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt};

static DEFAULT_SIZE: usize = 4096;

static LF_BYTE: u8 = b'\n';
static CR_BYTE: u8 = b'\r';

/// `RevLines` struct
pub struct RevLines<R: AsyncSeek + AsyncRead + Unpin + Send> {
    reader: BufReader<R>,
    reader_pos: u64,
    buf_size: u64,
}

impl<R: AsyncSeek + AsyncRead + Unpin + Send> RevLines<R> {
    /// Create a new `RevLines` struct from a `BufReader<R>`. Internal
    /// buffering for iteration will default to 4096 bytes at a time.
    pub async fn new(reader: BufReader<R>) -> Result<RevLines<R>> {
        RevLines::with_capacity(DEFAULT_SIZE, reader).await
    }

    /// Create a new `RevLines` struct from a `BufReader<R>`. Interal
    /// buffering for iteration will use `cap` bytes at a time.
    pub async fn with_capacity(cap: usize, mut reader: BufReader<R>) -> Result<RevLines<R>> {
        // Seek to end of reader now
        let reader_size = reader.seek(SeekFrom::End(0)).await?;

        let mut rev_lines = RevLines {
            reader,
            reader_pos: reader_size,
            buf_size: cap as u64,
        };

        // Handle any trailing new line characters for the reader
        // so the first next call does not return Some("")

        // Read at most 2 bytes
        let end_size = min(reader_size, 2);
        let end_buf = rev_lines.read_to_buffer(end_size).await?;

        if end_size == 1 {
            if end_buf[0] != LF_BYTE {
                rev_lines.move_reader_position(1).await?;
            }
        } else if end_size == 2 {
            if end_buf[0] != CR_BYTE {
                rev_lines.move_reader_position(1).await?;
            }

            if end_buf[1] != LF_BYTE {
                rev_lines.move_reader_position(1).await?;
            }
        }

        Ok(rev_lines)
    }

    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_possible_wrap)]
    async fn read_to_buffer(&mut self, size: u64) -> Result<Vec<u8>> {
        let mut buf = vec![0; size as usize];
        let offset = -(size as i64);

        self.reader.seek(SeekFrom::Current(offset)).await?;
        self.reader.read_exact(&mut buf[0..(size as usize)]).await?;
        self.reader.seek(SeekFrom::Current(offset)).await?;

        self.reader_pos -= size;

        Ok(buf)
    }

    #[allow(clippy::cast_possible_wrap)]
    async fn move_reader_position(&mut self, offset: u64) -> Result<()> {
        self.reader.seek(SeekFrom::Current(offset as i64)).await?;
        self.reader_pos += offset;

        Ok(())
    }

    #[allow(clippy::cast_possible_wrap)]
    pub async fn get_line(&mut self) -> Option<String> {
        let mut result: Vec<u8> = Vec::new();

        'outer: loop {
            if self.reader_pos < 1 {
                if !result.is_empty() {
                    break;
                }

                return None;
            }

            // Read the of minimum between the desired
            // buffer size or remaining length of the reader
            let size = min(self.buf_size, self.reader_pos);

            match self.read_to_buffer(size).await {
                Ok(buf) => {
                    for (idx, ch) in (&buf).iter().enumerate().rev() {
                        // Found a new line character to break on
                        if *ch == LF_BYTE {
                            let mut offset = idx as u64;

                            // Add an extra byte cause of CR character
                            if idx > 1 && buf[idx - 1] == CR_BYTE {
                                offset -= 1;
                            }

                            match self.reader.seek(SeekFrom::Current(offset as i64)).await {
                                Ok(_) => {
                                    self.reader_pos += offset;

                                    break 'outer;
                                }

                                Err(_) => return None,
                            }
                        }
                        result.push(*ch);
                    }
                }

                Err(_) => return None,
            }
        }

        // Reverse the results since they were written backwards
        result.reverse();

        // Convert to a String
        Some(String::from_utf8(result).unwrap_or_default())
    }
    pub fn lines(&mut self) -> Pin<Box<impl ?Sized + Stream<Item = String> + '_>> {
        let stream = stream! {
            while let Some(line) = self.get_line().await {
                yield line
            }
        };
        stream.boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::RevLines;
    use futures::StreamExt;
    use tokio::fs::File;
    use tokio::io::BufReader;

    #[tokio::test]
    async fn it_handles_empty_files() {
        let file = File::open("tests/empty_file").await.unwrap();
        let mut rev_lines = RevLines::new(BufReader::new(file)).await.unwrap();
        assert_eq!(rev_lines.lines().next().await, None);
    }

    #[tokio::test]
    async fn it_handles_file_with_one_line() {
        let file = File::open("tests/one_line_file").await.unwrap();
        let mut rev_lines = RevLines::new(BufReader::new(file)).await.unwrap();

        let mut lines = rev_lines.lines();

        assert_eq!(lines.next().await, Some("ABCD".to_string()));
        assert_eq!(lines.next().await, None);
    }

    #[tokio::test]
    async fn it_handles_file_with_multi_lines() {
        let file = File::open("tests/multi_line_file").await.unwrap();
        let mut rev_lines = RevLines::new(BufReader::new(file)).await.unwrap();
        let mut lines = rev_lines.lines();
        assert_eq!(lines.next().await, Some("UVWXYZ".to_string()));
        assert_eq!(lines.next().await, Some("LMNOPQRST".to_string()));
        assert_eq!(lines.next().await, Some("GHIJK".to_string()));
        assert_eq!(lines.next().await, Some("ABCDEF".to_string()));
        assert_eq!(lines.next().await, None);
    }

    #[tokio::test]
    async fn it_handles_file_with_blank_lines() {
    let file = File::open("tests/blank_line_file").await.unwrap();
    let mut rev_lines = RevLines::new(BufReader::new(file)).await.unwrap();
    let mut lines = rev_lines.lines();

    assert_eq!(lines.next().await, Some("".to_string()));
    assert_eq!(lines.next().await, Some("".to_string()));
    assert_eq!(lines.next().await, Some("XYZ".to_string()));
    assert_eq!(lines.next().await, Some("".to_string()));
    assert_eq!(lines.next().await, Some("ABCD".to_string()));
    assert_eq!(lines.next().await, None);
    }

    #[tokio::test]
    async fn it_handles_file_with_multi_lines_and_with_capacity() {
    let file = File::open("tests/multi_line_file").await.unwrap();
    let mut rev_lines = RevLines::with_capacity(5, BufReader::new(file)).await.unwrap();
    let mut lines = rev_lines.lines();

    assert_eq!(lines.next().await, Some("UVWXYZ".to_string()));
    assert_eq!(lines.next().await, Some("LMNOPQRST".to_string()));
    assert_eq!(lines.next().await, Some("GHIJK".to_string()));
    assert_eq!(lines.next().await, Some("ABCDEF".to_string()));
    assert_eq!(lines.next().await, None);
    }
}
