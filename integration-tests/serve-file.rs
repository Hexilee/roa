use log::info;
use roa::preload::*;
use roa::router::Router;
use roa::App;
use tokio::spawn;

#[tokio::test]
async fn serve_static_file() -> Result<(), Box<dyn std::error::Error>> {
    spawn(
        App::new(())
            .gate(|ctx, _next| async move { ctx.write_file("assets/author.txt").await })
            .listen("127.0.0.1:8000".parse()?, || {
                info!("Server is listening on 127.0.0.1:8000")
            }),
    );
    let resp = reqwest::get("http://127.0.0.1:8000").await?;
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
    spawn(
        App::new(())
            .gate(router.handler()?)
            .listen("127.0.0.1:8000".parse()?, || {
                info!("Server is listening on 127.0.0.1:8000")
            }),
    );
    let resp = reqwest::get("http://127.0.0.1:8000/author.txt").await?;
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
    spawn(
        App::new(())
            .gate(router.handler()?)
            .listen("127.0.0.1:8000".parse()?, || {
                info!("Server is listening on 127.0.0.1:8000")
            }),
    );
    let resp = reqwest::get("http://127.0.0.1:8000/assets/author.txt").await?;
    assert_eq!("Hexilee", resp.text().await?);
    Ok(())
}
