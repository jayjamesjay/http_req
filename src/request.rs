//! creating and sending HTTP requests
use crate::{
    error,
    response::{find_slice, Headers, Response, CR_LF_2},
    tls,
    uri::Uri,
};
use std::{
    fmt,
    io::{self, ErrorKind, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    path::Path,
    time::{Duration, Instant},
};

const CR_LF: &str = "\r\n";
const BUF_SIZE: usize = 8 * 1024;
const SMALL_BUF_SIZE: usize = 8 * 10;
const TEST_FREQ: usize = 100;

///Every iteration increases `count` by one. When `count` is equal to `stop`, `next()`
///returns `Some(true)` (and sets `count` to 0), otherwise returns `Some(false)`.
///Iterator never returns `None`.
pub struct Counter {
    count: usize,
    stop: usize,
}

impl Counter {
    pub fn new(stop: usize) -> Counter {
        Counter { count: 0, stop }
    }
}

impl Iterator for Counter {
    type Item = bool;

    fn next(&mut self) -> Option<Self::Item> {
        self.count += 1;
        let breakpoint = self.count == self.stop;

        if breakpoint {
            self.count = 0;
        }

        Some(breakpoint)
    }
}

///Copies data from `reader` to `writer` until the `deadline` is reached.
///Returns how many bytes has been read.
pub fn copy_with_timeout<R, W>(reader: &mut R, writer: &mut W, deadline: Instant) -> io::Result<u64>
where
    R: Read + ?Sized,
    W: Write + ?Sized,
{
    let mut buf = [0; BUF_SIZE];
    let mut copied = 0;
    let mut counter = Counter::new(TEST_FREQ);

    loop {
        let len = match reader.read(&mut buf) {
            Ok(0) => return Ok(copied),
            Ok(len) => len,
            Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        writer.write_all(&buf[..len])?;
        copied += len as u64;

        if counter.next().unwrap() && Instant::now() >= deadline {
            return Ok(copied);
        }
    }
}

///Copies a given amount of bytes from `reader` to `writer`.
pub fn copy_exact<R, W>(reader: &mut R, writer: &mut W, num_bytes: usize) -> io::Result<()>
where
    R: Read + ?Sized,
    W: Write + ?Sized,
{
    let mut buf = vec![0u8; num_bytes];

    reader.read_exact(&mut buf)?;
    writer.write_all(&mut buf)
}

///Reads data from `reader` and checks for specified `val`ue. When data contains specified value
///or `deadline` is reached, stops reading. Returns read data as array of two vectors: elements
///before and after the `val`.
pub fn copy_until<R>(
    reader: &mut R,
    val: &[u8],
    deadline: Instant,
) -> Result<[Vec<u8>; 2], io::Error>
where
    R: Read + ?Sized,
{
    let mut buf = [0; SMALL_BUF_SIZE];
    let mut writer = Vec::new();
    let mut counter = Counter::new(TEST_FREQ);
    let mut split_idx = 0;

    loop {
        let len = match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(len) => len,
            Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };

        writer.write_all(&buf[..len])?;

        if let Some(i) = find_slice(&writer, val) {
            split_idx = i;
            break;
        }

        if counter.next().unwrap() && Instant::now() >= deadline {
            split_idx = writer.len();
            break;
        }
    }

    Ok([writer[..split_idx].to_vec(), writer[split_idx..].to_vec()])
}

///HTTP request methods
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Method {
    GET,
    HEAD,
    POST,
    PUT,
    DELETE,
    OPTIONS,
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
            OPTIONS => "OPTIONS",
            PATCH => "PATCH",
        };

        write!(f, "{}", method)
    }
}

///HTTP versions
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum HttpVersion {
    Http10,
    Http11,
    Http20,
}

impl HttpVersion {
    pub fn as_str(self) -> &'static str {
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

///Relatively low-level struct for making HTTP requests.
///
///It can work with any stream that implements `Read` and `Write`.
///By default it does not close the connection after completion of the response.
///
///# Examples
///```
///use std::net::TcpStream;
///use http_req::{request::RequestBuilder, tls, uri::Uri, response::StatusCode};
///
///let addr: Uri = "https://www.rust-lang.org/learn".parse().unwrap();
///let mut writer = Vec::new();
///
///let stream = TcpStream::connect((addr.host().unwrap(), addr.corr_port())).unwrap();
///let mut stream = tls::Config::default()
///    .connect(addr.host().unwrap_or(""), stream)
///    .unwrap();
///
///let response = RequestBuilder::new(&addr)
///    .header("Connection", "Close")
///    .send(&mut stream, &mut writer)
///    .unwrap();
///
///assert_eq!(response.status_code(), StatusCode::new(200));
///```
#[derive(Clone, Debug, PartialEq)]
pub struct RequestBuilder<'a> {
    uri: &'a Uri,
    method: Method,
    version: HttpVersion,
    headers: Headers,
    body: Option<&'a [u8]>,
    timeout: Option<Duration>,
}

