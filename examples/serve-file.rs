#![feature(async_closure)]
use async_std::fs::File;
use roa::Group;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();
    Group::new()
        .handle_fn(async move |mut ctx, _next| {
            ctx.response.write(File::open("assets/welcome.html").await?);
            Ok(())
        })
        .app(())
        .listen("127.0.0.1:8000".parse()?)
        .await
        .map_err(Into::into)
}