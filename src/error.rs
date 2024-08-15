//! error system used around the library.
use std::{error, fmt, io, num, str, sync::mpsc};

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
            Utf8(_) => "Invalid character",
            Int(_) => "Cannot parse number",
            Invalid => "Invalid value",
            Empty => "Nothing to parse",
            StatusErr => "Status line contains invalid values",
            HeadersErr => "Headers contain invalid values",
            UriErr => "URI contains invalid characters",
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
    Timeout,
    Tls,
    Thread,
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        use self::Error::*;

        match self {
            IO(e) => Some(e),
            Parse(e) => Some(e),
            Timeout | Tls | Thread => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Error::*;

        let err = match self {
            IO(_) => "IO error",
            Parse(err) => return err.fmt(f),
            Timeout => "Timeout error",
            Tls => "TLS error",
            Thread => "Thread communication error",
        };
        write!(f, "Error: {}", err)
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

impl From<mpsc::RecvTimeoutError> for Error {
    fn from(_e: mpsc::RecvTimeoutError) -> Self {
        Error::Timeout
    }
}

#[cfg(feature = "rust-tls")]
impl From<rustls::Error> for Error {
    fn from(_e: rustls::Error) -> Self {
        Error::Tls
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

impl<T> From<mpsc::SendError<T>> for Error {
    fn from(_e: mpsc::SendError<T>) -> Self {
        Error::Thread
    }
}
