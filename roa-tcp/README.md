[![Stable Test](https://github.com/Hexilee/roa/workflows/Stable%20Test/badge.svg)](https://github.com/Hexilee/roa/actions)
[![codecov](https://codecov.io/gh/Hexilee/roa/branch/master/graph/badge.svg)](https://codecov.io/gh/Hexilee/roa)
[![Rust Docs](https://docs.rs/roa-tcp/badge.svg)](https://docs.rs/roa-tcp)
[![Crate version](https://img.shields.io/crates/v/roa-tcp.svg)](https://crates.io/crates/roa-tcp)
[![Download](https://img.shields.io/crates/d/roa-tcp.svg)](https://crates.io/crates/roa-tcp)
[![Version](https://img.shields.io/badge/rustc-1.40+-lightgray.svg)](https://blog.rust-lang.org/2019/12/19/Rust-1.40.0.html)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/Hexilee/roa/blob/master/LICENSE)

## Roa-tcp

This crate provides an acceptor implementing `roa_core::Accept` and an app extension.

### TcpIncoming

```rust,no_run
use roa_core::{self as roa, App, Context};
use roa_tcp::TcpIncoming;
use std::error::Error;

async fn end(_ctx: &mut Context<()>) -> roa::Result {
    Ok(())
}

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let app = App::new(()).end(end);
    let incoming = TcpIncoming::bind("127.0.0.1:0")?;
    let server = app.accept(incoming);
    server.await?;
    Ok(())
}
```

### Listener

```rust,no_run
use roa_core::{self as roa, App, Context};
use roa_tcp::Listener;
use std::error::Error;

async fn end(_ctx: &mut Context<()>) -> roa::Result {
    Ok(())
}

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let app = App::new(()).end(end);
    let (addr, server) = app.bind("127.0.0.1:0")?;
    server.await?;
    Ok(())
}
```