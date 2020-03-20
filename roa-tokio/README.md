## Roa-tokio

This crate provides tokio-based runtime and acceptor for roa.

```rust,no_run
use roa_core::http::StatusCode;
use roa_core::{self as roa, App, Context};
use roa_tokio::{TcpIncoming, Exec};
use std::error::Error;
async fn end(_ctx: &mut Context<()>) -> roa::Result {
    Ok(())
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let app = App::with_exec((), Exec).end(end);
    let incoming = TcpIncoming::bind("127.0.0.1:0")?;
    println!("server is listening on {}", incoming.local_addr());
    app.accept(incoming).await?;
    Ok(())
}
```