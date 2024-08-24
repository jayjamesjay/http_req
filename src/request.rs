//! creating and sending HTTP requests
use futures_lite::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncReadExt, AsyncWriteExt};

use crate::{
    async_stream::{AsyncStream, AsyncThreadReceive, AsyncThreadSend}, chunked::ChunkReader, error, response::{Headers, Response}, stream::{Stream, ThreadReceive, ThreadSend}, uri::Uri
};
use std::{
    convert::TryFrom, fmt, io::{BufReader, Write}, path::Path, sync::mpsc, thread, time::{Duration, Instant}
};

const CR_LF: &str = "\r\n";
const DEFAULT_REQ_TIMEOUT: u64 = 60 * 60;
const DEFAULT_CALL_TIMEOUT: u64 = 60;

/// HTTP request methods
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Method {
    GET,
    HEAD,
    POST,
    PUT,
    DELETE,
    CONNECT,
    OPTIONS,
    TRACE,
    PATCH,
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Method::*;

        let method = match self {
            GET => "GET",
            HEAD => "HEAD",
            POST => "POST",
            PUT => "PUT",
            DELETE => "DELETE",
            CONNECT => "CONNECT",
            OPTIONS => "OPTIONS",
            TRACE => "TRACE",
            PATCH => "PATCH",
        };

        write!(f, "{}", method)
    }
}

/// HTTP versions
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum HttpVersion {
    Http10,
    Http11,
    Http20,
}

impl HttpVersion {
    pub const fn as_str(&self) -> &str {
        use self::HttpVersion::*;

        match self {
            Http10 => "HTTP/1.0",
            Http11 => "HTTP/1.1",
            Http20 => "HTTP/2.0",
        }
    }
}

impl fmt::Display for HttpVersion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

pub struct RequestBuilder {}

#[deprecated(
    since = "0.12.0",
    note = "RequestBuilder was replaced with RequestMessage"
)]
impl<'a> RequestBuilder {
    pub fn new(uri: &'a Uri<'a>) -> RequestMessage<'a> {
        RequestMessage::new(uri)
    }
}

/// Allows to control redirects
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum RedirectPolicy<F> {
    /// Follows redirect if limit is greater than 0.
    Limit(usize),
    /// Runs functions `F` to determine if redirect should be followed.
    Custom(F),
}

impl<F: Fn() -> bool> RedirectPolicy<F> {
    /// Checks the policy againt specified conditions.
    /// Returns `true` if redirect should be followed.
    pub fn follow(&mut self) -> bool {
        use self::RedirectPolicy::*;

        match self {
            Limit(limit) => match limit {
                0 => false,
                _ => {
                    *limit = *limit - 1;
                    true
                }
            },
            Custom(func) => func(),
        }
    }
}

impl<F: Fn() -> bool> Default for RedirectPolicy<F> {
    fn default() -> Self {
        RedirectPolicy::Limit(5)
    }
}

/// Raw HTTP request message that can be sent to any stream
///
/// # Examples
/// ```
/// use std::convert::TryFrom;
/// use http_req::{request::RequestMessage, uri::Uri};
///
/// let addr: Uri = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
///
/// let mut request_msg = RequestMessage::new(&addr)
///      .header("Connection", "Close")
///      .parse();
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct RequestMessage<'a> {
    uri: &'a Uri<'a>,
    method: Method,
    version: HttpVersion,
    headers: Headers,
    body: Option<&'a [u8]>,
}

