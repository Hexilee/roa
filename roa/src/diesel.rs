//! This crate provides two context extensions.
//!
//! ### AsyncPool
//! A context extension to access r2d2 pool asynchronously.
//!
//! ```
//! use roa::{Context, Result};
//! use diesel::sqlite::SqliteConnection;
//! use roa::diesel::{Pool, AsyncPool};
//! use diesel::r2d2::ConnectionManager;
//!
//! #[derive(Clone)]
//! struct State(Pool<SqliteConnection>);
//!
//! impl AsRef<Pool<SqliteConnection>> for State {
//!     fn as_ref(&self) -> &Pool<SqliteConnection> {
//!         &self.0
//!     }
//! }
//!
//! async fn get(ctx: Context<State>) -> Result {
//!     let conn = ctx.get_conn().await?;
//!     // handle conn
//!     Ok(())
//! }
//! ```
//!
//! ### SqlQuery
//! A context extension to execute diesel dsl asynchronously.
//!
//! Refer to [integration example](https://github.com/Hexilee/roa/tree/master/integration/diesel-example)
//! for more usage.
//!

mod async_ext;
mod err;
mod pool;

#[doc(inline)]
pub use async_ext::SqlQuery;

#[doc(inline)]
pub use err::WrapError;

#[doc(inline)]
pub use pool::{builder, make_pool, AsyncPool, Pool, WrapConnection};
