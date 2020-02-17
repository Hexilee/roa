// Create assets/upload directory before running this example.

use async_std::fs::File;
use async_std::io;
use log::info;
use roa::core::App;
use roa::router::Router;
use std::error::Error as StdError;

// Post to http://127.0.0.1:8000/file
#[async_std::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    pretty_env_logger::init();
    let mut app = App::new(());
    let mut router = Router::new();
    router.post("/", |mut ctx| async move {
        // content-disposition is not standard in request header.
        // use a custom appointment to transfer filename
        // TODO: using multipart-form.
        let mut file = File::create("./assets/upload/filename").await?;
        let mut req = ctx.req_mut().await;
        // double deref: RwLockWriteGuard<Request> -> Request -> Body
        io::copy(&mut **req, &mut file).await?;
        Ok(())
    });
    app.gate(router.routes("/file")?)
        .listen("127.0.0.1:8000", |addr| {
            info!("Server is listening on {}", addr)
        })?
        .await?;
    Ok(())
}