impl<'a> RequestBuilder<'a> {
    ///Creates new `RequestBuilder` with default parameters
    ///
    ///# Examples
    ///```
    ///use std::net::TcpStream;
    ///use http_req::{request::RequestBuilder, tls, uri::Uri};
    ///
    ///let addr: Uri = "https://www.rust-lang.org/learn".parse().unwrap();
    ///let mut writer = Vec::new();
    ///
    ///let stream = TcpStream::connect((addr.host().unwrap(), addr.corr_port())).unwrap();
    ///let mut stream = tls::Config::default()
    ///    .connect(addr.host().unwrap_or(""), stream)
    ///    .unwrap();
    ///
    ///let response = RequestBuilder::new(&addr)
    ///    .header("Connection", "Close")
    ///    .send(&mut stream, &mut writer)
    ///    .unwrap();
    ///```
    pub fn new(uri: &'a Uri) -> RequestBuilder<'a> {
        RequestBuilder {
            headers: Headers::default_http(uri),
            uri,
            method: Method::GET,
            version: HttpVersion::Http11,
            body: None,
            timeout: None,
        }
    }

    ///Sets request method
    ///
    ///# Examples
    ///```
    ///use std::net::TcpStream;
    ///use http_req::{request::{RequestBuilder, Method}, tls, uri::Uri};
    ///
    ///let addr: Uri = "https://www.rust-lang.org/learn".parse().unwrap();
    ///let mut writer = Vec::new();
    ///
    ///let stream = TcpStream::connect((addr.host().unwrap(), addr.corr_port())).unwrap();
    ///let mut stream = tls::Config::default()
    ///    .connect(addr.host().unwrap_or(""), stream)
    ///    .unwrap();
    ///
    ///let response = RequestBuilder::new(&addr)
    ///    .method(Method::HEAD)
    ///    .header("Connection", "Close")
    ///    .send(&mut stream, &mut writer)
    ///    .unwrap();
    ///```
    pub fn method<T>(&mut self, method: T) -> &mut Self
    where
        Method: From<T>,
    {
        self.method = Method::from(method);
        self
    }

    ///Sets HTTP version
    ///
    ///# Examples
    ///```
    ///use std::net::TcpStream;
    ///use http_req::{request::{RequestBuilder, HttpVersion}, tls, uri::Uri};
    ///
    ///let addr: Uri = "https://www.rust-lang.org/learn".parse().unwrap();
    ///let mut writer = Vec::new();
    ///
    ///let stream = TcpStream::connect((addr.host().unwrap(), addr.corr_port())).unwrap();
    ///let mut stream = tls::Config::default()
    ///    .connect(addr.host().unwrap_or(""), stream)
    ///    .unwrap();
    ///
    ///let response = RequestBuilder::new(&addr)
    ///    .version(HttpVersion::Http10)
    ///    .header("Connection", "Close")
    ///    .send(&mut stream, &mut writer)
    ///    .unwrap();
    ///```

    pub fn version<T>(&mut self, version: T) -> &mut Self
    where
        HttpVersion: From<T>,
    {
        self.version = HttpVersion::from(version);
        self
    }

    ///Replaces all it's headers with headers passed to the function
    ///
    ///# Examples
    ///```
    ///use std::net::TcpStream;
    ///use http_req::{request::{RequestBuilder, Method}, response::Headers, tls, uri::Uri};
    ///
    ///let addr: Uri = "https://www.rust-lang.org/learn".parse().unwrap();
    ///let mut writer = Vec::new();
    ///let mut headers = Headers::new();
    ///headers.insert("Accept-Charset", "utf-8");
    ///headers.insert("Accept-Language", "en-US");
    ///headers.insert("Host", "rust-lang.org");
    ///headers.insert("Connection", "Close");
    ///
    ///let stream = TcpStream::connect((addr.host().unwrap(), addr.corr_port())).unwrap();
    ///let mut stream = tls::Config::default()
    ///    .connect(addr.host().unwrap_or(""), stream)
    ///    .unwrap();
    ///
    ///let response = RequestBuilder::new(&addr)
    ///    .headers(headers)
    ///    .send(&mut stream, &mut writer)
    ///    .unwrap();
    ///```
    pub fn headers<T>(&mut self, headers: T) -> &mut Self
    where
        Headers: From<T>,
    {
        self.headers = Headers::from(headers);
        self
    }

    ///Adds new header to existing/default headers
    ///
    ///# Examples
    ///```
    ///use std::net::TcpStream;
    ///use http_req::{request::{RequestBuilder, Method}, tls, uri::Uri};
    ///
    ///let addr: Uri = "https://www.rust-lang.org/learn".parse().unwrap();
    ///let mut writer = Vec::new();
    ///
    ///let stream = TcpStream::connect((addr.host().unwrap(), addr.corr_port())).unwrap();
    ///let mut stream = tls::Config::default()
    ///    .connect(addr.host().unwrap_or(""), stream)
    ///    .unwrap();
    ///
    ///let response = RequestBuilder::new(&addr)
    ///    .header("Connection", "Close")
    ///    .send(&mut stream, &mut writer)
    ///    .unwrap();
    ///```
    pub fn header<T, U>(&mut self, key: &T, val: &U) -> &mut Self
    where
        T: ToString + ?Sized,
        U: ToString + ?Sized,
    {
        self.headers.insert(key, val);
        self
    }

    ///Sets body for request
    ///
    ///# Examples
    ///```
    ///use std::net::TcpStream;
    ///use http_req::{request::{RequestBuilder, Method}, tls, uri::Uri};
    ///
    ///let addr: Uri = "https://www.rust-lang.org/learn".parse().unwrap();
    ///const body: &[u8; 27] = b"field1=value1&field2=value2";
    ///let mut writer = Vec::new();
    ///
    ///let stream = TcpStream::connect((addr.host().unwrap(), addr.corr_port())).unwrap();
    ///let mut stream = tls::Config::default()
    ///    .connect(addr.host().unwrap_or(""), stream)
    ///    .unwrap();
    ///
    ///let response = RequestBuilder::new(&addr)
    ///    .method(Method::POST)
    ///    .body(body)
    ///    .header("Content-Length", &body.len())
    ///    .header("Connection", "Close")
    ///    .send(&mut stream, &mut writer)
    ///    .unwrap();
    ///```
    pub fn body(&mut self, body: &'a [u8]) -> &mut Self {
        self.body = Some(body);
        self
    }

    ///Sets timeout for entire connection.
    ///
    ///# Examples
    ///```
    ///use std::{net::TcpStream, time::{Duration, Instant}};
    ///use http_req::{request::RequestBuilder, tls, uri::Uri};
    ///
    ///let addr: Uri = "https://www.rust-lang.org/learn".parse().unwrap();
    ///let mut writer = Vec::new();
    ///
    ///let stream = TcpStream::connect((addr.host().unwrap(), addr.corr_port())).unwrap();
    ///let mut stream = tls::Config::default()
    ///    .connect(addr.host().unwrap_or(""), stream)
    ///    .unwrap();
    ///let timeout = Some(Duration::from_secs(3600));
    ///
    ///let response = RequestBuilder::new(&addr)
    ///    .timeout(timeout)
    ///    .header("Connection", "Close")
    ///    .send(&mut stream, &mut writer)
    ///    .unwrap();
    ///```
    pub fn timeout<T>(&mut self, timeout: Option<T>) -> &mut Self
    where
        Duration: From<T>,
    {
        self.timeout = timeout.map(Duration::from);
        self
    }

    ///Sends HTTP request in these steps:
    ///
    ///- Writes request message to `stream`.
    ///- Writes response's body to `writer`.
    ///- Returns response for this request.
    ///
    ///# Examples
    ///
    ///HTTP
    ///```
    ///use std::net::TcpStream;
    ///use http_req::{request::RequestBuilder, uri::Uri};
    ///
    /// //This address is automatically redirected to HTTPS, so response code will not ever be 200
    ///let addr: Uri = "http://www.rust-lang.org/learn".parse().unwrap();
    ///let mut writer = Vec::new();
    ///let mut stream = TcpStream::connect((addr.host().unwrap(), addr.corr_port())).unwrap();
    ///
    ///let response = RequestBuilder::new(&addr)
    ///    .header("Connection", "Close")
    ///    .send(&mut stream, &mut writer)
    ///    .unwrap();
    ///```
    ///
    ///HTTPS
    ///```
    ///use std::net::TcpStream;
    ///use http_req::{request::RequestBuilder, tls, uri::Uri};
    ///
    ///let addr: Uri = "https://www.rust-lang.org/learn".parse().unwrap();
    ///let mut writer = Vec::new();
    ///
    ///let stream = TcpStream::connect((addr.host().unwrap(), addr.corr_port())).unwrap();
    ///let mut stream = tls::Config::default()
    ///    .connect(addr.host().unwrap_or(""), stream)
    ///    .unwrap();
    ///
    ///let response = RequestBuilder::new(&addr)
    ///    .header("Connection", "Close")
    ///    .send(&mut stream, &mut writer)
    ///    .unwrap();
    ///```
    pub fn send<T, U>(&self, stream: &mut T, writer: &mut U) -> Result<Response, error::Error>
    where
        T: Write + Read,
        U: Write,
    {
        self.write_msg(stream, &self.parse_msg())?;

        let head_deadline = match self.timeout {
            Some(t) => Instant::now() + t,
            None => Instant::now() + Duration::from_secs(360),
        };
        let (res, body_part) = self.read_head(stream, head_deadline)?;

        if self.method == Method::HEAD {
            return Ok(res);
        }

        if let Some(v) = res.headers().get("Transfer-Encoding") {
            if *v == "chunked" {
                let mut dechunked = crate::chunked::Reader::new(body_part.as_slice().chain(stream));

                if let Some(timeout) = self.timeout {
                    let deadline = Instant::now() + timeout;
                    copy_with_timeout(&mut dechunked, writer, deadline)?;
                } else {
                    io::copy(&mut dechunked, writer)?;
                }

                return Ok(res);
            }
        }

        writer.write_all(&body_part)?;

        if let Some(timeout) = self.timeout {
            let deadline = Instant::now() + timeout;
            copy_with_timeout(stream, writer, deadline)?;
        } else {
            let num_bytes = res.content_len().unwrap_or(0);

            if num_bytes > 0 {
                copy_exact(stream, writer, num_bytes - body_part.len())?;
            } else {
                io::copy(stream, writer)?;
            }
        }

        Ok(res)
    }

    ///Writes message to `stream` and flushes it
    pub fn write_msg<T, U>(&self, stream: &mut T, msg: &U) -> Result<(), io::Error>
    where
        T: Write,
        U: AsRef<[u8]>,
    {
        stream.write_all(msg.as_ref())?;
        stream.flush()?;

        Ok(())
    }

    ///Reads head of server's response
    pub fn read_head<T: Read>(
        &self,
        stream: &mut T,
        deadline: Instant,
    ) -> Result<(Response, Vec<u8>), error::Error> {
        let [head, body_part] = copy_until(stream, &CR_LF_2, deadline)?;

        Ok((Response::from_head(&head)?, body_part))
    }

    ///Parses request message for this `RequestBuilder`
    pub fn parse_msg(&self) -> Vec<u8> {
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

        if let Some(b) = &self.body {
            request_msg.extend(*b);
        }

        request_msg
    }
}

///Relatively higher-level struct for making HTTP requests.
///
///It creates stream (`TcpStream` or `TlsStream`) appropriate for the type of uri (`http`/`https`)
///By default it closes connection after completion of the response.
///
///# Examples
///```
///use http_req::{request::Request, uri::Uri, response::StatusCode};
///
///let mut writer = Vec::new();
///let uri: Uri = "https://www.rust-lang.org/learn".parse().unwrap();
///
///let response = Request::new(&uri).send(&mut writer).unwrap();;
///assert_eq!(response.status_code(), StatusCode::new(200));
///```
///
#[derive(Clone, Debug, PartialEq)]
pub struct Request<'a> {
    inner: RequestBuilder<'a>,
    connect_timeout: Option<Duration>,
    read_timeout: Option<Duration>,
    write_timeout: Option<Duration>,
    root_cert_file_pem: Option<&'a Path>,
}

