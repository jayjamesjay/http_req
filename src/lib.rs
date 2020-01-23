//!Simple HTTP client with built-in HTTPS support.
//!Currently it's in heavy development and may frequently change.
//!
//!## Example
//!Basic GET request
//!```
//!use http_req::request;
//!
//!fn main() {
//!    let mut writer = Vec::new(); //container for body of a response
//!    let res = request::get("https://doc.rust-lang.org/", &mut writer).unwrap();
//!
//!    println!("Status: {} {}", res.status_code(), res.reason());
//!}
//!```
pub mod error;
pub mod request;
pub mod response;
#[cfg(not(feature = "async"))]
pub mod tls;
pub mod uri;

#[cfg(feature = "async")]
pub use async_std;
