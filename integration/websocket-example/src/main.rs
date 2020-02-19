use futures::StreamExt;
use http::Method;
use log::{error, info};
use roa::core::App;
use roa::logger::logger;
use roa::router::Router;
use roa_websocket::Websocket;
use std::error::Error as StdError;

#[async_std::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    pretty_env_logger::init();
    let mut app = App::new(());
    let mut router = Router::<()>::new();
    router.end(
        [Method::GET].as_ref(),
        "/chat",
        Websocket::new(|_state: (), stream| async move {
            let (write, read) = stream.split();
            if let Err(err) = read.forward(write).await {
                error!("forward err: {}", err);
            }
        }),
    );
    app.gate(logger)
        .gate(router.routes("/")?)
        .listen("127.0.0.1:8000", |addr| {
            info!("Server is listening on {}", addr)
        })?
        .await?;
    Ok(())
}
