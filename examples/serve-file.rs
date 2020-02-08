use log::info;
use roa::logger::logger;
use roa::preload::*;
use roa::router::Router;
use roa::{
    compress::{compress, Level},
    core::App,
};

#[async_std::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();
    let mut router = Router::new("/");
    router.on("/:filename")?.get(|ctx| async move {
        let filename = ctx.param("filename").await?;
        ctx.write_file(format!("assets/{}", &*filename)).await
    });
    App::new(())
        .gate(logger)
        .gate(compress(Level::Best))
        .end(router.handler()?)
        .listen("127.0.0.1:8000", |addr| {
            info!("Server is listening on {}", addr)
        })?
        .await
        .map_err(Into::into)
}
