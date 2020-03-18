[![Stable Test](https://github.com/Hexilee/roa/workflows/Stable%20Test/badge.svg)](https://github.com/Hexilee/roa/actions)
[![codecov](https://codecov.io/gh/Hexilee/roa/branch/master/graph/badge.svg)](https://codecov.io/gh/Hexilee/roa)
[![Rust Docs](https://docs.rs/roa-multipart/badge.svg)](https://docs.rs/roa-multipart)
[![Crate version](https://img.shields.io/crates/v/roa-multipart.svg)](https://crates.io/crates/roa-multipart)
[![Download](https://img.shields.io/crates/d/roa-multipart.svg)](https://crates.io/crates/roa-multipart)
[![Version](https://img.shields.io/badge/rustc-1.40+-lightgray.svg)](https://blog.rust-lang.org/2019/12/19/Rust-1.40.0.html)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/Hexilee/roa/blob/master/LICENSE)

## Roa-multipart

This crate provides a wrapper for `actix_multipart::Multipart`,
which may cause heavy dependencies.

It won't be used as a module of crate `roa` until implementing a cleaner Multipart.  

### Example
```
use tokio::fs::File;
use async_std::io;
use async_std::path::Path;
use futures::stream::TryStreamExt;
use futures::StreamExt;
use roa::http::StatusCode;
use roa::preload::*;
use roa::router::Router;
use roa::{throw, App};
use roa_multipart::Multipart;
use std::error::Error as StdError;

# fn main() -> Result<(), Box<dyn StdError>> {
let mut app = App::new(());
let mut router = Router::<()>::new();
router.post("/file", |mut ctx| async move {
    let mut form = Multipart::new(&mut ctx);
    while let Some(item) = form.next().await {
        let field = item?;
        match field.content_disposition() {
            None => throw!(StatusCode::BAD_REQUEST, "content disposition not set"),
            Some(content_disposition) => match content_disposition.get_filename() {
                None => continue, // ignore non-file field
                Some(filename) => {
                    let path = Path::new("./upload");
                    let mut file = File::create(path.join(filename)).await?;
                    io::copy(&mut field.into_async_read(), &mut file).await?;
                }
            },
        }
    }
    Ok(())
});
let (addr, server) = app.run()?;
// server.await
Ok(())
# }
```