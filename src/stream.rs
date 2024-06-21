use crate::error::Error;
use std::io;
use std::net::TcpStream;
use std::time::Duration;

use crate::tls::Conn;

pub enum Stream {
    Http(TcpStream),
    Https(Conn<TcpStream>),
}

impl Stream {
    pub fn set_read_timeout(&mut self, dur: Option<Duration>) -> Result<(), Error> {
        match self {
            Stream::Http(stream) => Ok(stream.set_read_timeout(dur)?),
            Stream::Https(_) => Err(Error::Tls),
        }
    }

    pub fn set_write_timeout(&mut self, dur: Option<Duration>) -> Result<(), Error> {
        match self {
            Stream::Http(stream) => Ok(stream.set_write_timeout(dur)?),
            Stream::Https(_) => Err(Error::Tls),
        }
    }
}

impl io::Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        match self {
            Stream::Http(stream) => stream.read(buf),
            Stream::Https(stream) => stream.read(buf),
        }
    }
}

impl io::Write for Stream {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        match self {
            Stream::Http(stream) => stream.write(buf),
            Stream::Https(stream) => stream.write(buf),
        }
    }
    fn flush(&mut self) -> Result<(), io::Error> {
        match self {
            Stream::Http(stream) => stream.flush(),
            Stream::Https(stream) => stream.flush(),
        }
    }
}
