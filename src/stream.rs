use crate::{error::Error, tls, uri::Uri};
use std::{
    io,
    net::{TcpStream, ToSocketAddrs},
    path::Path,
    time::Duration,
};

use crate::tls::Conn;

pub enum Stream {
    Http(TcpStream),
    Https(Conn<TcpStream>),
    //Custom(R)
}

impl Stream {
    pub fn new(uri: &Uri, connect_timeout: Option<Duration>) -> Result<Stream, Error> {
        let host = uri.host().unwrap_or("");
        let port = uri.corr_port();

        let stream = match connect_timeout {
            Some(timeout) => connect_with_timeout(host, port, timeout)?,
            None => TcpStream::connect((host, port))?,
        };

        Ok(Stream::Http(stream))
    }

    pub fn try_to_https(
        stream: Stream,
        uri: &Uri,
        root_cert_file_pem: Option<&Path>,
    ) -> Result<Stream, Error> {
        match stream {
            Stream::Http(http_stream) => {
                if uri.scheme() == "https" {
                    let host = uri.host().unwrap_or("");
                    let mut cnf = tls::Config::default();

                    let cnf = match root_cert_file_pem {
                        Some(p) => cnf.add_root_cert_file_pem(p)?,
                        None => &mut cnf,
                    };

                    let stream = cnf.connect(host, http_stream)?;
                    Ok(Stream::Https(stream))
                } else {
                    Ok(Stream::Http(http_stream))
                }
            }
            Stream::Https(_) => Ok(stream),
        }
    }

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

///Connects to target host with a timeout
pub fn connect_with_timeout<T, U>(host: T, port: u16, timeout: U) -> io::Result<TcpStream>
where
    Duration: From<U>,
    T: AsRef<str>,
{
    let host = host.as_ref();
    let timeout = Duration::from(timeout);
    let addrs: Vec<_> = (host, port).to_socket_addrs()?.collect();
    let count = addrs.len();

    for (idx, addr) in addrs.into_iter().enumerate() {
        match TcpStream::connect_timeout(&addr, timeout) {
            Ok(stream) => return Ok(stream),
            Err(err) => match err.kind() {
                io::ErrorKind::TimedOut => return Err(err),
                _ => {
                    if idx + 1 == count {
                        return Err(err);
                    }
                }
            },
        };
    }

    Err(io::Error::new(
        io::ErrorKind::AddrNotAvailable,
        format!("Could not resolve address for {:?}", host),
    ))
}
