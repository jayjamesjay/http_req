use http_req::{request::RequestBuilder, uri::Uri};
use native_tls::TlsConnector;
use std::net::TcpStream;

fn main() {
    //Parse uri and assign it to variable `addr`
    let addr: Uri = "https://doc.rust-lang.org/".parse().unwrap();

    //Connect to remote host
    let stream = TcpStream::connect((addr.host().unwrap(), addr.corr_port())).unwrap();

    //Open secure connection over TlsStream, because of `addr` (https)
    let connector = TlsConnector::new().unwrap();
    let mut stream = connector.connect(addr.host().unwrap(), stream).unwrap();

    //Container for response's body
    let mut writer = Vec::new();

    //Add header `Connection: Close`
    let response = RequestBuilder::new(&addr)
        .header("Connection", "Close")
        .send(&mut stream, &mut writer)
        .unwrap();

    println!("Status: {} {}", response.status_code(), response.reason());
    //println!("{}", String::from_utf8_lossy(&writer));
}