impl<'a> RequestMessage<'a> {
    /// Creates a new `RequestMessage` with default parameters
    ///
    /// # Examples
    /// ```
    /// use std::convert::TryFrom;
    /// use http_req::{request::RequestMessage, uri::Uri};
    ///
    /// let addr = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
    ///
    /// let request_msg = RequestMessage::new(&addr)
    ///     .header("Connection", "Close");
    /// ```
    pub fn new(uri: &'a Uri<'a>) -> RequestMessage<'a> {
        RequestMessage {
            headers: Headers::default_http(uri),
            uri,
            method: Method::GET,
            version: HttpVersion::Http11,
            body: None,
        }
    }

    /// Sets the request method
    ///
    /// # Examples
    /// ```
    /// use std::convert::TryFrom;
    /// use http_req::{request::{RequestMessage, Method}, uri::Uri};
    ///
    /// let addr = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
    ///
    /// let request_msg = RequestMessage::new(&addr)
    ///     .method(Method::HEAD);
    /// ```
    pub fn method<T>(&mut self, method: T) -> &mut Self
    where
        Method: From<T>,
    {
        self.method = Method::from(method);
        self
    }

    /// Sets the HTTP version
    ///
    /// # Examples
    /// ```
    /// use std::convert::TryFrom;
    /// use http_req::{request::{RequestMessage, HttpVersion}, uri::Uri};
    ///
    /// let addr = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
    ///
    /// let request_msg = RequestMessage::new(&addr)
    ///     .version(HttpVersion::Http10);
    /// ```
    pub fn version<T>(&mut self, version: T) -> &mut Self
    where
        HttpVersion: From<T>,
    {
        self.version = HttpVersion::from(version);
        self
    }

    /// Replaces all it's headers with headers passed to the function
    ///
    /// # Examples
    /// ```
    /// use std::convert::TryFrom;
    /// use http_req::{request::RequestMessage, response::Headers, uri::Uri};
    ///
    /// let addr = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
    ///
    /// let mut headers = Headers::new();
    /// headers.insert("Accept-Charset", "utf-8");
    /// headers.insert("Accept-Language", "en-US");
    /// headers.insert("Host", "rust-lang.org");
    /// headers.insert("Connection", "Close");
    ///
    /// let request_msg = RequestMessage::new(&addr)
    ///     .headers(headers);
    /// ```
    pub fn headers<T>(&mut self, headers: T) -> &mut Self
    where
        Headers: From<T>,
    {
        self.headers = Headers::from(headers);
        self
    }

    /// Adds a new header to existing/default headers
    ///
    /// # Examples
    /// ```
    /// use std::convert::TryFrom;
    /// use http_req::{request::RequestMessage, response::Headers, uri::Uri};
    ///
    /// let addr = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
    ///
    /// let request_msg = RequestMessage::new(&addr)
    ///     .header("Connection", "Close");
    /// ```
    pub fn header<T, U>(&mut self, key: &T, val: &U) -> &mut Self
    where
        T: ToString + ?Sized,
        U: ToString + ?Sized,
    {
        self.headers.insert(key, val);
        self
    }

    /// Sets the body for request
    ///
    /// # Examples
    /// ```
    /// use std::convert::TryFrom;
    /// use http_req::{request::{RequestMessage, Method}, response::Headers, uri::Uri};
    ///
    /// let addr = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
    /// const BODY: &[u8; 27] = b"field1=value1&field2=value2";
    ///
    /// let request_msg = RequestMessage::new(&addr)
    ///     .method(Method::POST)
    ///     .body(BODY)
    ///     .header("Content-Length", &BODY.len())
    ///     .header("Connection", "Close");
    /// ```
    pub fn body(&mut self, body: &'a [u8]) -> &mut Self {
        self.body = Some(body);
        self
    }

    /// Parses the request message for this `RequestMessage`
    ///
    /// # Examples
    /// ```
    /// use std::convert::TryFrom;
    /// use http_req::{request::RequestMessage, uri::Uri};
    ///
    /// let addr: Uri = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
    ///
    /// let mut request_msg = RequestMessage::new(&addr)
    ///      .header("Connection", "Close")
    ///      .parse();
    /// ```
    pub fn parse(&self) -> Vec<u8> {
        let request_line = format!(
            "{} {} {}{}",
            self.method,
            self.uri.resource(),
            self.version,
            CR_LF
        );

        let headers: String = self
            .headers
            .iter()
            .map(|(k, v)| format!("{}: {}{}", k, v, CR_LF))
            .collect();

        let mut request_msg = (request_line + &headers + CR_LF).as_bytes().to_vec();

        if let Some(b) = self.body {
            request_msg.extend(b);
        }

        request_msg
    }
}

