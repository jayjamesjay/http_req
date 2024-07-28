//! uri operations
use crate::error::{Error, ParseErr};
use std::{
    convert::TryFrom,
    fmt,
    ops::{Index, Range},
    str,
    string::ToString,
};

const HTTP_PORT: u16 = 80;
const HTTPS_PORT: u16 = 443;

/// A (half-open) range bounded inclusively below and exclusively above (start..end) with `Copy`.
#[derive(Copy, Clone, Debug, PartialOrd, PartialEq)]
pub struct RangeC {
    pub start: usize,
    pub end: usize,
}

impl RangeC {
    /// Creates new `RangeC` with `start` and `end`.
    ///
    /// # Exmaples
    /// ```
    /// use http_req::uri::RangeC;
    ///
    /// const range: RangeC = RangeC::new(0, 20);
    /// ```
    pub const fn new(start: usize, end: usize) -> RangeC {
        RangeC { start, end }
    }
}

impl From<RangeC> for Range<usize> {
    fn from(range: RangeC) -> Range<usize> {
        Range {
            start: range.start,
            end: range.end,
        }
    }
}

impl Index<RangeC> for str {
    type Output = str;

    #[inline]
    fn index(&self, index: RangeC) -> &str {
        &self[..][Range::from(index)]
    }
}

impl Index<RangeC> for String {
    type Output = str;

    #[inline]
    fn index(&self, index: RangeC) -> &str {
        &self[..][Range::from(index)]
    }
}

/// Representation of Uniform Resource Identifier
///
/// # Example
/// ```
/// use http_req::uri::Uri;
/// use std::convert::TryFrom;
///
/// let uri: Uri = Uri::try_from("https://user:info@foo.com:12/bar/baz?query#fragment").unwrap();;
/// assert_eq!(uri.host(), Some("foo.com"));
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Uri<'a> {
    inner: &'a str,
    scheme: RangeC,
    authority: Option<Authority<'a>>,
    path: Option<RangeC>,
    query: Option<RangeC>,
    fragment: Option<RangeC>,
}

impl<'a> Uri<'a> {
    /// Returns a reference to the underlying &str.
    pub fn get_ref(&self) -> &str {
        self.inner
    }

    /// Returns scheme of this `Uri`.
    ///
    /// # Example
    /// ```
    /// use http_req::uri::Uri;
    /// use std::convert::TryFrom;
    ///
    /// let uri: Uri = Uri::try_from("https://user:info@foo.com:12/bar/baz?query#fragment").unwrap();;
    /// assert_eq!(uri.scheme(), "https");
    /// ```
    pub fn scheme(&self) -> &str {
        &self.inner[self.scheme]
    }

    /// Returns information about the user included in this `Uri`.
    ///
    /// # Example
    /// ```
    /// use http_req::uri::Uri;
    /// use std::convert::TryFrom;
    ///
    /// let uri: Uri = Uri::try_from("https://user:info@foo.com:12/bar/baz?query#fragment").unwrap();;
    /// assert_eq!(uri.user_info(), Some("user:info"));
    /// ```
    pub fn user_info(&self) -> Option<&str> {
        self.authority.as_ref().and_then(|a| a.user_info())
    }

    /// Returns host of this `Uri`.
    ///
    /// # Example
    /// ```
    /// use http_req::uri::Uri;
    /// use std::convert::TryFrom;
    ///
    /// let uri: Uri = Uri::try_from("https://user:info@foo.com:12/bar/baz?query#fragment").unwrap();;
    /// assert_eq!(uri.host(), Some("foo.com"));
    /// ```
    pub fn host(&self) -> Option<&str> {
        self.authority.as_ref().map(|a| a.host())
    }

    /// Returns host of this `Uri` to use in a header.
    ///
    /// # Example
    /// ```
    /// use http_req::uri::Uri;
    /// use std::convert::TryFrom;
    ///
    /// let uri: Uri = Uri::try_from("https://user:info@foo.com:12/bar/baz?query#fragment").unwrap();;
    /// assert_eq!(uri.host_header(), Some("foo.com:12".to_string()));
    /// ```
    pub fn host_header(&self) -> Option<String> {
        self.host().map(|h| match self.corr_port() {
            HTTP_PORT | HTTPS_PORT => h.to_string(),
            p => format!("{}:{}", h, p),
        })
    }

