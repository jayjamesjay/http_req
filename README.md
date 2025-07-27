# http_req

[![Rust](https://github.com/jayjamesjay/http_req/actions/workflows/rust.yml/badge.svg)](https://github.com/jayjamesjay/http_req/actions/workflows/rust.yml)
[![Crates.io](https://img.shields.io/badge/crates.io-v0.14.1-orange.svg?longCache=true)](https://crates.io/crates/http_req)
[![Docs.rs](https://docs.rs/http_req/badge.svg)](https://docs.rs/http_req/0.14.1/http_req/)

Simple and lightweight HTTP client with built-in HTTPS support.

- HTTP and HTTPS via [rust-native-tls](https://crates.io/crates/native-tls) (or optionally [rustls](https://crates.io/crates/rustls))
- Small binary size (0.7 MB for a basic GET request in the default configuratio)
- Minimal number of dependencies

## Requirements

http_req by default uses [rust-native-tls](https://crates.io/crates/native-tls),
which relies on TLS framework provided by OS on Windows and macOS, and OpenSSL
on all other platforms. But it also supports [rustls](https://crates.io/crates/rustls).

## All functionalities

- Support for both HTTP and HTTPS protocols via [rust-native-tls](https://crates.io/crates/native-tls) (or optionally [rustls](https://crates.io/crates/rustls))
- Creating and sending HTTP requests using the `Request` type (with extended capabilities provided via `RequestMessage` and `Stream`)
- Representing HTTP responses with the `Response` type, allowing easy access to details like the status code and headers
- Handling redirects using the `RedirectPolicy`
- Support for Basic and Bearer authentication
- Processing responses with `Transfer-Encoding: chunked`
- Managing absolute `Uri`s and partial support for relative `Uri`s
- Enforcing timeouts on requests
- Downloading data in a streaming fashion, allowing direct saving to disk (minimizing RAM usage)
- `Error` handling system allowing for better debugging
- Utility functions for easily sending common request types: `get`, `head`, `post`

## Usage

### Default configuration

In order to use `http_req` with default configuration, add the following lines to `Cargo.toml`:

```toml
[dependencies]
http_req = "^0.14"
```

### Rustls

In order to use `http_req` with `rustls` in your project, add the following lines to `Cargo.toml`:

```toml
[dependencies]
http_req = { version="^0.14", default-features = false, features = ["rust-tls"] }
```

### HTTP only

In order to use `http_req` without any additional features in your project (no HTTPS, no Authentication), add the following lines to `Cargo.toml`:

```toml
[dependencies]
http_req = { version="^0.14", default-features = false }
```

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

## License

Licensed under [MIT](https://github.com/jayjamesjay/http_req/blob/master/LICENSE).
