# http_req

[![Rust](https://github.com/jayjamesjay/http_req/actions/workflows/rust.yml/badge.svg)](https://github.com/jayjamesjay/http_req/actions/workflows/rust.yml)
[![Crates.io](https://img.shields.io/badge/crates.io-v0.12.0-orange.svg?longCache=true)](https://crates.io/crates/http_req)
[![Docs.rs](https://docs.rs/http_req/badge.svg)](https://docs.rs/http_req/0.12.0/http_req/)

Simple and lightweight HTTP client with built-in HTTPS support.

- HTTP and HTTPS via [rust-native-tls](https://github.com/sfackler/rust-native-tls) (or optionally [rus-tls](https://crates.io/crates/rustls))
- Small binary size (0.7 MB for basic GET request in default configuration)
- Minimal amount of dependencies

## Requirements

http_req by default uses [rust-native-tls](https://github.com/sfackler/rust-native-tls),
which relies on TLS framework provided by OS on Windows and macOS, and OpenSSL
on all other platforms. But it also supports [rus-tls](https://crates.io/crates/rustls).

## Example

Basic HTTP GET request

```rust
use http_req::request;

fn main() {
    let mut body = Vec::new(); //Container for body of a response.
    let res = request::get("https://doc.rust-lang.org/", &mut body).unwrap();

    println!("Status: {} {}", res.status_code(), res.reason());
}
```

Take a look at [more examples](https://github.com/jayjamesjay/http_req/tree/master/examples)

## Usage

### Default configuration

In order to use `http_req` with default configuration, add the following lines to `Cargo.toml`:

```toml
[dependencies]
http_req = "^0.13"
```

### Rustls

In order to use `http_req` with `rustls` in your project, add the following lines to `Cargo.toml`:

```toml
[dependencies]
http_req = { version="^0.13", default-features = false, features = ["rust-tls"] }
```

## License

Licensed under [MIT](https://github.com/jayjamesjay/http_req/blob/master/LICENSE).
