#[macro_use]
extern crate diesel;

mod data_object;
mod endpoints;
pub mod models;
pub mod schema;

use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use roa_diesel::{make_pool, Pool};

#[derive(Clone)]
pub struct State(pub Pool<SqliteConnection>);

impl AsRef<Pool<SqliteConnection>> for State {
    fn as_ref(&self) -> &Pool<SqliteConnection> {
        &self.0
    }
}

pub fn create_pool() -> anyhow::Result<State> {
    let pool = make_pool(":memory:")?;
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
    Ok(State(pool))
}

pub use endpoints::post_router;
