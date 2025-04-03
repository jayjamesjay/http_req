//! support for Transfer-Encoding: chunked

use crate::CR_LF;
use std::{
    cmp,
    io::{self, BufRead, BufReader, Error, ErrorKind, Read},
};

const MAX_LINE_LENGTH: usize = 4096;

/// Implements the wire protocol for HTTP's Transfer-Encoding: chunked.
///
/// It's a Rust version of the [reference implementation in Go](https://golang.google.cn/src/net/http/internal/chunked.go)
pub struct ChunkReader<R> {
    check_end: bool,
    eof: bool,
    err: Option<Error>,
    n: usize,
    reader: BufReader<R>,
}

impl<R> Read for ChunkReader<R>
where
    R: Read,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut consumed = 0;
        let mut footer: [u8; 2] = [0; 2];

        while !self.eof && self.err.is_none() {
            if self.check_end {
                if consumed > 0 && self.reader.buffer().len() < 2 {
                    // We have some data. Return early (per the io.Reader
                    // contract) instead of potentially blocking while
                    // reading more.
                    break;
                }

                if self.reader.read_exact(&mut footer).is_ok() && &footer != CR_LF {
                    self.err = Some(Error::new(
                        ErrorKind::InvalidData,
                        "Malformed chunked encoding",
                    ));
                    break;
                }

                self.check_end = false;
            }

            if self.n == 0 {
                if consumed > 0 && !self.chunk_header_available() {
                    break;
                }

                self.begin_chunk();
                continue;
            }

            if buf.len() == consumed {
                break;
            }

            let end = cmp::min(consumed + self.n, buf.len());

            let mut n0: usize = 0;
            match self.reader.read(&mut buf[consumed..end]) {
                Ok(v) => n0 = v,
                Err(err) => self.err = Some(err),
            };

            consumed += n0;
            self.n -= n0;

            // If we're at the end of a chunk, read the next two
            // bytes to verify they are "\r\n".
            if self.n == 0 && self.err.is_none() {
                self.check_end = true;
            }
        }

        match self.err.as_ref() {
            Some(v) => Err(Error::new(v.kind(), v.to_string())),
            None => Ok(consumed),
        }
    }
}

impl<R> BufRead for ChunkReader<R>
where
    R: Read,
{
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.reader.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.reader.consume(amt)
    }
}

impl<R> From<BufReader<R>> for ChunkReader<R>
where
    R: Read,
{
    fn from(value: BufReader<R>) -> Self {
        ChunkReader {
            check_end: false,
            eof: false,
            err: None,
            n: 0,
            reader: value,
        }
    }
}

impl<R> ChunkReader<R>
where
    R: Read,
{
    /// Creates a new `ChunkReader` from `reader`
    pub fn new(reader: R) -> Self
    where
        R: Read,
    {
        Self {
            check_end: false,
            eof: false,
            err: None,
            n: 0,
            reader: BufReader::new(reader),
        }
    }

    /// Begins a new chunk by reading and parsing its header.
    ///
    /// This function reads one full line representing the size of the next HTTP/1.x chunk.
    /// If there is an error during this process (such as malformed data), it records that error for later use.
    fn begin_chunk(&mut self) {
        let line = match read_chunk_line(&mut self.reader) {
            Ok(v) => v,
            Err(err) => {
                self.err = Some(err);
                return;
            }
        };

        match parse_hex_uint(line) {
            Ok(v) => self.n = v,
            Err(err) => self.err = Some(Error::new(ErrorKind::InvalidData, err)),
        }

        self.eof = self.n == 0;
    }

    /// Checks whether a chunk header is available.
    fn chunk_header_available(&self) -> bool {
        self.reader.buffer().iter().any(|&c| c == b'\n')
    }
}

/// Checks if a given byte is an ASCII space character.
///
/// This function checks whether a single byte, b,
/// represents one of the following characters:
/// - Space (ASCII 0x20)
/// - Tab (ASCII 0x09)
/// - Line Feed (ASCII 0xA) or Carriage Return (ASCII 0xD), which are used to move
///   positions in text. These two together indicate an end of line.
fn is_ascii_space(b: u8) -> bool {
    match b {
        b' ' | b'\t' | b'\n' | b'\r' => true,
        _ => false,
    }
}

