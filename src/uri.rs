//! uri operations
use crate::error::{Error, ParseErr};
use std::str;

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

#[derive(Clone, Debug, PartialEq)]
pub struct Uri {
    scheme: String,
    authority: Option<Authority>,
    path: Option<String>,
    query: Option<String>,
    fragment: Option<String>,
}

impl Uri {
    ///Returs scheme of this `Uri`.
    pub fn scheme(&self) -> &str {
        &self.scheme
    }

    ///Returs information about the user included in this `Uri`.
    pub fn user_info(&self) -> &str {
        match self.authority {
            Some(ref a) => a.user_info.ref_or(""),
            None => "",
        }
    }

    ///Returs host of this `Uri`.
    pub fn host(&self) -> &str {
        match self.authority {
            Some(ref a) => a.host.ref_or(""),
            None => "",
        }
    }

    ///Returs port of this `Uri`. If it hasn't been set in the parsed Uri, returns default port.
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

    ///Returs path of this `Uri`.
    pub fn path(&self) -> &str {
        self.path.ref_or("")
    }

    ///Returs query of this `Uri`.
    pub fn query(&self) -> &str {
        self.query.ref_or("")
    }

    ///Returs fragment of this `Uri`.
    pub fn fragment(&self) -> &str {
        self.fragment.ref_or("")
    }

    ///Returs resource `Uri` points to.
    pub fn resource(&self) -> String {
        let path = self.path().to_string();

        if self.query().is_empty() {
            path
        } else {
            path + "?" + self.query()
        }
    }
}

impl str::FromStr for Uri {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (scheme, mut uri_part) = get_chunks(s, ":");
        let scheme = match scheme {
            Some(s) => s,
            None => return Err(Error::Parse(ParseErr::Empty)),
        };

        let mut authority = None;
        if let Some(u) = uri_part.clone() {
            if u.contains("//") {
                let (auth, part) = get_chunks(&u[2..], "/");

                authority = match auth {
                    Some(a) => match a.parse::<Authority>() {
                        Ok(i) => Some(i),
                        Err(e) => return Err(Error::Parse(e)),
                    },
                    None => None,
                };

                uri_part = part;
            }
        }

        let (path, uri_part) = chunk(&uri_part, "?");
        let (query, fragment) = chunk(&uri_part, "#");

