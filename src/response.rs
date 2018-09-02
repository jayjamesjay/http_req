//! parsing server response
use super::*;
use std::str;

const CR_LF_2: [u8; 4] = [13, 10, 13, 10];

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
    ///Parses server's response
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

        let mut head: Vec<_> = str::from_utf8(head)?.lines().collect();
        head.pop();

        let status = Self::parse_status_line(&head.remove(0))?;
        let headers = Self::parse_headers(&head);

        Ok(Response {
            status,
            headers,
            body,
        })
    }

    fn parse_status_line(status_line: &str) -> Result<Status, ParseIntError> {
        let status_line: Vec<&str> = status_line.split_whitespace().collect();

        let version = status_line[0];
        let code = status_line[1].parse()?;
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
            })
            .collect()
    }

    ///Returns status code
    pub fn status_code(&self) -> u16 {
        self.status.code
    }

    ///Returns HTTP version
    pub fn version(&self) -> &str {
        &self.status.version
    }

    ///Returns reason
    pub fn reason(&self) -> &str {
        &self.status.reason
    }

    ///Returns headers
    pub fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }

    ///Returns body
    pub fn body(&self) -> &[u8] {
        match self.body {
            Some(ref b) => &b,
            None => &[],
        }
    }
}

pub struct Status {
    version: String,
    code: u16,
    reason: String,
}

impl<T, U> From<(T, u16, U)> for Status
where
    T: ToString,
    U: ToString,
{
    fn from(status: (T, u16, U)) -> Status {
        Status {
            version: status.0.to_string(),
            code: status.1,
            reason: status.2.to_string(),
        }
    }
}

///Finds elements slice `e` inside slice `data`. Returns position of the end of first match.
fn find_slice<T>(data: &[T], e: &[T]) -> Option<usize>
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

    //"HTTP/1.1 200 OK\r\nDate: Sat, 11 Jan 2003 02:44:04 GMT\r\nContent-Type: text/html\r\n\r\n<html>hello</html>\r\n\r\nhello"
    const RESPONSE: [u8; 108] = [
        72, 84, 84, 80, 47, 49, 46, 49, 32, 50, 48, 48, 32, 79, 75, 13, 10, 68, 97, 116, 101, 58,
        32, 83, 97, 116, 44, 32, 49, 49, 32, 74, 97, 110, 32, 50, 48, 48, 51, 32, 48, 50, 58, 52,
        52, 58, 48, 52, 32, 71, 77, 84, 13, 10, 67, 111, 110, 116, 101, 110, 116, 45, 84, 121, 112,
        101, 58, 32, 116, 101, 120, 116, 47, 104, 116, 109, 108, 13, 10, 13, 10, 60, 104, 116, 109,
        108, 62, 104, 101, 108, 108, 111, 60, 47, 104, 116, 109, 108, 62, 13, 10, 13, 10, 104, 101,
        108, 108, 111,
    ];

    const RESPONSE_H: [u8; 81] = [
        72, 84, 84, 80, 47, 49, 46, 49, 32, 50, 48, 48, 32, 79, 75, 13, 10, 68, 97, 116, 101, 58,
        32, 83, 97, 116, 44, 32, 49, 49, 32, 74, 97, 110, 32, 50, 48, 48, 51, 32, 48, 50, 58, 52,
        52, 58, 48, 52, 32, 71, 77, 84, 13, 10, 67, 111, 110, 116, 101, 110, 116, 45, 84, 121, 112,
        101, 58, 32, 116, 101, 120, 116, 47, 104, 116, 109, 108, 13, 10, 13, 10,
    ];

    #[test]
    fn elements_find() {
        const WORDS: [&str; 8] = ["Good", "job", "Great", "work", "Have", "fun", "See", "you"];
        const SEARCH: [&str; 3] = ["Great", "work", "Have"];

        assert_eq!(find_slice(&WORDS, &SEARCH), Some(5));
    }

    #[test]
    fn new_response() {
        Response::try_from(&RESPONSE).unwrap();
        Response::try_from(&RESPONSE_H).unwrap();
    }

    #[test]
    #[should_panic]
    fn response_from_empty() {
        Response::try_from(&[]).unwrap();
    }

    #[test]
    fn response_status() {
        let res = Response::try_from(&RESPONSE).unwrap();
        assert_eq!(res.status_code(), 200);
    }

    #[test]
    fn response_version() {
        let res = Response::try_from(&RESPONSE).unwrap();
        assert_eq!(res.version(), "HTTP/1.1");
    }

    #[test]
    fn response_headers() {
        let res = Response::try_from(&RESPONSE).unwrap();
        let mut headers = HashMap::with_capacity(2);
        headers.insert(
            "Date".to_string(),
            "Sat, 11 Jan 2003 02:44:04 GMT".to_string(),
        );
        headers.insert("Content-Type".to_string(), "text/html".to_string());

        assert_eq!(res.headers(), &headers);
    }

    #[test]
    fn response_body() {
        let res = Response::try_from(&RESPONSE).unwrap();
        const BODY: [u8; 27] = [
            60, 104, 116, 109, 108, 62, 104, 101, 108, 108, 111, 60, 47, 104, 116, 109, 108, 62,
            13, 10, 13, 10, 104, 101, 108, 108, 111,
        ];

        assert_eq!(res.body(), BODY);

        let res = Response::try_from(&RESPONSE_H).unwrap();
        assert_eq!(res.body(), []);
    }
}
