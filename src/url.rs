//! url operations
use error::{Error, ParseErr};
use std::str::FromStr;

const HTTP_PORT: u16 = 80;
const HTTPS_PORT: u16 = 443;

pub trait RefOr<'a> {
    fn ref_or(&'a self, def: &'a str) -> &'a str;
}

impl<'a> RefOr<'a> for Option<String> {
    fn ref_or(&'a self, def: &'a str) -> &'a str {
        match self {
            Some(ref s) => s,
            None => def,
        }
    }
}

pub struct Url {
    scheme: String,
    authority: Option<Authority>,
    path: Option<String>,
    query: Option<String>,
    fragment: Option<String>,
}

impl Url {
    ///Returs scheme of this `Url`.
    pub fn scheme(&self) -> &str {
        &self.scheme
    }

    ///Returs information about the user included in this `Url`.
    pub fn user_info(&self) -> &str {
        match self.authority {
            Some(ref a) => a.user_info.ref_or(""),
            None => "",
        }
    }

    ///Returs host of this `Url`.
    pub fn host(&self) -> &str {
        match self.authority {
            Some(ref a) => a.host.ref_or(""),
            None => "",
        }
    }

    ///Returs port of this `Url`. If it hasn't been set in the parsed Url, returns default port.
    pub fn port(&self) -> u16 {
        let default_port = match self.scheme.as_ref() {
            "https" => HTTPS_PORT,
            _ => HTTP_PORT,
        };

        match self.authority {
            Some(ref a) => a.port.unwrap_or(default_port),
            None => default_port,
        }
    }

    ///Returs path of this `Url`.
    pub fn path(&self) -> &str {
        self.path.ref_or("")
    }

    ///Returs query of this `Url`.
    pub fn query(&self) -> &str {
        self.query.ref_or("")
    }

    ///Returs fragment of this `Url`.
    pub fn fragment(&self) -> &str {
        self.fragment.ref_or("")
    }

    ///Returs resource `Url` points to.
    pub fn resource(&self) -> String {
        let path = self.path().to_string();

        if self.query().is_empty() {
            path
        } else {
            path + "?" + &self.query()
        }
    }
}

impl FromStr for Url {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (scheme, mut url_part) = get_chunks(s, ":");
        let scheme = match scheme {
            Some(s) => s,
            None => return Err(Error::Parse(ParseErr::Empty)),
        };

        let mut authority = None;
        if let Some(u) = url_part.clone() {
            if u.contains("//") {
                let (auth, part) = get_chunks(&u[2..], "/");

                authority = match auth {
                    Some(a) => match a.parse::<Authority>() {
                        Ok(i) => Some(i),
                        Err(e) => return Err(Error::Parse(e)),
                    },
                    None => None,
                };

                url_part = part;
            }
        }

        let (path, url_part) = chunk(url_part, "?");
        let (query, fragment) = chunk(url_part, "#");

        Ok(Url {
            scheme,
            authority,
            path,
            query,
            fragment,
        })
    }
}

struct Authority {
    user_info: Option<String>,
    host: Option<String>,
    port: Option<u16>,
}

impl FromStr for Authority {
    type Err = ParseErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut s = s.to_string();
        remove_spaces(&mut s);

        let mut user_info = None;

        let url_part = if s.contains("@") {
            let (info, part) = get_chunks(&s, "@");
            user_info = info;
            part
        } else {
            Some(s)
        };

        let (host, url_part) = chunk(url_part, ":");

        let port = match url_part {
            Some(p) => Some(p.parse()?),
            None => None,
        };

        Ok(Authority {
            user_info,
            host,
            port,
        })
    }
}

fn remove_spaces(text: &mut String) {
    text.retain(|c| !c.is_whitespace());
}

fn chunk(base: Option<String>, separator: &str) -> (Option<String>, Option<String>) {
    match base {
        Some(ref u) => get_chunks(u, separator),
        None => (None, None),
    }
}

