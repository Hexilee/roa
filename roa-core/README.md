[![Build status](https://img.shields.io/travis/Hexilee/roa/master.svg)](https://travis-ci.org/Hexilee/roa)
[![codecov](https://codecov.io/gh/Hexilee/roa/branch/master/graph/badge.svg)](https://codecov.io/gh/Hexilee/roa) 
[![Rust Docs](https://docs.rs/roa-core/badge.svg)](https://docs.rs/roa-core)
[![Crate version](https://img.shields.io/crates/v/roa-core.svg)](https://crates.io/crates/roa-core)
[![Download](https://img.shields.io/crates/d/roa-core.svg)](https://crates.io/crates/roa-core)
[![Version](https://img.shields.io/badge/rustc-1.39+-lightgray.svg)](https://blog.rust-lang.org/2019/11/07/Rust-1.39.0.html)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/Hexilee/roa/blob/master/LICENSE)

### Introduction

Roa is an async web framework inspired by koajs, lightweight but powerful.

### Application

A Roa application is a structure containing a middleware group which composes and executes middleware functions in a stack-like manner.

The obligatory hello world application:

```rust
use roa_core::App;
use log::info;
use std::error::Error as StdError;

#[async_std::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    let mut app = App::new(());
    app.end(|ctx| async move {
      	ctx.resp_mut().await.write("Hello, World");
      	Ok(())
  	});
    app.listen("127.0.0.1:8000", |addr| {
        info!("Server is listening on {}", addr)
    })?
  	.await?;
    Ok(())
}
```

#### Cascading

The following example responds with "Hello World", however, the request flows through
the `logging` middleware to mark when the request started, then continue
to yield control through the response middleware. When a middleware invokes `next().await`
the function suspends and passes control to the next middleware defined. After there are no more
middleware to execute downstream, the stack will unwind and each middleware is resumed to perform
its upstream behaviour.

```rust
use roa_core::App;
use log::info;
use std::error::Error as StdError;
use std::time::Instant;

#[async_std::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    let mut app = App::new(());
  	app.gate(|_ctx, next| async move {
    		let inbound = Instant::now();
        next().await?;
        info!("time elapsed: {} ms", inbound.elapsed().as_millis());
        Ok(())
  	});
  
    app.end(|ctx| async move {
      	ctx.resp_mut().await.write("Hello, World");
      	Ok(())
  	});
    app.listen("127.0.0.1:8000", |addr| {
        info!("Server is listening on {}", addr)
    })?
  	.await?;
    Ok(())
}
```

### Middleware

`Middleware` is a trait:

```rust
trait Middleware<M, F> = 'static + Sync + Send + Fn(Context<M>) -> F,
where 
```

