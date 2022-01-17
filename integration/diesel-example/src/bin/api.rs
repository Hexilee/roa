use diesel_example::{create_pool, post_router};
use roa::logger::logger;
use roa::preload::*;
use roa::App;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init()
        .map_err(|err| anyhow::anyhow!("fail to init tracing subscriber: {}", err))?;
    let app = App::state(create_pool()?)
        .gate(logger)
        .end(post_router().routes("/post")?);
    app.listen("127.0.0.1:8000", |addr| {
        info!("Server is listening on {}", addr)
    })?
    .await?;
    Ok(())
}
