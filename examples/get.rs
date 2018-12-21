use http_req::request;

fn main() {
    let mut writer = Vec::new();
    let res = request::get("https://doc.rust-lang.org/", &mut writer).unwrap();

    println!("Status: {} {}", res.status_code(), res.reason());
    println!("{:?}", res.headers());
    //println!("{}", String::from_utf8_lossy(&writer));
}
