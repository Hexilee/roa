use log::info;
use roa::preload::*;
use roa::App;
use tokio::spawn;

#[tokio::test]
async fn serve() -> Result<(), Box<dyn std::error::Error>> {
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
