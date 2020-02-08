use askama::Template;
use async_std::fs;
use async_std::path::{Path, PathBuf};
use async_std::prelude::*;
use log::info;
use roa::compress::{compress, Level};
use roa::core::{throw, App, Context, Next, Result, StatusCode};
use roa::logger::logger;
use roa::preload::*;
use roa::router::Router;
use std::result::Result as StdResult;

#[derive(Template)]
#[template(path = "directory.html")]
struct Dir {
    dirs: Vec<DirInfo>,
    files: Vec<FileInfo>,
}

struct DirInfo {}

struct FileInfo {}

impl Default for Dir {
    fn default() -> Self {
        Self {
            dirs: Vec::new(),
            files: Vec::new(),
        }
    }
}

async fn path_checker(ctx: Context<()>, next: Next) -> Result {
    if ctx.param("path").await?.contains("..") {
        throw(StatusCode::BAD_REQUEST, "invalid path")
    } else {
        next().await
    }
}

async fn serve_path(ctx: Context<()>) -> Result {
    let base_path = Path::new("./");
    let path_buf = base_path.join(ctx.param("path").await?.as_ref());
    let path = path_buf.as_path();
    if path.is_file().await {
        ctx.write_file(path).await
    } else if path.is_dir().await {
        serve_dir(ctx, path_buf).await
    } else {
        throw(StatusCode::NOT_FOUND, "path not found")
    }
}

async fn serve_dir(ctx: Context<()>, root: PathBuf) -> Result {
    let mut entries = root.read_dir().await?;
    let mut dir = Dir::default();
    while let Some(res) = entries.next().await {
        let entry = res?;
        info!("{}", entry.file_name().to_string_lossy());
    }
    ctx.render(&dir).await
}

#[async_std::main]
async fn main() -> StdResult<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();
    let mut router = Router::new("/");
    router.on("/static/*{path}")?.get(serve_path);
    App::new(())
        .gate(logger)
        .gate(compress(Level::Best))
        .gate(path_checker)
        .end(router.handler()?)
        .listen("127.0.0.1:8000", |addr| {
            info!("Server is listening on {}", addr)
        })?
        .await
        .map_err(Into::into)
}
