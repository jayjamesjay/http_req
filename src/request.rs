//! creating and sending HTTP requests
use crate::{
    error,
    response::{Headers, Response, CR_LF_2},
    tls,
    uri::Uri,
};
use std::{
    fmt,
    io::{self, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    time::Duration,
};

const CR_LF: &str = "\r\n";
const HTTP_V: &str = "HTTP/1.1";

///Copies data from `reader` to `writer` until the specified `val`ue is reached.
///Returns how many bytes has been read.
pub fn copy_until<R, W>(reader: &mut R, writer: &mut W, val: &[u8]) -> Result<usize, io::Error>
where
    R: Read + ?Sized,
    W: Write + ?Sized,
{
    let mut buf = Vec::with_capacity(200);

    let mut pre_buf = [0; 10];
    let mut read = reader.read(&mut pre_buf)?;
    buf.extend(&pre_buf[..read]);

    for byte in reader.bytes() {
        buf.push(byte?);
        read += 1;

        if &buf[(buf.len() - val.len())..] == val {
            break;
        }
    }

    writer.write_all(&buf)?;
    writer.flush()?;

    Ok(read)
}

///HTTP request methods
#[derive(Debug, PartialEq, Clone)]
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

///Relatively low-level struct for making HTTP requests.
///
///It can work with any stream that implements `Read` and `Write`.
///By default it does not close the connection after completion of the response.
#[derive(Clone, Debug, PartialEq)]
pub struct RequestBuilder<'a> {
    uri: &'a Uri,
    method: Method,
    version: &'a str,
    headers: Headers,
    body: Option<&'a [u8]>,
}

