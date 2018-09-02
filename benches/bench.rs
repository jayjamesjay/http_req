#![feature(test)]
extern crate http_req;
extern crate test;

use http_req::{Url, response::Response};
use std::{fs::File, io::Read};
use test::Bencher;

#[bench]
fn parse_response(b: &mut Bencher) {
    let mut buf = Vec::new();
    let mut response = File::open("benches/res.txt").unwrap();
    response.read_to_end(&mut buf).unwrap();

    b.iter(|| Response::try_from(&buf));
}

#[bench]
fn parse_url(b: &mut Bencher) {
    const URL: &str = "https://doc.rust-lang.org/stable/std/string/struct.String.html";

    b.iter(|| URL.parse::<Url>());
}
