#[macro_use]
extern crate diesel;

mod data_object;
mod endpoints;
mod models;
mod schema;

use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use roa_diesel::{MakePool, Pool};

pub type State = Pool<SqliteConnection>;

pub use std::error::Error as StdError;

pub fn create_pool() -> Result<State, Box<dyn StdError>> {
    let pool = MakePool::make(":memory:")?;
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

use diesel::r2d2::ConnectionManager;
pub use endpoints::post_router;
