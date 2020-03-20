[![Stable Test](https://github.com/Hexilee/roa/workflows/Stable%20Test/badge.svg)](https://github.com/Hexilee/roa/actions)
[![codecov](https://codecov.io/gh/Hexilee/roa/branch/master/graph/badge.svg)](https://codecov.io/gh/Hexilee/roa)
[![Rust Docs](https://docs.rs/roa-diesel/badge.svg)](https://docs.rs/roa-diesel)
[![Crate version](https://img.shields.io/crates/v/roa-diesel.svg)](https://crates.io/crates/roa-diesel)
[![Download](https://img.shields.io/crates/d/roa-diesel.svg)](https://crates.io/crates/roa-diesel)
[![Version](https://img.shields.io/badge/rustc-1.40+-lightgray.svg)](https://blog.rust-lang.org/2019/12/19/Rust-1.40.0.html)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/Hexilee/roa/blob/master/LICENSE)

## Roa-diesel

A diesel integration for roa.

This crate provides two context extensions.

### AsyncPool
A context extension to access r2d2 pool asynchronously.

```rust
use roa_core::{Context, Result};
use diesel::sqlite::SqliteConnection;
use roa_diesel::{Pool, AsyncPool};
use diesel::r2d2::ConnectionManager;

#[derive(Clone)]
struct State(Pool<SqliteConnection>);

impl AsRef<Pool<SqliteConnection>> for State {
    fn as_ref(&self) -> &Pool<SqliteConnection> {
        &self.0
    }
}

async fn get(ctx: Context<State>) -> Result {
    let conn = ctx.get_conn().await?;
    // handle conn
    Ok(())
}
```

### SqlQuery
A context extension to execute diesel dsl asynchronously.

Refer to [integration example](https://github.com/Hexilee/roa/tree/master/integration/diesel-example)
for more usage.

