//! parsing server response
use super::*;
use std::{fmt, str};

pub(crate) const CR_LF_2: [u8; 4] = [13, 10, 13, 10];

pub struct Response {
    status: Status,
    headers: HashMap<String, String>,
    body: Option<Vec<u8>>,
}

#[derive(Debug)]
struct ResponseError(&'static str);

impl fmt::Display for ResponseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Error: {}", self)
    }
}

impl Error for ResponseError {
    fn description(&self) -> &str {
        "Cannot parse Response"
    }
}

impl Response {
    ///Parses response.
    pub fn try_from(data: &[u8]) -> Result<Response, Box<Error>> {
        if data.len() == 0 {
            Err(ResponseError("Cannot parse Response from an empty slice."))?;
        }

        let head;
        let mut body = None;

        let pos = match find_slice(&data, &CR_LF_2) {
            Some(p) => p,
            None => 0,
        };

        match pos {
            0 => head = data,
            _ => {
                let (h, b) = data.split_at(pos);
                head = h;
                body = Some(b.to_vec());
            }
        }

        let (headers, status) = Self::parse_head(head)?;

        Ok(Response {
            status,
            headers,
            body,
        })
    }

    ///Creates new `Response` with head - status and headers - created from a slice of bytes
    ///and an empty body.
    pub fn new(head: &[u8]) -> Result<Response, Box<Error>> {
        let (headers, status) = Self::parse_head(head)?;

        Ok(Response {
            status,
            headers,
            body: None,
        })
    }

    ///Adds body to a response.
    pub fn append_body(&mut self, mut body: Vec<u8>) {
        let body_empty = self.body.is_none();

        if body_empty {
            self.body = Some(body);
        } else {
            if let Some(v) = &mut self.body {
                v.append(&mut body);
            }
        }
    }

    ///Parses head of a `Response` - status and headers - from slice of bytes.
    pub fn parse_head(head: &[u8]) -> Result<(HashMap<String, String>, Status), Box<Error>> {
        let mut head: Vec<_> = str::from_utf8(head)?.lines().collect();
        head.pop();

        let status = Self::parse_status_line(&head.remove(0))?;
        let headers = Self::parse_headers(&head);

        Ok((headers, status))
    }

    fn parse_status_line(status_line: &str) -> Result<Status, ParseIntError> {
        let status_line: Vec<_> = status_line.split_whitespace().collect();

        let version = status_line[0];
        let code: u16 = status_line[1].parse()?;
        let reason = status_line[2];

        Ok(Status::from((version, code, reason)))
    }

    fn parse_headers(headers: &[&str]) -> HashMap<String, String> {
        headers
            .iter()
            .map(|elem| {
                let pos = elem.find(":").unwrap();
                let (key, value) = elem.split_at(pos);
                (key.to_string(), value[2..].to_string())
            }).collect()
    }

    ///Returns status code of this `Response`.
    pub fn status_code(&self) -> StatusCode {
        self.status.code
    }

    ///Returns HTTP version of this `Response`.
    pub fn version(&self) -> &str {
        &self.status.version
    }

    ///Returns reason of this `Response`.
    pub fn reason(&self) -> &str {
        &self.status.reason
    }

