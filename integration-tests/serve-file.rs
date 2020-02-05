use log::info;
use roa::preload::*;
use roa::router::Router;
use roa::App;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use tokio::spawn;

fn random_addr() -> SocketAddr {
    let loopback = Ipv4Addr::new(127, 0, 0, 1);
    let socket = SocketAddrV4::new(loopback, 0);
    socket.into()
}

#[tokio::test]
async fn serve_static_file() -> Result<(), Box<dyn std::error::Error>> {
    let addr = random_addr();
    spawn(
        App::new(())
            .gate(|ctx, _next| async move { ctx.write_file("assets/author.txt").await })
            .listen(addr, || info!("Server is listening on {}", addr)),
    );
    let resp = reqwest::get(&format!("http://{}", addr)).await?;
    assert_eq!("Hexilee", resp.text().await?);
    Ok(())
}

#[tokio::test]
async fn serve_router_variable() -> Result<(), Box<dyn std::error::Error>> {
    let addr = random_addr();
    let mut router = Router::new("/");
    router.on("/:filename")?.get(|ctx| async move {
        let filename = ctx.param("filename").await?;
        ctx.write_file(format!("assets/{}", &*filename)).await
    });
    spawn(
        App::new(())
            .gate(router.handler()?)
            .listen(addr, || info!("Server is listening on {}", addr)),
    );
    let resp = reqwest::get(&format!("http://{}/author.txt", addr)).await?;
    assert_eq!("Hexilee", resp.text().await?);
    Ok(())
}

#[tokio::test]
async fn serve_router_wildcard() -> Result<(), Box<dyn std::error::Error>> {
    let addr = random_addr();
    let mut router = Router::new("/");
    router.on("/*{path}")?.get(|ctx| async move {
        let path = ctx.param("path").await?;
        ctx.write_file(format!("./{}", &*path)).await
    });
    spawn(
        App::new(())
            .gate(router.handler()?)
            .listen(addr, || info!("Server is listening on {}", addr)),
    );
    let resp = reqwest::get(&format!("http://{}/assets/author.txt", addr)).await?;
    assert_eq!("Hexilee", resp.text().await?);
    Ok(())
}
