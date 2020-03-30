use log::info;
use roa::preload::*;
use roa::{App, Context};
use std::error::Error as StdError;

async fn hello(ctx: &mut Context) -> roa::Result {
    ctx.write("Hello, World");
    Ok(())
}

#[async_std::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    pretty_env_logger::init();
    let app = App::new(()).end(hello);
    app.listen("127.0.0.1:8000", |addr| {
        info!("Server is listening on {}", addr)
    })?
    .await?;
    Ok(())
}