    /// Returns port of this `Uri`
    ///
    /// # Example
    /// ```
    /// use http_req::uri::Uri;
    /// use std::convert::TryFrom;
    ///
    /// let uri: Uri = Uri::try_from("https://user:info@foo.com:12/bar/baz?query#fragment").unwrap();;
    /// assert_eq!(uri.port(), Some(12));
    /// ```
    pub fn port(&self) -> Option<u16> {
        self.authority.as_ref().and_then(|a| a.port())
    }

    /// Returns port corresponding to this `Uri`.
    /// Returns default port if it hasn't been set in the uri.
    ///
    /// # Example
    /// ```
    /// use http_req::uri::Uri;
    /// use std::convert::TryFrom;
    ///
    /// let uri: Uri = Uri::try_from("https://user:info@foo.com:12/bar/baz?query#fragment").unwrap();;
    /// assert_eq!(uri.corr_port(), 12);
    /// ```
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

    /// Returns path of this `Uri`.
    ///
    /// # Example
    /// ```
    /// use http_req::uri::Uri;
    /// use std::convert::TryFrom;
    ///
    /// let uri: Uri = Uri::try_from("https://user:info@foo.com:12/bar/baz?query#fragment").unwrap();;
    /// assert_eq!(uri.path(), Some("/bar/baz"));
    /// ```
    pub fn path(&self) -> Option<&str> {
        self.path.map(|r| &self.inner[r])
    }

    /// Returns query of this `Uri`.
    ///
    /// # Example
    /// ```
    /// use http_req::uri::Uri;
    /// use std::convert::TryFrom;
    ///
    /// let uri: Uri = Uri::try_from("https://user:info@foo.com:12/bar/baz?query#fragment").unwrap();;
    /// assert_eq!(uri.query(), Some("query"));
    /// ```
    pub fn query(&self) -> Option<&str> {
        self.query.map(|r| &self.inner[r])
    }

    /// Returns fragment of this `Uri`.
    ///
    /// # Example
    /// ```
    /// use http_req::uri::Uri;
    /// use std::convert::TryFrom;
    ///
    /// let uri: Uri = Uri::try_from("https://user:info@foo.com:12/bar/baz?query#fragment").unwrap();;
    /// assert_eq!(uri.fragment(), Some("fragment"));
    /// ```
    pub fn fragment(&self) -> Option<&str> {
        self.fragment.map(|r| &self.inner[r])
    }

    /// Returns resource `Uri` points to.
    ///
    /// # Example
    /// ```
    /// use http_req::uri::Uri;
    /// use std::convert::TryFrom;
    ///
    /// let uri: Uri = Uri::try_from("https://user:info@foo.com:12/bar/baz?query#fragment").unwrap();;
    /// assert_eq!(uri.resource(), "/bar/baz?query#fragment");
    /// ```
    pub fn resource(&self) -> &str {
        match self.path {
            Some(p) => &self.inner[p.start..],
            None => "/",
        }
    }

    /// Checks if &str is an relative uri.
    pub fn is_relative(raw_uri: &str) -> bool {
        raw_uri.starts_with("/")
            || raw_uri.starts_with("?")
            || raw_uri.starts_with("#")
            || raw_uri.starts_with("@")
    }

    /// Creates a new `Uri` from current uri and relative uri.
    /// Transforms the relative uri into an absolute uri.
    pub fn from_relative(&'a self, relative_uri: &'a mut String) -> Result<Uri<'a>, Error> {
        let inner_uri = self.inner;
        let mut relative_part = relative_uri.as_ref();

        if inner_uri.ends_with("/") && relative_uri.starts_with("/") {
            relative_part = relative_uri.trim_start_matches("/");
        }

        *relative_uri = inner_uri.to_owned() + relative_part;
        Uri::try_from(relative_uri.as_str())
    }
}

impl<'a> fmt::Display for Uri<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut uri = self.inner.to_string();

        if let Some(auth) = &self.authority {
            let auth = auth.to_string();
            let start = self.scheme.end + 3;

            uri.replace_range(start..(start + auth.len()), &auth);
        }

        write!(f, "{}", uri)
    }
}

