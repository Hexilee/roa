[![Stable Test](https://github.com/Hexilee/roa/workflows/Stable%20Test/badge.svg)](https://github.com/Hexilee/roa/actions)
[![codecov](https://codecov.io/gh/Hexilee/roa/branch/master/graph/badge.svg)](https://codecov.io/gh/Hexilee/roa)
[![Rust Docs](https://docs.rs/roa-async-std/badge.svg)](https://docs.rs/roa-async-std)
[![Crate version](https://img.shields.io/crates/v/roa-async-std.svg)](https://crates.io/crates/roa-async-std)
[![Download](https://img.shields.io/crates/d/roa-async-std.svg)](https://crates.io/crates/roa-async-std)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/Hexilee/roa/blob/master/LICENSE)

This crate provides async-std-based runtime and acceptor for roa.

```rust,no_run
use roa::http::StatusCode;
use roa::{App, Context};
use roa_async_std::{Listener, Exec};
use std::error::Error;

async fn end(_ctx: &mut Context) -> roa::Result {
    Ok(())
}

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let (addr, server) = App::with_exec((), Exec).end(end).run()?;
    println!("server is listening on {}", addr);
    server.await?;
    Ok(())
}
```
