//!error system
use std::{error, fmt, io, num, str};

#[derive(Debug, PartialEq)]
pub enum ParseErr {
    Utf8(str::Utf8Error),
    Int(num::ParseIntError),
    StatusErr,
    HeadersErr,
    UriErr,
    Invalid,
    Empty,
}

impl error::Error for ParseErr {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        use self::ParseErr::*;

        match self {
            Utf8(e) => Some(e),
            Int(e) => Some(e),
            StatusErr | HeadersErr | UriErr | Invalid | Empty => None,
        }
    }
}

impl fmt::Display for ParseErr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::ParseErr::*;

        let err = match self {
            Utf8(_) => "invalid character",
            Int(_) => "cannot parse number",
            Invalid => "invalid value",
            Empty => "nothing to parse",
            StatusErr => "status line contains invalid values",
            HeadersErr => "headers contain invalid values",
            UriErr => "uri contains invalid characters",
        };
        write!(f, "ParseErr: {}", err)
    }
}

impl From<num::ParseIntError> for ParseErr {
    fn from(e: num::ParseIntError) -> Self {
        ParseErr::Int(e)
    }
}

impl From<str::Utf8Error> for ParseErr {
    fn from(e: str::Utf8Error) -> Self {
        ParseErr::Utf8(e)
    }
}

#[derive(Debug)]
pub enum Error {
    IO(io::Error),
    Parse(ParseErr),
    Tls,
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        use self::Error::*;

        match self {
            IO(e) => Some(e),
            Parse(e) => Some(e),
            Tls => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Error::*;

        let err = match self {
            IO(_) => "IO error",
            Tls => "TLS error",
            Parse(err) => return err.fmt(f),
        };
        write!(f, "Error: {}", err)
    }
}

#[cfg(feature = "native-tls")]
impl From<native_tls::Error> for Error {
    fn from(_e: native_tls::Error) -> Self {
        Error::Tls
    }
}

#[cfg(feature = "native-tls")]
impl<T> From<native_tls::HandshakeError<T>> for Error {
    fn from(_e: native_tls::HandshakeError<T>) -> Self {
        Error::Tls
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::IO(e)
    }
}

impl From<ParseErr> for Error {
    fn from(e: ParseErr) -> Self {
        Error::Parse(e)
    }
}

impl From<str::Utf8Error> for Error {
    fn from(e: str::Utf8Error) -> Self {
        Error::Parse(ParseErr::Utf8(e))
    }
}
