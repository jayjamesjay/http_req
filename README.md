# wasmedge_http_req

Simple and lightweight HTTP client for the low level [wasmedge_wasi_socket](https://github.com/second-state/wasmedge_wasi_socket) library. It is to be compiled into WebAssembly bytecode targets and run on the [WasmEdge Runtime](https://github.com/WasmEdge/WasmEdge).

> This project is forked and derived from the [http_req](https://github.com/jayjamesjay/http_req) project created by [jayjamesjay](https://github.com/jayjamesjay).

## Example

Basic GET request

```rust
use wasmedge_http_req::request;

fn main() {
    let mut writer = Vec::new(); //container for body of a response
    let res = request::get("http://127.0.0.1/", &mut writer).unwrap();

    println!("Status: {} {}", res.status_code(), res.reason());
}
```

## How to use:

```toml
[dependencies]
wasmedge_http_req  = "0.8.1"
```

