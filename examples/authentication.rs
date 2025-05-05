use http_req::{
    request::{Authentication, Request},
    uri::Uri,
};

fn main() {
    // Container for body of a response.
    let mut body = Vec::new();
    // URL of the website.
    let uri = Uri::try_from("https://httpbin.org/basic-auth/foo/bar").unwrap();
    // Authentication details: username and password.
    let auth = Authentication::basic("foo", "bar");

    // Sends a HTTP GET request and processes the response. Saves body of the response to `body` variable.
    let res = Request::new(&uri)
        .authentication(auth)
        .send(&mut body)
        .unwrap();

    //Prints details about the response.
    println!("Status: {} {}", res.status_code(), res.reason());
    println!("Headers: {}", res.headers());
    println!("{}", String::from_utf8_lossy(&body));
}
