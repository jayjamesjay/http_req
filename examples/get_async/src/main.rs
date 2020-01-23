use http_req::request;
use http_req::async_std;

#[async_std::main]
async fn main() {
    let mut writer = Vec::new(); //container for body of a response
    let res = request::get("https://www.rust-lang.org/learn?asdf=1", &mut writer).await.unwrap();

    println!("Status:\n{} {}", res.status_code(), res.reason());
    println!("Headers:\n{}", res.headers());
    println!("Body:\n{}", String::from_utf8_lossy(&writer));
}
