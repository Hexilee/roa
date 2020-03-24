//! This module provides integration with tokio-postgres.
//!
//! ### Example
//!
//! ```rust,no_run
//! use roa::{App, Context, Result};
//! use roa::pg::{connect, Client};
//! use std::sync::Arc;
//!
//! #[derive(Clone)]
//! struct State {
//!     pg: Arc<Client>
//! }
//!
//! async fn query(ctx: &mut Context<State>) -> Result {
//!     ctx.pg.query_one("SELECT * FROM user WHERE id=$1", &[&1]).await?;
//! }
//! ```

#[cfg(feature = "tls")]
#[cfg_attr(feature = "docs", doc(cfg(tls)))]
pub mod tls;
pub use tokio_postgres::{Client, Config};

mod client;
use crate::tcp::WrapStream;
use async_std::net::TcpStream;
use std::io;
use tokio_postgres::config::Host;
use tokio_postgres::tls::{NoTls, NoTlsStream, TlsConnect};
#[doc(inline)]
use tokio_postgres::Connection;

/// Default port of postgres.
const DEFAULT_PORT: u16 = 5432;

/// Try to get TCP hostname from postgres config.
#[inline]
fn try_tcp_host(config: &Config) -> io::Result<&str> {
    match config
        .get_hosts()
        .into_iter()
        .filter_map(|host| {
            if let Host::Tcp(value) = host {
                Some(value)
            } else {
                None
            }
        })
        .next()
    {
        Some(host) => Ok(host),
        None => Err(io::Error::new(
            io::ErrorKind::Other,
            "At least one tcp hostname is required",
        )),
    }
}

/// Establish connection to postgres server by async_std::net::TcpStream.
#[inline]
async fn connect_stream(config: &Config) -> io::Result<TcpStream> {
    let host = try_tcp_host(&config)?;
    let port = config
        .get_ports()
        .into_iter()
        .map(|port| *port)
        .next()
        .unwrap_or(DEFAULT_PORT);

    TcpStream::connect((host, port)).await
}

/// Connect to postgres server.
#[inline]
pub async fn connect(
    config: &Config,
) -> io::Result<(Client, Connection<WrapStream<TcpStream>, NoTlsStream>)> {
    let stream = connect_stream(config).await?;
    let (client, conn) = config
        .connect_raw(WrapStream(stream), NoTls)
        .await
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
    Ok((Client(client), conn))
}
