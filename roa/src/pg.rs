//! This module provides integration with tokio-postgres.
//!
//! ### Example
//!
//! ```rust,no_run
//! use roa::{App, Context, throw};
//! use roa::http::StatusCode;
//! use roa::pg::{connect, Client};
//! use std::sync::Arc;
//! use std::error::Error;
//! use roa::query::query_parser;
//! use roa::preload::*;
//! use async_std::task::spawn;
//!
//! #[derive(Clone)]
//! struct State {
//!     pg: Arc<Client>
//! }
//!
//! impl State {
//!     pub async fn new(pg_url: &str) -> Result<Self, Box<dyn Error>> {
//!         let (client, conn) = connect(&pg_url.parse()?).await?;
//!         spawn(conn);
//!         Ok(Self {pg: Arc::new(client)})
//!     }
//! }
//!
//! async fn query(ctx: &mut Context<State>) -> roa::Result {
//!     let id: u32 = ctx.must_query("id")?.parse()?;
//!     match ctx.pg.query_opt("SELECT * FROM user WHERE id=$1", &[&id]).await? {
//!         Some(row) => {
//!             let value: String = row.get(0);
//!             ctx.write_text(value);
//!             Ok(())
//!         }         
//!         None => throw!(StatusCode::NOT_FOUND),
//!     }
//! }
//!
//! #[async_std::main]
//! async fn main() -> Result<(), Box<dyn Error>> {
//!     let url = "host=localhost user=postgres";
//!     let state = State::new(url).await?;
//!     App::new(state)
//!         .gate(query_parser)
//!         .end(query)
//!         .listen("127.0.0.1:0", |addr| {
//!             println!("Server is listening on {}", addr)
//!         })?.await?;
//!     Ok(())
//! }
//! ```

#[cfg(feature = "tls")]
#[cfg_attr(feature = "docs", doc(cfg(tls)))]
pub mod tls;
pub use tokio_postgres::{Client, Config};

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
    config
        .connect_raw(WrapStream(stream), NoTls)
        .await
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
}
