//! TCP stream

#[cfg(any(feature = "native-tls", feature = "rust-tls"))]
use crate::tls::{self, Conn};
use crate::{
    error::{Error, ParseErr},
    uri::Uri,
    CR_LF, LF,
};
#[cfg(any(feature = "native-tls", feature = "rust-tls"))]
use std::path::Path;
use std::{
    io::{self, BufRead, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    sync::mpsc::{Receiver, RecvTimeoutError, Sender},
    time::{Duration, Instant},
};

const BUF_SIZE: usize = 16 * 1000;

/// Wrapper around TCP stream for HTTP and HTTPS protocols.
/// Allows to perform common operations on underlying stream.
#[derive(Debug)]
pub enum Stream {
    Http(TcpStream),
    #[cfg(any(feature = "native-tls", feature = "rust-tls"))]
    Https(Conn<TcpStream>),
}

impl Stream {
    /// Opens a TCP connection to a remote host with a connection timeout (if specified).
    pub fn connect(uri: &Uri, connect_timeout: Option<Duration>) -> Result<Stream, Error> {
        let host = uri.host().ok_or(Error::Parse(ParseErr::UriErr))?;
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
    /// - If yes, attempts to establish a secure connection
    /// - Otherwise, returns the `stream` without any modification
    #[cfg(any(feature = "native-tls", feature = "rust-tls"))]
    pub fn try_to_https(
        stream: Stream,
        uri: &Uri,
        root_cert_file_pem: Option<&Path>,
    ) -> Result<Stream, Error> {
        match stream {
            Stream::Http(http_stream) => {
                if uri.scheme() == "https" {
                    let host = uri.host().ok_or(Error::Parse(ParseErr::UriErr))?;
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
            #[cfg(any(feature = "native-tls", feature = "rust-tls"))]
            Stream::Https(conn) => Ok(conn.get_mut().set_read_timeout(dur)?),
        }
    }

    /// Sets the write timeout on the underlying TCP stream.
    pub fn set_write_timeout(&mut self, dur: Option<Duration>) -> Result<(), Error> {
        match self {
            Stream::Http(stream) => Ok(stream.set_write_timeout(dur)?),
            #[cfg(any(feature = "native-tls", feature = "rust-tls"))]
            Stream::Https(conn) => Ok(conn.get_mut().set_write_timeout(dur)?),
        }
    }
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        match self {
            Stream::Http(stream) => stream.read(buf),
            #[cfg(any(feature = "native-tls", feature = "rust-tls"))]
            Stream::Https(stream) => stream.read(buf),
        }
    }
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        match self {
            Stream::Http(stream) => stream.write(buf),
            #[cfg(any(feature = "native-tls", feature = "rust-tls"))]
            Stream::Https(stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        match self {
            Stream::Http(stream) => stream.flush(),
            #[cfg(any(feature = "native-tls", feature = "rust-tls"))]
            Stream::Https(stream) => stream.flush(),
        }
    }
}

/// Trait that allows to send data from readers to other threads
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
        sender.send(buf).unwrap_or(());
    }

    fn send_all(&mut self, sender: &Sender<Vec<u8>>) {
        loop {
            let mut buf = [0; BUF_SIZE];

            match self.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(len) => {
                    let filled_buf = buf[..len].to_vec();
                    if sender.send(filled_buf).is_err() {
                        break;
                    }
                }
            }
        }
    }
}

/// Trait that allows to receive data from receivers
pub trait ThreadReceive {
    /// Receives data from `receiver` and writes them into this writer.
    /// Fails if `deadline` is exceeded.
    fn receive(&mut self, receiver: &Receiver<Vec<u8>>, deadline: Instant) -> Result<(), Error>;

    /// Continuously receives data from `receiver` until there is no more data
    /// or `deadline` is exceeded. Writes received data into this writer.
    fn receive_all(&mut self, receiver: &Receiver<Vec<u8>>, deadline: Instant)
        -> Result<(), Error>;
}

impl<T> ThreadReceive for T
where
    T: Write,
{
    fn receive(&mut self, receiver: &Receiver<Vec<u8>>, deadline: Instant) -> Result<(), Error> {
        let now = Instant::now();

        match receiver.recv_timeout(deadline - now) {
            Ok(data_read) => self.write_all(&data_read).map_err(Error::IO),
            Err(RecvTimeoutError::Timeout) => Err(Error::Timeout),
            Err(RecvTimeoutError::Disconnected) => Ok(()),
        }
    }

    fn receive_all(
        &mut self,
        receiver: &Receiver<Vec<u8>>,
        deadline: Instant,
    ) -> Result<(), Error> {
        execute_with_deadline(deadline, |remaining_time| {
            match receiver.recv_timeout(remaining_time) {
                Ok(data_read) => {
                    if let Err(e) = self.write_all(&data_read) {
                        return Err(Error::IO(e));
                    }
                    Ok(false)
                }
                Err(RecvTimeoutError::Timeout) => Err(Error::Timeout),
                Err(RecvTimeoutError::Disconnected) => Ok(true),
            }
        })
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

/// Executes a function in a loop until operation is completed or deadline is exceeded.
///
/// It checks if a timeout was exceeded every iteration, therefore it limits
/// how many times a specific function can be called before the deadline.
/// For `execute_with_deadline` to meet the deadline, each call
/// to `func` needs to finish before the deadline.
///
/// Key information about function `func`:
/// - is provided with information about remaining time
/// - must ensure that its execution will not take more time than specified in `remaining_time`
/// - needs to return `Some(true)` when the operation is complete, and `Some(false)` - when the operation is in progress
pub fn execute_with_deadline<F>(deadline: Instant, mut func: F) -> Result<(), Error>
where
    F: FnMut(Duration) -> Result<bool, Error>,
{
    loop {
        let now = Instant::now();
        let remaining_time = deadline - now;

        if deadline < now {
            return Err(Error::Timeout);
        }

        match func(remaining_time) {
            Ok(true) => break,
            Ok(false) => continue,
            Err(e) => return Err(e),
        }
    }

    Ok(())
}

/// Reads the head of HTTP response from `reader`.
///
/// Reads from `reader` (line by line) until a blank line is identified,
/// which indicates that all meta-information has been read.
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::BufReader, sync::mpsc, thread};

    const URI: &str = "http://doc.rust-lang.org/std/string/index.html";
    const URI_S: &str = "https://en.wikipedia.org/wiki/Hypertext_Transfer_Protocol";
    const TIMEOUT: Duration = Duration::from_secs(3);
    const RESPONSE: &[u8; 129] = b"HTTP/1.1 200 OK\r\n\
                                 Date: Sat, 11 Jan 2003 02:44:04 GMT\r\n\
                                 Content-Type: text/html\r\n\
                                 Content-Length: 100\r\n\r\n\
                                 <html>hello</html>\r\n\r\nhello";

    const RESPONSE_H: &[u8; 102] = b"HTTP/1.1 200 OK\r\n\
                                   Date: Sat, 11 Jan 2003 02:44:04 GMT\r\n\
                                   Content-Type: text/html\r\n\
                                   Content-Length: 100\r\n\r\n";

    #[test]
    fn stream_new() {
        {
            let uri = Uri::try_from(URI).unwrap();
            let stream = Stream::connect(&uri, None);

            assert!(stream.is_ok());
        }
        {
            let uri = Uri::try_from(URI).unwrap();
            let stream = Stream::connect(&uri, Some(TIMEOUT));

            assert!(stream.is_ok());
        }
    }

    #[test]
    #[cfg(any(feature = "native-tls", feature = "rust-tls"))]
    fn stream_try_to_https() {
        {
            let uri = Uri::try_from(URI_S).unwrap();
            let stream = Stream::connect(&uri, None).unwrap();
            let https_stream = Stream::try_to_https(stream, &uri, None);

            assert!(https_stream.is_ok());

            // Scheme is `https`, therefore stream should be converted into HTTPS variant
            match https_stream.unwrap() {
                Stream::Http(_) => assert!(false),
                Stream::Https(_) => assert!(true),
            }
        }
        {
            let uri = Uri::try_from(URI).unwrap();
            let stream = Stream::connect(&uri, None).unwrap();
            let https_stream = Stream::try_to_https(stream, &uri, None);

            assert!(https_stream.is_ok());

            // Scheme is `http`, therefore stream should returned without changes
            match https_stream.unwrap() {
                Stream::Http(_) => assert!(true),
                Stream::Https(_) => assert!(false),
            }
        }
    }

    #[test]
    fn stream_set_read_timeot() {
        {
            let uri = Uri::try_from(URI).unwrap();
            let mut stream = Stream::connect(&uri, None).unwrap();
            stream.set_read_timeout(Some(TIMEOUT)).unwrap();

            let inner_read_timeout = if let Stream::Http(inner) = stream {
                inner.read_timeout().unwrap()
            } else {
                None
            };

            assert_eq!(inner_read_timeout, Some(TIMEOUT));
        }
        #[cfg(any(feature = "native-tls", feature = "rust-tls"))]
        {
            let uri = Uri::try_from(URI_S).unwrap();
            let mut stream = Stream::connect(&uri, None).unwrap();
            stream = Stream::try_to_https(stream, &uri, None).unwrap();
            stream.set_read_timeout(Some(TIMEOUT)).unwrap();

            let inner_read_timeout = if let Stream::Https(inner) = stream {
                inner.get_ref().read_timeout().unwrap()
            } else {
                None
            };

            assert_eq!(inner_read_timeout, Some(TIMEOUT));
        }
    }

    #[test]
    fn stream_set_write_timeot() {
        {
            let uri = Uri::try_from(URI).unwrap();
            let mut stream = Stream::connect(&uri, None).unwrap();
            stream.set_write_timeout(Some(TIMEOUT)).unwrap();

            let inner_read_timeout = if let Stream::Http(inner) = stream {
                inner.write_timeout().unwrap()
            } else {
                None
            };

            assert_eq!(inner_read_timeout, Some(TIMEOUT));
        }
        #[cfg(any(feature = "native-tls", feature = "rust-tls"))]
        {
            let uri = Uri::try_from(URI_S).unwrap();
            let mut stream = Stream::connect(&uri, None).unwrap();
            stream = Stream::try_to_https(stream, &uri, None).unwrap();
            stream.set_write_timeout(Some(TIMEOUT)).unwrap();

            let inner_read_timeout = if let Stream::Https(inner) = stream {
                inner.get_ref().write_timeout().unwrap()
            } else {
                None
            };

            assert_eq!(inner_read_timeout, Some(TIMEOUT));
        }
    }

    #[test]
    fn thread_send_send_head() {
        let (sender, receiver) = mpsc::channel();

        thread::spawn(move || {
            let mut reader = BufReader::new(RESPONSE.as_slice());
            reader.send_head(&sender);
        });

        let raw_head = receiver.recv().unwrap();
        assert_eq!(raw_head, RESPONSE_H);
    }

    #[test]
    fn thread_send_send_all() {
        let (sender, receiver) = mpsc::channel();

        thread::spawn(move || {
            let mut reader = BufReader::new(RESPONSE.as_slice());
            reader.send_all(&sender);
        });

        let raw_head = receiver.recv().unwrap();
        assert_eq!(raw_head, RESPONSE);
    }

    #[test]
    fn thread_receive_receive() {
        let (sender, receiver) = mpsc::channel();
        let deadline = Instant::now() + TIMEOUT;

        thread::spawn(move || {
            let res = [RESPONSE[..50].to_vec(), RESPONSE[50..].to_vec()];

            for part in res {
                sender.send(part).unwrap();
            }
        });

        let mut buf = Vec::with_capacity(BUF_SIZE);
        buf.receive(&receiver, deadline).unwrap();

        assert_eq!(buf, RESPONSE[..50]);
    }

    #[test]
    fn thread_receive_receive_all() {
        let (sender, receiver) = mpsc::channel();
        let deadline = Instant::now() + TIMEOUT;

        thread::spawn(move || {
            let res = [RESPONSE[..50].to_vec(), RESPONSE[50..].to_vec()];

            for part in res {
                sender.send(part).unwrap();
            }
        });

        let mut buf = Vec::with_capacity(BUF_SIZE);
        buf.receive_all(&receiver, deadline).unwrap();

        assert_eq!(buf, RESPONSE);
    }

    #[ignore]
    #[test]
    fn fn_execute_with_deadline() {
        {
            let star_time = Instant::now();
            let deadline = star_time + TIMEOUT;

            let timeout_err = execute_with_deadline(deadline, |_| {
                let sleep_time = Duration::from_millis(500);
                thread::sleep(sleep_time);

                Ok(false)
            });

            let end_time = Instant::now();
            let total_time = end_time.duration_since(star_time).as_secs();

            assert_eq!(total_time, TIMEOUT.as_secs());
            assert!(timeout_err.is_err());
        }
        {
            let star_time = Instant::now();
            let deadline = star_time + TIMEOUT;

            execute_with_deadline(deadline, |_| {
                let sleep_time = Duration::from_secs(1);
                thread::sleep(sleep_time);

                Ok(true)
            })
            .unwrap();

            let end_time = Instant::now();
            let total_time = end_time.duration_since(star_time).as_secs();

            assert_eq!(total_time, 1);
        }
    }

    #[test]
    fn fn_read_head() {
        let reader = RESPONSE.as_slice();
        let mut buf_reader = BufReader::new(reader);
        let raw_head = read_head(&mut buf_reader);

        assert_eq!(raw_head, RESPONSE_H);
    }
}
