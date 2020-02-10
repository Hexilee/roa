use askama::Template;
use async_std::path::{Path, PathBuf};
use async_std::prelude::*;
use bytesize::ByteSize;
use chrono::offset::Local;
use chrono::DateTime;
use log::info;
use roa::compress::{compress, Level};
use roa::core::{throw, App, Context, Next, Result, StatusCode};
use roa::logger::logger;
use roa::preload::*;
use roa::router::Router;
use std::borrow::Cow;
use std::result::Result as StdResult;
use std::time::SystemTime;

#[derive(Template)]
#[template(path = "directory.html")]
struct Dir<'a> {
    title: &'a str,
    root: &'a str,
    dirs: Vec<DirInfo>,
    files: Vec<FileInfo>,
}

struct DirInfo {
    link: String,
    name: String,
    modified: String,
}

struct FileInfo {
    link: String,
    name: String,
    modified: String,
    size: String,
}

impl<'a> Dir<'a> {
    fn new(title: &'a str, root: &'a str) -> Self {
        Self {
            title,
            root,
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
    let base_path = Path::new(".");
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
    let title = root
        .file_name()
        .map(|os_str| os_str.to_string_lossy())
        .unwrap_or(Cow::Borrowed("."));
    let root_str = root.to_string_lossy();
    let mut dir = Dir::new(&title, &root_str);
    while let Some(res) = entries.next().await {
        let entry = res?;
        let metadata = entry.metadata().await?;
        if metadata.is_dir() {
            dir.dirs.push(DirInfo {
                link: root.join(entry.file_name()).to_string_lossy().to_string(),
                name: entry.file_name().to_string_lossy().to_string(),
                modified: format_time(metadata.modified()?),
            })
        }
        if metadata.is_file() {
            dir.files.push(FileInfo {
                link: root.join(entry.file_name()).to_string_lossy().to_string(),
                name: entry.file_name().to_string_lossy().to_string(),
                modified: format_time(metadata.modified()?),
                size: ByteSize(metadata.len()).to_string(),
            })
        }
    }
    ctx.render(&dir).await
}

fn format_time(time: SystemTime) -> String {
    let datetime: DateTime<Local> = time.into();
    datetime.format("%d/%m/%Y %T").to_string()
}

#[async_std::main]
async fn main() -> StdResult<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();
    let mut router = Router::new("/");
    router
        .on("/")?
        .get(|ctx| serve_dir(ctx, Path::new(".").to_path_buf()));
    router.on("/*{path}")?.gate(path_checker).get(serve_path);
    App::new(())
        .gate_fn(logger)
        .gate_fn(compress(Level::Best))
        .end_fn(router.handler()?)
        .listen("127.0.0.1:8000", |addr| {
            info!("Server is listening on {}", addr)
        })?
        .await
        .map_err(Into::into)
}
