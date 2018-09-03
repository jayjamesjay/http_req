extern crate http_req;
use http_req::request;

fn main() {
    let res = request::get("https://doc.rust-lang.org/").unwrap();

    println!("Status: {} {}", res.status_code(), res.reason());
}
