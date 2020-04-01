use log::info;
use roa::logger::logger;
use roa::preload::*;
use roa::App;
use std::error::Error as StdError;

#[async_std::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    pretty_env_logger::init();
    let app = App::new()
        .gate(logger)
        .end(include_str!("../assets/welcome.html"));
    app.listen("127.0.0.1:8000", |addr| {
        info!("Server is listening on {}", addr)
    })?
    .await?;
    Ok(())
}
