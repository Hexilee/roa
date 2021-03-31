<div align="center">
  <h1>Roa</h1>
  <p><strong>Roa is an async web framework inspired by koajs, lightweight but powerful. </strong> </p>
  <p>

[![Stable Test](https://github.com/Hexilee/roa/workflows/Stable%20Test/badge.svg)](https://github.com/Hexilee/roa/actions)
[![codecov](https://codecov.io/gh/Hexilee/roa/branch/master/graph/badge.svg)](https://codecov.io/gh/Hexilee/roa) 
[![wiki](https://img.shields.io/badge/roa-wiki-purple.svg)](https://github.com/Hexilee/roa/wiki)
[![Rust Docs](https://docs.rs/roa/badge.svg)](https://docs.rs/roa)
[![Crate version](https://img.shields.io/crates/v/roa.svg)](https://crates.io/crates/roa)
[![Download](https://img.shields.io/crates/d/roa.svg)](https://crates.io/crates/roa)
[![MSRV-1.46](https://img.shields.io/badge/MSRV-1.46-blue.svg)](https://blog.rust-lang.org/2020/08/27/Rust-1.46.0.html)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/Hexilee/roa/blob/master/LICENSE)

  </p>

  <h3>
    <a href="https://github.com/Hexilee/roa/tree/master/examples">Examples</a>
    <span> | </span>
    <a href="https://github.com/Hexilee/roa/wiki/Guide">Guide</a>
    <span> | </span>
    <a href="https://github.com/Hexilee/roa/wiki/Cookbook">Cookbook</a>
  </h3>
</div>
<br>


#### Feature highlights

- A lightweight, solid and well extensible core.
    - Supports HTTP/1.x and HTTP/2.0 protocols.
    - Full streaming.
    - Highly extensible middleware system.
    - Based on [`hyper`](https://github.com/hyperium/hyper), runtime-independent, you can chose async runtime as you like.
- Many useful extensions.
    - Official runtime schemes:
        - [async-std](https://github.com/async-rs/async-std) runtime and TcpStream;
        - [tokio](https://github.com/tokio-rs/tokio) runtime and TcpStream.
    - Transparent content compression (br, gzip, deflate, zstd).
    - Configurable and nestable router.
    - Named uri parameters(query and router parameter).
    - Cookie and jwt support.
    - HTTPS support.
    - WebSocket support.
    - Asynchronous multipart form support.
    - Other middlewares(logger, CORS .etc).
- Integrations
    - roa-diesel, integration with [diesel](https://github.com/diesel-rs/diesel).
    - roa-juniper, integration with [juniper](https://github.com/graphql-rust/juniper).
- Works on stable Rust.

#### Get start

```toml
# Cargo.toml

[dependencies]
roa = "0.5"
async-std = { version = "1.6", features = ["attributes"] }
```

```rust,no_run
use roa::App;
use roa::preload::*;
use std::error::Error as StdError;

#[async_std::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    let app = App::new().end("Hello, World");
    app.listen("127.0.0.1:8000", |addr| {
        println!("Server is listening on {}", addr)
    })?
    .await?;
    Ok(())
}
```
Refer to [wiki](https://github.com/Hexilee/roa/wiki) for more details.
