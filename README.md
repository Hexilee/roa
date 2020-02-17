<div align="center">
  <h1>Roa</h1>
  <p><strong>Roa is an async web framework inspired by koajs, lightweight but powerful. </strong> </p>
  <p>

[![Build status](https://img.shields.io/travis/Hexilee/roa/master.svg)](https://travis-ci.org/Hexilee/roa)
[![codecov](https://codecov.io/gh/Hexilee/roa/branch/master/graph/badge.svg)](https://codecov.io/gh/Hexilee/roa) 
[![Rust Docs](https://docs.rs/roa/badge.svg)](https://docs.rs/roa)
[![Crate version](https://img.shields.io/crates/v/roa.svg)](https://crates.io/crates/roa)
[![Download](https://img.shields.io/crates/d/roa.svg)](https://crates.io/crates/roa)
[![Version](https://img.shields.io/badge/rustc-1.39+-lightgray.svg)](https://blog.rust-lang.org/2019/11/07/Rust-1.39.0.html)
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
    - Based on [`hyper`](https://github.com/hyperium/hyper) and [`async-std`](https://github.com/async-rs/async-std), runtime-independent, you can chose any async runtime you like.
- Many useful extensions and middlewares.
    - Transparent content compression (br, gzip, deflate, zstd).
    - Configurable and nestable router.
    - Named uri parameters(query and router parameter).
    - Cookie and jwt support.
    - Integration with [serde](https://github.com/serde-rs/serde) and [askama](https://github.com/djc/askama). JSON, urlencoded form, html template support.
    - Other middlewares(logger, CORS .etc).
- Works on stable Rust.


#### Next step

- [ ] Juniper integration.
- [x] Better context storage.
- [ ] Database integration.
  - [ ] diesel
  - [ ] sled
- [ ] Streaming multipart form support.
- [ ] Websocket support.
- [ ] HTTPS support.

#### Get start

```toml
# Cargo.toml

[dependencies]
roa = "0.4"
async-std = { version = "1.4", features = ["attributes"] }
```

```rust
lib

use roa::core::App;
use roa::preload::*;
use std::error::Error as StdError;

#[async_std::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    let mut app = App::new(());
    app.end(|mut ctx| async move {
        ctx.write_text("Hello, World").await
    });
    app.listen("127.0.0.1:8000", |addr| {
        println!("Server is listening on {}", addr)
    })?
    .await?;
    Ok(())
}
```

