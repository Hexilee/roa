[![Stable Test](https://github.com/Hexilee/roa/workflows/Stable%20Test/badge.svg)](https://github.com/Hexilee/roa/actions)
[![codecov](https://codecov.io/gh/Hexilee/roa/branch/master/graph/badge.svg)](https://codecov.io/gh/Hexilee/roa)
[![Rust Docs](https://docs.rs/roa-pg/badge.svg)](https://docs.rs/roa-pg)
[![Crate version](https://img.shields.io/crates/v/roa-pg.svg)](https://crates.io/crates/roa-pg)
[![Download](https://img.shields.io/crates/d/roa-pg.svg)](https://crates.io/crates/roa-pg)
[![Version](https://img.shields.io/badge/rustc-1.40+-lightgray.svg)](https://blog.rust-lang.org/2019/12/19/Rust-1.40.0.html)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/Hexilee/roa/blob/master/LICENSE)

This crate provides integration with tokio-postgres.

### Example

```rust,no_run
use roa::{App, Context, throw};
use roa::http::StatusCode;
use roa_pg::{connect, Client};
use std::sync::Arc;
use std::error::Error;
use roa::query::query_parser;
use roa::preload::*;
use async_std::task::spawn;

#[derive(Clone)]
struct State {
    pg: Arc<Client>
}

impl State {
    pub async fn new(pg_url: &str) -> Result<Self, Box<dyn Error>> {
        let (client, conn) = connect(&pg_url.parse()?).await?;
        spawn(conn);
        Ok(Self {pg: Arc::new(client)})
    }
}

async fn query(ctx: &mut Context<State>) -> roa::Result {
    let id: u32 = ctx.must_query("id")?.parse()?;
    match ctx.pg.query_opt("SELECT * FROM user WHERE id=$1", &[&id]).await? {
        Some(row) => {
            let value: String = row.get(0);
            ctx.write(value);
            Ok(())
        }
        None => throw!(StatusCode::NOT_FOUND),
    }
}

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let url = "postgres://fred:secret@localhost/test";
    let state = State::new(url).await?;
    App::new(state)
        .gate(query_parser)
        .end(query)
        .listen("127.0.0.1:0", |addr| {
            println!("Server is listening on {}", addr)
        })?.await?;
    Ok(())
}
```