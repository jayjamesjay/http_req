//! uri operations
use crate::error::{Error, ParseErr};
use std::{convert::AsRef, fmt, str};

const HTTP_PORT: u16 = 80;
const HTTPS_PORT: u16 = 443;

pub trait RefInner<'a, T, U: ?Sized> {
    fn ref_in(&'a self) -> Option<&'a U>;
    fn ref_or_default(&'a self, def: &'a U) -> &'a U;
}

impl<'a, U: ?Sized, T: AsRef<U>> RefInner<'a, T, U> for Option<T> {
    ///Returns None if the option is None, otherwise
    ///transforms `Option<T>` to `Option<&U>` by calling `as_ref` on contained value
    fn ref_in(&'a self) -> Option<&'a U> {
        match self {
            Some(ref v) => Some(v.as_ref()),
            None => None,
        }
    }

    ///Returns reference to contained value or a default.
    fn ref_or_default(&'a self, def: &'a U) -> &'a U {
        match self {
            Some(ref s) => s.as_ref(),
            None => def,
        }
    }
}

///Representation of Uniform Resource Identifier
///
///# Example
///```
///use http_req::uri::Uri;
///
///let uri: Uri = "https://user:info@foo.com:12/bar/baz?query#fragment".parse().unwrap();
///assert_eq!(uri.host(), Some("foo.com"));
///```
#[derive(Clone, Debug, PartialEq)]
pub struct Uri {
    scheme: String,
    authority: Option<Authority>,
    path: Option<String>,
    query: Option<String>,
    fragment: Option<String>,
}

impl Uri {
    ///Returns scheme of this `Uri`.
    pub fn scheme(&self) -> &str {
        &self.scheme
    }

    ///Returns information about the user included in this `Uri`.
    pub fn user_info(&self) -> Option<String> {
        match self.authority {
            Some(ref a) => a.user_info(),
            None => None,
        }
    }

    ///Returns host of this `Uri`.
    pub fn host(&self) -> Option<&str> {
        match self.authority {
            Some(ref a) => a.host(),
            None => None,
        }
    }

    ///Returns host of this `Uri` to use in a header.
    pub fn host_header(&self) -> Option<String> {
        match self.authority {
            Some(ref a) => match (a.host(), a.port()) {
                (Some(h), Some(p)) => Some(match *p {
                    HTTP_PORT | HTTPS_PORT => h.to_string(),
                    _ => format!("{}:{}", h, p),
                }),
                (Some(h), None) => Some(h.to_string()),
                _ => None,
            },
            None => None,
        }
    }

    ///Returns port of this `Uri`
    pub fn port(&self) -> &Option<u16> {
        match &self.authority {
            Some(a) => a.port(),
            None => &None,
        }
    }

    ///Returns port corresponding to this `Uri`.
    ///Returns default port if it hasn't been set in the uri.
    pub fn corr_port(&self) -> u16 {
        let default_port = match self.scheme() {
            "https" => HTTPS_PORT,
            _ => HTTP_PORT,
        };

        match self.authority {
            Some(ref a) => a.port().unwrap_or(default_port),
            None => default_port,
        }
    }

    ///Returns path of this `Uri`.
    pub fn path(&self) -> Option<&str> {
        self.path.ref_in()
    }

    ///Returns query of this `Uri`.
    pub fn query(&self) -> Option<&str> {
        self.query.ref_in()
    }

    ///Returns fragment of this `Uri`.
    pub fn fragment(&self) -> Option<&str> {
        self.fragment.ref_in()
    }

    ///Returns resource `Uri` points to.
    pub fn resource(&self) -> String {
        let mut resource = (&self.path().unwrap_or("/")).to_string();
        let query = self.query();
        let fragment = self.fragment();

        if query.is_some() {
            resource = resource + "?" + query.unwrap_or("");
        }

        if fragment.is_some() {
            resource + "#" + fragment.unwrap_or("")
        } else {
            resource
        }
    }
}

impl fmt::Display for Uri {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let authority = match self.authority {
            Some(ref a) => format!("//{}", a),
            None => "".to_string(),
        };

        write!(f, "{}:{}{}", self.scheme(), authority, self.resource())
    }
}

