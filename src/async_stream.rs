//! TCP stream

use async_net::TcpStream;
use futures_lite::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncReadExt, AsyncWriteExt};

use crate::{error::Error, tls, uri::Uri, CR_LF, LF};
use std::{
    io::{self}, net::ToSocketAddrs, path::Path, pin::Pin, sync::mpsc::{Receiver, RecvTimeoutError, Sender}, time::{Duration, Instant}
};

const BUF_SIZE: usize = 16 * 1000;

/// Wrapper around TCP stream for HTTP and HTTPS protocols.
/// Allows to perform common operations on underlying stream.
#[derive(Debug)]
pub enum AsyncStream {
    Http(TcpStream),
    Https(tls::AsyncConn<TcpStream>),
}

impl<'a> AsyncStream {
    /// Opens a TCP connection to a remote host with a connection timeout (if specified).
    #[deprecated(
        since = "0.12.0",
        note = "Stream::new(uri, connect_timeout) was replaced with Stream::connect(uri, connect_timeout)"
    )]
    pub async fn new(uri: &Uri<'a>, connect_timeout: Option<Duration>) -> Result<AsyncStream, Error> {
        AsyncStream::connect(uri, connect_timeout).await
    }

    /// Opens a TCP connection to a remote host with a connection timeout (if specified).
    pub async fn connect(uri: &Uri<'a>, connect_timeout: Option<Duration>) -> Result<AsyncStream, Error> {
        let host = uri.host().unwrap_or("");
        let port = uri.corr_port();

        let stream = match connect_timeout {
            Some(timeout) => connect_with_timeout(host, port, timeout).await?,
            None => TcpStream::connect((host, port)).await?,
        };

        Ok(AsyncStream::Http(stream))
    }

    /// Tries to establish a secure connection over TLS.
    ///
    /// Checks if `uri` scheme denotes a HTTPS protocol:
    /// - If yes, attemps to establish a secure connection
    /// - Otherwise, returns the `stream` without any modification
    pub async fn try_to_https(
        stream: AsyncStream,
        uri: &Uri<'a>,
        root_cert_file_pem: Option<&Path>,
    ) -> Result<AsyncStream, Error> {
        match stream {
            AsyncStream::Http(http_stream) => {
                if uri.scheme() == "https" {
                    let host = uri.host().unwrap_or("");
                    let mut cnf = tls::Config::default();

                    let cnf = match root_cert_file_pem {
                        Some(p) => cnf.add_root_cert_file_pem(p)?,
                        None => &mut cnf,
                    };

                    let stream = cnf.async_connect(host, http_stream).await?;
                    Ok(AsyncStream::Https(stream))
                } else {
                    Ok(AsyncStream::Http(http_stream))
                }
            }
            AsyncStream::Https(_) => Ok(stream),
        }
    }

    /// Sets the read timeout on the underlying TCP stream.
    pub fn set_read_timeout(&mut self, dur: Option<Duration>) -> Result<(), Error> {
        todo!()
    }

    /// Sets the write timeout on the underlying TCP stream.
    pub fn set_write_timeout(&mut self, dur: Option<Duration>) -> Result<(), Error> {
        todo!()
    }
}