impl<'a> Request<'a> {
    ///Creates new `Request` with default parameters
    ///
    ///# Examples
    ///```
    ///use http_req::{request::Request, uri::Uri};
    ///
    ///let mut writer = Vec::new();
    ///let uri: Uri = "https://www.rust-lang.org/learn".parse().unwrap();
    ///
    ///let response = Request::new(&uri).send(&mut writer).unwrap();;
    ///```
    pub fn new(uri: &'a Uri) -> Request<'a> {
        let mut builder = RequestBuilder::new(&uri);
        builder.header("Connection", "Close");

        Request {
            inner: builder,
            connect_timeout: Some(Duration::from_secs(60)),
            read_timeout: Some(Duration::from_secs(60)),
            write_timeout: Some(Duration::from_secs(60)),
            root_cert_file_pem: None,
        }
    }

    ///Sets request method
    ///
    ///# Examples
    ///```
    ///use http_req::{request::{Request, Method}, uri::Uri};
    ///
    ///let mut writer = Vec::new();
    ///let uri: Uri = "https://www.rust-lang.org/learn".parse().unwrap();
    ///
    ///let response = Request::new(&uri)
    ///    .method(Method::HEAD)
    ///    .send(&mut writer)
    ///    .unwrap();
    ///```
    pub fn method<T>(&mut self, method: T) -> &mut Self
    where
        Method: From<T>,
    {
        self.inner.method(method);
        self
    }

    ///Sets HTTP version
    ///
    ///# Examples
    ///```
    ///use http_req::{request::{Request, HttpVersion}, uri::Uri};
    ///
    ///let mut writer = Vec::new();
    ///let uri: Uri = "https://www.rust-lang.org/learn".parse().unwrap();
    ///
    ///let response = Request::new(&uri)
    ///    .version(HttpVersion::Http10)
    ///    .send(&mut writer)
    ///    .unwrap();
    ///```

    pub fn version<T>(&mut self, version: T) -> &mut Self
    where
        HttpVersion: From<T>,
    {
        self.inner.version(version);
        self
    }

    ///Replaces all it's headers with headers passed to the function
    ///
    ///# Examples
    ///```
    ///use http_req::{request::Request, uri::Uri, response::Headers};
    ///
    ///let mut writer = Vec::new();
    ///let uri: Uri = "https://www.rust-lang.org/learn".parse().unwrap();
    ///
    ///let mut headers = Headers::new();
    ///headers.insert("Accept-Charset", "utf-8");
    ///headers.insert("Accept-Language", "en-US");
    ///headers.insert("Host", "rust-lang.org");
    ///headers.insert("Connection", "Close");
    ///
    ///let response = Request::new(&uri)
    ///    .headers(headers)
    ///    .send(&mut writer)
    ///    .unwrap();;
    ///```
    pub fn headers<T>(&mut self, headers: T) -> &mut Self
    where
        Headers: From<T>,
    {
        self.inner.headers(headers);
        self
    }

    ///Adds header to existing/default headers
    ///
    ///# Examples
    ///```
    ///use http_req::{request::Request, uri::Uri};
    ///
    ///let mut writer = Vec::new();
    ///let uri: Uri = "https://www.rust-lang.org/learn".parse().unwrap();
    ///
    ///let response = Request::new(&uri)
    ///    .header("Accept-Language", "en-US")
    ///    .send(&mut writer)
    ///    .unwrap();
    ///```
    pub fn header<T, U>(&mut self, key: &T, val: &U) -> &mut Self
    where
        T: ToString + ?Sized,
        U: ToString + ?Sized,
    {
        self.inner.header(key, val);
        self
    }

    ///Sets body for request
    ///
    ///# Examples
    ///```
    ///use http_req::{request::{Request, Method}, uri::Uri};
    ///
    ///let mut writer = Vec::new();
    ///let uri: Uri = "https://www.rust-lang.org/learn".parse().unwrap();
    ///const body: &[u8; 27] = b"field1=value1&field2=value2";
    ///
    ///let response = Request::new(&uri)
    ///    .method(Method::POST)
    ///    .header("Content-Length", &body.len())
    ///    .body(body)
    ///    .send(&mut writer)
    ///    .unwrap();
    ///```
    pub fn body(&mut self, body: &'a [u8]) -> &mut Self {
        self.inner.body(body);
        self
    }

    ///Sets connection timeout of request.
    ///
    ///# Examples
    ///```
    ///use std::time::{Duration, Instant};
    ///use http_req::{request::Request, uri::Uri};
    ///
    ///let mut writer = Vec::new();
    ///let uri: Uri = "https://www.rust-lang.org/learn".parse().unwrap();
    ///const body: &[u8; 27] = b"field1=value1&field2=value2";
    ///let timeout = Some(Duration::from_secs(3600));
    ///
    ///let response = Request::new(&uri)
    ///    .timeout(timeout)
    ///    .send(&mut writer)
    ///    .unwrap();
    ///```
    pub fn timeout<T>(&mut self, timeout: Option<T>) -> &mut Self
    where
        Duration: From<T>,
    {
        self.inner.timeout = timeout.map(Duration::from);
        self
    }

    ///Sets connect timeout while using internal `TcpStream` instance
    ///
    ///- If there is a timeout, it will be passed to
    ///  [`TcpStream::connect_timeout`][TcpStream::connect_timeout].
    ///- If `None` is provided, [`TcpStream::connect`][TcpStream::connect] will
    ///  be used. A timeout will still be enforced by the operating system, but
    ///  the exact value depends on the platform.
    ///
    ///[TcpStream::connect]: https://doc.rust-lang.org/std/net/struct.TcpStream.html#method.connect
    ///[TcpStream::connect_timeout]: https://doc.rust-lang.org/std/net/struct.TcpStream.html#method.connect_timeout
    ///
    ///# Examples
    ///```
    ///use http_req::{request::Request, uri::Uri};
    ///use std::time::Duration;
    ///
    ///let mut writer = Vec::new();
    ///let uri: Uri = "https://www.rust-lang.org/learn".parse().unwrap();
    ///const time: Option<Duration> = Some(Duration::from_secs(10));
    ///
    ///let response = Request::new(&uri)
    ///    .connect_timeout(time)
    ///    .send(&mut writer)
    ///    .unwrap();
    ///```
    pub fn connect_timeout<T>(&mut self, timeout: Option<T>) -> &mut Self
    where
        Duration: From<T>,
    {
        self.connect_timeout = timeout.map(Duration::from);
        self
    }

    ///Sets read timeout on internal `TcpStream` instance
    ///
    ///`timeout` will be passed to
    ///[`TcpStream::set_read_timeout`][TcpStream::set_read_timeout].
    ///
    ///[TcpStream::set_read_timeout]: https://doc.rust-lang.org/std/net/struct.TcpStream.html#method.set_read_timeout
    ///
    ///# Examples
    ///```
    ///use http_req::{request::Request, uri::Uri};
    ///use std::time::Duration;
    ///
    ///let mut writer = Vec::new();
    ///let uri: Uri = "https://www.rust-lang.org/learn".parse().unwrap();
    ///const time: Option<Duration> = Some(Duration::from_secs(15));
    ///
    ///let response = Request::new(&uri)
    ///    .read_timeout(time)
    ///    .send(&mut writer)
    ///    .unwrap();
    ///```
    pub fn read_timeout<T>(&mut self, timeout: Option<T>) -> &mut Self
    where
        Duration: From<T>,
    {
        self.read_timeout = timeout.map(Duration::from);
        self
    }

    ///Sets write timeout on internal `TcpStream` instance
    ///
    ///`timeout` will be passed to
    ///[`TcpStream::set_write_timeout`][TcpStream::set_write_timeout].
    ///
    ///[TcpStream::set_write_timeout]: https://doc.rust-lang.org/std/net/struct.TcpStream.html#method.set_write_timeout
    ///
    ///# Examples
    ///```
    ///use http_req::{request::Request, uri::Uri};
    ///use std::time::Duration;
    ///
    ///let mut writer = Vec::new();
    ///let uri: Uri = "https://www.rust-lang.org/learn".parse().unwrap();
    ///const time: Option<Duration> = Some(Duration::from_secs(5));
    ///
    ///let response = Request::new(&uri)
    ///    .write_timeout(time)
    ///    .send(&mut writer)
    ///    .unwrap();
    ///```
    pub fn write_timeout<T>(&mut self, timeout: Option<T>) -> &mut Self
    where
        Duration: From<T>,
    {
        self.write_timeout = timeout.map(Duration::from);
        self
    }

    ///Add a file containing the PEM-encoded certificates that should be added in the trusted root store.
    pub fn root_cert_file_pem(&mut self, file_path: &'a Path) -> &mut Self {
        self.root_cert_file_pem = Some(file_path);
        self
    }

    ///Sends HTTP request.
    ///
    ///Creates `TcpStream` (and wraps it with `TlsStream` if needed). Writes request message
    ///to created stream. Returns response for this request. Writes response's body to `writer`.
    ///
    ///# Examples
    ///```
    ///use http_req::{request::Request, uri::Uri};
    ///
    ///let mut writer = Vec::new();
    ///let uri: Uri = "https://www.rust-lang.org/learn".parse().unwrap();
    ///
    ///let response = Request::new(&uri).send(&mut writer).unwrap();
    ///```
    pub fn send<T: Write>(&self, writer: &mut T) -> Result<Response, error::Error> {
        let host = self.inner.uri.host().unwrap_or("");
        let port = self.inner.uri.corr_port();
        let mut stream = match self.connect_timeout {
            Some(timeout) => connect_timeout(host, port, timeout)?,
            None => TcpStream::connect((host, port))?,
        };

        stream.set_read_timeout(self.read_timeout)?;
        stream.set_write_timeout(self.write_timeout)?;

        if self.inner.uri.scheme() == "https" {
            let mut cnf = tls::Config::default();
            let cnf = match self.root_cert_file_pem {
                Some(p) => cnf.add_root_cert_file_pem(p)?,
                None => &mut cnf,
            };
            let mut stream = cnf.connect(host, stream)?;
            self.inner.send(&mut stream, writer)
        } else {
            self.inner.send(&mut stream, writer)
        }
    }
}

///Connects to target host with a timeout
pub fn connect_timeout<T, U>(host: T, port: u16, timeout: U) -> io::Result<TcpStream>
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

///Creates and sends GET request. Returns response for this request.
///
///# Examples
///```
///use http_req::request;
///
///let mut writer = Vec::new();
///const uri: &str = "https://www.rust-lang.org/learn";
///
///let response = request::get(uri, &mut writer).unwrap();
///```
pub fn get<T: AsRef<str>, U: Write>(uri: T, writer: &mut U) -> Result<Response, error::Error> {
    let uri = uri.as_ref().parse::<Uri>()?;

    Request::new(&uri).send(writer)
}

///Creates and sends HEAD request. Returns response for this request.
///
///# Examples
///```
///use http_req::request;
///
///const uri: &str = "https://www.rust-lang.org/learn";
///let response = request::head(uri).unwrap();
///```
pub fn head<T: AsRef<str>>(uri: T) -> Result<Response, error::Error> {
    let mut writer = Vec::new();
    let uri = uri.as_ref().parse::<Uri>()?;

    Request::new(&uri).method(Method::HEAD).send(&mut writer)
}

///Creates and sends POST request. Returns response for this request.
///
///# Examples
///```
///use http_req::request;
///
///let mut writer = Vec::new();
///const uri: &str = "https://www.rust-lang.org/learn";
///const body: &[u8; 27] = b"field1=value1&field2=value2";
///
///let response = request::post(uri, body, &mut writer).unwrap();
///```
pub fn post<T: AsRef<str>, U: Write>(
    uri: T,    
    body: &[u8],
    writer: &mut U,
) -> Result<Response, error::Error> {
    let uri = uri.as_ref().parse::<Uri>()?;

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
    use std::io::Cursor;

    const UNSUCCESS_CODE: StatusCode = StatusCode::new(400);
    const URI: &str = "http://doc.rust-lang.org/std/string/index.html";
    const URI_S: &str = "https://doc.rust-lang.org/std/string/index.html";
    const BODY: [u8; 14] = [78, 97, 109, 101, 61, 74, 97, 109, 101, 115, 43, 74, 97, 121];

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
    fn counter_new() {
        let counter = Counter::new(200);

        assert_eq!(counter.count, 0);
        assert_eq!(counter.stop, 200);
    }

    #[test]
    fn counter_next() {
        let mut counter = Counter::new(5);

        assert_eq!(counter.next(), Some(false));
        assert_eq!(counter.next(), Some(false));
        assert_eq!(counter.next(), Some(false));
        assert_eq!(counter.next(), Some(false));
        assert_eq!(counter.next(), Some(true));
        assert_eq!(counter.next(), Some(false));
        assert_eq!(counter.next(), Some(false));
    }

    #[test]
    fn copy_data_until() {
        let mut reader = Vec::new();
        reader.extend(&RESPONSE[..]);

        let mut reader = Cursor::new(reader);

        let [head, _body] = copy_until(
            &mut reader,
            &CR_LF_2,
            Instant::now() + Duration::from_secs(360),
        )
        .unwrap();
        assert_eq!(&head[..], &RESPONSE_H[..]);
    }

    #[test]
    fn method_display() {
        const METHOD: Method = Method::HEAD;
        assert_eq!(&format!("{}", METHOD), "HEAD");
    }

    #[test]
    fn request_b_new() {
        RequestBuilder::new(&URI.parse().unwrap());
        RequestBuilder::new(&URI_S.parse().unwrap());
    }

    #[test]
    fn request_b_method() {
        let uri: Uri = URI.parse().unwrap();
        let mut req = RequestBuilder::new(&uri);
        let req = req.method(Method::HEAD);

        assert_eq!(req.method, Method::HEAD);
    }

    #[test]
    fn request_b_headers() {
        let mut headers = Headers::new();
        headers.insert("Accept-Charset", "utf-8");
        headers.insert("Accept-Language", "en-US");
        headers.insert("Host", "doc.rust-lang.org");
        headers.insert("Connection", "Close");

        let uri: Uri = URI.parse().unwrap();
        let mut req = RequestBuilder::new(&uri);
        let req = req.headers(headers.clone());

        assert_eq!(req.headers, headers);
    }

    #[test]
    fn request_b_header() {
        let uri: Uri = URI.parse().unwrap();
        let mut req = RequestBuilder::new(&uri);
        let k = "Connection";
        let v = "Close";

        let mut expect_headers = Headers::new();
        expect_headers.insert("Host", "doc.rust-lang.org");
        expect_headers.insert("Referer", "http://doc.rust-lang.org/std/string/index.html");
        expect_headers.insert(k, v);

        let req = req.header(k, v);

        assert_eq!(req.headers, expect_headers);
    }

    #[test]
    fn request_b_body() {
        let uri: Uri = URI.parse().unwrap();
        let mut req = RequestBuilder::new(&uri);
        let req = req.body(&BODY);

        assert_eq!(req.body, Some(BODY.as_ref()));
    }

    #[test]
    fn request_b_timeout() {
        let uri = URI.parse().unwrap();
        let mut req = RequestBuilder::new(&uri);
        let timeout = Some(Duration::from_secs(360));

        req.timeout(timeout);
        assert_eq!(req.timeout, timeout);
    }

    #[ignore]
    #[test]
    fn request_b_send() {
        let mut writer = Vec::new();
        let uri: Uri = URI.parse().unwrap();
        let mut stream = TcpStream::connect((uri.host().unwrap_or(""), uri.corr_port())).unwrap();

        RequestBuilder::new(&URI.parse().unwrap())
            .header("Connection", "Close")
            .send(&mut stream, &mut writer)
            .unwrap();
    }

    #[ignore]
    #[test]
    fn request_b_send_secure() {
        let mut writer = Vec::new();
        let uri: Uri = URI_S.parse().unwrap();

        let stream = TcpStream::connect((uri.host().unwrap_or(""), uri.corr_port())).unwrap();
        let mut secure_stream = tls::Config::default()
            .connect(uri.host().unwrap_or(""), stream)
            .unwrap();

        RequestBuilder::new(&URI_S.parse().unwrap())
            .header("Connection", "Close")
            .send(&mut secure_stream, &mut writer)
            .unwrap();
    }

    #[test]
    fn request_b_parse_msg() {
        let uri = URI.parse().unwrap();
        let req = RequestBuilder::new(&uri);

        const DEFAULT_MSG: &str = "GET /std/string/index.html HTTP/1.1\r\n\
                                   Referer: http://doc.rust-lang.org/std/string/index.html\r\n\
                                   Host: doc.rust-lang.org\r\n\r\n";
        let msg = req.parse_msg();
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
        let uri = URI.parse().unwrap();
        Request::new(&uri);
    }

    #[test]
    fn request_method() {
        let uri = URI.parse().unwrap();
        let mut req = Request::new(&uri);
        req.method(Method::HEAD);

        assert_eq!(req.inner.method, Method::HEAD);
    }

    #[test]
    fn request_headers() {
        let mut headers = Headers::new();
        headers.insert("Accept-Charset", "utf-8");
        headers.insert("Accept-Language", "en-US");
        headers.insert("Host", "doc.rust-lang.org");
        headers.insert("Connection", "Close");

        let uri: Uri = URI.parse().unwrap();
        let mut req = Request::new(&uri);
        let req = req.headers(headers.clone());

        assert_eq!(req.inner.headers, headers);
    }

    #[test]
    fn request_header() {
        let uri: Uri = URI.parse().unwrap();
        let mut req = Request::new(&uri);
        let k = "Accept-Language";
        let v = "en-US";

        let mut expect_headers = Headers::new();
        expect_headers.insert("Host", "doc.rust-lang.org");
        expect_headers.insert("Referer", "http://doc.rust-lang.org/std/string/index.html");
        expect_headers.insert("Connection", "Close");
        expect_headers.insert(k, v);

        let req = req.header(k, v);

        assert_eq!(req.inner.headers, expect_headers);
    }

    #[test]
    fn request_body() {
        let uri = URI.parse().unwrap();
        let mut req = Request::new(&uri);
        let req = req.body(&BODY);

        assert_eq!(req.inner.body, Some(BODY.as_ref()));
    }

    #[test]
    fn request_timeout() {
        let uri = URI.parse().unwrap();
        let mut request = Request::new(&uri);
        let timeout = Some(Duration::from_secs(360));

        request.timeout(timeout);
        assert_eq!(request.inner.timeout, timeout);
    }

    #[test]
    fn request_connect_timeout() {
        let uri = URI.parse().unwrap();
        let mut request = Request::new(&uri);
        request.connect_timeout(Some(Duration::from_nanos(1)));

        assert_eq!(request.connect_timeout, Some(Duration::from_nanos(1)));

        let err = request.send(&mut io::sink()).unwrap_err();
        match err {
            Error::IO(err) => assert_eq!(err.kind(), io::ErrorKind::TimedOut),
            other => panic!("Expected error to be io::Error, got: {:?}", other),
        };
    }

    #[ignore]
    #[test]
    fn request_read_timeout() {
        let uri = URI.parse().unwrap();
        let mut request = Request::new(&uri);
        request.read_timeout(Some(Duration::from_nanos(1)));

        assert_eq!(request.read_timeout, Some(Duration::from_nanos(1)));

        let err = request.send(&mut io::sink()).unwrap_err();
        match err {
            Error::IO(err) => match err.kind() {
                io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => {}
                other => panic!(
                    "Expected error kind to be one of WouldBlock/TimedOut, got: {:?}",
                    other
                ),
            },
            other => panic!("Expected error to be io::Error, got: {:?}", other),
        };
    }

    #[test]
    fn request_write_timeout() {
        let uri = URI.parse().unwrap();
        let mut request = Request::new(&uri);
        request.write_timeout(Some(Duration::from_nanos(100)));

        assert_eq!(request.write_timeout, Some(Duration::from_nanos(100)));
    }

    #[test]
    fn request_send() {
        let mut writer = Vec::new();
        let uri = URI.parse().unwrap();
        let res = Request::new(&uri).send(&mut writer).unwrap();

        assert_ne!(res.status_code(), UNSUCCESS_CODE);
    }

    #[ignore]
    #[test]
    fn request_get() {
        let mut writer = Vec::new();
        let res = get(URI, &mut writer).unwrap();

        assert_ne!(res.status_code(), UNSUCCESS_CODE);

        let mut writer = Vec::with_capacity(200);
        let res = get(URI_S, &mut writer).unwrap();

        assert_ne!(res.status_code(), UNSUCCESS_CODE);
    }

    #[ignore]
    #[test]
    fn request_head() {
        let res = head(URI).unwrap();
        assert_ne!(res.status_code(), UNSUCCESS_CODE);

        let res = head(URI_S).unwrap();
        assert_ne!(res.status_code(), UNSUCCESS_CODE);
    }

    #[ignore]
    #[test]
    fn request_post() {
        let mut writer = Vec::new();
        let res = post(URI, &BODY, &mut writer).unwrap();

        assert_ne!(res.status_code(), UNSUCCESS_CODE);

        let mut writer = Vec::with_capacity(200);
        let res = post(URI_S, &BODY, &mut writer).unwrap();

        assert_ne!(res.status_code(), UNSUCCESS_CODE);
    }
}
