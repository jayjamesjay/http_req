# http_req
[![Rust](https://github.com/jayjamesjay/http_req/actions/workflows/rust.yml/badge.svg)](https://github.com/jayjamesjay/http_req/actions/workflows/rust.yml)
[![Crates.io](https://img.shields.io/badge/crates.io-v0.8.1-orange.svg?longCache=true)](https://crates.io/crates/http_req)
[![Docs.rs](https://docs.rs/http_req/badge.svg)](https://docs.rs/http_req/0.7.2/http_req/)

Simple and lightweight HTTP client with `[w13e_wasi_socket]`.

## Example
Basic GET request
```rust
use http_req::request;

fn main() {
    let mut writer = Vec::new(); //container for body of a response
    let res = request::get("http://127.0.0.1/", &mut writer).unwrap();

    println!("Status: {} {}", res.status_code(), res.reason());
}
```

## How to use:
```toml
[dependencies]
http_req  = { git = "https://github.com/L-jasmine/http_req" }
```

## License
Licensed under [MIT](https://github.com/L-jasmine/http_req/blob/master/LICENSE).
