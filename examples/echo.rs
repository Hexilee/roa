use log::info;
use roa::preload::*;
use roa::{App, Context};
use std::error::Error as StdError;

async fn echo(ctx: &mut Context) -> roa::Result {
    let stream = ctx.req.stream();
    ctx.resp.write_stream(stream);
    Ok(())
}

#[async_std::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    pretty_env_logger::init();
    let app = App::new(()).end(echo);
    app.listen("127.0.0.1:8000", |addr| {
        info!("Server is listening on {}", addr)
    })?
    .await?;
    Ok(())
}
