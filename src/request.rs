//! creating and sending HTTP requests
use crate::{
    error,
    response::{Headers, Response},
    stream::Stream,
    uri::Uri,
};
use std::{
    fmt,
    io::{BufRead, BufReader, Read, Write},
    path::Path,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

const CR_LF: &str = "\r\n";
const BUF_SIZE: usize = 1024 * 1024;
const RECEIVING_TIMEOUT: u64 = 60;
const DEFAULT_REQ_TIMEOUT: u64 = 12 * 60 * 60;

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
    pub const fn as_str(self) -> &'static str {
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
///use http_req::{request::RequestBuilder, response::{find_slice, Response, StatusCode}, stream::Stream, uri::Uri};
///use std::{convert::TryFrom, io::{Read, Write}, time::Duration};
///
///let addr: Uri = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
///let mut writer = Vec::new();
///
///let mut request_builder = RequestBuilder::new(&addr);
///request_builder.header("Connection", "Close");
///let request_msg = request_builder.parse_msg();
///
///let mut stream = Stream::new(&addr, Some(Duration::from_secs(60))).unwrap();
///stream = Stream::try_to_https(stream, &addr, None).unwrap();
///stream.write_all(&request_msg).unwrap();
///stream.read_to_end(&mut writer).unwrap();
///
///let pos = find_slice(&writer, &[13, 10, 13, 10].to_owned()).unwrap();
///let response = Response::from_head(&writer[..pos]).unwrap();
///let body = writer[pos..].to_vec();
///
///assert_eq!(response.status_code(), StatusCode::new(200));
///```
#[derive(Clone, Debug, PartialEq)]
pub struct RequestBuilder<'a> {
    uri: &'a Uri<'a>,
    method: Method,
    version: HttpVersion,
    headers: Headers,
    body: Option<&'a [u8]>,
}

impl<'a> RequestBuilder<'a> {
    ///Creates new `RequestBuilder` with default parameters
    ///
    ///# Examples
    ///```
    ///use std::convert::TryFrom;
    ///use http_req::{request::RequestBuilder, uri::Uri};
    ///
    ///let addr = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
    ///
    ///let request_builder = RequestBuilder::new(&addr)
    ///    .header("Connection", "Close");
    ///```
    pub fn new(uri: &'a Uri<'a>) -> RequestBuilder<'a> {
        RequestBuilder {
            headers: Headers::default_http(uri),
            uri,
            method: Method::GET,
            version: HttpVersion::Http11,
            body: None,
        }
    }

    ///Sets request method
    ///
    ///# Examples
    ///```
    ///use std::convert::TryFrom;
    ///use http_req::{request::{RequestBuilder, Method}, uri::Uri};
    ///
    ///let addr = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
    ///
    ///let request_builder = RequestBuilder::new(&addr)
    ///    .method(Method::HEAD);
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
    ///use std::convert::TryFrom;
    ///use http_req::{request::{RequestBuilder, HttpVersion}, uri::Uri};
    ///
    ///let addr = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
    ///
    ///let request_builder = RequestBuilder::new(&addr)
    ///    .version(HttpVersion::Http10);
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
    ///use std::convert::TryFrom;
    ///use http_req::{request::RequestBuilder, response::Headers, uri::Uri};
    ///
    ///let addr = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
    ///
    ///let mut headers = Headers::new();
    ///headers.insert("Accept-Charset", "utf-8");
    ///headers.insert("Accept-Language", "en-US");
    ///headers.insert("Host", "rust-lang.org");
    ///headers.insert("Connection", "Close");
    ///
    ///let request_builder = RequestBuilder::new(&addr)
    ///    .headers(headers);
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
    ///use std::convert::TryFrom;
    ///use http_req::{request::RequestBuilder, response::Headers, uri::Uri};
    ///
    ///let addr = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
    ///
    ///let request_builder = RequestBuilder::new(&addr)
    ///    .header("Connection", "Close");
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
    ///use std::convert::TryFrom;
    ///use http_req::{request::{RequestBuilder, Method}, response::Headers, uri::Uri};
    ///
    ///let addr = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
    ///const BODY: &[u8; 27] = b"field1=value1&field2=value2";
    ///
    ///let request_builder = RequestBuilder::new(&addr)
    ///    .method(Method::POST)
    ///    .body(BODY)
    ///    .header("Content-Length", &BODY.len())
    ///    .header("Connection", "Close");
    ///```
    pub fn body(&mut self, body: &'a [u8]) -> &mut Self {
        self.body = Some(body);
        self
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
///use std::convert::TryFrom;
///
///let mut writer = Vec::new();
///let uri = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
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
    timeout: Duration,
    root_cert_file_pem: Option<&'a Path>,
}

