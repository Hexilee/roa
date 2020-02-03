use roa::{
    compress::{compress, Level},
    logger, App, PowerBody,
};
use log::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();
    App::new(())
        .join(logger)
        .join(compress(Level::Balance))
        .join(|ctx, _next| async move { ctx.write_file("assets/welcome.html").await })
        .listen("127.0.0.1:8000".parse()?, || info!("Server is listenning on 127.0.0.1:8000"))
        .await
        .map_err(Into::into)
}