    ///Returns headers of this `Response`.
    pub fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }

    ///Returns body of this `Response`.
    pub fn body(&self) -> &[u8] {
        match self.body {
            Some(ref b) => &b,
            None => &[],
        }
    }

    ///Returns length of the content of this `Response` as a `Result`, according to information
    ///included in headers. If there is no such an information, returns `Ok(0)`.
    pub fn content_len(&self) -> Result<usize, ParseIntError> {
        if let Some(p) = self.headers().get("Content-Length") {
            Ok(p.parse()?)
        } else {
            Ok(0)
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct StatusCode(u16);

impl StatusCode {
    pub fn new(code: u16) -> StatusCode {
        StatusCode(code)
    }

    ///Checks if this `StatusCode` is within 100-199, which indicates that it's Informational.
    pub fn is_info(&self) -> bool {
        self.0 >= 100 && self.0 < 200
    }

    ///Checks if this `StatusCode` is within 200-299, which indicates that it's Successful.
    pub fn is_success(&self) -> bool {
        self.0 >= 200 && self.0 < 300
    }

    ///Checks if this `StatusCode` is within 300-399, which indicates that it's Redirection.
    pub fn is_redirect(&self) -> bool {
        self.0 >= 300 && self.0 < 400
    }

    ///Checks if this `StatusCode` is within 400-499, which indicates that it's Client Error.
    pub fn is_client_err(&self) -> bool {
        self.0 >= 400 && self.0 < 500
    }

    ///Checks if this `StatusCode` is within 500-599, which indicates that it's Server Error.
    pub fn is_server_err(&self) -> bool {
        self.0 >= 500 && self.0 < 600
    }
}

impl From<StatusCode> for u16 {
    fn from(code: StatusCode) -> Self {
        code.0
    }
}

impl From<u16> for StatusCode {
    fn from(code: u16) -> Self {
        StatusCode(code)
    }
}

impl fmt::Display for StatusCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(PartialEq, Debug)]
pub struct Status {
    version: String,
    code: StatusCode,
    reason: String,
}

impl<T, U, V> From<(T, U, V)> for Status
where
    T: ToString,
    V: ToString,
    StatusCode: From<U>,
{
    fn from(status: (T, U, V)) -> Status {
        Status {
            version: status.0.to_string(),
            code: StatusCode::from(status.1),
            reason: status.2.to_string(),
        }
    }
}

///Finds elements slice `e` inside slice `data`. Returns position of the end of first match.
pub fn find_slice<T>(data: &[T], e: &[T]) -> Option<usize>
where
    [T]: PartialEq,
{
    for i in 0..=data.len() - e.len() {
        if data[i..(i + e.len())] == *e {
            return Some(i + e.len());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    //"HTTP/1.1 200 OK\r\nDate: Sat, 11 Jan 2003 02:44:04 GMT\r\nContent-Type: text/html\r\n
    //Content-Length: 100\r\n\r\n<html>hello</html>\r\n\r\nhello"
    const RESPONSE: [u8; 129] = [
        72, 84, 84, 80, 47, 49, 46, 49, 32, 50, 48, 48, 32, 79, 75, 13, 10, 68, 97, 116, 101, 58,
        32, 83, 97, 116, 44, 32, 49, 49, 32, 74, 97, 110, 32, 50, 48, 48, 51, 32, 48, 50, 58, 52,
        52, 58, 48, 52, 32, 71, 77, 84, 13, 10, 67, 111, 110, 116, 101, 110, 116, 45, 84, 121, 112,
        101, 58, 32, 116, 101, 120, 116, 47, 104, 116, 109, 108, 13, 10, 67, 111, 110, 116, 101,
        110, 116, 45, 76, 101, 110, 103, 116, 104, 58, 32, 49, 48, 48, 13, 10, 13, 10, 60, 104,
        116, 109, 108, 62, 104, 101, 108, 108, 111, 60, 47, 104, 116, 109, 108, 62, 13, 10, 13, 10,
        104, 101, 108, 108, 111,
    ];

    const RESPONSE_H: [u8; 102] = [
        72, 84, 84, 80, 47, 49, 46, 49, 32, 50, 48, 48, 32, 79, 75, 13, 10, 68, 97, 116, 101, 58,
        32, 83, 97, 116, 44, 32, 49, 49, 32, 74, 97, 110, 32, 50, 48, 48, 51, 32, 48, 50, 58, 52,
        52, 58, 48, 52, 32, 71, 77, 84, 13, 10, 67, 111, 110, 116, 101, 110, 116, 45, 84, 121, 112,
        101, 58, 32, 116, 101, 120, 116, 47, 104, 116, 109, 108, 13, 10, 67, 111, 110, 116, 101,
        110, 116, 45, 76, 101, 110, 103, 116, 104, 58, 32, 49, 48, 48, 13, 10, 13, 10,
    ];

    const BODY: [u8; 27] = [
        60, 104, 116, 109, 108, 62, 104, 101, 108, 108, 111, 60, 47, 104, 116, 109, 108, 62, 13,
        10, 13, 10, 104, 101, 108, 108, 111,
    ];

    const STATUS_LINE: &str = "HTTP/1.1 200 OK";
    const VERSION: &str = "HTTP/1.1";
    const CODE: u16 = 200;
    const REASON: &str = "OK";

    const HEADERS: [&str; 3] = [
        "Date: Sat, 11 Jan 2003 02:44:04 GMT",
        "Content-Type: text/html",
        "Content-Length: 100",
    ];

    const CODE_S: StatusCode = StatusCode(200);

    #[test]
    fn u16_from_status_code() {
        assert_eq!(u16::from(CODE_S), 200);
    }

    #[test]
    fn status_code_from() {
        assert_eq!(StatusCode::from(200), StatusCode(200));
    }

    #[test]
    fn status_code_info() {
        for i in 100..200 {
            assert!(StatusCode::new(i).is_info())
        }

        for i in (0..1000).filter(|&i| i < 100 || i >= 200) {
            assert!(!StatusCode::new(i).is_info())
        }
    }

    #[test]
    fn status_code_success() {
        for i in 200..300 {
            assert!(StatusCode::new(i).is_success())
        }

        for i in (0..1000).filter(|&i| i < 200 || i >= 300) {
            assert!(!StatusCode::new(i).is_success())
        }
    }

    #[test]
    fn status_code_redirect() {
        for i in 300..400 {
            assert!(StatusCode::new(i).is_redirect())
        }

        for i in (0..1000).filter(|&i| i < 300 || i >= 400) {
            assert!(!StatusCode::new(i).is_redirect())
        }
    }

    #[test]
    fn status_code_client_err() {
        for i in 400..500 {
            assert!(StatusCode::new(i).is_client_err())
        }

        for i in (0..1000).filter(|&i| i < 400 || i >= 500) {
            assert!(!StatusCode::new(i).is_client_err())
        }
    }

    #[test]
    fn status_code_server_err() {
        for i in 500..600 {
            assert!(StatusCode::new(i).is_server_err())
        }

        for i in (0..1000).filter(|&i| i < 500 || i >= 600) {
            assert!(!StatusCode::new(i).is_server_err())
        }
    }

    #[test]
    fn status_from() {
        let status = Status::from((VERSION, CODE, REASON));

        assert_eq!(status.version, VERSION);
        assert_eq!(status.code, CODE_S);
        assert_eq!(status.reason, REASON);
    }

    #[test]
    fn elements_find() {
        const WORDS: [&str; 8] = ["Good", "job", "Great", "work", "Have", "fun", "See", "you"];
        const SEARCH: [&str; 3] = ["Great", "work", "Have"];

        assert_eq!(find_slice(&WORDS, &SEARCH), Some(5));
    }

    #[test]
    fn res_try_from() {
        Response::try_from(&RESPONSE).unwrap();
        Response::try_from(&RESPONSE_H).unwrap();
    }

    #[test]
    #[should_panic]
    fn res_from_empty() {
        Response::try_from(&[]).unwrap();
    }

    #[test]
    fn res_new() {
        Response::try_from(&RESPONSE_H).unwrap();
    }

    #[test]
    fn res_append_body() {
        let mut res = Response::try_from(&RESPONSE_H).unwrap();
        let body = BODY.to_vec();

        res.append_body(body);
        assert_eq!(res.body(), BODY);
    }

    #[test]
    fn res_parse_head() {
        let mut headers = HashMap::with_capacity(2);
        headers.insert(
            "Date".to_string(),
            "Sat, 11 Jan 2003 02:44:04 GMT".to_string(),
        );
        headers.insert("Content-Type".to_string(), "text/html".to_string());
        headers.insert("Content-Length".to_string(), "100".to_string());

        let head = Response::parse_head(&RESPONSE_H).unwrap();

        assert_eq!(head.0, headers);
        assert_eq!(head.1, Status::from((VERSION, CODE, REASON)));
    }

    #[test]
    fn res_parse_headers() {
        let mut headers = HashMap::with_capacity(2);
        headers.insert(
            "Date".to_string(),
            "Sat, 11 Jan 2003 02:44:04 GMT".to_string(),
        );
        headers.insert("Content-Type".to_string(), "text/html".to_string());
        headers.insert("Content-Length".to_string(), "100".to_string());

        let headers = Response::parse_headers(&HEADERS);
        assert_eq!(headers, headers);
    }

    #[test]
    fn res_parse_status_line() {
        let status = Response::parse_status_line(STATUS_LINE).unwrap();
        assert_eq!(status, Status::from((VERSION, CODE, REASON)))
    }

    #[test]
    fn res_status_code() {
        let res = Response::try_from(&RESPONSE).unwrap();
        assert_eq!(res.status_code(), CODE_S);
    }

    #[test]
    fn res_version() {
        let res = Response::try_from(&RESPONSE).unwrap();
        assert_eq!(res.version(), "HTTP/1.1");
    }

    #[test]
    fn res_reason() {
        let res = Response::try_from(&RESPONSE).unwrap();
        assert_eq!(res.reason(), "OK");
    }

    #[test]
    fn res_headers() {
        let res = Response::try_from(&RESPONSE).unwrap();
        let mut headers = HashMap::with_capacity(2);
        headers.insert(
            "Date".to_string(),
            "Sat, 11 Jan 2003 02:44:04 GMT".to_string(),
        );
        headers.insert("Content-Type".to_string(), "text/html".to_string());
        headers.insert("Content-Length".to_string(), "100".to_string());

        assert_eq!(res.headers(), &headers);
    }

    #[test]
    fn res_body() {
        let res = Response::try_from(&RESPONSE).unwrap();
        assert_eq!(res.body(), BODY);

        let res = Response::try_from(&RESPONSE_H).unwrap();
        assert_eq!(res.body(), []);
    }

    #[test]
    fn res_content_len() {
        let res = Response::try_from(&RESPONSE).unwrap();
        assert_eq!(res.content_len(), Ok(100));
    }
}
