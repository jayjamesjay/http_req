use http_req::{request::RequestBuilder, tls, uri::Uri};
use std::{convert::TryFrom, net::TcpStream};

fn main() {
    //Parses a URI and assigns it to a variable `addr`.
    let addr: Uri = Uri::try_from("https://www.rust-lang.org/learn").unwrap();

    //Connects to a remote host. Uses information from `addr`.
    let stream = TcpStream::connect((addr.host().unwrap(), addr.corr_port())).unwrap();

    //Opens a secure connection over TlsStream. This is required due to use of `https` protocol.
    let mut stream = tls::Config::default()
        .connect(addr.host().unwrap_or(""), stream)
        .unwrap();

    //Container for a response's body.
    let mut writer = Vec::new();

    //Adds a header `Connection: Close`.
    let response = RequestBuilder::new(&addr)
        .header("Connection", "Close")
        .send(&mut stream, &mut writer)
        .unwrap();

    println!("Status: {} {}", response.status_code(), response.reason());
    println!("Headers: {}", response.headers());
    //println!("{}", String::from_utf8_lossy(&writer));
}
