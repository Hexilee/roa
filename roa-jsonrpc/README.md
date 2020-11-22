[![Stable Test](https://github.com/Hexilee/roa/workflows/Stable%20Test/badge.svg)](https://github.com/Hexilee/roa/actions)
[![codecov](https://codecov.io/gh/Hexilee/roa/branch/master/graph/badge.svg)](https://codecov.io/gh/Hexilee/roa)
[![Rust Docs](https://docs.rs/roa-jsonrpc/badge.svg)](https://docs.rs/roa-jsonrpc)
[![Crate version](https://img.shields.io/crates/v/roa-jsonrpc.svg)](https://crates.io/crates/roa-jsonrpc)
[![Download](https://img.shields.io/crates/d/roa-jsonrpc.svg)](https://crates.io/crates/roa-jsonrpc)
:[![MSRV-1.42](https://img.shields.io/badge/MSRV-1.42-blue.svg)](https://blog.rust-lang.org/2020/03/12/Rust-1.42.html)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/Hexilee/roa/blob/master/LICENSE)

## roa-jsonrpc

This crate provides a json rpc endpoint.

### Example

```rust,no_run
use roa::App;
use roa_jsonrpc::{RpcEndpoint, Data, Error, Params, Server};
use std::error::Error as StdError;

#[derive(serde::Deserialize)]
struct TwoNums {
    a: usize,
    b: usize,
}

async fn add(Params(params): Params<TwoNums>) -> Result<usize, Error> {
    Ok(params.a + params.b)
}

async fn sub(Params(params): Params<(usize, usize)>) -> Result<usize, Error> {
    Ok(params.0 - params.1)
}

async fn message(data: Data<String>) -> Result<String, Error> {
    Ok(String::from(&*data))
}

#[async_std::main]
async fn main() -> std::io::Result<()> {
    let rpc = Server::new()
        .with_data(Data::new(String::from("Hello!")))
        .with_method("sub", sub)
        .with_method("message", message)
        .finish();

    let app = App::new().end(RpcEndpoint(rpc));
    app.listen("127.0.0.1:8000", |addr| {
        info!("Server is listening on {}", addr)
    })?
    .await?;
    Ok(())
}
```

