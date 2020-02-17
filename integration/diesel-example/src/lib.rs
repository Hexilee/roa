#[macro_use]
extern crate diesel;

mod data_object;
mod endpoints;
mod models;
mod schema;

use diesel::sqlite::SqliteConnection;
use roa_diesel::{AsyncQuery, BuilderExt, Pool};

pub type State = Pool<SqliteConnection>;
pub use std::error::Error as StdError;

pub async fn create_pool() -> Result<State, Box<dyn StdError>> {
    let pool = Pool::builder().build_on(":memory:")?;
    diesel::sql_query(
        r"
        CREATE TABLE posts (
          id INTEGER PRIMARY KEY,
          title VARCHAR NOT NULL,
          body TEXT NOT NULL,
          published BOOLEAN NOT NULL DEFAULT 'f'
        )
    ",
    )
    .execute_async(pool.get().await?)
    .await?;
    Ok(pool)
}

pub use endpoints::post_router;