/// Allows for making HTTP requests based on specified parameters.
///
/// It creates a stream (`TcpStream` or `TlsStream`) appropriate for the type of uri (`http`/`https`).
/// By default it closes connection after completion of the response.
///
/// # Examples
/// ```
/// use http_req::{request::Request, uri::Uri, response::StatusCode};
/// use std::convert::TryFrom;
///
/// let mut writer = Vec::new();
/// let uri = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
///
/// let response = Request::new(&uri).send(&mut writer).unwrap();;
/// assert_eq!(response.status_code(), StatusCode::new(200));
/// ```
///
#[derive(Clone, Debug, PartialEq)]
pub struct Request<'a> {
    messsage: RequestMessage<'a>,
    redirect_policy: RedirectPolicy<fn() -> bool>,
    connect_timeout: Option<Duration>,
    read_timeout: Option<Duration>,
    write_timeout: Option<Duration>,
    timeout: Duration,
    root_cert_file_pem: Option<&'a Path>,
}

impl<'a> Request<'a> {
    /// Creates a new `Request` with default parameters.
    ///
    /// # Examples
    /// ```
    /// use http_req::{request::Request, uri::Uri};
    /// use std::convert::TryFrom;
    ///
    /// let uri: Uri = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
    ///
    /// let request = Request::new(&uri);
    /// ```
    pub fn new(uri: &'a Uri) -> Request<'a> {
        let mut message = RequestMessage::new(&uri);
        message.header("Connection", "Close");

        Request {
            messsage: message,
            redirect_policy: RedirectPolicy::default(),
            connect_timeout: Some(Duration::from_secs(DEFAULT_CALL_TIMEOUT)),
            read_timeout: Some(Duration::from_secs(DEFAULT_CALL_TIMEOUT)),
            write_timeout: Some(Duration::from_secs(DEFAULT_CALL_TIMEOUT)),
            timeout: Duration::from_secs(DEFAULT_REQ_TIMEOUT),
            root_cert_file_pem: None,
        }
    }

    /// Sets the request method.
    ///
    /// # Examples
    /// ```
    /// use http_req::{request::{Request, Method}, uri::Uri};
    /// use std::convert::TryFrom;
    ///
    /// let uri: Uri = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
    ///
    /// let request = Request::new(&uri)
    ///     .method(Method::HEAD);
    /// ```
    pub fn method<T>(&mut self, method: T) -> &mut Self
    where
        Method: From<T>,
    {
        self.messsage.method(method);
        self
    }

    /// Sets the HTTP version.
    ///
    /// # Examples
    /// ```
    /// use http_req::{request::{Request, HttpVersion}, uri::Uri};
    /// use std::convert::TryFrom;
    ///
    /// let uri = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
    ///
    /// let request = Request::new(&uri)
    ///     .version(HttpVersion::Http10);
    /// ```

    pub fn version<T>(&mut self, version: T) -> &mut Self
    where
        HttpVersion: From<T>,
    {
        self.messsage.version(version);
        self
    }

    /// Replaces all it's headers with headers passed to the function.
    ///
    /// # Examples
    /// ```
    /// use http_req::{request::Request, uri::Uri, response::Headers};
    /// use std::convert::TryFrom;
    ///
    /// let uri: Uri = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
    ///
    /// let mut headers = Headers::new();
    /// headers.insert("Accept-Charset", "utf-8");
    /// headers.insert("Accept-Language", "en-US");
    /// headers.insert("Host", "rust-lang.org");
    /// headers.insert("Connection", "Close");
    ///
    /// let request = Request::new(&uri)
    ///     .headers(headers);
    /// ```
    pub fn headers<T>(&mut self, headers: T) -> &mut Self
    where
        Headers: From<T>,
    {
        self.messsage.headers(headers);
        self
    }

    /// Adds the header to existing/default headers.
    ///
    /// # Examples
    /// ```
    /// use http_req::{request::Request, uri::Uri};
    /// use std::convert::TryFrom;
    ///
    /// let uri = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
    ///
    /// let request = Request::new(&uri)
    ///     .header("Accept-Language", "en-US");
    /// ```
    pub fn header<T, U>(&mut self, key: &T, val: &U) -> &mut Self
    where
        T: ToString + ?Sized,
        U: ToString + ?Sized,
    {
        self.messsage.header(key, val);
        self
    }

    /// Sets the body for request.
    ///
    /// # Examples
    /// ```
    /// use http_req::{request::{Request, Method}, uri::Uri};
    /// use std::convert::TryFrom;
    ///
    /// let uri = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
    /// const body: &[u8; 27] = b"field1=value1&field2=value2";
    ///
    /// let request = Request::new(&uri)
    ///     .method(Method::POST)
    ///     .header("Content-Length", &body.len())
    ///     .body(body);
    /// ```
    pub fn body(&mut self, body: &'a [u8]) -> &mut Self {
        self.messsage.body(body);
        self
    }

    /// Sets the connect timeout while using internal `TcpStream` instance.
    ///
    /// - If there is a timeout, it will be passed to
    ///   [`TcpStream::connect_timeout`][TcpStream::connect_timeout].
    /// - If `None` is provided, [`TcpStream::connect`][TcpStream::connect] will
    ///   be used. A timeout will still be enforced by the operating system, but
    ///   the exact value depends on the platform.
    ///
    /// [TcpStream::connect]: https://doc.rust-lang.org/std/net/struct.TcpStream.html#method.connect
    /// [TcpStream::connect_timeout]: https://doc.rust-lang.org/std/net/struct.TcpStream.html#method.connect_timeout
    ///
    /// # Examples
    /// ```
    /// use http_req::{request::Request, uri::Uri};
    /// use std::{time::Duration, convert::TryFrom};
    ///
    /// let uri = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
    /// const time: Option<Duration> = Some(Duration::from_secs(10));
    ///
    /// let request = Request::new(&uri)
    ///     .connect_timeout(time);
    /// ```
    pub fn connect_timeout<T>(&mut self, timeout: Option<T>) -> &mut Self
    where
        Duration: From<T>,
    {
        self.connect_timeout = timeout.map(Duration::from);
        self
    }

    /// Sets the read timeout on internal `TcpStream` instance.
    ///
    /// `timeout` will be passed to
    /// [`TcpStream::set_read_timeout`][TcpStream::set_read_timeout].
    ///
    /// [TcpStream::set_read_timeout]: https://doc.rust-lang.org/std/net/struct.TcpStream.html#method.set_read_timeout
    ///
    /// # Examples
    /// ```
    /// use http_req::{request::Request, uri::Uri};
    /// use std::{time::Duration, convert::TryFrom};
    ///
    /// let uri: Uri = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
    /// const time: Option<Duration> = Some(Duration::from_secs(15));
    ///
    /// let request = Request::new(&uri)
    ///     .read_timeout(time);
    /// ```
    pub fn read_timeout<T>(&mut self, timeout: Option<T>) -> &mut Self
    where
        Duration: From<T>,
    {
        self.read_timeout = timeout.map(Duration::from);
        self
    }

    /// Sets the write timeout on internal `TcpStream` instance.
    ///
    /// `timeout` will be passed to
    /// [`TcpStream::set_write_timeout`][TcpStream::set_write_timeout].
    ///
    /// [TcpStream::set_write_timeout]: https://doc.rust-lang.org/std/net/struct.TcpStream.html#method.set_write_timeout
    ///
    /// # Examples
    /// ```
    /// use http_req::{request::Request, uri::Uri};
    /// use std::{time::Duration, convert::TryFrom};
    ///
    /// let uri = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
    /// const time: Option<Duration> = Some(Duration::from_secs(5));
    ///
    /// let request = Request::new(&uri)
    ///     .write_timeout(time);
    /// ```
    pub fn write_timeout<T>(&mut self, timeout: Option<T>) -> &mut Self
    where
        Duration: From<T>,
    {
        self.write_timeout = timeout.map(Duration::from);
        self
    }

    /// Sets the timeout on entire request.
    /// Data is read from a stream until there is no more data to read or the timeout is exceeded.
    ///
    /// # Examples
    /// ```
    /// use http_req::{request::Request, uri::Uri};
    /// use std::{time::Duration, convert::TryFrom};
    ///
    /// let uri = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
    /// const time: Duration = Duration::from_secs(5);
    ///
    /// let request = Request::new(&uri)
    ///     .timeout(time);
    /// ```
    pub fn timeout<T>(&mut self, timeout: T) -> &mut Self
    where
        Duration: From<T>,
    {
        self.timeout = Duration::from(timeout);
        self
    }

    /// Adds the file containing the PEM-encoded certificates that should be added in the trusted root store.
    ///
    /// # Examples
    /// ```
    /// use http_req::{request::Request, uri::Uri};
    /// use std::{time::Duration, convert::TryFrom, path::Path};
    ///
    /// let uri = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
    /// let path = Path::new("./foo/bar.txt");
    ///
    /// let request = Request::new(&uri)
    ///     .root_cert_file_pem(&path);
    /// ```
    pub fn root_cert_file_pem(&mut self, file_path: &'a Path) -> &mut Self {
        self.root_cert_file_pem = Some(file_path);
        self
    }

    /// Sets the redirect policy for the request.
    ///
    /// # Examples
    /// ```
    /// use http_req::{request::{Request, RedirectPolicy}, uri::Uri};
    /// use std::{time::Duration, convert::TryFrom, path::Path};
    ///
    /// let uri = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
    ///
    /// let request = Request::new(&uri)
    ///     .redirect_policy(RedirectPolicy::Limit(5));
    /// ```
    pub fn redirect_policy<T>(&mut self, policy: T) -> &mut Self
    where
        RedirectPolicy<fn() -> bool>: From<T>,
    {
        self.redirect_policy = RedirectPolicy::from(policy);
        self
    }

    /// Sends the HTTP request and returns `Response`.
    ///
    /// Creates `TcpStream` (and wraps it with `TlsStream` if needed). Writes request message
    /// to created stream. Returns response for this request. Writes response's body to `writer`.
    ///
    /// # Examples
    /// ```
    /// use http_req::{request::Request, uri::Uri};
    /// use std::convert::TryFrom;
    ///
    /// let mut writer = Vec::new();
    /// let uri: Uri = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
    ///
    /// let response = Request::new(&uri).send(&mut writer).unwrap();
    /// ```
    pub fn send<T>(&mut self, writer: &mut T) -> Result<Response, error::Error>
    where
        T: Write,
    {
        // Set up a stream.
        let mut stream = Stream::connect(self.messsage.uri, self.connect_timeout)?;
        stream.set_read_timeout(self.read_timeout)?;
        stream.set_write_timeout(self.write_timeout)?;
        stream = Stream::try_to_https(stream, self.messsage.uri, self.root_cert_file_pem)?;

        // Send the request message to stream.
        let request_msg = self.messsage.parse();
        stream.write_all(&request_msg)?;

        // Set up variables
        let deadline = Instant::now() + self.timeout;
        let (sender, receiver) = mpsc::channel();
        let (sender_supp, receiver_supp) = mpsc::channel();
        let mut raw_response_head: Vec<u8> = Vec::new();
        let mut buf_reader = BufReader::new(stream);

        // Read from the stream and send over data via `sender`.
        thread::spawn(move || {
            buf_reader.send_head(&sender);

            let params: Vec<&str> = receiver_supp.recv().unwrap_or(Vec::new());
            if params.contains(&"non-empty") {
                if params.contains(&"chunked") {
                    let mut buf_reader = ChunkReader::from(buf_reader);
                    buf_reader.send_all(&sender);
                } else {
                    buf_reader.send_all(&sender);
                }
            }
        });

        // Receive and process `head` of the response.
        raw_response_head.receive(&receiver, deadline)?;
        let response = Response::from_head(&raw_response_head)?;

        if response.status_code().is_redirect() && self.redirect_policy.follow() {
            if let Some(location) = response.headers().get("Location") {
                let mut raw_uri = location.to_string();
                let uri = if Uri::is_relative(&raw_uri) {
                    self.messsage.uri.from_relative(&mut raw_uri)
                } else {
                    Uri::try_from(raw_uri.as_str())
                }?;

                return Request::new(&uri)
                    .redirect_policy(self.redirect_policy)
                    .send(writer);
            }
        }

        let params = response.basic_info(&self.messsage.method).to_vec();
        sender_supp.send(params)?;

        // Receive and process `body` of the response.
        let content_len = response.content_len().unwrap_or(1);
        if content_len > 0 {
            writer.receive_all(&receiver, deadline)?;
        }

        Ok(response)
    }

    pub async fn async_send<T>(&mut self, writer: &mut T) -> Result<Response, error::Error>
    where
        T: AsyncWrite + AsyncRead + AsyncThreadReceive + AsyncThreadSend,
    {
        // Set up a stream.
        let mut stream = AsyncStream::connect(self.messsage.uri, self.connect_timeout).await?;
        stream.set_read_timeout(self.read_timeout)?;
        stream.set_write_timeout(self.write_timeout)?;
        stream = AsyncStream::try_to_https(stream, self.messsage.uri, self.root_cert_file_pem).await?;

        // Send the request message to stream.
        let request_msg = self.messsage.parse();
        stream.write_all(&request_msg).await?;

        // Set up variables
        let deadline = Instant::now() + self.timeout;
        let (sender, receiver) = mpsc::channel();
        let (sender_supp, receiver_supp) = mpsc::channel();
        let mut raw_response_head: Vec<u8> = Vec::new();
        let mut buf_reader = futures_lite::io::BufReader::new(stream);

        // Read from the stream and send over data via `sender`.
        buf_reader.async_send_head(&sender).await;

        let params: Vec<&str> = receiver_supp.recv().unwrap_or(Vec::new());
        if params.contains(&"non-empty") {
            if params.contains(&"chunked") {
                // TODO: async chunked reader
                todo!()
            } else {
                buf_reader.async_send_all(&sender);
            }
        }

        // Receive and process `head` of the response.
        raw_response_head.receive(&receiver, deadline)?;
        let response = Response::from_head(&raw_response_head)?;

        if response.status_code().is_redirect() && self.redirect_policy.follow() {
            if let Some(location) = response.headers().get("Location") {
                let mut raw_uri = location.to_string();
                let uri = if Uri::is_relative(&raw_uri) {
                    self.messsage.uri.from_relative(&mut raw_uri)
                } else {
                    Uri::try_from(raw_uri.as_str())
                }?;

                // Use `Box::pin` here to handle recursion
                return Box::pin(Request::new(&uri)
                    .redirect_policy(self.redirect_policy)
                    .async_send(writer))
                    .await;
            }
        }

        let params = response.basic_info(&self.messsage.method).to_vec();
        sender_supp.send(params)?;

        // Receive and process `body` of the response.
        let content_len = response.content_len().unwrap_or(1);
        if content_len > 0 {
            writer.async_receive_all(&receiver, deadline).await?;
        }

        Ok(response)
    }
}

/// Creates and sends GET request. Returns response for this request.
///
/// # Examples
/// ```
/// use http_req::request;
///
/// let mut writer = Vec::new();
/// const uri: &str = "https://www.rust-lang.org/learn";
///
/// let response = request::get(uri, &mut writer).unwrap();
/// ```
pub fn get<T, U>(uri: T, writer: &mut U) -> Result<Response, error::Error>
where
    T: AsRef<str>,
    U: Write,
{
    let uri = Uri::try_from(uri.as_ref())?;
    Request::new(&uri).send(writer)
}

/// Creates and sends HEAD request. Returns response for this request.
///
/// # Examples
/// ```
/// use http_req::request;
///
/// const uri: &str = "https://www.rust-lang.org/learn";
/// let response = request::head(uri).unwrap();
/// ```
pub fn head<T>(uri: T) -> Result<Response, error::Error>
where
    T: AsRef<str>,
{
    let mut writer = Vec::new();
    let uri = Uri::try_from(uri.as_ref())?;

    Request::new(&uri).method(Method::HEAD).send(&mut writer)
}

/// Creates and sends POST request. Returns response for this request.
///
/// # Examples
/// ```
/// use http_req::request;
///
/// let mut writer = Vec::new();
/// const uri: &str = "https://www.rust-lang.org/learn";
/// const body: &[u8; 27] = b"field1=value1&field2=value2";
///
/// let response = request::post(uri, body, &mut writer).unwrap();
/// ```
pub fn post<T, U>(uri: T, body: &[u8], writer: &mut U) -> Result<Response, error::Error>
where
    T: AsRef<str>,
    U: Write,
{
    let uri = Uri::try_from(uri.as_ref())?;

    Request::new(&uri)
        .method(Method::POST)
        .header("Content-Length", &body.len())
        .body(body)
        .send(writer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{error::Error, response::StatusCode};
    use std::io;

    const UNSUCCESS_CODE: StatusCode = StatusCode::new(400);
    const URI: &str = "http://doc.rust-lang.org/std/string/index.html";
    const URI_S: &str = "https://doc.rust-lang.org/std/string/index.html";
    const BODY: [u8; 14] = [78, 97, 109, 101, 61, 74, 97, 109, 101, 115, 43, 74, 97, 121];

    #[test]
    fn method_display() {
        const METHOD: Method = Method::HEAD;
        assert_eq!(&format!("{}", METHOD), "HEAD");
    }

    #[test]
    fn request_m_new() {
        RequestMessage::new(&Uri::try_from(URI).unwrap());
        RequestMessage::new(&Uri::try_from(URI_S).unwrap());
    }

    #[test]
    fn request_m_method() {
        let uri = Uri::try_from(URI).unwrap();
        let mut req = RequestMessage::new(&uri);
        let req = req.method(Method::HEAD);

        assert_eq!(req.method, Method::HEAD);
    }

    #[test]
    fn request_m_headers() {
        let mut headers = Headers::new();
        headers.insert("Accept-Charset", "utf-8");
        headers.insert("Accept-Language", "en-US");
        headers.insert("Host", "doc.rust-lang.org");
        headers.insert("Connection", "Close");

        let uri = Uri::try_from(URI).unwrap();
        let mut req = RequestMessage::new(&uri);
        let req = req.headers(headers.clone());

        assert_eq!(req.headers, headers);
    }

    #[test]
    fn request_m_header() {
        let uri = Uri::try_from(URI).unwrap();
        let mut req = RequestMessage::new(&uri);
        let k = "Connection";
        let v = "Close";

        let mut expect_headers = Headers::new();
        expect_headers.insert("Host", "doc.rust-lang.org");
        expect_headers.insert(k, v);

        let req = req.header(k, v);

        assert_eq!(req.headers, expect_headers);
    }

    #[test]
    fn request_m_body() {
        let uri = Uri::try_from(URI).unwrap();
        let mut req = RequestMessage::new(&uri);
        let req = req.body(&BODY);

        assert_eq!(req.body, Some(BODY.as_ref()));
    }

    #[test]
    fn request_m_parse() {
        let uri = Uri::try_from(URI).unwrap();
        let req = RequestMessage::new(&uri);

        const DEFAULT_MSG: &str = "GET /std/string/index.html HTTP/1.1\r\n\
                                   Host: doc.rust-lang.org\r\n\r\n";
        let msg = req.parse();
        let msg = String::from_utf8_lossy(&msg).into_owned();

        for line in DEFAULT_MSG.lines() {
            assert!(msg.contains(line));
        }

        for line in msg.lines() {
            assert!(DEFAULT_MSG.contains(line));
        }
    }

    #[test]
    fn request_new() {
        let uri = Uri::try_from(URI).unwrap();
        Request::new(&uri);
    }

    #[test]
    fn request_method() {
        let uri = Uri::try_from(URI).unwrap();
        let mut req = Request::new(&uri);
        req.method(Method::HEAD);

        assert_eq!(req.messsage.method, Method::HEAD);
    }

    #[test]
    fn request_headers() {
        let mut headers = Headers::new();
        headers.insert("Accept-Charset", "utf-8");
        headers.insert("Accept-Language", "en-US");
        headers.insert("Host", "doc.rust-lang.org");
        headers.insert("Connection", "Close");

        let uri = Uri::try_from(URI).unwrap();
        let mut req = Request::new(&uri);
        let req = req.headers(headers.clone());

        assert_eq!(req.messsage.headers, headers);
    }

    #[test]
    fn request_header() {
        let uri = Uri::try_from(URI).unwrap();
        let mut req = Request::new(&uri);
        let k = "Accept-Language";
        let v = "en-US";

        let mut expect_headers = Headers::new();
        expect_headers.insert("Host", "doc.rust-lang.org");
        expect_headers.insert("Connection", "Close");
        expect_headers.insert(k, v);

        let req = req.header(k, v);

        assert_eq!(req.messsage.headers, expect_headers);
    }

    #[test]
    fn request_body() {
        let uri = Uri::try_from(URI).unwrap();
        let mut req = Request::new(&uri);
        let req = req.body(&BODY);

        assert_eq!(req.messsage.body, Some(BODY.as_ref()));
    }

    #[test]
    fn request_connect_timeout() {
        let uri = Uri::try_from(URI).unwrap();
        let mut request = Request::new(&uri);
        request.connect_timeout(Some(Duration::from_nanos(1)));

        assert_eq!(request.connect_timeout, Some(Duration::from_nanos(1)));

        let err = request.send(&mut io::sink()).unwrap_err();
        match err {
            Error::IO(err) => assert_eq!(err.kind(), io::ErrorKind::TimedOut),
            other => panic!("Expected error to be io::Error, got: {:?}", other),
        };
    }

    #[test]
    fn request_read_timeout() {
        let uri = Uri::try_from(URI).unwrap();
        let mut request = Request::new(&uri);
        request.read_timeout(Some(Duration::from_nanos(100)));

        assert_eq!(request.read_timeout, Some(Duration::from_nanos(100)));
    }

    #[test]
    fn request_write_timeout() {
        let uri = Uri::try_from(URI).unwrap();
        let mut request = Request::new(&uri);
        request.write_timeout(Some(Duration::from_nanos(100)));

        assert_eq!(request.write_timeout, Some(Duration::from_nanos(100)));
    }

    #[test]
    fn request_timeout() {
        let uri = Uri::try_from(URI).unwrap();
        let mut request = Request::new(&uri);
        let timeout = Duration::from_secs(360);

        request.timeout(timeout);
        assert_eq!(request.timeout, timeout);
    }

    #[test]
    fn request_send() {
        let mut writer = Vec::new();
        let uri = Uri::try_from(URI).unwrap();
        let res = Request::new(&uri).send(&mut writer).unwrap();

        assert_ne!(res.status_code(), UNSUCCESS_CODE);
    }

    #[ignore]
    #[test]
    fn fn_get() {
        let mut writer = Vec::new();
        let res = get(URI, &mut writer).unwrap();

        assert_ne!(res.status_code(), UNSUCCESS_CODE);

        let mut writer = Vec::with_capacity(200);
        let res = get(URI_S, &mut writer).unwrap();

        assert_ne!(res.status_code(), UNSUCCESS_CODE);
    }

    #[ignore]
    #[test]
    fn fn_head() {
        let res = head(URI).unwrap();
        assert_ne!(res.status_code(), UNSUCCESS_CODE);

        let res = head(URI_S).unwrap();
        assert_ne!(res.status_code(), UNSUCCESS_CODE);
    }

    #[ignore]
    #[test]
    fn fn_post() {
        let mut writer = Vec::new();
        let res = post(URI, &BODY, &mut writer).unwrap();

        assert_ne!(res.status_code(), UNSUCCESS_CODE);

        let mut writer = Vec::with_capacity(200);
        let res = post(URI_S, &BODY, &mut writer).unwrap();

        assert_ne!(res.status_code(), UNSUCCESS_CODE);
    }
}
