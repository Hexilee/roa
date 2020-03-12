use log::info;
use roa::preload::*;
use roa::App;
use std::error::Error as StdError;

#[async_std::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    pretty_env_logger::init();
    let mut app = App::new(());
    app.call(|mut ctx| async move {
        let stream = ctx.req_mut().stream();
        ctx.resp_mut().write_stream(stream);
        Ok(())
    });
    app.listen("127.0.0.1:8000", |addr| {
        info!("Server is listening on {}", addr)
    })?
    .await?;
    Ok(())
}
