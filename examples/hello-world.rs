//! RUST_LOG=info Cargo run --example hello-world,
//! then request http://127.0.0.1:8000.

use log::info;
use roa::logger::logger;
use roa::preload::*;
use roa::App;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init()
        .map_err(|err| anyhow::anyhow!("fail to init tracing subscriber: {}", err))?;
    let app = App::new().gate(logger).end("Hello, World!");
    app.listen("127.0.0.1:8000", |addr| {
        info!("Server is listening on {}", addr)
    })?
    .await?;
    Ok(())
}