impl<'a> TryFrom<&'a str> for Uri<'a> {
    type Error = Error;

    fn try_from(s: &'a str) -> Result<Self, Self::Error> {
        let (scheme, mut uri_part) = get_chunks(&s, Some(RangeC::new(0, s.len())), ":");
        let scheme = scheme.ok_or(ParseErr::UriErr)?;
        let (mut authority, mut query, mut fragment) = (None, None, None);

        if let Some(u) = uri_part {
            if s[u].contains("//") {
                let (auth, part) = get_chunks(&s, Some(RangeC::new(u.start + 2, u.end)), "/");

                if let Some(a) = auth {
                    authority = Some(Authority::try_from(&s[a])?)
                };

                uri_part = part;
            }
        }

        if let Some(u) = uri_part {
            if &s[u.start - 1..u.start] == "/" {
                uri_part = Some(RangeC::new(u.start - 1, u.end));
            }
        }

        let mut path = uri_part;

        if let Some(u) = uri_part {
            if s[u].contains("?") && s[u].contains("#") {
                (path, uri_part) = get_chunks(&s, uri_part, "?");
                (query, fragment) = get_chunks(&s, uri_part, "#");
            } else if s[u].contains("?") {
                (path, query) = get_chunks(&s, uri_part, "?");
            } else if s[u].contains("#") {
                (path, fragment) = get_chunks(&s, uri_part, "#");
            }
        }

        Ok(Uri {
            inner: s,
            scheme,
            authority,
            path,
            query,
            fragment,
        })
    }
}

/// Authority of Uri
///
/// # Example
/// ```
/// use http_req::uri::Authority;
/// use std::convert::TryFrom;
///
/// let auth: Authority = Authority::try_from("user:info@foo.com:443").unwrap();
/// assert_eq!(auth.host(), "foo.com");
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Authority<'a> {
    inner: &'a str,
    username: Option<RangeC>,
    password: Option<RangeC>,
    host: RangeC,
    port: Option<RangeC>,
}

impl<'a> Authority<'a> {
    /// Returns username of this `Authority`
    ///
    /// # Example
    /// ```
    /// use http_req::uri::Authority;
    /// use std::convert::TryFrom;
    ///
    /// let auth: Authority = Authority::try_from("user:info@foo.com:443").unwrap();
    /// assert_eq!(auth.username(), Some("user"));
    /// ```
    pub fn username(&self) -> Option<&'a str> {
        self.username.map(|r| &self.inner[r])
    }

    /// Returns password of this `Authority`
    ///
    /// # Example
    /// ```
    /// use http_req::uri::Authority;
    /// use std::convert::TryFrom;
    ///
    /// let auth: Authority = Authority::try_from("user:info@foo.com:443").unwrap();
    /// assert_eq!(auth.password(), Some("info"));
    /// ```
    pub fn password(&self) -> Option<&str> {
        self.password.map(|r| &self.inner[r])
    }

    /// Returns information about the user
    ///
    /// # Example
    /// ```
    /// use http_req::uri::Authority;
    /// use std::convert::TryFrom;
    ///
    /// let auth: Authority = Authority::try_from("user:info@foo.com:443").unwrap();
    /// assert_eq!(auth.user_info(), Some("user:info"));
    /// ```
    pub fn user_info(&self) -> Option<&str> {
        match (&self.username, &self.password) {
            (Some(u), Some(p)) => Some(&self.inner[u.start..p.end]),
            (Some(u), None) => Some(&self.inner[*u]),
            _ => None,
        }
    }

    /// Returns host of this `Authority`
    ///
    /// # Example
    /// ```
    /// use http_req::uri::Authority;
    /// use std::convert::TryFrom;
    ///
    /// let auth: Authority = Authority::try_from("user:info@foo.com:443").unwrap();
    /// assert_eq!(auth.host(), "foo.com");
    /// ```
    pub fn host(&self) -> &str {
        &self.inner[self.host]
    }

    /// Returns port of this `Authority`
    ///
    /// # Example
    /// ```
    /// use http_req::uri::Authority;
    /// use std::convert::TryFrom;
    ///
    /// let auth: Authority = Authority::try_from("user:info@foo.com:443").unwrap();
    /// assert_eq!(auth.port(), Some(443));
    /// ```
    pub fn port(&self) -> Option<u16> {
        self.port.as_ref().map(|p| self.inner[*p].parse().unwrap())
    }
}

