#![feature(test)]
extern crate http_req;
extern crate test;

use http_req::{request::RequestMessage, response::Response, uri::Uri};
use std::{convert::TryFrom, fs::File, io::Read};
use test::Bencher;

const URI: &str = "https://www.rust-lang.org/";
const BODY: [u8; 14] = [78, 97, 109, 101, 61, 74, 97, 109, 101, 115, 43, 74, 97, 121];

#[bench]
fn parse_uri(b: &mut Bencher) {
    b.iter(|| Uri::try_from(URI));
}

#[bench]
fn parse_request(b: &mut Bencher) {
    let uri = Uri::try_from(URI).unwrap();

    b.iter(|| {
        RequestMessage::new(&uri)
            .header("Accept", "*/*")
            .body(&BODY)
            .parse();
    });
}

#[bench]
fn parse_response(b: &mut Bencher) {
    let mut content = Vec::new();
    let mut response = File::open("benches/res.txt").unwrap();
    response.read_to_end(&mut content).unwrap();

    b.iter(|| {
        let mut body = Vec::new();
        Response::try_from(&content, &mut body)
    });
}
