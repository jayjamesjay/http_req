extern crate http_req;

use http_req::request;

fn main() {
    const URL_S: &str = "https://doc.rust-lang.org/std/io/prelude/index.html";
    let res = request::get(URL_S).unwrap();
    println!("Status: {} {}\r\n", res.status_code(), res.reason());
    println!("Headers: {:?}\r\n", res.headers());
    println!("Content: {:?}", String::from_utf8_lossy(res.body()));
}
