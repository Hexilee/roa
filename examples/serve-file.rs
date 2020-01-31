use async_std::fs::File;
use roa::Middleware;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();
    Middleware::new()
        .join(|mut ctx, _next| {
            async move {
                ctx.response.write(File::open("assets/welcome.html").await?);
                Ok(())
            }
        })
        .app(())
        .listen("127.0.0.1:8000".parse()?)
        .await
        .map_err(Into::into)
}
