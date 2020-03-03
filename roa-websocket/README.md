[![Stable Test](https://github.com/Hexilee/roa/workflows/Stable%20Test/badge.svg)](https://github.com/Hexilee/roa/actions)
[![codecov](https://codecov.io/gh/Hexilee/roa/branch/master/graph/badge.svg)](https://codecov.io/gh/Hexilee/roa)
[![Rust Docs](https://docs.rs/roa-websocket/badge.svg)](https://docs.rs/roa-websocket)
[![Crate version](https://img.shields.io/crates/v/roa-websocket.svg)](https://crates.io/crates/roa-websocket)
[![Download](https://img.shields.io/crates/d/roa-websocket.svg)](https://crates.io/crates/roa-websocket)
[![Version](https://img.shields.io/badge/rustc-1.40+-lightgray.svg)](https://blog.rust-lang.org/2019/12/19/Rust-1.40.0.html)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/Hexilee/roa/blob/master/LICENSE)

## Roa-websocket

This crate provides a websocket endpoint.

### Example

```rust
use futures::StreamExt;
use roa_router::{Router, RouterError};
use roa_websocket::Websocket;
use roa_core::{App, SyncContext};
use roa_core::http::Method;

# fn main() -> Result<(), RouterError> {
let mut app = App::new(());
let mut router = Router::new();
router.end(
    "/chat",
    [Method::GET],
    Websocket::new(|_ctx: SyncContext<()>, stream| async move {
        let (write, read) = stream.split();
        // echo
        if let Err(err) = read.forward(write).await {
            println!("forward err: {}", err);
        }
    }),
);
app.gate(router.routes("/")?);
Ok(())
# }
```