impl<'a> TryFrom<&'a str> for Authority<'a> {
    type Error = ParseErr;

    fn try_from(s: &'a str) -> Result<Self, Self::Error> {
        let (mut username, mut password) = (None, None);

        let uri_part = if s.contains('@') {
            let (info, part) = get_chunks(&s, Some(RangeC::new(0, s.len())), "@");
            (username, password) = get_chunks(&s, info, ":");

            part
        } else {
            Some(RangeC::new(0, s.len()))
        };

        let split_by = if s.contains('[') && s.contains(']') {
            "]:"
        } else {
            ":"
        };
        let (host, port) = get_chunks(&s, uri_part, split_by);
        let host = host.ok_or(ParseErr::UriErr)?;

        if let Some(p) = port {
            if s[p].parse::<u16>().is_err() {
                return Err(ParseErr::UriErr);
            }
        }

        Ok(Authority {
            inner: s,
            username,
            password,
            host,
            port,
        })
    }
}

impl<'a> fmt::Display for Authority<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut auth = self.inner.to_string();

        if let Some(pass) = self.password {
            let range = Range::from(pass);
            let hidden_pass = "*".repeat(range.len());

            auth.replace_range(range, &hidden_pass);
        }

        write!(f, "{}", auth)
    }
}

/// Removes whitespace from `text`
pub fn remove_spaces(text: &mut String) {
    text.retain(|c| !c.is_whitespace());
}

