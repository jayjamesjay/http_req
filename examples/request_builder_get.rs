use http_req::{request::RequestBuilder, response::Response, stream::Stream, uri::Uri};
use std::{
    convert::TryFrom,
    io::{BufRead, BufReader, Read, Write},
    time::Duration,
};

fn main() {
    //Parses a URI and assigns it to a variable `addr`.
    let addr: Uri = Uri::try_from("https://www.rust-lang.org/learn").unwrap();

    //Containers for a server's response.
    let mut raw_head = Vec::new();
    let mut body = Vec::new();

    //Prepares a request message.
    let request_msg = RequestBuilder::new(&addr)
        .header("Connection", "Close")
        .parse();

    println!("{:?}", String::from_utf8(request_msg.clone()));

    //Connects to a server. Uses information from `addr`.
    let mut stream = Stream::new(&addr, Some(Duration::from_secs(60))).unwrap();
    stream = Stream::try_to_https(stream, &addr, None).unwrap();

    //Makes a request to server - sends a prepared message.
    stream.write_all(&request_msg).unwrap();

    //Wraps the stream in BufReader to make it easier to read from it.
    //Reads a response from the server and saves the head to `raw_head`, and the body to `body`.
    let mut stream = BufReader::new(stream);
    loop {
        match stream.read_until(0xA, &mut raw_head) {
            Ok(0) | Err(_) => break,
            Ok(len) => {
                let full_len = raw_head.len();

                if len == 2 && &raw_head[full_len - 2..] == b"\r\n" {
                    break;
                }
            }
        }
    }
    stream.read_to_end(&mut body).unwrap();

    //Parses and processes the response.
    let response = Response::from_head(&raw_head).unwrap();

    //Prints infromation about the response.
    println!("Status: {} {}", response.status_code(), response.reason());
    println!("Headers: {}", response.headers());
    //println!("{}", String::from_utf8_lossy(&body));
}