fn get_chunks(s: &str, separator: &str) -> (Option<String>, Option<String>) {
    match s.find(separator) {
        Some(i) => {
            let (chunk, rest) = s.split_at(i);
            let rest = &rest[separator.len()..];
            let mut rest = if rest.is_empty() {
                None
            } else {
                Some(rest.to_string())
            };

            (Some(chunk.to_string()), rest)
        }
        None => {
            if !s.is_empty() {
                (Some(s.to_string()), None)
            } else {
                (None, None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_URLS: [&str; 4] = [
        "https://user:info@foo.com:12/bar/baz?query#fragment",
        "file:///C:/Users/User/Pictures/screenshot.png",
        "https://en.wikipedia.org/wiki/Hypertext_Transfer_Protocol",
        "mailto:John.Doe@example.com",
    ];

    #[test]
    fn remove_space() {
        let mut text = String::from("Hello World     !");
        let expect = String::from("HelloWorld!");

        remove_spaces(&mut text);
        assert_eq!(text, expect);
    }

    #[test]
    fn full_parse() {
        let url = "abc://username:password@example.com:123/path/data?key=value&key2=value2#fragid1"
            .parse::<Url>()
            .unwrap();
        assert_eq!(url.scheme(), "abc");

        assert_eq!(url.user_info(), "username:password");
        assert_eq!(url.host(), "example.com");
        assert_eq!(url.port(), 123);

        assert_eq!(url.path(), "path/data");
        assert_eq!(url.query(), "key=value&key2=value2");
        assert_eq!(url.fragment(), "fragid1");
    }

    #[test]
    fn parse_url() {
        for url in TEST_URLS.iter() {
            url.parse::<Url>().unwrap();
        }
    }

    #[test]
    fn scheme_url() {
        let urls: Vec<_> = TEST_URLS
            .iter()
            .map(|url| url.parse::<Url>().unwrap())
            .collect();

        assert_eq!(urls[0].scheme(), "https");
        assert_eq!(urls[1].scheme(), "file");
        assert_eq!(urls[2].scheme(), "https");
        assert_eq!(urls[3].scheme(), "mailto");
    }

    #[test]
    fn uesr_info_url() {
        let urls: Vec<_> = TEST_URLS
            .iter()
            .map(|url| url.parse::<Url>().unwrap())
            .collect();

        assert_eq!(urls[0].user_info(), "user:info");
        assert_eq!(urls[1].user_info(), "");
        assert_eq!(urls[2].user_info(), "");
        assert_eq!(urls[3].user_info(), "");
    }

    #[test]
    fn host_url() {
        let urls: Vec<_> = TEST_URLS
            .iter()
            .map(|url| url.parse::<Url>().unwrap())
            .collect();

        assert_eq!(urls[0].host(), "foo.com");
        assert_eq!(urls[1].host(), "");
        assert_eq!(urls[2].host(), "en.wikipedia.org");
        assert_eq!(urls[3].host(), "");
    }

    #[test]
    fn port_url() {
        let urls: Vec<_> = TEST_URLS
            .iter()
            .map(|url| url.parse::<Url>().unwrap())
            .collect();

        assert_eq!(urls[0].port(), 12);
        assert_eq!(urls[1].port(), HTTP_PORT);
        assert_eq!(urls[2].port(), HTTPS_PORT);
        assert_eq!(urls[3].port(), HTTP_PORT);
    }

    #[test]
    fn path_url() {
        let urls: Vec<_> = TEST_URLS
            .iter()
            .map(|url| url.parse::<Url>().unwrap())
            .collect();

        assert_eq!(urls[0].path(), "bar/baz");
        assert_eq!(urls[1].path(), "C:/Users/User/Pictures/screenshot.png");
        assert_eq!(urls[2].path(), "wiki/Hypertext_Transfer_Protocol");
        assert_eq!(urls[3].path(), "John.Doe@example.com");
    }

    #[test]
    fn query_url() {
        let urls: Vec<_> = TEST_URLS
            .iter()
            .map(|url| url.parse::<Url>().unwrap())
            .collect();

        assert_eq!(urls[0].query(), "query");
        assert_eq!(urls[1].query(), "");
        assert_eq!(urls[2].query(), "");
        assert_eq!(urls[3].query(), "");
    }

    #[test]
    fn fragment_url() {
        let urls: Vec<_> = TEST_URLS
            .iter()
            .map(|url| url.parse::<Url>().unwrap())
            .collect();

        assert_eq!(urls[0].fragment(), "fragment");
        assert_eq!(urls[1].fragment(), "");
        assert_eq!(urls[2].fragment(), "");
        assert_eq!(urls[3].fragment(), "");
    }

    #[test]
    fn resource_url() {
        let urls: Vec<_> = TEST_URLS
            .iter()
            .map(|url| url.parse::<Url>().unwrap())
            .collect();

        assert_eq!(urls[0].resource(), "bar/baz?query");
        assert_eq!(urls[1].resource(), "C:/Users/User/Pictures/screenshot.png");
        assert_eq!(urls[2].resource(), "wiki/Hypertext_Transfer_Protocol");
        assert_eq!(urls[3].resource(), "John.Doe@example.com");
    }
}
