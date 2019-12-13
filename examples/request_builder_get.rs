use http_req::{request::RequestBuilder, tls, uri::Uri};
use std::net::TcpStream;
use std::fs::File;
use http_req::request::Method;
use std::io::Read;

fn main() {
    //Parse uri and assign it to variable `addr`
    let addr: Uri = "https://qualiflps.services-ps.ameli.fr/lps".parse().unwrap();

    //Connect to remote host
    let stream = TcpStream::connect((addr.host().unwrap(), addr.corr_port())).unwrap();

    //Open secure connection over TlsStream, because of `addr` (https)
    let mut stream = tls::Config::default()
        .connect(addr.host().unwrap_or(""), stream)
        .unwrap();

    //Container for response's body
    let mut writer = Vec::new();

    //Add header `Connection: Close`



    let mut f = File::open("/Users/cedric.couton/dev/billeo-engine/WSTest.xml").unwrap();
    let mut content = Vec::new();
    f.read_to_end(&mut content).unwrap();
    let content = content.as_slice();
    println!("content length : {}", content.len());

    let mut request_builder = RequestBuilder::new(&addr);
    let request= request_builder.method(Method::POST)
        //.header("Connection", "Close")
        //.header("Host", "qualiflps.services-ps.ameli.fr")
        .header("Content-Length", &format!("{}", 7062))
        .header("User-Agent", "reqwest/0.10.0-alpha.2")
        .header("accept", "*/*")
    //"user-agent": "reqwest/0.10.0-alpha.2", "accept": "*/*"
        //.header("Content-Type", "application/soap+xml")
        .body(content);

    println!("request : {}", &String::from_utf8_lossy(&request.clone().parse_msg()));


     let response =   request.send(&mut stream, &mut writer)
        .unwrap();


    println!("Status: {} {}", response.status_code(), response.reason());
    //println!("{:?}", writer);
    println!("{}", String::from_utf8_lossy(&writer));
}
