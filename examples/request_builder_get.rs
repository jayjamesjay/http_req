use http_req::{
    request::RequestBuilder,
    response::Response,
    stream::{self, Stream},
    uri::Uri,
};
use std::{
    convert::TryFrom,
    io::{BufReader, Read, Write},
    time::Duration,
};

fn main() {
    // Parses a URI and assigns it to a variable `addr`.
    let addr: Uri = Uri::try_from("https://www.rust-lang.org/learn").unwrap();

    // Containers for a server's response.
    let raw_head;
    let mut body = Vec::new();

    // Prepares a request message.
    let request_msg = RequestBuilder::new(&addr)
        .header("Connection", "Close")
        .parse();

    // Connects to a server. Uses information from `addr`.
    let mut stream = Stream::new(&addr, Some(Duration::from_secs(60))).unwrap();
    stream = Stream::try_to_https(stream, &addr, None).unwrap();

    // Makes a request to server. Sends the prepared message.
    stream.write_all(&request_msg).unwrap();

    // Wraps the stream in BufReader to make it easier to read from it.
    // Reads a response from the server and saves the head to `raw_head`, and the body to `body`.
    let mut stream = BufReader::new(stream);
    raw_head = stream::read_head(&mut stream);
    stream.read_to_end(&mut body).unwrap();

    // Parses and processes the response.
    let response = Response::from_head(&raw_head).unwrap();

    // Prints infromation about the response.
    println!("Status: {} {}", response.status_code(), response.reason());
    println!("Headers: {}", response.headers());
    //println!("{}", String::from_utf8_lossy(&body));
}
