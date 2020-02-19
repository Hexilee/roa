use async_std::fs::File;
use async_std::io;
use async_std::path::Path;
use futures::stream::TryStreamExt;
use futures::StreamExt;
use log::info;
use roa::body::{DispositionType, PowerBody};
use roa::core::{throw, App, StatusCode};
use roa::logger::logger;
use roa::router::Router;
use std::error::Error as StdError;

#[async_std::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    pretty_env_logger::init();
    let mut app = App::new(());
    let mut router = Router::<()>::new();
    router.get("/", |mut ctx| async move {
        ctx.write_file("./assets/index.html", DispositionType::Inline)
            .await
    });
    router.post("/file", |mut ctx| async move {
        let mut form = ctx.multipart();
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
    });
    app.gate(logger)
        .gate(router.routes("/")?)
        .listen("127.0.0.1:8000", |addr| {
            info!("Server is listening on {}", addr)
        })?
        .await?;
    Ok(())
}
