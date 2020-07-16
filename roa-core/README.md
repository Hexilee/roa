[![Stable Test](https://github.com/Hexilee/roa/workflows/Stable%20Test/badge.svg)](https://github.com/Hexilee/roa/actions)
[![codecov](https://codecov.io/gh/Hexilee/roa/branch/master/graph/badge.svg)](https://codecov.io/gh/Hexilee/roa)
[![Rust Docs](https://docs.rs/roa-core/badge.svg)](https://docs.rs/roa-core)
[![Crate version](https://img.shields.io/crates/v/roa-core.svg)](https://crates.io/crates/roa-core)
[![Download](https://img.shields.io/crates/d/roa-core.svg)](https://crates.io/crates/roa-core)
[![MSRV-1.40](https://img.shields.io/badge/MSRV-1.40-blue.svg)](https://blog.rust-lang.org/2019/12/19/Rust-1.40.0.html)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/Hexilee/roa/blob/master/LICENSE)

### Introduction

Core components of Roa framework.

If you are new to roa, please go to the documentation of [roa framework](https://docs.rs/roa).

### Application

A Roa application is a structure composing and executing middlewares and an endpoint in a stack-like manner.

The obligatory hello world application:

```rust
use roa_core::App;
let app = App::new().end("Hello, World");
```

#### Endpoint

An endpoint is a request handler.

There are some build-in endpoints in roa_core.

- Functional endpoint

    A normal functional endpoint is an async function with signature:
    `async fn(&mut Context) -> Result`.
    
    ```rust
    use roa_core::{App, Context, Result};
    
    async fn endpoint(ctx: &mut Context) -> Result {
        Ok(())
    }
    
    let app = App::new().end(endpoint);
    ```
  
- Ok endpoint

    `()` is an endpoint always return `Ok(())`
    
    ```rust
    let app = roa_core::App::new().end(());
    ```

- Status endpoint

    `Status` is an endpoint always return `Err(Status)`
    
    ```rust
    use roa_core::{App, status};
    use roa_core::http::StatusCode;
    let app = App::new().end(status!(StatusCode::BAD_REQUEST));
    ```

- String endpoint

    Write string to body.
    
    ```rust
    use roa_core::App;
    
    let app = App::new().end("Hello, world"); // static slice
    let app = App::new().end("Hello, world".to_owned()); // string
    ```

- Redirect endpoint

    Redirect to an uri.
    
    ```rust
    use roa_core::App;
    use roa_core::http::Uri;
    
    let app = App::new().end("/target".parse::<Uri>().unwrap());
    ```


#### Cascading

The following example responds with "Hello World", however, the request flows through
the `logging` middleware to mark when the request started, then continue
to yield control through the endpoint. When a middleware invokes `next.await`
the function suspends and passes control to the next middleware or endpoint. After the endpoint is called,
the stack will unwind and each middleware is resumed to perform
its upstream behaviour.

```rust
use roa_core::{App, Context, Result, Status, MiddlewareExt, Next};
use std::time::Instant;
use log::info;

let app = App::new().gate(logging).end("Hello, World");

async fn logging(ctx: &mut Context, next: Next<'_>) -> Result {
    let inbound = Instant::now();
    next.await?;
    info!("time elapsed: {} ms", inbound.elapsed().as_millis());
    Ok(())
}
```

### Status Handling

You can catch or straightly throw a status returned by next.

```rust
use roa_core::{App, Context, Result, Status, MiddlewareExt, Next, throw};
use roa_core::http::StatusCode;
        
let app = App::new().gate(catch).gate(gate).end(end);

async fn catch(ctx: &mut Context, next: Next<'_>) -> Result {
    // catch
    if let Err(status) = next.await {
        // teapot is ok
        if status.status_code != StatusCode::IM_A_TEAPOT {
            return Err(status);
        }
    }
    Ok(())
}
async fn gate(ctx: &mut Context, next: Next<'_>) -> Result {
    next.await?; // just throw
    unreachable!()
}

async fn end(ctx: &mut Context) -> Result {
    throw!(StatusCode::IM_A_TEAPOT, "I'm a teapot!")
}
```

#### status_handler
App has an status_handler to handle `Status` thrown by the top middleware.
This is the status_handler:

```rust
use roa_core::{Context, Status, Result, State};
pub fn status_handler<S: State>(ctx: &mut Context<S>, status: Status) {
    ctx.resp.status = status.status_code;
    if status.expose {
        ctx.resp.write(status.message);
    } else {
        log::error!("{}", status);
    }
}
```

### HTTP Server.

Use `roa_core::accept` to construct a http server.
Please refer to `roa::tcp` for more information.