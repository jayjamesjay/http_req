use http_req::request;

fn main() {
    // Container for body of a response.
    let mut res_body = Vec::new();

    // Body of a request.
    const REQ_BODY: &[u8; 27] = b"field1=value1&field2=value2";

    // Sends a HTTP POST request and processes the response.
    let res = request::post("https://httpbin.org/post", REQ_BODY, &mut res_body).unwrap();

    // Prints details about the response.
    println!("Status: {} {}", res.status_code(), res.reason());
    println!("Headers: {}", res.headers());
    //println!("{}", String::from_utf8_lossy(&res_body));
}
