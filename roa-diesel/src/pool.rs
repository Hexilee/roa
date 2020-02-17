use async_std::task::spawn;
use diesel::r2d2::{ConnectionManager, PoolError};
use diesel::Connection;
use r2d2::{Builder, PooledConnection};
use roa_core::{Error, StatusCode};
use std::time::Duration;

pub trait BuilderExt<Conn>
where
    Conn: Connection + Send + 'static,
{
    fn build_on<S: Into<String>>(self, url: S) -> Result<Pool<Conn>, PoolError>;
}

impl<Conn> BuilderExt<Conn> for Builder<ConnectionManager<Conn>>
where
    Conn: Connection + Send + 'static,
{
    fn build_on<S: Into<String>>(self, url: S) -> Result<Pool<Conn>, PoolError> {
        self.build(ConnectionManager::<Conn>::new(url))
            .map(Into::into)
    }
}

pub type WrapConnection<Conn> = PooledConnection<ConnectionManager<Conn>>;

fn handle_pool_error(err: PoolError) -> Error {
    Error::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Database connection pool error: {}", err),
        false,
    )
}

pub struct Pool<Conn>(r2d2::Pool<ConnectionManager<Conn>>)
where
    Conn: Connection + Send + 'static;

impl<Conn> Pool<Conn>
where
    Conn: Connection + Send + 'static,
{
    pub fn new(url: impl Into<String>) -> Result<Pool<Conn>, PoolError> {
        r2d2::Pool::new(ConnectionManager::<Conn>::new(url)).map(Into::into)
    }

    pub fn builder() -> Builder<ConnectionManager<Conn>> {
        r2d2::Pool::builder()
    }

    pub async fn get(&self) -> Result<WrapConnection<Conn>, Error> {
        let pool = self.0.clone();
        spawn(async move { pool.get() })
            .await
            .map_err(handle_pool_error)
    }

    pub async fn get_timeout(
        &self,
        timeout: Duration,
    ) -> Result<WrapConnection<Conn>, Error> {
        let pool = self.0.clone();
        spawn(async move { pool.get_timeout(timeout) })
            .await
            .map_err(handle_pool_error)
    }

    pub async fn state(&self) -> r2d2::State {
        let pool = self.0.clone();
        spawn(async move { pool.state() }).await
    }
}

impl<Conn> Clone for Pool<Conn>
where
    Conn: Connection + Send + 'static,
{
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<Conn> From<r2d2::Pool<ConnectionManager<Conn>>> for Pool<Conn>
where
    Conn: Connection + Send + 'static,
{
    fn from(pool: r2d2::Pool<ConnectionManager<Conn>>) -> Self {
        Self(pool)
    }
}
