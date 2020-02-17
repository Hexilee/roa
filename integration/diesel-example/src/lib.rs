#[macro_use]
extern crate diesel;

mod data_object;
mod endpoints;
mod models;
mod schema;

use diesel::r2d2::{ConnectionManager, Pool};
use diesel::sqlite::SqliteConnection;
use diesel::RunQueryDsl;

pub use std::error::Error as StdError;

type State = Pool<ConnectionManager<SqliteConnection>>;

pub fn create_pool() -> Result<State, Box<dyn StdError>> {
    let manager = ConnectionManager::<SqliteConnection>::new(":memory:");
    let pool = Pool::builder().build(manager)?;
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
    .execute(&*pool.get()?)?;
    Ok(pool)
}

pub use endpoints::post_router;