impl<'a> RequestBuilder<'a> {
    ///Creates new `RequestBuilder` with default parameters
    pub fn new(uri: &'a Uri) -> RequestBuilder<'a> {
        RequestBuilder {
            headers: Headers::default_http(&uri),
            uri,
            method: Method::GET,
            version: HTTP_V,
            body: None,
        }
    }

    ///Sets request method
    pub fn method<T>(&mut self, method: T) -> &mut Self
    where
        Method: From<T>,
    {
        self.method = Method::from(method);
        self
    }

    ///Replaces all it's headers with headers passed to the function
    pub fn headers<T>(&mut self, headers: T) -> &mut Self
    where
        Headers: From<T>,
    {
        self.headers = Headers::from(headers);
        self
    }

    ///Adds new header to existing/default headers
    pub fn header<T, U>(&mut self, key: &T, val: &U) -> &mut Self
    where
        T: ToString + ?Sized,
        U: ToString + ?Sized,
    {
        self.headers.insert(key, val);
        self
    }

    ///Sets body for request
    pub fn body(&mut self, body: &'a [u8]) -> &mut Self {
        self.body = Some(body);
        self
    }

    ///Sends HTTP request in these steps:
    ///
    ///- Writes request message to `stream`.
    ///- Writes response's body to `writer`.
    ///- Returns response for this request.
    pub fn send<T, U>(&self, stream: &mut T, writer: &mut U) -> Result<Response, error::Error>
    where
        T: Write + Read,
        U: Write,
    {
        self.write_msg(stream, &self.parse_msg())?;
        let res = self.read_head(stream)?;

        if self.method != Method::HEAD {
            io::copy(stream, writer)?;
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
    pub fn read_head<T: Read>(&self, stream: &mut T) -> Result<Response, error::Error> {
        let mut head = Vec::with_capacity(200);
        copy_until(stream, &mut head, &CR_LF_2)?;

        Response::from_head(&head)
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
///# About timeouts:
///
///- Default timeout for starting connection is 1 minute.
///- On Linux, `man 7 socket` says that read/write timeouts default to zero, which means
///  the operations will _never_ time out. However, default value for this builder is 1 minute each.
///
///# Examples
///```
///use http_req::{request::Request, uri::Uri, response::StatusCode};
///
///let mut writer = Vec::new();
///let uri: Uri = "https://doc.rust-lang.org/".parse().unwrap();
///
///let mut  req = Request::new(&uri);
///let res = req.send(&mut writer).unwrap();
///
///assert_eq!(res.status_code(), StatusCode::new(200));
///```
///
#[derive(Clone, Debug, PartialEq)]
pub struct Request<'a> {
    inner: RequestBuilder<'a>,
    connect_timeout: Option<Duration>,
    read_timeout: Option<Duration>,
    write_timeout: Option<Duration>,
}

impl<'a> Request<'a> {
    ///Creates new `Request` with default parameters
    pub fn new(uri: &'a Uri) -> Request<'a> {
        let mut builder = RequestBuilder::new(&uri);
        builder.header("Connection", "Close");

        Request {
            inner: builder,
            connect_timeout: Some(Duration::from_secs(60)),
            read_timeout: Some(Duration::from_secs(60)),
            write_timeout: Some(Duration::from_secs(60)),
        }
    }

    ///Replaces all it's headers with headers passed to the function
    pub fn headers<T>(&mut self, headers: T) -> &mut Self
    where
        Headers: From<T>,
    {
        self.inner.headers(headers);

        self
    }

    ///Adds header to existing/default headers
    pub fn header<T, U>(&mut self, key: &T, val: &U) -> &mut Self
    where
        T: ToString + ?Sized,
        U: ToString + ?Sized,
    {
        self.inner.header(key, val);

        self
    }

    ///Sets body for request
    pub fn body(&mut self, body: &'a [u8]) -> &mut Self {
        self.inner.body(body);
        self
    }

    ///Changes request's method
    pub fn set_method<T>(&mut self, method: T)
    where
        Method: From<T>,
    {
        self.inner.method(method);
    }

    ///Sets connect timeout while using internal `TcpStream` instance
    ///
    ///- If there is a timeout, it will be passed to
    ///  [`TcpStream::connect_timeout`][TcpStream::connect_timeout].
    ///- If `None` is provided, [`TcpStream::connect`][TcpStream::connect] will
    ///  be used, and it will _not_ time out.
    ///
    ///[TcpStream::connect]: https://doc.rust-lang.org/std/net/struct.TcpStream.html#method.connect
    ///[TcpStream::connect_timeout]: https://doc.rust-lang.org/std/net/struct.TcpStream.html#method.connect_timeout
    pub fn set_connect_timeout(&mut self, timeout: Option<Duration>) -> &mut Self {
        self.connect_timeout = timeout;
        self
    }

    ///Sets read timeout on internal `TcpStream` instance
    ///
    ///`timeout` will be passed to
    ///[`TcpStream::set_read_timeout`][TcpStream::set_read_timeout].
    ///
    ///[TcpStream::set_read_timeout]: https://doc.rust-lang.org/std/net/struct.TcpStream.html#method.set_read_timeout
    pub fn set_read_timeout(&mut self, timeout: Option<Duration>) -> &mut Self {
        self.read_timeout = timeout;
        self
    }

    ///Sets write timeout on internal `TcpStream` instance
    ///
    ///`timeout` will be passed to
    ///[`TcpStream::set_write_timeout`][TcpStream::set_write_timeout].
    ///
    ///[TcpStream::set_write_timeout]: https://doc.rust-lang.org/std/net/struct.TcpStream.html#method.set_write_timeout
    pub fn set_write_timeout(&mut self, timeout: Option<Duration>) -> &mut Self {
        self.write_timeout = timeout;
        self
    }

    ///Sends HTTP request.
    ///
    ///Creates `TcpStream` (and wraps it with `TlsStream` if needed). Writes request message
    ///to created stream. Returns response for this request. Writes response's body to `writer`.
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
            let mut stream =
                tls::Config::default().connect(self.inner.uri.host().unwrap_or(""), stream)?;
            self.inner.send(&mut stream, writer)
        } else {
            self.inner.send(&mut stream, writer)
        }
    }
}

///Connects to target host with a timeout
fn connect_timeout(host: &str, port: u16, timeout: Duration) -> io::Result<TcpStream> {
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
pub fn get<T: AsRef<str>, U: Write>(uri: T, writer: &mut U) -> Result<Response, error::Error> {
    let uri = uri.as_ref().parse::<Uri>()?;
    let request = Request::new(&uri);

    request.send(writer)
}

///Creates and sends HEAD request. Returns response for this request.
pub fn head<T: AsRef<str>>(uri: T) -> Result<Response, error::Error> {
    let mut writer = Vec::new();
    let uri = uri.as_ref().parse::<Uri>()?;

    let mut request = Request::new(&uri);
    request.set_method(Method::HEAD);

    request.send(&mut writer)
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
    fn copy_data_until() {
        let mut reader = Vec::new();
        reader.extend(&RESPONSE[..]);

        let mut reader = Cursor::new(reader);
        let mut writer = Vec::new();

        copy_until(&mut reader, &mut writer, &CR_LF_2).unwrap();
        assert_eq!(writer, &RESPONSE_H[..]);
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
        req.set_method(Method::HEAD);

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
        let uri: Uri = URI.parse().unwrap();
        let mut req = Request::new(&uri);
        let req = req.body(&BODY);

        assert_eq!(req.inner.body, Some(BODY.as_ref()));
    }

    #[test]
    fn request_send() {
        let mut writer = Vec::new();

        let uri = URI.parse().unwrap();
        let res = Request::new(&uri).send(&mut writer).unwrap();

        assert_ne!(res.status_code(), UNSUCCESS_CODE);
    }

    #[test]
    fn request_connect_timeout() {
        let uri = URI.parse().unwrap();

        let mut request = Request::new(&uri);
        request.set_connect_timeout(Some(Duration::from_nanos(1)));
        assert_eq!(request.connect_timeout, Some(Duration::from_nanos(1)));

        let err = request.send(&mut io::sink()).unwrap_err();
        match err {
            Error::IO(err) => assert_eq!(err.kind(), io::ErrorKind::TimedOut),
            other => panic!("Expected error to be io::Error, got: {:?}", other),
        };
    }

    #[test]
    fn request_read_timeout() {
        let uri = URI.parse().unwrap();

        let mut request = Request::new(&uri);
        request.set_read_timeout(Some(Duration::from_nanos(1)));
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
        request.set_write_timeout(Some(Duration::from_nanos(100)));
        assert_eq!(request.write_timeout, Some(Duration::from_nanos(100)));
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
}
