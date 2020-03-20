[![Stable Test](https://github.com/Hexilee/roa/workflows/Stable%20Test/badge.svg)](https://github.com/Hexilee/roa/actions)
[![codecov](https://codecov.io/gh/Hexilee/roa/branch/master/graph/badge.svg)](https://codecov.io/gh/Hexilee/roa)
[![Rust Docs](https://docs.rs/roa-router/badge.svg)](https://docs.rs/roa-router)
[![Crate version](https://img.shields.io/crates/v/roa-router.svg)](https://crates.io/crates/roa-router)
[![Download](https://img.shields.io/crates/d/roa-router.svg)](https://crates.io/crates/roa-router)
[![Version](https://img.shields.io/badge/rustc-1.40+-lightgray.svg)](https://blog.rust-lang.org/2019/12/19/Rust-1.40.0.html)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/Hexilee/roa/blob/master/LICENSE)

## Roa-router

The router crate of roa.
This crate provides many endpoint wrappers like `Router`, `Dispatcher` and a context extension `RouterParam`.

### Example

```rust
use roa_router::{Router, RouterParam, get, allow};
use roa_core::{App, Context, Error, MiddlewareExt, Next};
use roa_core::http::{StatusCode, Method};
use roa_tcp::Listener;
use async_std::task::spawn;


async fn gate(_ctx: &mut Context<()>, next: Next<'_>) -> Result<(), Error> {
    next.await
}

async fn query(ctx: &mut Context<()>) -> Result<(), Error> {
    Ok(())
}

async fn create(ctx: &mut Context<()>) -> Result<(), Error> {
    Ok(())
}

async fn graphql(ctx: &mut Context<()>) -> Result<(), Error> {
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let router = Router::new()
        .gate(gate)
        .on("/restful", get(query).post(create))
        .on("/graphql", allow([Method::GET, Method::POST], graphql));
    let app = App::new(())
        .end(router.routes("/api")?);
    let (addr, server) = app.run()?;
    spawn(server);
    let resp = reqwest::get(&format!("http://{}/api/restful", addr)).await?;
    assert_eq!(StatusCode::OK, resp.status());

    let resp = reqwest::get(&format!("http://{}/restful", addr)).await?;
    assert_eq!(StatusCode::NOT_FOUND, resp.status());
    Ok(())
}
```
