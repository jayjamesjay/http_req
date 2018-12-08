//! creating and sending HTTP requests
use crate::{
    error,
    response::{self, Response},
    url::Url,
};
use native_tls::{TlsConnector, TlsStream};
use std::{
    collections::HashMap,
    io::{self, Read, Write},
    net::TcpStream,
};

const CR_LF: &str = "\r\n";
const HTTP_V: &str = "HTTP/1.1";

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

impl Method {
    fn as_str(&self) -> &'static str {
        use self::Method::*;

        match self {
            GET => "GET",
            HEAD => "HEAD",
            POST => "POST",
            PUT => "PUT",
            DELETE => "DELETE",
            OPTIONS => "OPTIONS",
            PATCH => "PATCH",
        }
    }
}

#[derive(Clone, Debug)]
pub struct RequestBuilder<'a> {
    url: Url,
    method: Method,
    version: &'a str,
    headers: HashMap<String, String>,
    body: Option<&'a [u8]>,
}

impl<'a> RequestBuilder<'a> {
    ///Creates new RequestBuilder with default parameters
    pub fn new(url: Url) -> RequestBuilder<'a> {
        RequestBuilder {
            headers: Self::parse_default_headers(url.host()),
            url: url,
            method: Method::GET,
            version: HTTP_V,
            body: None,
        }
    }

    ///Sets request method
    pub fn method(&mut self, method: Method) -> &mut Self {
        self.method = method;
        self
    }

    ///Replaces all it's headers with headers passed to the function
    pub fn set_headers(&mut self, headers: HashMap<String, String>) -> &mut Self {
        self.headers = headers;
        self
    }

    ///Adds new header to existing/default headers
    pub fn add_header<V, U>(&mut self, key: V, val: U) -> &mut Self
    where
        V: ToString,
        U: ToString,
    {
        self.headers.insert(key.to_string(), val.to_string());
        self
    }

    ///Sets body for reequest
    pub fn body(&mut self, body: &'a [u8]) -> &mut Self {
        self.body = Some(body);
        self
    }

    ///Sends HTTP request. Opens TCP connection, writes request message to stream.
    ///Returns response if operation suceeded.
    pub fn send<T: Write + Read, U: Write>(
        &self,
        writer: &mut U,
    ) -> Result<Response, error::Error> {
        let res;
        let msg = self.parse_msg();
        let mut stream = self.connect()?;

        if self.url.scheme() == "https" {
            let mut stream = self.secure_conn(stream)?;
            res = self.handle_stream(&mut stream, &msg, writer)?;
        } else {
            res = self.handle_stream(&mut stream, &msg, writer)?;
        }

        Ok(res)
    }

    //Writes message to stream. Reads server response.
    fn handle_stream<T: Write + Read, U: Write>(
        &self,
        stream: &mut T,
        msg: &[u8],
        writer: &mut U,
    ) -> Result<Response, error::Error> {
        stream.write_all(msg)?;
        stream.flush()?;

        let (res_head, body_start) = Self::read_head(stream)?;
        let res: Response = Response::from_head(&res_head)?;

        if self.method != Method::HEAD {
            writer.write_all(&body_start)?;
            io::copy(stream, writer)?;
        }

        Ok(res)
    }

    fn read_head<T: Read>(stream: &mut T) -> Result<(Vec<u8>, Vec<u8>), io::Error> {
        let mut res_head = Vec::new();
        let pos;

        loop {
            let mut buf = [0; 10];
            let size = stream.read(&mut buf)?;
            res_head.append(&mut buf[..size].to_vec());

            if let Some(v) = response::find_slice(&res_head, &response::CR_LF_2) {
                pos = v;
                break;
            }
        }

        let (res_head, res_body) = res_head.split_at(pos);

        Ok((res_head.to_vec(), res_body.to_vec()))
    }

    ///Parses request message
    fn parse_msg(&self) -> Vec<u8> {
        let request_line = format!(
            "{} /{} {}{}",
            self.method.as_str(),
            self.url.resource(),
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
            let mut body = b.to_vec();
            request_msg.append(&mut body);
        }

        request_msg
    }

    //Creates default headers for a `Request`
    fn parse_default_headers<T: ToString>(host: T) -> HashMap<String, String> {
        let mut headers = HashMap::with_capacity(4);
        headers.insert("Host".to_string(), host.to_string());
        headers.insert("Connection".to_string(), "Close".to_string());

        headers
    }

    //Opens a TCP connection
    fn connect(&self) -> Result<TcpStream, io::Error> {
        let addr = format!("{}:{}", self.url.host(), self.url.port());
        TcpStream::connect(addr)
    }

    //Opens secure connnection over TlsStream
    fn secure_conn<S: Read + Write>(&self, stream: S) -> Result<TlsStream<S>, error::Error> {
        let connector = TlsConnector::new()?;
        Ok(connector.connect(self.url.host(), stream)?)
    }
}

///Creates and sends GET request. Returns response for that request.
pub fn get<T: AsRef<str>, U: Write>(url: T, writer: &mut U) -> Result<Response, error::Error> {
    let url = url.as_ref().parse::<Url>()?;
    RequestBuilder::new(url).send::<TcpStream, U>(writer)
}

