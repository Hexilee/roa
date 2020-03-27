//! This submodule provides tls support for postgres.
//!
//! ### Example
//!
//! ```rust,no_run
//! use roa::{App, Context, throw};
//! use roa::http::StatusCode;
//! use roa_pg::{Client, connect_tls};
//! use roa_pg::ClientConfig;
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
//!         let (client, conn) = connect_tls(&pg_url.parse()?, ClientConfig::new()).await?;
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
//!     let url = "postgres://fred:secret@localhost/test";
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

pub use tokio_rustls::rustls::ClientConfig;

use async_std::net::TcpStream;
use bytes::{Buf, BufMut};
use roa::tcp::AsyncStream;
use std::future::Future;
use std::io;
use std::mem::MaybeUninit;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_postgres::config::{Config, Host};
use tokio_postgres::tls::TlsConnect;
use tokio_postgres::tls::{self, ChannelBinding};
use tokio_postgres::{Client, Connection};
use tokio_rustls::client;
use tokio_rustls::TlsConnector;
use webpki::DNSNameRef;

/// Default port of postgres.
const DEFAULT_PORT: u16 = 5432;

/// Try to get TCP hostname from postgres config.
#[inline]
fn try_tcp_host(config: &Config) -> io::Result<&str> {
    match config
        .get_hosts()
        .iter()
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
        .iter()
        .copied()
        .next()
        .unwrap_or(DEFAULT_PORT);

    TcpStream::connect((host, port)).await
}

/// A TLS connector.
pub struct Connector<'a> {
    connector: TlsConnector,
    dns_name_ref: DNSNameRef<'a>,
}

impl<'a> Connector<'a> {
    /// Construct a TLS connector.
    #[inline]
    pub fn new(connector: TlsConnector, dns_name_ref: DNSNameRef<'a>) -> Self {
        Self {
            connector,
            dns_name_ref,
        }
    }
}

/// A wrapper for tokio_rustls::Connect.
pub struct Connect<IO>(tokio_rustls::Connect<IO>);

/// A wrapper for tokio_rustls::client::TlsStream.
pub struct TlsStream<IO>(client::TlsStream<IO>);

impl<IO> Future for Connect<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin,
{
    type Output = io::Result<TlsStream<IO>>;

    #[inline]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let stream = futures::ready!(Pin::new(&mut self.0).poll(cx))?;
        Poll::Ready(Ok(TlsStream(stream)))
    }
}

impl<IO> AsyncRead for TlsStream<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin,
{
    #[inline]
    unsafe fn prepare_uninitialized_buffer(&self, buf: &mut [MaybeUninit<u8>]) -> bool {
        self.0.prepare_uninitialized_buffer(buf)
    }

    #[inline]
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
    }

    #[inline]
    fn poll_read_buf<B: BufMut>(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut B,
    ) -> Poll<io::Result<usize>>
    where
        Self: Sized,
    {
        Pin::new(&mut self.0).poll_read_buf(cx, buf)
    }
}

impl<IO> AsyncWrite for TlsStream<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin,
{
    #[inline]
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    #[inline]
    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    #[inline]
    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_shutdown(cx)
    }

    #[inline]
    fn poll_write_buf<B: Buf>(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut B,
    ) -> Poll<io::Result<usize>>
    where
        Self: Sized,
    {
        Pin::new(&mut self.0).poll_write_buf(cx, buf)
    }
}

impl<IO> tls::TlsStream for TlsStream<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin,
{
    #[inline]
    fn channel_binding(&self) -> ChannelBinding {
        ChannelBinding::none()
    }
}

impl<IO> TlsConnect<IO> for Connector<'_>
where
    IO: AsyncRead + AsyncWrite + Unpin,
{
    type Stream = TlsStream<IO>;
    type Error = io::Error;
    type Future = Connect<IO>;

    #[inline]
    fn connect(self, stream: IO) -> Self::Future {
        let Connector {
            connector,
            dns_name_ref,
        } = self;
        Connect(connector.connect(dns_name_ref, stream))
    }
}

/// Connect to postgres server with tls.
///
/// ```rust
/// use roa_pg::{Client, connect_tls, ClientConfig};
/// use std::error::Error;
/// use async_std::task::spawn;
///
/// async fn play() -> Result<(), Box<dyn Error>> {
///     let url = "host=localhost user=postgres";
///     let (client, conn) = connect_tls(&url.parse()?, ClientConfig::new()).await?;
///     spawn(conn);
///     let row = client.query_one("SELECT * FROM user WHERE id=$1", &[&0]).await?;
///     let value: &str = row.get(0);
///     println!("value: {}", value);
///     Ok(())
/// }
/// ```
#[inline]
pub async fn connect_tls(
    config: &Config,
    tls_config: ClientConfig,
) -> io::Result<(
    Client,
    Connection<AsyncStream<TcpStream>, TlsStream<AsyncStream<TcpStream>>>,
)> {
    let stream = connect_stream(config).await?;
    let dns_name_ref = DNSNameRef::try_from_ascii_str(try_tcp_host(config)?)
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
    let connector = TlsConnector::from(Arc::new(tls_config));
    config
        .connect_raw(AsyncStream(stream), Connector::new(connector, dns_name_ref))
        .await
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
}
