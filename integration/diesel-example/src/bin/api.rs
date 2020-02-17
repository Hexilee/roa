use diesel_example::{create_pool, post_router, StdError};
use log::info;
use roa::core::App;
use roa::logger::logger;

#[async_std::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    pretty_env_logger::init();
    let mut app = App::new(create_pool().await?);
    app.gate(logger)
        .gate(post_router().routes("/post")?)
        .listen("127.0.0.1:8000", |addr| {
            info!("Server is listening on {}", addr)
        })?
        .await?;
    Ok(())
}
