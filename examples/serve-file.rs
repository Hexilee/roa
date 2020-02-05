use log::info;
use roa::logger::logger;
use roa::preload::*;
use roa::{
    compress::{compress, Level},
    App,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();
    App::new(())
        .gate(logger)
        .gate(compress(Level::Balance))
        .gate(|ctx, _next| async move { ctx.write_file("assets/welcome.html").await })
        .listen("127.0.0.1:8000", |addr| {
            info!("Server is listening on {}", addr)
        })?
        .await
        .map_err(Into::into)
}
