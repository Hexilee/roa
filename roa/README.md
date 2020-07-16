[![Build status](https://img.shields.io/travis/Hexilee/roa/master.svg)](https://travis-ci.org/Hexilee/roa)
[![codecov](https://codecov.io/gh/Hexilee/roa/branch/master/graph/badge.svg)](https://codecov.io/gh/Hexilee/roa)
[![Rust Docs](https://docs.rs/roa/badge.svg)](https://docs.rs/roa)
[![Crate version](https://img.shields.io/crates/v/roa.svg)](https://crates.io/crates/roa)
[![Download](https://img.shields.io/crates/d/roa.svg)](https://crates.io/crates/roa)
[![MSRV-1.40](https://img.shields.io/badge/MSRV-1.40-blue.svg)](https://blog.rust-lang.org/2019/12/19/Rust-1.40.0.html)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/Hexilee/roa/blob/master/LICENSE)

### Introduction

Roa is an async web framework inspired by koajs, lightweight but powerful.

### Application

A Roa application is a structure composing and executing middlewares and an endpoint in a stack-like manner.

The obligatory hello world application:

```rust,no_run
use roa::App;
use roa::preload::*;
use log::info;
use std::error::Error as StdError;

#[async_std::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    let app = App::new().end("Hello, World");
    app.listen("127.0.0.1:8000", |addr| {
        info!("Server is listening on {}", addr)
    })?
    .await?;
    Ok(())
}
```

#### Endpoint

An endpoint is a request handler.

There are some build-in endpoints in roa.

- Functional endpoint

    A normal functional endpoint is an async function with signature:
    `async fn(&mut Context) -> Result`.
    
    ```rust
    use roa::{App, Context, Result};
    
    async fn endpoint(ctx: &mut Context) -> Result {
        Ok(())
    }
    
    let app = App::new().end(endpoint);
    ```
  
- Ok endpoint

    `()` is an endpoint always return `Ok(())`
    
    ```rust
    let app = roa::App::new().end(());
    ```

- Status endpoint

    `Status` is an endpoint always return `Err(Status)`
    
    ```rust
    use roa::{App, status};
    use roa::http::StatusCode;
    let app = App::new().end(status!(StatusCode::BAD_REQUEST));
    ```

- String endpoint

    Write string to body.
    
    ```rust
    use roa::App;
    
    let app = App::new().end("Hello, world"); // static slice
    let app = App::new().end("Hello, world".to_owned()); // string
    ```

- Redirect endpoint

    Redirect to an uri.
    
    ```rust
    use roa::App;
    use roa::http::Uri;
    
    let app = App::new().end("/target".parse::<Uri>().unwrap());
    ```


#### Cascading
Like koajs, middleware suspends and passes control to "downstream" by invoking `next.await`.
Then control flows back "upstream" when `next.await` returns.

The following example responds with "Hello World",
however first the request flows through the x-response-time and logging middleware to mark
when the request started, then continue to yield control through the endpoint.
When a middleware invokes next the function suspends and passes control to the next middleware or endpoint.
After the endpoint is called,
the stack will unwind and each middleware is resumed to perform
its upstream behaviour.

```rust,no_run
use roa::{App, Context, Next};
use roa::preload::*;
use log::info;
use std::error::Error as StdError;
use std::time::Instant;

#[async_std::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    let app = App::new()
        .gate(logger)
        .gate(x_response_time)
        .end("Hello, World");
    app.listen("127.0.0.1:8000", |addr| {
        info!("Server is listening on {}", addr)
    })?
    .await?;
    Ok(())
}

async fn logger(ctx: &mut Context, next: Next<'_>) -> roa::Result {
    next.await?;
    let rt = ctx.resp.must_get("x-response-time")?;
    info!("{} {} - {}", ctx.method(), ctx.uri(), rt);
    Ok(())
}

async fn x_response_time(ctx: &mut Context, next: Next<'_>) -> roa::Result {
    let start = Instant::now();
    next.await?;
    let ms = start.elapsed().as_millis();
    ctx.resp.insert("x-response-time", format!("{}ms", ms))?;
    Ok(())
}

```

### Status Handling

You can catch or straightly throw a status returned by next.

```rust,no_run
use roa::{App, Context, Next, status};
use roa::preload::*;
use roa::http::StatusCode;
use async_std::task::spawn;
use log::info;

#[async_std::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = App::new()
        .gate(catch)
        .gate(not_catch)
        .end(status!(StatusCode::IM_A_TEAPOT, "I'm a teapot!");
    app.listen("127.0.0.1:8000", |addr| {
        info!("Server is listening on {}", addr)
    })?
    .await?;
    Ok(())
}

async fn catch(_ctx: &mut Context, next: Next<'_>) -> roa::Result {
    // catch
    if let Err(status) = next.await {
        // teapot is ok
        if status.status_code != StatusCode::IM_A_TEAPOT {
            return Err(status);
        }
    }
    Ok(())
}

async fn not_catch(ctx: &mut Context, next: Next<'_>) -> roa::Result {
    next.await?; // just throw
    unreachable!()
}
```

#### status_handler
App has an status_handler to handle status thrown by the top middleware.
This is the status_handler:

```rust,no_run
use roa::{Context, Status};
pub fn status_handler<S>(ctx: &mut Context<S>, status: Status) {
    ctx.resp.status = status.status_code;
    if status.expose {
        ctx.resp.write(status.message);
    } else {
        log::error!("{}", status);
    }
}
```

### Router.
Roa provides a configurable and nestable router.

```rust,no_run
use roa::preload::*;
use roa::router::{Router, get};
use roa::{App, Context};
use async_std::task::spawn;
use log::info;

#[async_std::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let router = Router::new()
        .on("/:id", get(end)); // get dynamic "/:id"
    let app = App::new()
        .end(router.routes("/user")?); // route with prefix "/user"
    app.listen("127.0.0.1:8000", |addr| {
        info!("Server is listening on {}", addr)
    })?
    .await?;
    
    Ok(())
}

async fn end(ctx: &mut Context) -> roa::Result {
    // get "/user/1", then id == 1.
    let id: u64 = ctx.must_param("id")?.parse()?;
    // do something
    Ok(())
}
```

### Query

Roa provides a middleware `query_parser`.

```rust,no_run
use roa::preload::*;
use roa::query::query_parser;
use roa::{App, Context};
use async_std::task::spawn;
use log::info;

async fn must(ctx: &mut Context) -> roa::Result {
    // request "/?id=1", then id == 1.
    let id: u64 = ctx.must_query("id")?.parse()?;
    Ok(())
}

#[async_std::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = App::new()
        .gate(query_parser)
        .end(must);
    app.listen("127.0.0.1:8080", |addr| {
        info!("Server is listening on {}", addr)
    })?
    .await?;     
    Ok(())
}
```

### Other modules

- body: dealing with body more conveniently.
- compress: supports transparent content compression.
- cookie: cookies getter or setter.
- cors: CORS support.
- forward: "X-Forwarded-*" parser.
- jwt: json web token support.
- logger: a logger middleware.
- tls: https supports.
- websocket: websocket supports.
