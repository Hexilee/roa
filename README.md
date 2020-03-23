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
[![Version](https://img.shields.io/badge/rustc-1.40+-lightgray.svg)](https://blog.rust-lang.org/2019/12/19/Rust-1.40.0.html)
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
- Many useful extensions and middlewares.
    - Transparent content compression (br, gzip, deflate, zstd).
    - Configurable and nestable router.
    - Named uri parameters(query and router parameter).
    - Cookie and jwt support.
    - Integration with [serde](https://github.com/serde-rs/serde) and [askama](https://github.com/djc/askama). JSON, urlencoded form, html template support.
    - ORM integration (with [diesel](https://github.com/diesel-rs/diesel)).
    - HTTPS support.
    - WebSocket support.
    - GraphQL support(based on [juniper](https://github.com/graphql-rust/juniper)).
    - Asynchronous multipart form support.
    - Other middlewares(logger, CORS .etc).
- Works on stable Rust.

#### Get start

```
# Cargo.toml

[dependencies]
roa = "0.5.0-rc"
async-std = { version = "1.5", features = ["attributes"] }
```

```rust,no_run
use roa::{App, Context};
use roa::preload::*;
use std::error::Error as StdError;

async fn hello(ctx: &mut Context<()>) -> roa::Result {
    ctx.write_text("Hello, World");
    Ok(())
}

#[async_std::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    let app = App::new(()).end(hello);
    app.listen("127.0.0.1:8000", |addr| {
        println!("Server is listening on {}", addr)
    })?
    .await?;
    Ok(())
}
```

