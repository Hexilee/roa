use crate::WrapError;
use diesel::r2d2::{ConnectionManager, PoolError};
use diesel::Connection;
use r2d2::{Builder, PooledConnection};
use roa_core::{async_trait, State, SyncContext};
use std::time::Duration;

pub type Pool<Conn> = r2d2::Pool<ConnectionManager<Conn>>;

pub type WrapConnection<Conn> = PooledConnection<ConnectionManager<Conn>>;

pub fn make_pool<Conn>(url: impl Into<String>) -> Result<Pool<Conn>, PoolError>
where
    Conn: Connection + 'static,
{
    r2d2::Pool::new(ConnectionManager::<Conn>::new(url))
}

pub fn builder<Conn>() -> Builder<ConnectionManager<Conn>>
where
    Conn: Connection + 'static,
{
    r2d2::Pool::builder()
}

#[async_trait]
pub trait AsyncPool<Conn>
where
    Conn: Connection + 'static,
{
    async fn get_conn(&self) -> Result<WrapConnection<Conn>, WrapError>;

    async fn get_timeout(
        &self,
        timeout: Duration,
    ) -> Result<WrapConnection<Conn>, WrapError>;

    async fn pool_state(&self) -> r2d2::State;
}

#[async_trait]
impl<S, Conn> AsyncPool<Conn> for SyncContext<S>
where
    S: State + AsRef<Pool<Conn>>,
    Conn: Connection + 'static,
{
    async fn get_conn(&self) -> Result<WrapConnection<Conn>, WrapError> {
        let pool = self.as_ref().clone();
        Ok(self.exec.spawn_blocking(move || pool.get()).await?)
    }

    async fn get_timeout(
        &self,
        timeout: Duration,
    ) -> Result<WrapConnection<Conn>, WrapError> {
        let pool = self.as_ref().clone();
        Ok(self
            .exec
            .spawn_blocking(move || pool.get_timeout(timeout))
            .await?)
    }

    async fn pool_state(&self) -> r2d2::State {
        let pool = self.as_ref().clone();
        self.exec.spawn_blocking(move || pool.state()).await
    }
}
