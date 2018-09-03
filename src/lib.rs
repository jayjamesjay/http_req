//!Simple HTTP client with built-in HTTPS support.
//!Currently it's in heavy development and may frequently change.
//!
//!## Example
//!Basic GET request
//!```
//!extern crate http_req;
//!use http_req::request;
//!
//!fn main() {
//!    let res = request::get("https://doc.rust-lang.org/").unwrap();
//!
//!    println!("Status: {} {}", res.status_code(), res.reason());
//!}
//!```
extern crate native_tls;
use std::{collections::HashMap, error::Error, fmt, num::ParseIntError};

pub mod request;
pub mod response;
pub mod url;
