//! Simple HTTP client with built-in HTTPS support.
//!
//! By default uses [rust-native-tls](https://github.com/sfackler/rust-native-tls),
//! which relies on TLS framework provided by OS on Windows and macOS, and OpenSSL
//! on all other platforms. But it also supports [rus-tls](https://crates.io/crates/rustls).
//!
//! ## Examples
//! Basic GET request
//! ```
//! use http_req::request;
//!
//! fn main() {
//!     // Container for body of a response
//!     let mut body = Vec::new();
//!     let res = request::get("https://doc.rust-lang.org/", &mut body).unwrap();
//!
//!     println!("Status: {} {}", res.status_code(), res.reason());
//! }
//! ```

pub mod chunked;
pub mod error;
pub mod request;
pub mod response;
pub mod stream;
#[cfg(any(feature = "native-tls", feature = "rust-tls"))]
pub mod tls;
pub mod uri;

pub(crate) const CR_LF: &[u8; 2] = b"\r\n";
pub(crate) const LF: u8 = 0xA;
