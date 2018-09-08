//! creating and sending HTTP requests
use super::*;
use native_tls::{TlsConnector, TlsStream};
use response::Response;
use std::{
    io::{self, Read, Write},
    net::TcpStream,
};
use url::Url;

const CR_LF: &str = "\r\n";
const HTTP_V: &str = "HTTP/1.1";

#[derive(Debug, PartialEq)]
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
            headers: Self::default_headers(url.host()),
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
    pub fn add_header<T, U>(&mut self, key: T, val: U) -> &mut Self
    where
        T: ToString,
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
    pub fn send(&self) -> Result<Response, Box<Error>> {
        let res;
        let msg = self.parse_msg();
        let mut stream = self.connect()?;

        if self.url.scheme() == "https" {
            let mut stream = self.secure_conn(stream)?;
            res = self.handle_stream(&mut stream, &msg)?;
        } else {
            res = self.handle_stream(&mut stream, &msg)?;
        }

        Ok(res)
    }

    //Writes message to stream. Reads server response.
    fn handle_stream<T>(&self, stream: &mut T, msg: &[u8]) -> Result<Response, Box<Error>>
    where
        T: Write + Read,
    {
        stream.write_all(msg)?;
        stream.flush()?;

        let (res_head, body_start) = Self::res_head(stream)?;
        let mut res = Response::new(&res_head)?;

        if self.method != Method::HEAD {
            let mut res_body = Vec::with_capacity(res.content_len()?);
            stream.read_to_end(&mut res_body)?;
            res.append_body(body_start);
            res.append_body(res_body);
        }

        Ok(res)
    }

    fn res_head<T>(stream: &mut T) -> Result<(Vec<u8>, Vec<u8>), Box<Error>>
    where
        T: Write + Read,
    {
        let mut res_head = Vec::new();
        let pos;

        loop {
            let mut buf = [0; 200];
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
    fn default_headers<T: ToString>(host: T) -> HashMap<String, String> {
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
    fn secure_conn(&self, stream: TcpStream) -> Result<TlsStream<TcpStream>, Box<Error>> {
        let connector = TlsConnector::new()?;
        Ok(connector.connect(self.url.host(), stream)?)
    }
}

///Creates and sends GET request. Returns response for that request.
pub fn get(url: &str) -> Result<Response, Box<Error>> {
    let url = url.parse::<Url>()?;
    RequestBuilder::new(url).send()
}

///Creates and sends HEAD request. Returns response for that request.
pub fn head(url: &str) -> Result<Response, Box<Error>> {
    let url = url.parse::<Url>()?;
    RequestBuilder::new(url).method(Method::HEAD).send()
}

#[cfg(test)]
mod tests {
    use super::*;

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
        RequestBuilder::new(URL.parse().unwrap()).send().unwrap();
        RequestBuilder::new(URL_S.parse().unwrap()).send().unwrap();
    }

    #[ignore]
    #[test]
    fn request_b_handle_stream() {
        let req = RequestBuilder::new(URL.parse().unwrap());
        let mut stream = req.connect().unwrap();
        let msg = req.parse_msg();

        let res = req.handle_stream(&mut stream, &msg).unwrap();

        assert_ne!(res.status_code(), 400);
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

        assert_eq!(RequestBuilder::default_headers(url.host()), headers)
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
        let req = RequestBuilder::new(URL.parse().unwrap());
        let stream = req.connect().unwrap();

        req.secure_conn(stream).unwrap();
    }

    #[ignore]
    #[test]
    fn request_get() {
        let res = get(URL).unwrap();
        assert_ne!(res.status_code(), 400);

        let res = get(URL_S).unwrap();
        assert_ne!(res.status_code(), 400);
    }

    #[ignore]
    #[test]
    fn request_head() {
        let res = head(URL).unwrap();
        assert_ne!(res.status_code(), 400);

        let res = head(URL_S).unwrap();
        assert_ne!(res.status_code(), 400);
    }
}
