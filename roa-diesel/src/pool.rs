use std::time::Duration;

use diesel::r2d2::{ConnectionManager, PoolError};
use diesel::Connection;
use r2d2::{Builder, PooledConnection};
use roa::{async_trait, Context, State, Status};

/// An alias for r2d2::Pool<diesel::r2d2::ConnectionManager<Conn>>.
pub type Pool<Conn> = r2d2::Pool<ConnectionManager<Conn>>;

/// An alias for r2d2::PooledConnection<diesel::r2d2::ConnectionManager<Conn>>.
pub type WrapConnection<Conn> = PooledConnection<ConnectionManager<Conn>>;

/// Create a connection pool.
///
/// ### Example
///
/// ```
/// use roa_diesel::{make_pool, Pool};
/// use diesel::sqlite::SqliteConnection;
/// use std::error::Error;
///
/// # fn main() -> Result<(), Box<dyn Error>> {
/// let pool: Pool<SqliteConnection> = make_pool(":memory:")?;
/// Ok(())
/// # }
/// ```
pub fn make_pool<Conn>(url: impl Into<String>) -> Result<Pool<Conn>, PoolError>
where
    Conn: Connection + 'static,
{
    r2d2::Pool::new(ConnectionManager::<Conn>::new(url))
}

/// Create a pool builder.
pub fn builder<Conn>() -> Builder<ConnectionManager<Conn>>
where
    Conn: Connection + 'static,
{
    r2d2::Pool::builder()
}

/// A context extension to access r2d2 pool asynchronously.
#[async_trait]
pub trait AsyncPool<Conn>
where
    Conn: Connection + 'static,
{
    /// Retrieves a connection from the pool.
    ///
    /// Waits for at most the configured connection timeout before returning an
    /// error.
    ///
    /// ```
    /// use roa::{Context, Result};
    /// use diesel::sqlite::SqliteConnection;
    /// use roa_diesel::preload::AsyncPool;
    /// use roa_diesel::Pool;
    /// use diesel::r2d2::ConnectionManager;
    ///
    /// #[derive(Clone)]
    /// struct State(Pool<SqliteConnection>);
    ///
    /// impl AsRef<Pool<SqliteConnection>> for State {
    ///     fn as_ref(&self) -> &Pool<SqliteConnection> {
    ///         &self.0
    ///     }
    /// }
    ///
    /// async fn get(ctx: Context<State>) -> Result {
    ///     let conn = ctx.get_conn().await?;
    ///     // handle conn
    ///     Ok(())
    /// }
    /// ```
    async fn get_conn(&self) -> Result<WrapConnection<Conn>, Status>;

    /// Retrieves a connection from the pool, waiting for at most `timeout`
    ///
    /// The given timeout will be used instead of the configured connection
    /// timeout.
    async fn get_timeout(&self, timeout: Duration) -> Result<WrapConnection<Conn>, Status>;

    /// Returns information about the current state of the pool.
    async fn pool_state(&self) -> r2d2::State;
}

#[async_trait]
impl<S, Conn> AsyncPool<Conn> for Context<S>
where
    S: State + AsRef<Pool<Conn>>,
    Conn: Connection + 'static,
{
    #[inline]
    async fn get_conn(&self) -> Result<WrapConnection<Conn>, Status> {
        let pool = self.as_ref().clone();
        Ok(self.exec.spawn_blocking(move || pool.get()).await?)
    }

    #[inline]
    async fn get_timeout(&self, timeout: Duration) -> Result<WrapConnection<Conn>, Status> {
        let pool = self.as_ref().clone();
        Ok(self
            .exec
            .spawn_blocking(move || pool.get_timeout(timeout))
            .await?)
    }

    #[inline]
    async fn pool_state(&self) -> r2d2::State {
        let pool = self.as_ref().clone();
        self.exec.spawn_blocking(move || pool.state()).await
    }
}
