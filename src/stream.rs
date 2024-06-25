//! TCP stream

use crate::{error::Error, tls, tls::Conn, uri::Uri, CR_LF, LF};
use std::{
    io::{self, BufRead, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    path::Path,
    sync::mpsc::{Receiver, Sender},
    time::{Duration, Instant},
};

const BUF_SIZE: usize = 1024 * 1024;

/// Wrapper around TCP stream for HTTP and HTTPS protocols.
/// Allows to perform common operations on underlying stream.
pub enum Stream {
    Http(TcpStream),
    Https(Conn<TcpStream>),
}

impl Stream {
    /// Opens a TCP connection to a remote host with a connection timeout (if specified).
    pub fn new(uri: &Uri, connect_timeout: Option<Duration>) -> Result<Stream, Error> {
        let host = uri.host().unwrap_or("");
        let port = uri.corr_port();

        let stream = match connect_timeout {
            Some(timeout) => connect_with_timeout(host, port, timeout)?,
            None => TcpStream::connect((host, port))?,
        };

        Ok(Stream::Http(stream))
    }

    /// Tries to establish a secure connection over TLS.
    ///
    /// Checks if `uri` scheme denotes a HTTPS protocol:
    /// - If yes, attemps to establish a secure connection
    /// - Otherwise, returns the `stream` without any modification
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

    /// Sets the read timeout on the underlying TCP stream.
    pub fn set_read_timeout(&mut self, dur: Option<Duration>) -> Result<(), Error> {
        match self {
            Stream::Http(stream) => Ok(stream.set_read_timeout(dur)?),
            Stream::Https(conn) => Ok(conn.get_mut().set_read_timeout(dur)?),
        }
    }

    /// Sets the write timeout on the underlying TCP stream.
    pub fn set_write_timeout(&mut self, dur: Option<Duration>) -> Result<(), Error> {
        match self {
            Stream::Http(stream) => Ok(stream.set_write_timeout(dur)?),
            Stream::Https(conn) => Ok(conn.get_mut().set_write_timeout(dur)?),
        }
    }
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        match self {
            Stream::Http(stream) => stream.read(buf),
            Stream::Https(stream) => stream.read(buf),
        }
    }
}

impl Write for Stream {
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

pub trait ThreadSend {
    /// Reads `head` of the response and sends it via `sender`
    fn send_head(&mut self, sender: &Sender<Vec<u8>>);

    /// Reads all bytes until EOF and sends them via `sender`
    fn send_all(&mut self, sender: &Sender<Vec<u8>>);
}

impl<T> ThreadSend for T
where
    T: BufRead,
{
    fn send_head(&mut self, sender: &Sender<Vec<u8>>) {
        let buf = read_head(self);
        sender.send(buf).unwrap();
    }

    fn send_all(&mut self, sender: &Sender<Vec<u8>>) {
        loop {
            let mut buf = vec![0; BUF_SIZE];

            match self.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(len) => {
                    let filled_buf = buf[..len].to_owned();
                    sender.send(filled_buf).unwrap();
                }
            }
        }
    }
}

pub trait ThreadReceive {
    /// Receives data from `receiver` and writes them into this writer.
    /// Fails if `deadline` is exceeded.
    fn receive(&mut self, receiver: &Receiver<Vec<u8>>, deadline: Instant);

    /// Continuosly receives data from `receiver` until there is no more data
    /// or `deadline` is exceeded. Writes received data into this writer.
    fn receive_all(&mut self, receiver: &Receiver<Vec<u8>>, deadline: Instant);
}

impl<T> ThreadReceive for T
where
    T: Write,
{
    fn receive(&mut self, receiver: &Receiver<Vec<u8>>, deadline: Instant) {
        execute_with_deadline(deadline, |remaining_time| {
            let data_read = match receiver.recv_timeout(remaining_time) {
                Ok(data) => data,
                Err(_) => return true,
            };

            self.write_all(&data_read).unwrap();
            true
        });
    }

    fn receive_all(&mut self, receiver: &Receiver<Vec<u8>>, deadline: Instant) {
        execute_with_deadline(deadline, |remaining_time| {
            let is_complete = false;

            let data_read = match receiver.recv_timeout(remaining_time) {
                Ok(data) => data,
                Err(_) => return true,
            };

            self.write_all(&data_read).unwrap();

            is_complete
        });
    }
}

/// Connects to the target host with a specified timeout.
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

/// Exexcutes a function in a loop until operation is completed
/// or deadline is reached.
///
/// Function `func` needs to return `true` when the operation is complete.
pub fn execute_with_deadline<F>(deadline: Instant, mut func: F)
where
    F: FnMut(Duration) -> bool,
{
    loop {
        let now = Instant::now();
        let remaining_time = deadline - now;

        if deadline < now || func(remaining_time) == true {
            break;
        }
    }
}

/// Reads the head of HTTP response from `reader`.
///
/// Reads from `reader` (line by line) until a blank line is found
/// indicating that all meta-information for the request has been sent.
pub fn read_head<B>(reader: &mut B) -> Vec<u8>
where
    B: BufRead,
{
    let mut buf = Vec::with_capacity(BUF_SIZE);

    loop {
        match reader.read_until(LF, &mut buf) {
            Ok(0) | Err(_) => break,
            Ok(len) => {
                let full_len = buf.len();

                if len == 2 && &buf[full_len - 2..] == CR_LF {
                    break;
                }
            }
        }
    }

    buf
}
