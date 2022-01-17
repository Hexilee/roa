use std::error::Error as StdError;
use std::path::Path;

use roa::body::{DispositionType, PowerBody};
use roa::logger::logger;
use roa::preload::*;
use roa::router::{get, post, Router};
use roa::{App, Context};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::info;
use tracing_subscriber::EnvFilter;

async fn get_form(ctx: &mut Context) -> roa::Result {
    ctx.write_file("./assets/index.html", DispositionType::Inline)
        .await
}

async fn post_file(ctx: &mut Context) -> roa::Result {
    let mut form = ctx.read_multipart().await?;
    while let Some(mut field) = form.next_field().await? {
        info!("{:?}", field.content_type());
        match field.file_name() {
            None => continue, // ignore non-file field
            Some(filename) => {
                let path = Path::new("./upload");
                let mut file = File::create(path.join(filename)).await?;
                while let Some(c) = field.chunk().await? {
                    file.write_all(&c).await?;
                }
            }
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init()
        .map_err(|err| anyhow::anyhow!("fail to init tracing subscriber: {}", err))?;

    let router = Router::new()
        .on("/", get(get_form))
        .on("/file", post(post_file));
    let app = App::new().gate(logger).end(router.routes("/")?);
    app.listen("127.0.0.1:8000", |addr| {
        info!("Server is listening on {}", addr)
    })?
    .await?;
    Ok(())
}
