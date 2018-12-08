use http_req::request;

fn main() {
    let mut buffer = Vec::new();
    let res = request::get("https://doc.rust-lang.org/", &mut buffer).unwrap();

    println!("Status: {} {}", res.status_code(), res.reason());
    println!("{:?}", res.headers());
    //println!("{}", String::from_utf8_lossy(res.body()));
}