/// Splits `s` by `separator`. If `separator` is found inside `s`, it will return two `Some` values
/// consisting `RangeC` of each `&str`. If `separator` is at the end of `s` or it's not found,
/// it will return tuple consisting `Some` with `RangeC` of entire `s` inside and None.
fn get_chunks<'a>(
    s: &'a str,
    range: Option<RangeC>,
    separator: &'a str,
) -> (Option<RangeC>, Option<RangeC>) {
    let (mut before, mut after) = (None, None);

    if let Some(range) = range {
        match s[range].find(separator) {
            Some(i) => {
                let mid = range.start + i + separator.len();
                before = Some(RangeC::new(range.start, mid - 1)).filter(|r| r.start != r.end);
                after = Some(RangeC::new(mid, range.end)).filter(|r| r.start != r.end);
            }
            None => {
                if !s[range].is_empty() {
                    before = Some(range);
                }
            }
        }
    }

    (before, after)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_URIS: [&str; 7] = [
        "https://user:info@foo.com:12/bar/baz?query#fragment",
        "file:///C:/Users/User/Pictures/screenshot.png",
        "https://en.wikipedia.org/wiki/Hypertext_Transfer_Protocol",
        "mailto:John.Doe@example.com",
        "https://[4b10:bbb0:0:d0::ba7:8001]:443/",
        "http://example.com/?query=val",
        "https://example.com/#fragment",
    ];

    const TEST_AUTH: [&str; 4] = [
        "user:info@foo.com:12",
        "en.wikipedia.org",
        "John.Doe@example.com",
        "[4b10:bbb0:0:d0::ba7:8001]:443",
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
        let uri = Uri::try_from(
            "abc://username:password@example.com:123/path/data?key=value&key2=value2#fragid1",
        )
        .unwrap();
        assert_eq!(uri.scheme(), "abc");

        assert_eq!(uri.user_info(), Some("username:password"));
        assert_eq!(uri.host(), Some("example.com"));
        assert_eq!(uri.port(), Some(123));

        assert_eq!(uri.path(), Some("/path/data"));
        assert_eq!(uri.query(), Some("key=value&key2=value2"));
        assert_eq!(uri.fragment(), Some("fragid1"));
    }

    #[test]
    fn uri_parse() {
        for uri in TEST_URIS.iter() {
            Uri::try_from(*uri).unwrap();
        }
    }

    #[test]
    fn uri_scheme() {
        let uris: Vec<_> = TEST_URIS
            .iter()
            .map(|uri| Uri::try_from(*uri).unwrap())
            .collect();
        const RESULT: [&str; 7] = ["https", "file", "https", "mailto", "https", "http", "https"];

        for i in 0..RESULT.len() {
            assert_eq!(uris[i].scheme(), RESULT[i]);
        }
    }

    #[test]
    fn uri_uesr_info() {
        let uris: Vec<_> = TEST_URIS
            .iter()
            .map(|uri| Uri::try_from(*uri).unwrap())
            .collect();
        const RESULT: [Option<&str>; 7] = [Some("user:info"), None, None, None, None, None, None];

        for i in 0..RESULT.len() {
            assert_eq!(uris[i].user_info(), RESULT[i]);
        }
    }

    #[test]
    fn uri_host() {
        let uris: Vec<_> = TEST_URIS
            .iter()
            .map(|uri| Uri::try_from(*uri).unwrap())
            .collect();

        const RESULT: [Option<&str>; 7] = [
            Some("foo.com"),
            None,
            Some("en.wikipedia.org"),
            None,
            Some("[4b10:bbb0:0:d0::ba7:8001]"),
            Some("example.com"),
            Some("example.com"),
        ];

        for i in 0..RESULT.len() {
            assert_eq!(uris[i].host(), RESULT[i]);
        }
    }

    #[test]
    fn uri_host_header() {
        let uri_def: Uri =
            Uri::try_from("https://en.wikipedia.org:443/wiki/Hypertext_Transfer_Protocol").unwrap();
        let uris: Vec<_> = TEST_URIS
            .iter()
            .map(|uri| Uri::try_from(*uri).unwrap())
            .collect();

        assert_eq!(uris[0].host_header(), Some("foo.com:12".to_string()));
        assert_eq!(uris[2].host_header(), Some("en.wikipedia.org".to_string()));
        assert_eq!(uri_def.host_header(), Some("en.wikipedia.org".to_string()));
    }

    #[test]
    fn uri_port() {
        let uris: Vec<_> = TEST_URIS
            .iter()
            .map(|uri| Uri::try_from(*uri).unwrap())
            .collect();

        assert_eq!(uris[0].port(), Some(12));
        assert_eq!(uris[4].port(), Some(443));

        for i in 1..4 {
            assert_eq!(uris[i].port(), None);
        }

        for i in 5..7 {
            assert_eq!(uris[i].port(), None);
        }
    }

    #[test]
    fn uri_corr_port() {
        let uris: Vec<_> = TEST_URIS
            .iter()
            .map(|uri| Uri::try_from(*uri).unwrap())
            .collect();

        const RESULT: [u16; 7] = [
            12, HTTP_PORT, HTTPS_PORT, HTTP_PORT, HTTPS_PORT, HTTP_PORT, HTTPS_PORT,
        ];

        for i in 0..RESULT.len() {
            assert_eq!(uris[i].corr_port(), RESULT[i]);
        }
    }

    #[test]
    fn uri_path() {
        let uris: Vec<_> = TEST_URIS
            .iter()
            .map(|uri| Uri::try_from(*uri).unwrap())
            .collect();

        const RESULT: [Option<&str>; 7] = [
            Some("/bar/baz"),
            Some("/C:/Users/User/Pictures/screenshot.png"),
            Some("/wiki/Hypertext_Transfer_Protocol"),
            Some("John.Doe@example.com"),
            None,
            Some("/"),
            Some("/"),
        ];

        for i in 0..RESULT.len() {
            assert_eq!(uris[i].path(), RESULT[i]);
        }
    }

    #[test]
    fn uri_query() {
        let uris: Vec<_> = TEST_URIS
            .iter()
            .map(|uri| Uri::try_from(*uri).unwrap())
            .collect();

        const RESULT: [Option<&str>; 7] = [
            Some("query"),
            None,
            None,
            None,
            None,
            Some("query=val"),
            None,
        ];

        for i in 0..RESULT.len() {
            assert_eq!(uris[i].query(), RESULT[i]);
        }
    }

    #[test]
    fn uri_fragment() {
        let uris: Vec<_> = TEST_URIS
            .iter()
            .map(|uri| Uri::try_from(*uri).unwrap())
            .collect();

        const RESULT: [Option<&str>; 7] = [
            Some("fragment"),
            None,
            None,
            None,
            None,
            None,
            Some("fragment"),
        ];

        for i in 0..RESULT.len() {
            assert_eq!(uris[i].fragment(), RESULT[i]);
        }
    }

    #[test]
    fn uri_resource() {
        let uris: Vec<_> = TEST_URIS
            .iter()
            .map(|uri| Uri::try_from(*uri).unwrap())
            .collect();

        const RESULT: [&str; 7] = [
            "/bar/baz?query#fragment",
            "/C:/Users/User/Pictures/screenshot.png",
            "/wiki/Hypertext_Transfer_Protocol",
            "John.Doe@example.com",
            "/",
            "/?query=val",
            "/#fragment",
        ];

        for i in 0..RESULT.len() {
            assert_eq!(uris[i].resource(), RESULT[i]);
        }
    }

    #[test]
    fn uri_display() {
        let uris: Vec<_> = TEST_URIS
            .iter()
            .map(|uri| Uri::try_from(*uri).unwrap())
            .collect();

        assert_eq!(
            uris[0].to_string(),
            "https://user:****@foo.com:12/bar/baz?query#fragment"
        );

        for i in 1..uris.len() {
            let s = uris[i].to_string();
            assert_eq!(s, TEST_URIS[i]);
        }
    }

    #[test]
    fn authority_username() {
        let auths: Vec<_> = TEST_AUTH
            .iter()
            .map(|auth| Authority::try_from(*auth).unwrap())
            .collect();

        assert_eq!(auths[0].username(), Some("user"));
        assert_eq!(auths[1].username(), None);
        assert_eq!(auths[2].username(), Some("John.Doe"));
        assert_eq!(auths[3].username(), None);
    }

    #[test]
    fn authority_password() {
        let auths: Vec<_> = TEST_AUTH
            .iter()
            .map(|auth| Authority::try_from(*auth).unwrap())
            .collect();

        assert_eq!(auths[0].password(), Some("info"));
        assert_eq!(auths[1].password(), None);
        assert_eq!(auths[2].password(), None);
        assert_eq!(auths[3].password(), None);
    }

    #[test]
    fn authority_host() {
        let auths: Vec<_> = TEST_AUTH
            .iter()
            .map(|auth| Authority::try_from(*auth).unwrap())
            .collect();

        assert_eq!(auths[0].host(), "foo.com");
        assert_eq!(auths[1].host(), "en.wikipedia.org");
        assert_eq!(auths[2].host(), "example.com");
        assert_eq!(auths[3].host(), "[4b10:bbb0:0:d0::ba7:8001]");
    }

    #[test]
    fn authority_port() {
        let auths: Vec<_> = TEST_AUTH
            .iter()
            .map(|auth| Authority::try_from(*auth).unwrap())
            .collect();

        assert_eq!(auths[0].port(), Some(12));
        assert_eq!(auths[1].port(), None);
        assert_eq!(auths[2].port(), None);
        assert_eq!(auths[3].port(), Some(443));
    }

    #[test]
    fn authority_from_str() {
        for auth in TEST_AUTH.iter() {
            Authority::try_from(*auth).unwrap();
        }
    }

    #[test]
    fn authority_display() {
        let auths: Vec<_> = TEST_AUTH
            .iter()
            .map(|auth| Authority::try_from(*auth).unwrap())
            .collect();

        assert_eq!("user:****@foo.com:12", auths[0].to_string());

        for i in 1..auths.len() {
            let s = auths[i].to_string();
            assert_eq!(s, TEST_AUTH[i]);
        }
    }

    #[test]
    fn range_c_new() {
        assert_eq!(
            RangeC::new(22, 343),
            RangeC {
                start: 22,
                end: 343,
            }
        )
    }

    #[test]
    fn range_from_range_c() {
        assert_eq!(
            Range::from(RangeC::new(222, 43)),
            Range {
                start: 222,
                end: 43,
            }
        )
    }

    #[test]
    fn range_c_index() {
        const RANGE: RangeC = RangeC::new(0, 4);
        let text = TEST_AUTH[0].to_string();

        assert_eq!(text[..4], text[RANGE])
    }
}
