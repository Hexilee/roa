use roa::preload::*;
use roa::router::Router;
use roa::App;
use tokio::spawn;

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
