//! module chunked implements the wire protocol for HTTP's "chunked" Transfer-Encoding.
//! And it's a rust version of the reference implementation in [Go][1].
//!
//! [1]: https://golang.google.cn/src/net/http/internal/chunked.go
//!

use std::io::{self, BufRead, BufReader, Error, ErrorKind, Read};

const MAX_LINE_LENGTH: usize = 4096;
const CR_LF: [u8; 2] = [b'\r', b'\n'];

pub struct Reader<R> {
    check_end: bool,
    eof: bool,
    err: Option<Error>,
    n: usize,
    reader: BufReader<R>,
}

impl<R> Read for Reader<R>
where
    R: Read,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // the length of data already read out
        let mut consumed = 0usize;
        let mut footer = [0u8; 2];

        while !self.eof && self.err.is_none() {
            if self.check_end {
                if consumed > 0 && self.reader.buffer().len() < 2 {
                    // We have some data. Return early (per the io.Reader
                    // contract) instead of potentially blocking while
                    // reading more.
                    break;
                }

                if let Ok(_) = self.reader.read_exact(&mut footer) {
                    if footer != CR_LF {
                        self.err = Some(error_malformed_chunked_encoding());
                        break;
                    }
                }

                self.check_end = false;
            }

            if self.n == 0 {
                if consumed > 0 && !self.chunk_header_avaliable() {
                    // We've read enough. Don't potentially block
                    // reading a new chunk header.
                    break;
                }

                self.begin_chunk();

                continue;
            }

            if buf.len() == consumed {
                break;
            }

            let end = if consumed + self.n < buf.len() {
                consumed + self.n
            } else {
                buf.len()
            };

            let mut n0 = 0usize;
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
            Some(v) => Err(Error::new(
                v.kind(),
                format!("wrapper by chunked: {}", v.to_string()),
            )),
            None => Ok(consumed),
        }
    }
}

impl<R> Reader<R>
where
    R: Read,
{
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

    fn begin_chunk(&mut self) {
        // chunk-size CRLF
        let line = match read_chunk_line(&mut self.reader) {
            Ok(v) => v,
            Err(err) => {
                self.err = Some(err);
                return;
            }
        };

        match parse_hex_uint(line) {
            Ok(v) => self.n = v,
            Err(err) => self.err = Some(Error::new(ErrorKind::Other, err)),
        }

        self.eof = self.n == 0;
    }

    fn chunk_header_avaliable(&self) -> bool {
        self.reader.buffer().iter().find(|&&c| c == b'\n').is_some()
    }
}

fn error_line_too_long() -> Error {
    Error::new(ErrorKind::Other, "header line too long")
}

fn error_malformed_chunked_encoding() -> Error {
    Error::new(ErrorKind::Other, "malformed chunked encoding")
}

fn is_ascii_space(b: u8) -> bool {
    match b {
        b' ' | b'\t' | b'\n' | b'\r' => true,
        _ => false,
    }
}

fn parse_hex_uint(data: Vec<u8>) -> Result<usize, &'static str> {
    let mut n = 0usize;
    for (i, v) in data.iter().enumerate() {
        if i == 16 {
            return Err("http chunk length too large");
        }

        let vv = match *v {
            b'0'..=b'9' => v - b'0',
            b'a'..=b'f' => v - b'a' + 10,
            b'A'..=b'F' => v - b'A' + 10,
            _ => return Err("invalid byte in chunk length"),
        };

        n <<= 4;
        n |= vv as usize;
    }

    Ok(n)
}

fn read_chunk_line<R>(b: &mut BufReader<R>) -> io::Result<Vec<u8>>
where
    R: Read,
{
    let mut line = vec![];
    b.read_until(b'\n', &mut line)?;

    if line.len() > MAX_LINE_LENGTH {
        return Err(error_line_too_long());
    }

    trim_trailing_whitespace(&mut line);
    remove_chunk_extension(&mut line);

    Ok(line)
}

fn remove_chunk_extension(v: &mut Vec<u8>) {
    if let Some(idx) = v.iter().position(|v| *v == b';') {
        v.resize(idx, 0);
    }
}

fn trim_trailing_whitespace(v: &mut Vec<u8>) {
    if v.len() == 0 {
        return;
    }

    for i in (0..(v.len() - 1)).rev() {
        if !is_ascii_space(v[i]) {
            v.resize(i + 1, 0);
            return;
        }
    }

    v.clear();
}

#[cfg(test)]
mod tests {
    use std::io::{self, Read};

    use super::*;

    #[test]
    fn read() {
        let data: &[u8] = b"7\r\nhello, \r\n17\r\nworld! 0123456789abcdef\r\n0\r\n";
        let mut reader = Reader::new(data);
        let mut writer = vec![];
        io::copy(&mut reader, &mut writer).expect("failed to dechunk");

        assert_eq!("hello, world! 0123456789abcdef".as_bytes(), &writer[..]);
    }
    #[test]
    fn read_multiple() {
        {
            let data: &[u8] = b"3\r\nfoo\r\n3\r\nbar\r\n0\r\n";
            let mut reader = Reader::new(data);
            let mut writer = vec![0u8; 10];
            let n = reader.read(&mut writer).expect("unexpect error");

            assert_eq!(6, n, "invalid buffer length: expect {}, got {}", 6, n);
            assert_eq!("foobar".as_bytes(), &writer[..6]);
        }
        {
            let data: &[u8] = b"3\r\nfoo\r\n0\r\n";
            let mut reader = Reader::new(data);
            let mut writer = vec![0u8; 3];
            let n = reader.read(&mut writer).expect("unexpect error");

            assert_eq!(3, n, "invalid buffer length");
            assert_eq!("foo".as_bytes(), &writer[..]);
        }
    }
    #[test]
    fn read_partial() {
        let data: &[u8] = b"7\r\n1234567";
        let mut reader = Reader::new(data);
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
        let mut reader = Reader::new(data);
        let mut writer = vec![];

        reader.read_to_end(&mut writer).expect("failed to dechunk");
        assert_eq!("hello, world! 0123456789abcdef".as_bytes(), &writer[..]);
    }
}
