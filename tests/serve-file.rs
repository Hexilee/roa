use async_std::fs::read_to_string;
use async_std::task::spawn;
use http::header::ACCEPT_ENCODING;
use roa::body::DispositionType;
use roa::compress::Compress;
use roa::preload::*;
use roa::router::{get, Router};
use roa::{App, Context};

#[tokio::test]
async fn serve_static_file() -> Result<(), Box<dyn std::error::Error>> {
    async fn test(ctx: &mut Context) -> roa::Result {
        ctx.write_file("assets/author.txt", DispositionType::Inline)
            .await
    }
    let app = App::new(()).end(get(test));
    let (addr, server) = app.run()?;
    spawn(server);
    let resp = reqwest::get(&format!("http://{}", addr)).await?;
    assert_eq!("Hexilee", resp.text().await?);
    Ok(())
}

#[tokio::test]
async fn serve_router_variable() -> Result<(), Box<dyn std::error::Error>> {
    async fn test(ctx: &mut Context) -> roa::Result {
        let filename = ctx.must_param("filename")?;
        ctx.write_file(format!("assets/{}", &*filename), DispositionType::Inline)
            .await
    }
    let router = Router::new().on("/:filename", get(test));
    let app = App::new(()).end(router.routes("/")?);
    let (addr, server) = app.run()?;
    spawn(server);
    let resp = reqwest::get(&format!("http://{}/author.txt", addr)).await?;
    assert_eq!("Hexilee", resp.text().await?);
    Ok(())
}

#[tokio::test]
async fn serve_router_wildcard() -> Result<(), Box<dyn std::error::Error>> {
    async fn test(ctx: &mut Context) -> roa::Result {
        let path = ctx.must_param("path")?;
        ctx.write_file(format!("./{}", &*path), DispositionType::Inline)
            .await
    }
    let router = Router::new().on("/*{path}", get(test));
    let app = App::new(()).end(router.routes("/")?);
    let (addr, server) = app.run()?;
    spawn(server);
    let resp = reqwest::get(&format!("http://{}/assets/author.txt", addr)).await?;
    assert_eq!("Hexilee", resp.text().await?);
    Ok(())
}

#[tokio::test]
async fn serve_gzip() -> Result<(), Box<dyn std::error::Error>> {
    async fn test(ctx: &mut Context) -> roa::Result {
        ctx.write_file("assets/welcome.html", DispositionType::Inline)
            .await
    }
    let app = App::new(()).gate(Compress::default()).end(get(test));
    let (addr, server) = app.run()?;
    spawn(server);
    let client = reqwest::Client::builder().gzip(true).build()?;
    let resp = client
        .get(&format!("http://{}", addr))
        .header(ACCEPT_ENCODING, "gzip")
        .send()
        .await?;

    assert_eq!(
        read_to_string("assets/welcome.html").await?,
        resp.text().await?
    );
    Ok(())
}
