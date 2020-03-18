[![Stable Test](https://github.com/Hexilee/roa/workflows/Stable%20Test/badge.svg)](https://github.com/Hexilee/roa/actions)
[![codecov](https://codecov.io/gh/Hexilee/roa/branch/master/graph/badge.svg)](https://codecov.io/gh/Hexilee/roa)
[![Rust Docs](https://docs.rs/roa-router/badge.svg)](https://docs.rs/roa-router)
[![Crate version](https://img.shields.io/crates/v/roa-router.svg)](https://crates.io/crates/roa-router)
[![Download](https://img.shields.io/crates/d/roa-router.svg)](https://crates.io/crates/roa-router)
[![Version](https://img.shields.io/badge/rustc-1.40+-lightgray.svg)](https://blog.rust-lang.org/2019/12/19/Rust-1.40.0.html)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/Hexilee/roa/blob/master/LICENSE)

## Roa-router

The router module of roa.
This module provides an endpoint `RouteEndpoint` and a context extension `RouterParam`.

### Example

```rust
use roa_router::{Router, RouterParam};
use roa_core::App;
use roa_core::http::StatusCode;
use roa_tcp::Listener;
use tokio::spawn;

#[tokio::test]
async fn gate() -> Result<(), Box<dyn std::error::Error>> {
    let mut router = Router::<()>::new();
    router
        .gate_fn(|_ctx, next| next)
        .get("/", |_ctx| async move {
            Ok(())
        });
    let (addr, server) = App::new(()).gate(router.routes("/route")?).run()?;
    spawn(server);
    let resp = reqwest::get(&format!("http://{}/route", addr)).await?;
    assert_eq!(StatusCode::OK, resp.status());

    let resp = reqwest::get(&format!("http://{}/endpoint", addr)).await?;
    assert_eq!(StatusCode::NOT_FOUND, resp.status());
    Ok(())
}
```
