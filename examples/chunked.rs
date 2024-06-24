use http_req::request;

fn main() {
    //Sends a HTTP GET request and processes the response.
    let mut body = Vec::new();
    let res = request::get("https://jigsaw.w3.org/HTTP/ChunkedScript", &mut body).unwrap();

    //Prints details about the response.
    println!("Status: {} {}", res.status_code(), res.reason());
    println!("Headers: {}", res.headers());
    //println!("{}", String::from_utf8_lossy(&body));
}