///Creates and sends HEAD request. Returns response for that request.
pub fn head<T: AsRef<str>>(url: T) -> Result<Response, error::Error> {
    let url = url.as_ref().parse::<Url>()?;
    let mut writer = Vec::with_capacity(200);

    RequestBuilder::new(url)
        .method(Method::HEAD)
        .send::<TcpStream, _>(&mut writer)
}

#[cfg(test)]
mod tests {
    use super::*;

    const UNSUCCESS_CODE: u16 = 400;
    const URL: &str = "http://doc.rust-lang.org/std/string/index.html";
    const URL_S: &str = "https://doc.rust-lang.org/std/string/index.html";
    const BODY: [u8; 14] = [78, 97, 109, 101, 61, 74, 97, 109, 101, 115, 43, 74, 97, 121];

    #[test]
    fn request_b_new() {
        RequestBuilder::new(URL.parse().unwrap());
        RequestBuilder::new(URL_S.parse().unwrap());
    }

    #[test]
    fn request_b_method() {
        let mut req = RequestBuilder::new(URL.parse().unwrap());
        let req = req.method(Method::HEAD);
        assert_eq!(req.method, Method::HEAD);
    }

    #[test]
    fn request_b_set_headers() {
        let mut req = RequestBuilder::new(URL.parse().unwrap());
        let mut headers = HashMap::with_capacity(4);
        headers.insert("Accept-Charset".to_string(), "utf-8".to_string());
        headers.insert("Accept-Language".to_string(), "en-US".to_string());
        headers.insert("Host".to_string(), "doc.rust-lang.org".to_string());
        headers.insert("Connection".to_string(), "Close".to_string());

        let req = req.set_headers(headers.clone());

        assert_eq!(req.headers, headers);
    }

    #[test]
    fn request_b_add_header() {
        let mut req = RequestBuilder::new(URL.parse().unwrap());
        let k = "Accept-Charset";
        let v = "utf-8";

        let mut expect_headers = HashMap::with_capacity(3);
        expect_headers.insert("Host".to_string(), "doc.rust-lang.org".to_string());
        expect_headers.insert("Connection".to_string(), "Close".to_string());
        expect_headers.insert(k.to_string(), v.to_string());

        let req = req.add_header(k, v);

        assert_eq!(req.headers, expect_headers);
    }

    #[test]
    fn request_b_body() {
        let mut req = RequestBuilder::new(URL.parse().unwrap());
        let req = req.body(&BODY);

        assert_eq!(req.body, Some(BODY.as_ref()));
    }

    #[ignore]
    #[test]
    fn request_b_send() {
        let mut writer = Vec::new();

        RequestBuilder::new(URL.parse().unwrap())
            .send::<TcpStream, Vec<u8>>(&mut writer)
            .unwrap();
        RequestBuilder::new(URL_S.parse().unwrap())
            .send::<TcpStream, Vec<u8>>(&mut writer)
            .unwrap();
    }

    #[ignore]
    #[test]
    fn request_b_handle_stream() {
        let req = RequestBuilder::new(URL.parse().unwrap());
        let mut stream = req.connect().unwrap();
        let msg = req.parse_msg();
        let mut writer = Vec::new();

        let res = req.handle_stream(&mut stream, &msg, &mut writer).unwrap();

        assert_ne!(u16::from(res.status_code()), UNSUCCESS_CODE);
    }

    #[test]
    fn request_b_msg() {
        let req = RequestBuilder::new(URL.parse().unwrap());
        req.parse_msg();
    }

    #[test]
    fn request_b_default_headers() {
        let url = URL.parse::<Url>().unwrap();
        let mut headers = HashMap::new();
        headers.insert("Host".to_string(), "doc.rust-lang.org".to_string());
        headers.insert("Connection".to_string(), "Close".to_string());

        assert_eq!(RequestBuilder::parse_default_headers(url.host()), headers)
    }

    #[ignore]
    #[test]
    fn request_b_connect() {
        let req = RequestBuilder::new(URL.parse().unwrap());
        req.connect().unwrap();
    }

    #[ignore]
    #[test]
    fn request_b_secure_conn() {
        let req = RequestBuilder::new(URL_S.parse().unwrap());
        let stream = req.connect().unwrap();

        req.secure_conn(stream).unwrap();
    }

    #[ignore]
    #[test]
    fn request_get() {
        let mut writer = Vec::new();
        let res = get(URL, &mut writer).unwrap();

        assert_ne!(u16::from(res.status_code()), UNSUCCESS_CODE);

        let mut writer = Vec::with_capacity(200);
        let res = get(URL_S, &mut writer).unwrap();

        assert_ne!(u16::from(res.status_code()), UNSUCCESS_CODE);
    }

    #[ignore]
    #[test]
    fn request_head() {
        let res = head(URL).unwrap();
        assert_ne!(u16::from(res.status_code()), UNSUCCESS_CODE);

        let res = head(URL_S).unwrap();
        assert_ne!(u16::from(res.status_code()), UNSUCCESS_CODE);
    }
}