impl AsyncRead for AsyncStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<io::Result<usize>> {
        let this = self.get_mut();
        match this {
            AsyncStream::Http(stream) => Pin::new(stream).poll_read(cx, buf),
            AsyncStream::Https(conn) => Pin::new(conn.get_mut()).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for AsyncStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<io::Result<usize>> {
        let this = self.get_mut();
        match this {
            AsyncStream::Http(stream) => Pin::new(stream).poll_write(cx, buf),
            AsyncStream::Https(conn) => Pin::new(conn.get_mut()).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<io::Result<()>> {
        let this = self.get_mut();
        match this {
            AsyncStream::Http(stream) => Pin::new(stream).poll_flush(cx),
            AsyncStream::Https(conn) => Pin::new(conn.get_mut()).poll_flush(cx),
        }
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<io::Result<()>> {
        let this = self.get_mut();
        match this {
            AsyncStream::Http(stream) => Pin::new(stream).poll_close(cx),
            AsyncStream::Https(conn) => Pin::new(conn.get_mut()).poll_close(cx),
        }
    }
}

/// Trait that allows to send data from readers to other threads
pub trait AsyncThreadSend {
    /// Reads `head` of the response and sends it via `sender`
    async fn async_send_head(&mut self, sender: &Sender<Vec<u8>>);

    /// Reads all bytes until EOF and sends them via `sender`
    async fn async_send_all(&mut self, sender: &Sender<Vec<u8>>);
}

impl<T> AsyncThreadSend for T
where
    T: AsyncBufRead + Unpin,
{
    async fn async_send_head(&mut self, sender: &Sender<Vec<u8>>) {
        let buf = read_head(self).await;
        sender.send(buf).unwrap_or(());
    }

    async fn async_send_all(&mut self, sender: &Sender<Vec<u8>>) {
        loop {
            let mut buf = [0; BUF_SIZE];

            match self.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(len) => {
                    let filled_buf = buf[..len].to_vec();
                    if let Err(_) = sender.send(filled_buf) {
                        break;
                    }
                }
            }
        }
    }
}

/// Trait that allows to receive data from receivers
pub trait AsyncThreadReceive {
    /// Receives data from `receiver` and writes them into this writer.
    /// Fails if `deadline` is exceeded.
    async fn async_receive(&mut self, receiver: &Receiver<Vec<u8>>, deadline: Instant) -> Result<(), Error>;

    /// Continuosly receives data from `receiver` until there is no more data
    /// or `deadline` is exceeded. Writes received data into this writer.
    async fn async_receive_all(&mut self, receiver: &Receiver<Vec<u8>>, deadline: Instant)
        -> Result<(), Error>;
}

impl<T> AsyncThreadReceive for T
where
    T: AsyncWriteExt + Unpin,
{
    async fn async_receive(&mut self, receiver: &Receiver<Vec<u8>>, deadline: Instant) -> Result<(), Error> {
        let now = Instant::now();
        let data_read = receiver.recv_timeout(deadline - now)?;

        Ok(self.write_all(&data_read).await?)
    }

    async fn async_receive_all(
        &mut self,
        receiver: &Receiver<Vec<u8>>,
        deadline: Instant,
    ) -> Result<(), Error> {
        // TODO: can't do a closure
        todo!()
        /*async_execute_with_deadline(deadline, async |remaining_time| {
            let data_read = match receiver.recv_timeout(remaining_time) {
                Ok(data) => data,
                Err(e) => match e {
                    RecvTimeoutError::Timeout => return Err(Error::Timeout),
                    RecvTimeoutError::Disconnected => return Ok(true),
                },
            };

            self.write_all(&data_read).await.map_err(|e| Error::IO(e))?;
            Ok(false)
        }).await*/
    }
}

/// Connects to the target host with a specified timeout.
pub async fn connect_with_timeout<T, U>(host: T, port: u16, timeout: U) -> io::Result<TcpStream>
where
    Duration: From<U>,
    T: AsRef<str>,
{
    let host = host.as_ref();
    let timeout = Duration::from(timeout);
    let addrs: Vec<_> = (host, port).to_socket_addrs()?.collect();
    let count = addrs.len();

    for (idx, addr) in addrs.into_iter().enumerate() {
        // TODO: don't have good timeout mechanism
        /*match TcpStream::connect_timeout(&addr, timeout) {
            Ok(stream) => return Ok(stream),
            Err(err) => match err.kind() {
                io::ErrorKind::TimedOut => return Err(err),
                _ => {
                    if idx + 1 == count {
                        return Err(err);
                    }
                }
            },
        };*/
        todo!()
    }

    Err(io::Error::new(
        io::ErrorKind::AddrNotAvailable,
        format!("Could not resolve address for {:?}", host),
    ))
}

/// Exexcutes a function in a loop until operation is completed or deadline is exceeded.
///
/// It checks if a timeout was exceeded every iteration, therefore it limits
/// how many time a specific function can be called before deadline.
/// For the `execute_with_deadline` to meet the deadline, each call
/// to `func` needs finish before the deadline.
///
/// Key information about function `func`:
/// - is provided with information about remaining time
/// - must ensure that its execution will not take more time than specified in `remaining_time`
/// - needs to return `Some(true)` when the operation is complete, and `Some(false)` - when operation is in progress
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

/// Executes an asynchronous function in a loop until the operation is completed or the deadline is exceeded.
///
/// It checks if a timeout was exceeded every iteration, therefore it limits
/// how many times a specific function can be called before the deadline.
/// For the `async_execute_with_deadline` to meet the deadline, each call
/// to `func` needs to finish before the deadline.
///
/// Key information about the function `func`:
/// - is provided with information about remaining time
/// - must ensure that its execution will not take more time than specified in `remaining_time`
/// - needs to return `Ok(true)` when the operation is complete, and `Ok(false)` when the operation is in progress
pub async fn async_execute_with_deadline<F, Fut>(deadline: Instant, mut func: F) -> Result<(), Error>
where
    F: FnMut(Duration) -> Fut + Send,
    Fut: std::future::Future<Output = Result<bool, Error>> + Send,
{
    loop {
        let now = Instant::now();
        let remaining_time = deadline - now;

        if remaining_time <= Duration::ZERO {
            return Err(Error::Timeout);
        }

        // no tokio timeout
        todo!()
    }

    Ok(())
}

/// Reads the head of HTTP response from `reader`.
///
/// Reads from `reader` (line by line) until a blank line is identified,
/// which indicates that all meta-information has been read,
pub async fn read_head<B>(reader: &mut B) -> Vec<u8>
where
    B: AsyncBufRead + Unpin,
{
    let mut buf = Vec::with_capacity(BUF_SIZE);

    loop {
        match reader.read_until(LF, &mut buf).await {
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