//! parsing server response
use crate::error::{Error, ParseErr};
use std::{collections::HashMap, fmt, io::Write, str};

pub(crate) const CR_LF_2: [u8; 4] = [13, 10, 13, 10];

pub struct Response {
    status: Status,
    headers: Headers,
}

impl Response {
    ///Creates new `Response` with head - status and headers - parsed from a slice of bytes
    pub fn from_head(head: &[u8]) -> Result<Response, Error> {
        let (status, headers) = Self::parse_head(head)?;

        Ok(Response { status, headers })
    }

    ///Parses `Response` from slice of bytes. Writes it's body to `writer`.
    pub fn try_from<T: Write>(res: &[u8], writer: &mut T) -> Result<Response, Error> {
        if res.is_empty() {
            return Err(Error::Parse(ParseErr::Empty));
        }

        let mut pos = res.len();
        if let Some(v) = find_slice(res, &CR_LF_2) {
            pos = v;
        }

        let response = Self::from_head(&res[..pos])?;
        writer.write_all(&res[pos..])?;

        Ok(response)
    }

    ///Parses head of a `Response` - status and headers - from slice of bytes.
    pub fn parse_head(head: &[u8]) -> Result<(Status, Headers), ParseErr> {
        let mut head = str::from_utf8(head)?.splitn(2, '\n');

        let status = head.next().unwrap().parse()?;
        let headers = head.next().unwrap().parse()?;

        Ok((status, headers))
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
    pub fn headers(&self) -> &Headers {
        &self.headers
    }

    ///Returns length of the content of this `Response` as a `Result`, according to information
    ///included in headers. If there is no such an information, returns `Ok(0)`.
    pub fn content_len(&self) -> Result<usize, ParseErr> {
        match self.headers().get("Content-Length") {
            Some(p) => Ok(p.parse()?),
            None => Ok(0),
        }
    }
}

///Code sent by a server in response to a client's request.
///# Example
///```
///use http_req::response::StatusCode;
///
///fn main() {
///   let code = StatusCode::from(200);
///
///   assert!(code.is_success())
///}
///```
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct StatusCode(u16);

impl StatusCode {
    pub fn new(code: u16) -> StatusCode {
        StatusCode(code)
    }

    ///Checks if this `StatusCode` is within 100-199, which indicates that it's Informational.
    pub fn is_info(self) -> bool {
        self.0 >= 100 && self.0 < 200
    }

    ///Checks if this `StatusCode` is within 200-299, which indicates that it's Successful.
    pub fn is_success(self) -> bool {
        self.0 >= 200 && self.0 < 300
    }

    ///Checks if this `StatusCode` is within 300-399, which indicates that it's Redirection.
    pub fn is_redirect(self) -> bool {
        self.0 >= 300 && self.0 < 400
    }

    ///Checks if this `StatusCode` is within 400-499, which indicates that it's Client Error.
    pub fn is_client_err(self) -> bool {
        self.0 >= 400 && self.0 < 500
    }

    ///Checks if this `StatusCode` is within 500-599, which indicates that it's Server Error.
    pub fn is_server_err(self) -> bool {
        self.0 >= 500 && self.0 < 600
    }

