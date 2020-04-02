//! RUST_LOG=info Cargo run --example hello-world,
//! then request http://127.0.0.1:8000.

use log::info;
use roa::logger::logger;
use roa::preload::*;
use roa::App;
use std::error::Error as StdError;

#[async_std::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    pretty_env_logger::init();
    let app = App::new().gate(logger).end("Hello, World!");
    app.listen("127.0.0.1:8000", |addr| {
        info!("Server is listening on {}", addr)
    })?
    .await?;
    Ok(())
}
