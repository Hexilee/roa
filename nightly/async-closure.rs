#[feature(async_closure)]
use log::info;
use roa::preload::*;
use roa::{App, Context};
use std::error::Error as StdError;

#[async_std::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    pretty_env_logger::init();
    let app = App::new(()).end(async move |ctx: &mut Context<()>| {
        let stream = ctx.req.stream();
        ctx.resp.write_stream(stream);
        Ok(())
    });
    app.listen("127.0.0.1:8000", |addr| {
        info!("Server is listening on {}", addr)
    })?
    .await?;
    Ok(())
}
