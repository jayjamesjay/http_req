//!Simple HTTP client with built-in HTTPS support.
//!Currently it's in heavy development and may frequently change.
//!
//!## Example
//!Basic GET request
//!```
//!use http_req::request;
//!
//!fn main() {
//!    let mut buff = Vec::new();
//!    let res = request::get("https://doc.rust-lang.org/", &mut buff).unwrap();
//!
//!    println!("Status: {} {}", res.status_code(), res.reason());
//!}
//!```
pub mod error;
pub mod request;
pub mod response;
pub mod url;
