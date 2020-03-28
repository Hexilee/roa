use futures::StreamExt;
use http::Method;
use log::{error, info};
use roa::cors::Cors;
use roa::logger::logger;
use roa::preload::*;
use roa::router::{allow, Router};
use roa::websocket::Websocket;
use roa::App;
use std::error::Error as StdError;

#[async_std::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    pretty_env_logger::init();
    let router = Router::new().on(
        "/chat",
        allow(
            [Method::GET],
            Websocket::new(|_ctx, stream| async move {
                let (write, read) = stream.split();
                if let Err(err) = read.forward(write).await {
                    error!("forward err: {}", err);
                }
            }),
        ),
    );
    let app = App::new(())
        .gate(logger)
        .gate(Cors::new())
        .end(router.routes("/")?);
    app.listen("127.0.0.1:8000", |addr| {
        info!("Server is listening on {}", addr)
    })?
    .await?;
    Ok(())
}
