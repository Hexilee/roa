[![Stable Test](https://github.com/Hexilee/roa/workflows/Stable%20Test/badge.svg)](https://github.com/Hexilee/roa/actions)
[![codecov](https://codecov.io/gh/Hexilee/roa/branch/master/graph/badge.svg)](https://codecov.io/gh/Hexilee/roa)
[![Rust Docs](https://docs.rs/roa-body/badge.svg)](https://docs.rs/roa-body)
[![Crate version](https://img.shields.io/crates/v/roa-body.svg)](https://crates.io/crates/roa-body)
[![Download](https://img.shields.io/crates/d/roa-body.svg)](https://crates.io/crates/roa-body)
[![Version](https://img.shields.io/badge/rustc-1.40+-lightgray.svg)](https://blog.rust-lang.org/2019/12/19/Rust-1.40.0.html)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/Hexilee/roa/blob/master/LICENSE)

## Roa-body

An extension crate for roa.
This module provides a context extension `PowerBody`.

### Read/write body in a easier way.

The `roa_core` provides several methods to read/write body.

```rust
use roa_core::{Context, Result};
use futures::AsyncReadExt;
use futures::io::BufReader;
use tokio::fs::File;

async fn get(mut ctx: Context<()>) -> Result {
    // roa_core::Body implements futures::AsyncRead.
    let mut data = String::new();
    ctx.req_mut().body().read_to_string(&mut data).await?;
    println!("data: {}", data);

    ctx.resp_mut()
       // write object implementing futures::AsyncRead
       .write(File::open("assets/author.txt").await?)
       // write `impl ToString`
       .write_str("I am Roa.")
       // write `impl Into<Vec<u8>>`
       .write_bytes(b"Hey Roa.".as_ref());
    Ok(())
}
```

These methods are useful, but they do not deal with headers, especially `Content-*` headers.

The `PowerBody` provides more powerful methods to handle it.

```rust
use roa_core::{Context, Result};
use roa_body::{PowerBody, DispositionType::*};
use serde::{Serialize, Deserialize};
use askama::Template;
use tokio::fs::File;
use futures::io::BufReader;

#[derive(Debug, Serialize, Deserialize, Template)]
#[template(path = "user.html")]
struct User {
    id: u64,
    name: String,
}

async fn get(mut ctx: Context<()>) -> Result {
    // deserialize as json.
    let mut user: User = ctx.read_json().await?;

    // deserialize as x-form-urlencoded.
    user = ctx.read_form().await?;

    // serialize object and write it to body,
    // set "Content-Type"
    ctx.write_json(&user)?;

    // open file and write it to body,
    // set "Content-Type" and "Content-Disposition"
    ctx.write_file("assets/welcome.html", Inline).await?;

    // write text,
    // set "Content-Type"
    ctx.write_text("Hello, World!")?;

    // write object implementing AsyncRead,
    // set "Content-Type"
    ctx.write_octet(File::open("assets/author.txt").await?)?;

    // render html template, based on [askama](https://github.com/djc/askama).
    // set "Content-Type"
    ctx.render(&user)?;
    Ok(())
}
```
 