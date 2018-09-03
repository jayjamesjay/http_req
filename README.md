# http_req
[![Build Status](https://travis-ci.org/jayjamesjay/http_req.svg?branch=master)](https://travis-ci.org/jayjamesjay/http_req)
[![Crates.io](https://img.shields.io/badge/crates.io-v0.1.2-orange.svg?longCache=true)](https://crates.io/crates/http_req)

Simple HTTP client with built-in HTTPS support.
Currently it's in heavy development and may frequently change.

## Example
Basic GET request
```rust
extern crate http_req;
use http_req::request;

fn main() {
    let res = request::get("https://doc.rust-lang.org/").unwrap();

    println!("Status: {} {}", res.status_code(), res.reason());
}
```

## License
Licensed under [MIT](https://github.com/jayjamesjay/http_req/blob/master/LICENSE).
