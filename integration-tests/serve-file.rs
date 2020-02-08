use async_std::fs::read_to_string;
use async_std::task::spawn;
use http::header::ACCEPT_ENCODING;
use roa::compress::{compress, Level};
use roa::core::App;
use roa::preload::*;
use roa::router::Router;

#[tokio::test]
async fn serve_static_file() -> Result<(), Box<dyn std::error::Error>> {
    let (addr, server) = App::new(())
        .gate(|ctx, _next| async move { ctx.write_file("assets/author.txt").await })
        .run_local()?;
    spawn(server);
    let resp = reqwest::get(&format!("http://{}", addr)).await?;
    assert_eq!("Hexilee", resp.text().await?);
    Ok(())
}

#[tokio::test]
async fn serve_router_variable() -> Result<(), Box<dyn std::error::Error>> {
    let mut router = Router::new("/");
    router.on("/:filename")?.get(|ctx| async move {
        let filename = ctx.param("filename").await?;
        ctx.write_file(format!("assets/{}", &*filename)).await
    });
    let (addr, server) = App::new(()).gate(router.handler()?).run_local()?;
    spawn(server);
    let resp = reqwest::get(&format!("http://{}/author.txt", addr)).await?;
    assert_eq!("Hexilee", resp.text().await?);
    Ok(())
}

#[tokio::test]
async fn serve_router_wildcard() -> Result<(), Box<dyn std::error::Error>> {
    let mut router = Router::new("/");
    router.on("/*{path}")?.get(|ctx| async move {
        let path = ctx.param("path").await?;
        ctx.write_file(format!("./{}", &*path)).await
    });
    let (addr, server) = App::new(()).gate(router.handler()?).run_local()?;
    spawn(server);
    let resp = reqwest::get(&format!("http://{}/assets/author.txt", addr)).await?;
    assert_eq!("Hexilee", resp.text().await?);
    Ok(())
}

#[tokio::test]
async fn serve_gzip() -> Result<(), Box<dyn std::error::Error>> {
    let (addr, server) = App::new(())
        .gate(compress(Level::Best))
        .gate(|ctx, _next| async move { ctx.write_file("assets/welcome.html").await })
        .run_local()?;
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
