//! error system used around the library.

use std::{error, fmt, io, num, str, sync::mpsc};

/// Enum representing different parsing errors encountered by the library.
#[derive(Debug, PartialEq)]
pub enum ParseErr {
    /// Error related to invalid UTF-8 character sequences encountered
    /// during string processing or conversion operations.
    Utf8(str::Utf8Error),

    /// Failure in parsing integer values from strings using standard
    /// number formats, such as those conforming to base 10 conventions.
    Int(num::ParseIntError),

    /// Issue encountered when processing status line from HTTP response message.
    StatusErr,

    /// Issue encountered when processing headers from HTTP response message.
    HeadersErr,

    /// Issue arising while processing URIs that contain invalid
    /// characters or do not follow the URI specification.
    UriErr,

    /// Error indicating that provided string, vector, or other element
    /// does not contain any values that could be parsed.
    Empty,
}

impl error::Error for ParseErr {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        use self::ParseErr::*;

        match self {
            Utf8(e) => Some(e),
            Int(e) => Some(e),
            _ => None,
        }
    }
}

impl fmt::Display for ParseErr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::ParseErr::*;

        let err = match self {
            Utf8(_) => "Invalid character sequence",
            Int(_) => "Cannot parse number",
            StatusErr => "Status line contains invalid values",
            HeadersErr => "Headers contain invalid values",
            UriErr => "URI contains invalid characters",
            Empty => "Nothing to parse",
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

/// Enum representing various errors encountered by the library.
#[derive(Debug)]
pub enum Error {
    /// IO error that occurred during file operations,
    /// network connections, or any other type of I/O operation.
    IO(io::Error),

    /// Error encountered while parsing data using the library's functions.
    Parse(ParseErr),

    /// Timeout error, indicating that an operation timed out
    /// after waiting for the specified duration.
    Timeout,

    /// Error encountered while using TLS/SSL cryptographic protocols,
    /// such as establishing secure connections with servers.
    Tls,

    /// Thread-related communication error, signifying an issue
    /// that occurred during inter-thread communication.
    Thread,
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        use self::Error::*;

        match self {
            IO(e) => Some(e),
            Parse(e) => Some(e),
            _ => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Error::*;

        let err = match self {
            IO(e) => &format!("IO Error - {}", e),
            Parse(e) => return e.fmt(f),
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
