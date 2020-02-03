use roa::{
    compress::{compress, Level},
    logger, App, PowerBody,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();
    App::new(())
        .join(logger)
        .join(compress(Level::Balance))
        .join(|ctx, _next| async move { ctx.write_file("assets/welcome.html").await })
        .listen("127.0.0.1:8000".parse()?)
        .await
        .map_err(Into::into)
}
