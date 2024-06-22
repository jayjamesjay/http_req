use http_req::{
    request::RequestBuilder,
    response::{find_slice, Response},
    stream::Stream,
    uri::Uri,
};
use std::{
    convert::TryFrom,
    io::{Read, Write},
    time::Duration,
};

fn main() {
    //Parses a URI and assigns it to a variable `addr`.
    let addr: Uri = Uri::try_from("https://www.rust-lang.org/learn").unwrap();

    //Prepare a request
    let mut request_builder = RequestBuilder::new(&addr);
    request_builder.header("Connection", "Close");

    //Container for a server's response.
    let mut writer = Vec::new();

    //Connects to a remote host. Uses information from `addr`.
    let mut stream = Stream::new(&addr, Some(Duration::from_secs(60))).unwrap();
    stream = Stream::try_to_https(stream, &addr, None).unwrap();

    //Generate a request (message) and send it to server.
    let request_msg = request_builder.parse_msg();
    stream.write_all(&request_msg).unwrap();

    //Read response from the server and save it to writer
    stream.read_to_end(&mut writer).unwrap();

    //Parse and process response.
    let pos = find_slice(&writer, &[13, 10, 13, 10].to_owned()).unwrap();
    let response = Response::from_head(&writer[..pos]).unwrap();
    let body = writer[pos..].to_vec();

    //Print infromation about the response.
    println!("Status: {} {}", response.status_code(), response.reason());
    println!("Headers: {}", response.headers());
    //println!("{}", String::from_utf8_lossy(&body));
}
