#![feature(async_closure)]
use async_std::fs::File;
use roa::App;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();
    App::builder()
        .handle_fn(async move |mut ctx, _next| {
            ctx.response.write(File::open("assets/welcome.html").await?);
            Ok(())
        })
        .model(())
        .listen("127.0.0.1:8000".parse()?)
        .await
        .map_err(Into::into)
}