        Ok(Uri {
            scheme,
            authority,
            path,
            query,
            fragment,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Authority {
    user_info: Option<String>,
    host: Option<String>,
    port: Option<u16>,
}

impl str::FromStr for Authority {
    type Err = ParseErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut s = s.to_string();
        remove_spaces(&mut s);

        let mut user_info = None;

        let uri_part = if s.contains('@') {
            let (info, part) = get_chunks(&s, "@");
            user_info = info;
            part
        } else {
            Some(s)
        };

        let (host, uri_part) = chunk(&uri_part, ":");

        let port = match uri_part {
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

//Removes whitespace from `text`
fn remove_spaces(text: &mut String) {
    text.retain(|c| !c.is_whitespace());
}

//Splits `String` from `base` by `separator`. If `base` is `None`, it will return
//tuple consisting two `None` values.
fn chunk(base: &Option<String>, separator: &str) -> (Option<String>, Option<String>) {
    match base {
        Some(ref u) => get_chunks(u, separator),
        None => (None, None),
    }
}

//Splits `s` by `separator`. If `separator` is found inside `s`, it will return two `Some` values
//consisting parts of splitted `String`. If `separator` is at the end of `s` or it's not found,
//it will return tuple consisting `Some` with `s` inside and None.
fn get_chunks(s: &str, separator: &str) -> (Option<String>, Option<String>) {
    match s.find(separator) {
        Some(i) => {
            let (chunk, rest) = s.split_at(i);
            let rest = &rest[separator.len()..];
            let rest = if rest.is_empty() {
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
        let uri = "abc://username:password@example.com:123/path/data?key=value&key2=value2#fragid1"
            .parse::<Uri>()
            .unwrap();
        assert_eq!(uri.scheme(), "abc");

        assert_eq!(uri.user_info(), "username:password");
        assert_eq!(uri.host(), "example.com");
        assert_eq!(uri.port(), 123);

        assert_eq!(uri.path(), "path/data");
        assert_eq!(uri.query(), "key=value&key2=value2");
        assert_eq!(uri.fragment(), "fragid1");
    }

    #[test]
    fn parse_uri() {
        for uri in TEST_URLS.iter() {
            uri.parse::<Uri>().unwrap();
        }
    }

    #[test]
    fn scheme_uri() {
        let uris: Vec<_> = TEST_URLS
            .iter()
            .map(|uri| uri.parse::<Uri>().unwrap())
            .collect();

        assert_eq!(uris[0].scheme(), "https");
        assert_eq!(uris[1].scheme(), "file");
        assert_eq!(uris[2].scheme(), "https");
        assert_eq!(uris[3].scheme(), "mailto");
    }

    #[test]
    fn uesr_info_uri() {
        let uris: Vec<_> = TEST_URLS
            .iter()
            .map(|uri| uri.parse::<Uri>().unwrap())
            .collect();

        assert_eq!(uris[0].user_info(), "user:info");
        assert_eq!(uris[1].user_info(), "");
        assert_eq!(uris[2].user_info(), "");
        assert_eq!(uris[3].user_info(), "");
    }

    #[test]
    fn host_uri() {
        let uris: Vec<_> = TEST_URLS
            .iter()
            .map(|uri| uri.parse::<Uri>().unwrap())
            .collect();

        assert_eq!(uris[0].host(), "foo.com");
        assert_eq!(uris[1].host(), "");
        assert_eq!(uris[2].host(), "en.wikipedia.org");
        assert_eq!(uris[3].host(), "");
    }

    #[test]
    fn port_uri() {
        let uris: Vec<_> = TEST_URLS
            .iter()
            .map(|uri| uri.parse::<Uri>().unwrap())
            .collect();

        assert_eq!(uris[0].port(), 12);
        assert_eq!(uris[1].port(), HTTP_PORT);
        assert_eq!(uris[2].port(), HTTPS_PORT);
        assert_eq!(uris[3].port(), HTTP_PORT);
    }

    #[test]
    fn path_uri() {
        let uris: Vec<_> = TEST_URLS
            .iter()
            .map(|uri| uri.parse::<Uri>().unwrap())
            .collect();

        assert_eq!(uris[0].path(), "bar/baz");
        assert_eq!(uris[1].path(), "C:/Users/User/Pictures/screenshot.png");
        assert_eq!(uris[2].path(), "wiki/Hypertext_Transfer_Protocol");
        assert_eq!(uris[3].path(), "John.Doe@example.com");
    }

    #[test]
    fn query_uri() {
        let uris: Vec<_> = TEST_URLS
            .iter()
            .map(|uri| uri.parse::<Uri>().unwrap())
            .collect();

        assert_eq!(uris[0].query(), "query");
        assert_eq!(uris[1].query(), "");
        assert_eq!(uris[2].query(), "");
        assert_eq!(uris[3].query(), "");
    }

    #[test]
    fn fragment_uri() {
        let uris: Vec<_> = TEST_URLS
            .iter()
            .map(|uri| uri.parse::<Uri>().unwrap())
            .collect();

        assert_eq!(uris[0].fragment(), "fragment");
        assert_eq!(uris[1].fragment(), "");
        assert_eq!(uris[2].fragment(), "");
        assert_eq!(uris[3].fragment(), "");
    }

    #[test]
    fn resource_uri() {
        let uris: Vec<_> = TEST_URLS
            .iter()
            .map(|uri| uri.parse::<Uri>().unwrap())
            .collect();

        assert_eq!(uris[0].resource(), "bar/baz?query");
        assert_eq!(uris[1].resource(), "C:/Users/User/Pictures/screenshot.png");
        assert_eq!(uris[2].resource(), "wiki/Hypertext_Transfer_Protocol");
        assert_eq!(uris[3].resource(), "John.Doe@example.com");
    }
}