impl str::FromStr for Uri {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut s = s.to_string();
        remove_spaces(&mut s);

        let (scheme, mut uri_part) = get_chunks(&s, ":");

        let scheme = match scheme {
            Some(s) => s.to_string(),
            None => return Err(Error::Parse(ParseErr::Empty)),
        };

        let mut authority = None;

        if let Some(u) = &uri_part {
            if u.contains("//") {
                let (auth, part) = get_chunks(&u[2..], "/");

                authority = match auth {
                    Some(a) => match a.parse() {
                        Ok(i) => Some(i),
                        Err(e) => return Err(Error::Parse(e)),
                    },
                    None => None,
                };

                uri_part = part;
            }
        }

        let (path, uri_part) = chunk(&uri_part, "?");

        let path = if authority.is_some() {
            path.and_then(|v| Some(format!("/{}", v)))
        } else {
            path.map(|s| s.to_string())
        };

        let (query, fragment) = chunk(&uri_part, "#");

        let query = query.map(|s| s.to_string());
        let fragment = fragment.map(|s| s.to_string());

        Ok(Uri {
            scheme,
            authority,
            path,
            query,
            fragment,
        })
    }
}

///Authority of Uri
///
///# Example
///```
///use http_req::uri::Authority;
///
///let auth: Authority = "user:info@foo.com:443".parse().unwrap();
///assert_eq!(auth.host(), Some("foo.com"));
///```
#[derive(Clone, Debug, PartialEq)]
pub struct Authority {
    username: Option<String>,
    password: Option<String>,
    host: Option<String>,
    port: Option<u16>,
}

impl Authority {
    ///Returns username of this `Authority`
    pub fn username(&self) -> Option<&str> {
        self.username.ref_in()
    }

    ///Returns password of this `Authority`
    pub fn password(&self) -> Option<&str> {
        self.password.ref_in()
    }

    ///Returns information about the user
    pub fn user_info(&self) -> Option<String> {
        let mut user_info = String::new();

        if let Some(name) = &self.username {
            user_info.push_str(&name);

            if self.password.is_some() {
                user_info.push(':');
            }
        }

        if let Some(pass) = &self.password {
            user_info.push_str(&pass);
        }

        if user_info.is_empty() {
            None
        } else {
            Some(user_info)
        }
    }

    ///Returns host of this `Authority`
    pub fn host(&self) -> Option<&str> {
        self.host.ref_in()
    }

    ///Returns port of this `Authority`
    pub fn port(&self) -> &Option<u16> {
        &self.port
    }
}

impl str::FromStr for Authority {
    type Err = ParseErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut s = s.to_string();
        remove_spaces(&mut s);

        let mut username = None;
        let mut password = None;

        let uri_part = if s.contains('@') {
            let (info, part) = get_chunks(&s, "@");

            let (name, pass) = chunk(&info, ":");
            username = name.map(|s| s.to_string());
            password = pass.map(|s| s.to_string());

            part
        } else {
            Some(&s[..])
        };

        let (host, uri_part) = chunk(&uri_part, ":");

        let host = host.map(|s| s.to_string());
        let port = match uri_part {
            Some(p) => Some(p.parse()?),
            None => None,
        };

        Ok(Authority {
            username,
            password,
            host,
            port,
        })
    }
}

impl fmt::Display for Authority {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let user_info = match self.user_info() {
            Some(ref u) => format!("{}@", u),
            None => "".to_string(),
        };

        let port = match self.port {
            Some(ref p) => format!(":{}", p),
            None => "".to_string(),
        };

        write!(f, "{}{}{}", user_info, self.host().unwrap_or(""), port)
    }
}

//Removes whitespace from `text`
fn remove_spaces(text: &mut String) {
    text.retain(|c| !c.is_whitespace());
}

//Splits `String` from `base` by `separator`. If `base` is `None`, it will return
//tuple consisting two `None` values.
fn chunk<'a>(base: &'a Option<&'a str>, separator: &'a str) -> (Option<&'a str>, Option<&'a str>) {
    match base {
        Some(ref u) => get_chunks(u, separator),
        None => (None, None),
    }
}