/// Parses an integer represented by hexadecimal digits from bytes.
fn parse_hex_uint<'a>(data: Vec<u8>) -> Result<usize, &'a str> {
    let mut n = 0;

    for (i, v) in data.iter().enumerate() {
        if i == 16 {
            return Err("HTTP chunk length is too large");
        }

        let vv = match *v {
            b'0'..=b'9' => v - b'0',
            b'a'..=b'f' => v - b'a' + 10,
            b'A'..=b'F' => v - b'A' + 10,
            _ => return Err("Invalid byte in chunk length"),
        };

        n <<= 4;
        n |= vv as usize;
    }

    Ok(n)
}

/// Reads a single chunk line from `BufReader<R>`.
fn read_chunk_line<R>(b: &mut BufReader<R>) -> io::Result<Vec<u8>>
where
    R: Read,
{
    let mut line = vec![];
    b.read_until(b'\n', &mut line)?;

    if line.len() > MAX_LINE_LENGTH {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "Exceeded maximum line length",
        ));
    }

    trim_trailing_whitespace(&mut line);
    remove_chunk_extension(&mut line);

    Ok(line)
}

/// Removes any trailing chunk extensions from a vector containing bytes (`Vec<u8>`).
fn remove_chunk_extension(v: &mut Vec<u8>) {
    if let Some(idx) = v.iter().position(|&v| v == b';') {
        v.truncate(idx);
    }
}

/// Remove any trailing whitespace characters (specifically ASCII spaces)
/// from the end of a vector containing bytes (`Vec<u8>`).
fn trim_trailing_whitespace(v: &mut Vec<u8>) {
    if v.is_empty() {
        return;
    }

    while let Some(&last_byte) = v.last() {
        if !is_ascii_space(last_byte) {
            break;
        }

        v.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Read};

    #[test]
    fn read() {
        let data: &[u8] = b"7\r\nhello, \r\n17\r\nworld! 0123456789abcdef\r\n0\r\n";
        let mut reader = ChunkReader::new(data);
        let mut writer = vec![];
        io::copy(&mut reader, &mut writer).expect("failed to dechunk");

        assert_eq!("hello, world! 0123456789abcdef".as_bytes(), &writer[..]);
    }

    #[test]
    fn read_multiple() {
        {
            let data: &[u8] = b"3\r\nfoo\r\n3\r\nbar\r\n0\r\n";
            let mut reader = ChunkReader::new(data);
            let mut writer = vec![0u8; 10];
            let n = reader.read(&mut writer).expect("unexpect error");

            assert_eq!(6, n, "invalid buffer length: expect {}, got {}", 6, n);
            assert_eq!("foobar".as_bytes(), &writer[..6]);
        }
        {
            let data: &[u8] = b"3\r\nfoo\r\n0\r\n";
            let mut reader = ChunkReader::new(data);
            let mut writer = vec![0u8; 3];
            let n = reader.read(&mut writer).expect("unexpect error");

            assert_eq!(3, n, "invalid buffer length");
            assert_eq!("foo".as_bytes(), &writer[..]);
        }
    }

    #[test]
    fn read_partial() {
        let data: &[u8] = b"7\r\n1234567";
        let mut reader = ChunkReader::new(data);
        let mut writer = vec![];
        io::copy(&mut reader, &mut writer).expect("failed to dechunk");

        assert_eq!("1234567".as_bytes(), &writer[..]);
    }

    #[test]
    fn read_ignore_extensions() {
        let data_str = String::from("7;ext=\"some quoted string\"\r\n")
            + "hello, \r\n"
            + "17;someext\r\n"
            + "world! 0123456789abcdef\r\n"
            + "0;someextension=sometoken\r\n";
        let data = data_str.as_bytes();
        let mut reader = ChunkReader::new(data);
        let mut writer = vec![];

        reader.read_to_end(&mut writer).expect("failed to dechunk");
        assert_eq!("hello, world! 0123456789abcdef".as_bytes(), &writer[..]);
    }
}