    ///Checks this `StatusCode` using closure `f`
    pub fn is<F: FnOnce(u16) -> bool>(self, f: F) -> bool {
        f(self.0)
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

impl str::FromStr for Status {
    type Err = ParseErr;

    fn from_str(status_line: &str) -> Result<Status, ParseErr> {
        let status_line: Vec<_> = status_line.trim().splitn(3, ' ').collect();

        let version = status_line[0];
        let code: u16 = status_line[1].parse()?;
        let reason = status_line[2];

        Ok(Status::from((version, code, reason)))
    }
}

#[derive(Debug, PartialEq)]
pub struct Headers(HashMap<String, String>);

impl Headers {
    ///Returns a reference to the value corresponding to the key.
    pub fn get(&self, v: &str) -> Option<&std::string::String> {
        self.0.get(v)
    }
}

impl str::FromStr for Headers {
    type Err = ParseErr;

    fn from_str(s: &str) -> Result<Headers, ParseErr> {
        let headers: Vec<_> = s.trim().lines().collect();
        let correct_headers = headers.iter().all(|e| e.contains(':'));

        if correct_headers {
            let headers = headers
                .iter()
                .map(|elem| {
                    let pos = elem.find(':').unwrap();
                    let (key, value) = elem.split_at(pos);
                    (key.to_string(), value[2..].to_string())
                })
                .collect();

            Ok(Headers(headers))
        } else {
            Err(ParseErr::Invalid)
        }
    }
}

impl From<HashMap<String, String>> for Headers {
    fn from(map: HashMap<String, String>) -> Headers {
        Headers(map)
    }
}

impl From<Headers> for HashMap<String, String> {
    fn from(map: Headers) -> HashMap<String, String> {
        map.0
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

    const RESPONSE: &'static [u8; 129] = b"HTTP/1.1 200 OK\r\n\
                                         Date: Sat, 11 Jan 2003 02:44:04 GMT\r\n\
                                         Content-Type: text/html\r\n\
                                         Content-Length: 100\r\n\r\n\
                                         <html>hello</html>\r\n\r\nhello";
    const RESPONSE_H: &'static [u8; 102] = b"HTTP/1.1 200 OK\r\n\
                                           Date: Sat, 11 Jan 2003 02:44:04 GMT\r\n\
                                           Content-Type: text/html\r\n\
                                           Content-Length: 100\r\n\r\n";
    const BODY: &'static [u8; 27] = b"<html>hello</html>\r\n\r\nhello";

    const STATUS_LINE: &str = "HTTP/1.1 200 OK";
    const VERSION: &str = "HTTP/1.1";
    const CODE: u16 = 200;
    const REASON: &str = "OK";

    const HEADERS: &str = "Date: Sat, 11 Jan 2003 02:44:04 GMT\r\n\
                           Content-Type: text/html\r\n\
                           Content-Length: 100\r\n";
    const CODE_S: StatusCode = StatusCode(200);

    #[test]
    fn status_code_new() {
        assert_eq!(StatusCode::new(200), StatusCode(200));
        assert_ne!(StatusCode::new(400), StatusCode(404));
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
    fn status_code_is() {
        let check = |i| i % 3 == 0;

        let code_1 = StatusCode::new(200);
        let code_2 = StatusCode::new(300);

        assert!(!code_1.is(check));
        assert!(code_2.is(check));
    }

    #[test]
    fn status_code_from() {
        assert_eq!(StatusCode::from(200), StatusCode(200));
    }

    #[test]
    fn u16_from_status_code() {
        assert_eq!(u16::from(CODE_S), 200);
    }

    #[test]
    fn status_code_display() {
        let code = format!("Status Code: {}", StatusCode::new(200));
        const CODE_EXPECT: &str = "Status Code: 200";

        assert_eq!(code, CODE_EXPECT);
    }

    #[test]
    fn status_from() {
        let status = Status::from((VERSION, CODE, REASON));

        assert_eq!(status.version, VERSION);
        assert_eq!(status.code, CODE_S);
        assert_eq!(status.reason, REASON);
    }

    #[test]
    fn status_from_str() {
        let status = STATUS_LINE.parse::<Status>().unwrap();

        assert_eq!(status.version, VERSION);
        assert_eq!(status.code, CODE_S);
        assert_eq!(status.reason, REASON);
    }

    #[test]
    fn headers_get() {
        let mut headers = HashMap::with_capacity(2);
        headers.insert(
            "Date".to_string(),
            "Sat, 11 Jan 2003 02:44:04 GMT".to_string(),
        );

        let headers = Headers::from(headers);

        assert_eq!(
            headers.get("Date"),
            Some(&"Sat, 11 Jan 2003 02:44:04 GMT".to_string())
        );
    }

    #[test]
    fn headers_from_str() {
        let mut headers_expect = HashMap::with_capacity(2);
        headers_expect.insert(
            "Date".to_string(),
            "Sat, 11 Jan 2003 02:44:04 GMT".to_string(),
        );
        headers_expect.insert("Content-Type".to_string(), "text/html".to_string());
        headers_expect.insert("Content-Length".to_string(), "100".to_string());

        let headers = HEADERS.parse::<Headers>().unwrap();
        assert_eq!(headers, Headers::from(headers_expect));
    }

    #[test]
    fn headers_from() {
        let mut headers_expect = HashMap::with_capacity(2);
        headers_expect.insert(
            "Date".to_string(),
            "Sat, 11 Jan 2003 02:44:04 GMT".to_string(),
        );
        headers_expect.insert("Content-Type".to_string(), "text/html".to_string());
        headers_expect.insert("Content-Length".to_string(), "100".to_string());

        assert_eq!(
            Headers(headers_expect.clone()),
            Headers::from(headers_expect)
        );
    }

    #[test]
    fn elements_find() {
        const WORDS: [&str; 8] = ["Good", "job", "Great", "work", "Have", "fun", "See", "you"];
        const SEARCH: [&str; 3] = ["Great", "work", "Have"];

        assert_eq!(find_slice(&WORDS, &SEARCH), Some(5));
    }

    #[test]
    fn res_from_head() {
        Response::from_head(RESPONSE_H).unwrap();
    }

    #[test]
    fn res_try_from() {
        let mut writer = Vec::new();

        Response::try_from(RESPONSE, &mut writer).unwrap();
        Response::try_from(RESPONSE_H, &mut writer).unwrap();
    }

    #[test]
    #[should_panic]
    fn res_from_empty() {
        let mut writer = Vec::new();
        Response::try_from(&[], &mut writer).unwrap();
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

        let head = Response::parse_head(RESPONSE_H).unwrap();

        assert_eq!(head.0, Status::from((VERSION, CODE, REASON)));
        assert_eq!(head.1, Headers::from(headers));
    }

    #[test]
    fn res_status_code() {
        let mut writer = Vec::new();
        let res = Response::try_from(RESPONSE, &mut writer).unwrap();

        assert_eq!(res.status_code(), CODE_S);
    }

    #[test]
    fn res_version() {
        let mut writer = Vec::new();
        let res = Response::try_from(RESPONSE, &mut writer).unwrap();

        assert_eq!(res.version(), "HTTP/1.1");
    }

    #[test]
    fn res_reason() {
        let mut writer = Vec::new();
        let res = Response::try_from(RESPONSE, &mut writer).unwrap();

        assert_eq!(res.reason(), "OK");
    }

    #[test]
    fn res_headers() {
        let mut writer = Vec::new();
        let res = Response::try_from(RESPONSE, &mut writer).unwrap();

        let mut headers = HashMap::with_capacity(2);
        headers.insert(
            "Date".to_string(),
            "Sat, 11 Jan 2003 02:44:04 GMT".to_string(),
        );
        headers.insert("Content-Type".to_string(), "text/html".to_string());
        headers.insert("Content-Length".to_string(), "100".to_string());

        assert_eq!(res.headers(), &Headers::from(headers));
    }

    #[test]
    fn res_content_len() {
        let mut writer = Vec::with_capacity(101);
        let res = Response::try_from(RESPONSE, &mut writer).unwrap();

        assert_eq!(res.content_len(), Ok(100));
    }

    #[test]
    fn res_body() {
        let mut writer = Vec::new();
        Response::try_from(RESPONSE, &mut writer).unwrap();

        assert_eq!(writer, BODY);
    }
}
