use async_std::fs::File;
use async_std::io;
use async_std::path::Path;
use futures::stream::TryStreamExt;
use futures::StreamExt;
use log::info;
use roa::body::{DispositionType, PowerBody};
use roa::http::StatusCode;
use roa::logger::logger;
use roa::preload::*;
use roa::router::{get, post, Router};
use roa::{throw, App, Context};
use roa_multipart::MultipartForm;
use std::error::Error as StdError;

async fn get_form(ctx: &mut Context<()>) -> roa::Result {
    ctx.write_file("./assets/index.html", DispositionType::Inline)
        .await
}

async fn post_file(ctx: &mut Context<()>) -> roa::Result {
    let mut form = ctx.form();
    while let Some(item) = form.next().await {
        let field = item?;
        info!("{}", field.content_type());
        match field.content_disposition() {
            None => throw!(StatusCode::BAD_REQUEST, "content disposition not set"),
            Some(content_disposition) => match content_disposition.get_filename() {
                None => continue, // ignore non-file field
                Some(filename) => {
                    let path = Path::new("./upload");
                    let mut file = File::create(path.join(filename)).await?;
                    io::copy(&mut field.into_async_read(), &mut file).await?;
                }
            },
        }
    }
    Ok(())
}

#[async_std::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    pretty_env_logger::init();
    let router = Router::new()
        .on("/", get(get_form))
        .on("/file", post(post_file));
    let app = App::new(()).gate(logger).end(router.routes("/")?);
    app.listen("127.0.0.1:8000", |addr| {
        info!("Server is listening on {}", addr)
    })?
    .await?;
    Ok(())
}
