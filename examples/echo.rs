//! RUST_LOG=info Cargo run --example echo,
//! then request http://127.0.0.1:8000 with some payload.

use std::error::Error as StdError;

use roa::logger::logger;
use roa::preload::*;
use roa::{App, Context};
use tracing::info;
use tracing_subscriber::EnvFilter;

async fn echo(ctx: &mut Context) -> roa::Result {
    let stream = ctx.req.stream();
    ctx.resp.write_stream(stream);
    Ok(())
}

#[async_std::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init()
        .map_err(|err| anyhow::anyhow!("fail to init tracing subscriber: {}", err))?;

    let app = App::new().gate(logger).end(echo);
    app.listen("127.0.0.1:8000", |addr| {
        info!("Server is listening on {}", addr)
    })?
    .await?;
    Ok(())
}