impl<'a> Request<'a> {
    ///Creates new `Request` with default parameters
    ///
    ///# Examples
    ///```
    ///use http_req::{request::Request, uri::Uri};
    ///use std::convert::TryFrom;
    ///
    ///let mut writer = Vec::new();
    ///let uri: Uri = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
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
            timeout: Duration::from_secs(DEFAULT_REQ_TIMEOUT),
            root_cert_file_pem: None,
        }
    }

    ///Sets request method
    ///
    ///# Examples
    ///```
    ///use http_req::{request::{Request, Method}, uri::Uri};
    ///use std::convert::TryFrom;
    ///
    ///let mut writer = Vec::new();
    ///let uri: Uri = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
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
    ///use std::convert::TryFrom;
    ///
    ///let mut writer = Vec::new();
    ///let uri = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
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
    ///use std::convert::TryFrom;
    ///
    ///let mut writer = Vec::new();
    ///let uri: Uri = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
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
    ///use std::convert::TryFrom;
    ///
    ///let mut writer = Vec::new();
    ///let uri = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
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
    ///use std::convert::TryFrom;
    ///
    ///let mut writer = Vec::new();
    ///let uri = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
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
    ///use std::{time::Duration, convert::TryFrom};
    ///
    ///let mut writer = Vec::new();
    ///let uri = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
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
    ///use std::{time::Duration, convert::TryFrom};
    ///
    ///let mut writer = Vec::new();
    ///let uri: Uri = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
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
    ///use std::{time::Duration, convert::TryFrom};
    ///
    ///let mut writer = Vec::new();
    ///let uri = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
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

    ///Sets timeout on entire request
    ///
    ///# Examples
    ///```
    ///use http_req::{request::Request, uri::Uri};
    ///use std::{time::Duration, convert::TryFrom};
    ///
    ///let mut writer = Vec::new();
    ///let uri = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
    ///const time: Duration = Duration::from_secs(5);
    ///
    ///let response = Request::new(&uri)
    ///    .timeout(time)
    ///    .send(&mut writer)
    ///    .unwrap();
    ///```
    pub fn timeout<T>(&mut self, timeout: T) -> &mut Self
    where
        Duration: From<T>,
    {
        self.timeout = Duration::from(timeout);
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
    ///use std::convert::TryFrom;
    ///
    ///let mut writer = Vec::new();
    ///let uri: Uri = Uri::try_from("https://www.rust-lang.org/learn").unwrap();
    ///
    ///let response = Request::new(&uri).send(&mut writer).unwrap();
    ///```
    pub fn send<T: Write>(&self, writer: &mut T) -> Result<Response, error::Error> {
        //Set up stream
        let (sender, receiver) = mpsc::channel();
        let mut raw_response_head: Vec<u8> = Vec::new();
        let request_msg = self.inner.parse_msg();

        let mut stream = Stream::new(self.inner.uri, self.connect_timeout)?;
        stream.set_read_timeout(self.read_timeout)?;
        stream.set_write_timeout(self.write_timeout)?;
        stream = Stream::try_to_https(stream, self.inner.uri, self.root_cert_file_pem)?;
        stream.write_all(&request_msg)?;

        //Read the response
        let mut buf_reader = BufReader::new(stream);
        let deadline = Instant::now() + self.timeout;
        let reciving_timeout = Duration::from_secs(RECEIVING_TIMEOUT);

        thread::spawn(move || {
            loop {
                let mut buf = Vec::new();
                match buf_reader.read_until(0xA, &mut buf) {
                    Ok(0) => break,
                    Ok(len) => {
                        let filled_buf = buf[..len].to_owned();
                        sender.send(filled_buf).unwrap();

                        if len == 2 && buf == CR_LF.as_bytes() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }

            loop {
                let mut buf = vec![0; BUF_SIZE];
                match buf_reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(len) => {
                        let filled_buf = buf[..len].to_owned();
                        sender.send(filled_buf).unwrap();
                    }
                    Err(_) => break,
                }
            }
        });

        loop {
            let now = Instant::now();

            if deadline > now {
                let data_read = match receiver.recv_timeout(reciving_timeout) {
                    Ok(data) => data,
                    Err(_) => break,
                };

                if data_read == CR_LF.as_bytes() {
                    break;
                }

                raw_response_head.write_all(&data_read)?;
            } else {
                break;
            }
        }

        let response = Response::from_head(&raw_response_head)?;
        let content_len = response.content_len().unwrap_or(1);

        if content_len > 0 {
            loop {
                let now = Instant::now();

                if deadline > now {
                    let data_read = match receiver.recv_timeout(reciving_timeout) {
                        Ok(data) => data,
                        Err(_) => break,
                    };

                    writer.write_all(&data_read)?;
                } else {
                    break;
                }
            }
        }

        Ok(response)
    }
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
    let uri = Uri::try_from(uri.as_ref())?;

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
    let uri = Uri::try_from(uri.as_ref())?;

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
    fn request_b_new() {
        RequestBuilder::new(&Uri::try_from(URI).unwrap());
        RequestBuilder::new(&Uri::try_from(URI_S).unwrap());
    }

    #[test]
    fn request_b_method() {
        let uri = Uri::try_from(URI).unwrap();
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

        let uri = Uri::try_from(URI).unwrap();
        let mut req = RequestBuilder::new(&uri);
        let req = req.headers(headers.clone());

        assert_eq!(req.headers, headers);
    }

    #[test]
    fn request_b_header() {
        let uri = Uri::try_from(URI).unwrap();
        let mut req = RequestBuilder::new(&uri);
        let k = "Connection";
        let v = "Close";

        let mut expect_headers = Headers::new();
        expect_headers.insert("Host", "doc.rust-lang.org");
        expect_headers.insert(k, v);

        let req = req.header(k, v);

        assert_eq!(req.headers, expect_headers);
    }

    #[test]
    fn request_b_body() {
        let uri = Uri::try_from(URI).unwrap();
        let mut req = RequestBuilder::new(&uri);
        let req = req.body(&BODY);

        assert_eq!(req.body, Some(BODY.as_ref()));
    }

    #[test]
    fn request_b_parse_msg() {
        let uri = Uri::try_from(URI).unwrap();
        let req = RequestBuilder::new(&uri);

        const DEFAULT_MSG: &str = "GET /std/string/index.html HTTP/1.1\r\n\
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
        let uri = Uri::try_from(URI).unwrap();
        Request::new(&uri);
    }

    #[test]
    fn request_method() {
        let uri = Uri::try_from(URI).unwrap();
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

        let uri = Uri::try_from(URI).unwrap();
        let mut req = Request::new(&uri);
        let req = req.headers(headers.clone());

        assert_eq!(req.inner.headers, headers);
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

        assert_eq!(req.inner.headers, expect_headers);
    }

    #[test]
    fn request_body() {
        let uri = Uri::try_from(URI).unwrap();
        let mut req = Request::new(&uri);
        let req = req.body(&BODY);

        assert_eq!(req.inner.body, Some(BODY.as_ref()));
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
    fn request_send() {
        let mut writer = Vec::new();
        let uri = Uri::try_from(URI).unwrap();
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
