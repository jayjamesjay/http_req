use http_req::request;

fn main() {
    //Sends a HTTP HEAD request and processes the response.
    let res = request::head("https://www.rust-lang.org/learn").unwrap();

    //Prints details about the response.
    println!("Status: {} {}", res.status_code(), res.reason());
    println!("Headers: {}", res.headers());
}
