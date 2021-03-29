//! RUST_LOG=info cargo run --example serve-file,
//! then request http://127.0.0.1:8000.

use std::borrow::Cow;
use std::result::Result as StdResult;
use std::time::SystemTime;

use askama::Template;
use async_std::path::Path;
use async_std::prelude::*;
use bytesize::ByteSize;
use chrono::offset::Local;
use chrono::DateTime;
use log::info;
use roa::body::DispositionType::*;
use roa::compress::Compress;
use roa::http::StatusCode;
use roa::logger::logger;
use roa::preload::*;
use roa::router::{get, Router};
use roa::{throw, App, Context, Next, Result};

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

async fn path_checker(ctx: &mut Context, next: Next<'_>) -> Result {
    if ctx.must_param("path")?.contains("..") {
        throw!(StatusCode::BAD_REQUEST, "invalid path")
    } else {
        next.await
    }
}

async fn serve_path(ctx: &mut Context) -> Result {
    let path_value = ctx.must_param("path")?;
    let path = path_value.as_ref();
    let file_path = Path::new(".").join(path);
    if file_path.is_file().await {
        ctx.write_file(file_path, Inline).await
    } else if file_path.is_dir().await {
        serve_dir(ctx, path).await
    } else {
        throw!(StatusCode::NOT_FOUND, "path not found")
    }
}

async fn serve_root(ctx: &mut Context) -> Result {
    serve_dir(ctx, "").await
}

async fn serve_dir(ctx: &mut Context, path: &str) -> Result {
    let uri_path = Path::new("/").join(path);
    let mut entries = Path::new(".").join(path).read_dir().await?;
    let title = uri_path
        .file_name()
        .map(|os_str| os_str.to_string_lossy())
        .unwrap_or(Cow::Borrowed("/"));
    let root_str = uri_path.to_string_lossy();
    let mut dir = Dir::new(&title, &root_str);
    while let Some(res) = entries.next().await {
        let entry = res?;
        let metadata = entry.metadata().await?;
        if metadata.is_dir() {
            dir.dirs.push(DirInfo {
                link: uri_path
                    .join(entry.file_name())
                    .to_string_lossy()
                    .to_string(),
                name: entry.file_name().to_string_lossy().to_string(),
                modified: format_time(metadata.modified()?),
            })
        }
        if metadata.is_file() {
            dir.files.push(FileInfo {
                link: uri_path
                    .join(entry.file_name())
                    .to_string_lossy()
                    .to_string(),
                name: entry.file_name().to_string_lossy().to_string(),
                modified: format_time(metadata.modified()?),
                size: ByteSize(metadata.len()).to_string(),
            })
        }
    }
    ctx.render(&dir)
}

fn format_time(time: SystemTime) -> String {
    let datetime: DateTime<Local> = time.into();
    datetime.format("%d/%m/%Y %T").to_string()
}

#[async_std::main]
async fn main() -> StdResult<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();
    let wildcard_router = Router::new().gate(path_checker).on("/", get(serve_path));
    let router = Router::new()
        .on("/", serve_root)
        .include("/*{path}", wildcard_router);
    let app = App::new()
        .gate(logger)
        .gate(Compress::default())
        .end(router.routes("/")?);
    app.listen("127.0.0.1:8000", |addr| {
        info!("Server is listening on {}", addr)
    })?
    .await
    .map_err(Into::into)
}
