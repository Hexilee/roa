//! RUST_LOG=info cargo run --example websocket-echo,
//! then request ws://127.0.0.1:8000/chat.

use std::error::Error as StdError;

use futures::StreamExt;
use http::Method;
use log::{error, info};
use roa::cors::Cors;
use roa::logger::logger;
use roa::preload::*;
use roa::router::{allow, Router};
use roa::websocket::Websocket;
use roa::App;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init()
        .map_err(|err| anyhow::anyhow!("fail to init tracing subscriber: {}", err))?;

    let router = Router::new().on(
        "/chat",
        allow(
            [Method::GET],
            Websocket::new(|_ctx, stream| async move {
                let (write, read) = stream.split();
                if let Err(err) = read.forward(write).await {
                    error!("{}", err);
                }
            }),
        ),
    );
    let app = App::new()
        .gate(logger)
        .gate(Cors::new())
        .end(router.routes("/")?);
    app.listen("127.0.0.1:8000", |addr| {
        info!("Server is listening on {}", addr)
    })?
    .await?;
    Ok(())
}