//Splits `s` by `separator`. If `separator` is found inside `s`, it will return two `Some` values
//consisting parts of splitted `&str`. If `separator` is at the end of `s` or it's not found,
//it will return tuple consisting `Some` with `s` inside and None.
fn get_chunks<'a>(s: &'a str, separator: &'a str) -> (Option<&'a str>, Option<&'a str>) {
    match s.find(separator) {
        Some(i) => {
            let (chunk, rest) = s.split_at(i);
            let rest = &rest[separator.len()..];
            let rest = if rest.is_empty() { None } else { Some(rest) };

            (Some(chunk), rest)
        }
        None => {
            if !s.is_empty() {
                (Some(s), None)
            } else {
                (None, None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_URIS: [&str; 4] = [
        "https://user:info@foo.com:12/bar/baz?query#fragment",
        "file:///C:/Users/User/Pictures/screenshot.png",
        "https://en.wikipedia.org/wiki/Hypertext_Transfer_Protocol",
        "mailto:John.Doe@example.com",
    ];

    const TEST_AUTH: [&str; 3] = [
        "user:info@foo.com:12",
        "en.wikipedia.org",
        "John.Doe@example.com",
    ];

    #[test]
    fn remove_space() {
        let mut text = String::from("Hello World     !");
        let expect = String::from("HelloWorld!");

        remove_spaces(&mut text);
        assert_eq!(text, expect);
    }

    #[test]
    fn uri_full_parse() {
        let uri = "abc://username:password@example.com:123/path/data?key=value&key2=value2#fragid1"
            .parse::<Uri>()
            .unwrap();
        assert_eq!(uri.scheme(), "abc");

        assert_eq!(uri.user_info(), Some("username:password".to_string()));
        assert_eq!(uri.host(), Some("example.com"));
        assert_eq!(uri.port(), &Some(123));

        assert_eq!(uri.path(), Some("/path/data"));
        assert_eq!(uri.query(), Some("key=value&key2=value2"));
        assert_eq!(uri.fragment(), Some("fragid1"));
    }

    #[test]
    fn uri_parse() {
        for uri in TEST_URIS.iter() {
            uri.parse::<Uri>().unwrap();
        }
    }

    #[test]
    fn uri_scheme() {
        let uris: Vec<_> = TEST_URIS
            .iter()
            .map(|uri| uri.parse::<Uri>().unwrap())
            .collect();

        assert_eq!(uris[0].scheme(), "https");
        assert_eq!(uris[1].scheme(), "file");
        assert_eq!(uris[2].scheme(), "https");
        assert_eq!(uris[3].scheme(), "mailto");
    }

    #[test]
    fn uri_uesr_info() {
        let uris: Vec<_> = TEST_URIS
            .iter()
            .map(|uri| uri.parse::<Uri>().unwrap())
            .collect();

        assert_eq!(uris[0].user_info(), Some("user:info".to_string()));
        assert_eq!(uris[1].user_info(), None);
        assert_eq!(uris[2].user_info(), None);
        assert_eq!(uris[3].user_info(), None);
    }

    #[test]
    fn uri_host() {
        let uris: Vec<_> = TEST_URIS
            .iter()
            .map(|uri| uri.parse::<Uri>().unwrap())
            .collect();

        assert_eq!(uris[0].host(), Some("foo.com"));
        assert_eq!(uris[1].host(), None);
        assert_eq!(uris[2].host(), Some("en.wikipedia.org"));
        assert_eq!(uris[3].host(), None);
    }

    #[test]
    fn uri_port() {
        let uris: Vec<_> = TEST_URIS
            .iter()
            .map(|uri| uri.parse::<Uri>().unwrap())
            .collect();

        assert_eq!(uris[0].port(), &Some(12));

        for uri in uris.iter().skip(1) {
            assert_eq!(uri.port(), &None);
        }
    }

    #[test]
    fn uri_corr_port() {
        let uris: Vec<_> = TEST_URIS
            .iter()
            .map(|uri| uri.parse::<Uri>().unwrap())
            .collect();

        assert_eq!(uris[0].corr_port(), 12);
        assert_eq!(uris[1].corr_port(), HTTP_PORT);
        assert_eq!(uris[2].corr_port(), HTTPS_PORT);
        assert_eq!(uris[3].corr_port(), HTTP_PORT);
    }

    #[test]
    fn uri_path() {
        let uris: Vec<_> = TEST_URIS
            .iter()
            .map(|uri| uri.parse::<Uri>().unwrap())
            .collect();

        assert_eq!(uris[0].path(), Some("/bar/baz"));
        assert_eq!(
            uris[1].path(),
            Some("/C:/Users/User/Pictures/screenshot.png")
        );
        assert_eq!(uris[2].path(), Some("/wiki/Hypertext_Transfer_Protocol"));
        assert_eq!(uris[3].path(), Some("John.Doe@example.com"));
    }

    #[test]
    fn uri_query() {
        let uris: Vec<_> = TEST_URIS
            .iter()
            .map(|uri| uri.parse::<Uri>().unwrap())
            .collect();

        assert_eq!(uris[0].query(), Some("query"));

        for i in 1..3 {
            assert_eq!(uris[i].query(), None);
        }
    }

    #[test]
    fn uri_fragment() {
        let uris: Vec<_> = TEST_URIS
            .iter()
            .map(|uri| uri.parse::<Uri>().unwrap())
            .collect();

        assert_eq!(uris[0].fragment(), Some("fragment"));

        for i in 1..3 {
            assert_eq!(uris[i].fragment(), None);
        }
    }

    #[test]
    fn uri_resource() {
        let uris: Vec<_> = TEST_URIS
            .iter()
            .map(|uri| uri.parse::<Uri>().unwrap())
            .collect();

        assert_eq!(uris[0].resource(), "/bar/baz?query#fragment");
        assert_eq!(uris[1].resource(), "/C:/Users/User/Pictures/screenshot.png");
        assert_eq!(uris[2].resource(), "/wiki/Hypertext_Transfer_Protocol");
        assert_eq!(uris[3].resource(), "John.Doe@example.com");
    }

    #[test]
    fn uri_display() {
        let uris: Vec<_> = TEST_URIS
            .iter()
            .map(|uri| uri.parse::<Uri>().unwrap())
            .collect();

        for i in 0..uris.len() {
            let s = uris[i].to_string();
            assert_eq!(s, TEST_URIS[i]);
        }
    }

    #[test]
    fn authority_username() {
        let auths: Vec<_> = TEST_AUTH
            .iter()
            .map(|auth| auth.parse::<Authority>().unwrap())
            .collect();

        assert_eq!(auths[0].username(), Some("user"));
        assert_eq!(auths[1].username(), None);
        assert_eq!(auths[2].username(), Some("John.Doe"));
    }

    #[test]
    fn authority_password() {
        let auths: Vec<_> = TEST_AUTH
            .iter()
            .map(|auth| auth.parse::<Authority>().unwrap())
            .collect();

        assert_eq!(auths[0].password(), Some("info"));
        assert_eq!(auths[1].password(), None);
        assert_eq!(auths[2].password(), None);
    }

    #[test]
    fn authority_host() {
        let auths: Vec<_> = TEST_AUTH
            .iter()
            .map(|auth| auth.parse::<Authority>().unwrap())
            .collect();

        assert_eq!(auths[0].host(), Some("foo.com"));
        assert_eq!(auths[1].host(), Some("en.wikipedia.org"));
        assert_eq!(auths[2].host(), Some("example.com"));
    }

    #[test]
    fn authority_port() {
        let auths: Vec<_> = TEST_AUTH
            .iter()
            .map(|auth| auth.parse::<Authority>().unwrap())
            .collect();

        assert_eq!(auths[0].port(), &Some(12));
        assert_eq!(auths[1].port(), &None);
        assert_eq!(auths[2].port(), &None);
    }

    #[test]
    fn authority_from_str() {
        for auth in TEST_AUTH.iter() {
            auth.parse::<Authority>().unwrap();
        }
    }

    #[test]
    fn authority_display() {
        let auths: Vec<_> = TEST_AUTH
            .iter()
            .map(|auth| auth.parse::<Authority>().unwrap())
            .collect();

        for i in 0..auths.len() {
            let s = auths[i].to_string();
            assert_eq!(s, TEST_AUTH[i]);
        }
    }
}
