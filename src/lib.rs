//!Simple HTTP client with built-in HTTPS support.
//!Currently it's in heavy development and may frequently change.
//!
//!## Example
//!Basic GET request
//!```
//!use http_req::request;
//!
//!fn main() {
//!    //Container for body of a response   
//!    let mut body = Vec::new();
//!    let res = request::get("https://doc.rust-lang.org/", &mut body).unwrap();
//!
//!    println!("Status: {} {}", res.status_code(), res.reason());
//!}
//!```
pub mod uri;
pub mod request;
pub mod response;
pub mod stream;
pub mod chunked;
pub mod tls;
pub mod error;
