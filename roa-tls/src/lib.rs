//! This crate provides an acceptor implementing `roa_core::Accept` and an app extension.
//!
//! ### TlsIncoming
//!
//! ```rust
//! use roa_core::{App, Context, Error};
//! use roa_tls::{TlsIncoming, ServerConfig, NoClientAuth};
//! use roa_tls::internal::pemfile::{certs, rsa_private_keys};
//! use std::fs::File;
//! use std::io::BufReader;
//!
//! async fn end(_ctx: &mut Context<()>) -> Result<(), Error> {
//!     Ok(())
//! }
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut config = ServerConfig::new(NoClientAuth::new());
//! let mut cert_file = BufReader::new(File::open("../assets/cert.pem")?);
//! let mut key_file = BufReader::new(File::open("../assets/key.pem")?);
//! let cert_chain = certs(&mut cert_file).unwrap();
//! let mut keys = rsa_private_keys(&mut key_file).unwrap();
//! config.set_single_cert(cert_chain, keys.remove(0))?;
//!
//! let incoming = TlsIncoming::bind("127.0.0.1:0", config)?;
//! let server = App::new(()).end(end).accept(incoming);
//! // server.await
//! Ok(())
//! # }
//! ```
//!
//! ### TlsListener
//!
//! ```rust
//! use roa_core::{App, Context, Error};
//! use roa_tls::{TlsListener, ServerConfig, NoClientAuth};
//! use roa_tls::internal::pemfile::{certs, rsa_private_keys};
//! use std::fs::File;
//! use std::io::BufReader;
//!
//! async fn end(_ctx: &mut Context<()>) -> Result<(), Error> {
//!     Ok(())
//! }
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut config = ServerConfig::new(NoClientAuth::new());
//! let mut cert_file = BufReader::new(File::open("../assets/cert.pem")?);
//! let mut key_file = BufReader::new(File::open("../assets/key.pem")?);
//! let cert_chain = certs(&mut cert_file).unwrap();
//! let mut keys = rsa_private_keys(&mut key_file).unwrap();
//! config.set_single_cert(cert_chain, keys.remove(0))?;
//! let (addr, server) = App::new(()).end(end).bind_tls("127.0.0.1:0", config)?;
//! // server.await
//! Ok(())
//! # }
//! ```

#![warn(missing_docs)]

#[cfg(doctest)]
doc_comment::doctest!("../README.md");

use bytes::{Buf, BufMut};
use futures::Future;
use roa_core::{Accept, AddrStream, App, Endpoint, Executor, Server, State};
use roa_tcp::TcpIncoming;
use std::io;
use std::net::{SocketAddr, ToSocketAddrs};
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{self, Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::server::TlsStream;
use tokio_rustls::TlsAcceptor;

pub use rustls::*;

/// A stream of connections from a TcpIncoming.
/// As an implementation of roa_core::Accept.
pub struct TlsIncoming<I> {
    incoming: I,
    acceptor: TlsAcceptor,
}

type AcceptFuture<IO> =
    dyn 'static + Sync + Send + Unpin + Future<Output = io::Result<TlsStream<IO>>>;

/// A finite-state machine to do tls handshake.
pub enum WrapTlsStream<IO> {
    /// Handshaking state.
    Handshaking(Box<AcceptFuture<IO>>),
    /// Streaming state.
    Streaming(Box<TlsStream<IO>>),
}

use std::mem::MaybeUninit;
use WrapTlsStream::*;

impl<IO> WrapTlsStream<IO> {
    #[inline]
    fn poll_handshake(
        handshake: &mut AcceptFuture<IO>,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<Self>> {
        let stream = futures::ready!(Pin::new(handshake).poll(cx))?;
        Poll::Ready(Ok(Streaming(Box::new(stream))))
    }
}

impl<IO> AsyncRead for WrapTlsStream<IO>
where
    IO: 'static + Unpin + AsyncRead + AsyncWrite,
{
    #[inline]
    unsafe fn prepare_uninitialized_buffer(&self, buf: &mut [MaybeUninit<u8>]) -> bool {
        match self {
            Streaming(stream) => stream.prepare_uninitialized_buffer(buf),
            _ => false,
        }
    }

    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        match &mut *self {
            Streaming(stream) => Pin::new(stream).poll_read(cx, buf),
            Handshaking(handshake) => {
                *self = futures::ready!(Self::poll_handshake(handshake, cx))?;
                self.poll_read(cx, buf)
            }
        }
    }

    fn poll_read_buf<B: BufMut>(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut B,
    ) -> Poll<io::Result<usize>>
    where
        Self: Sized,
    {
        match &mut *self {
            Streaming(stream) => Pin::new(stream).poll_read_buf(cx, buf),
            Handshaking(handshake) => {
                *self = futures::ready!(Self::poll_handshake(handshake, cx))?;
                self.poll_read_buf(cx, buf)
            }
        }
    }
}

impl<IO> AsyncWrite for WrapTlsStream<IO>
where
    IO: 'static + Unpin + AsyncRead + AsyncWrite,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match &mut *self {
            Streaming(stream) => Pin::new(stream).poll_write(cx, buf),
            Handshaking(handshake) => {
                *self = futures::ready!(Self::poll_handshake(handshake, cx))?;
                self.poll_write(cx, buf)
            }
        }
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        match &mut *self {
            Streaming(stream) => Pin::new(stream).poll_flush(cx),
            Handshaking(handshake) => {
                *self = futures::ready!(Self::poll_handshake(handshake, cx))?;
                self.poll_flush(cx)
            }
        }
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        match &mut *self {
            Streaming(stream) => Pin::new(stream).poll_shutdown(cx),
            Handshaking(handshake) => {
                *self = futures::ready!(Self::poll_handshake(handshake, cx))?;
                self.poll_shutdown(cx)
            }
        }
    }

    fn poll_write_buf<B: Buf>(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut B,
    ) -> Poll<io::Result<usize>>
    where
        Self: Sized,
    {
        match &mut *self {
            Streaming(stream) => Pin::new(stream).poll_write_buf(cx, buf),
            Handshaking(handshake) => {
                *self = futures::ready!(Self::poll_handshake(handshake, cx))?;
                self.poll_write_buf(cx, buf)
            }
        }
    }
}

impl<I> TlsIncoming<I> {
    /// Construct from inner incoming.
    pub fn new(incoming: I, config: ServerConfig) -> Self {
        Self {
            incoming,
            acceptor: Arc::new(config).into(),
        }
    }
}

impl TlsIncoming<TcpIncoming> {
    /// Bind a socket addr.
    pub fn bind(addr: impl ToSocketAddrs, config: ServerConfig) -> io::Result<Self> {
        Ok(Self::new(TcpIncoming::bind(addr)?, config))
    }
}

impl<I> Deref for TlsIncoming<I> {
    type Target = I;
    fn deref(&self) -> &Self::Target {
        &self.incoming
    }
}

impl<I> DerefMut for TlsIncoming<I> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.incoming
    }
}

impl<I, IO> Accept for TlsIncoming<I>
where
    IO: 'static + Send + Sync + Unpin + AsyncRead + AsyncWrite,
    I: Unpin + Accept<Conn = AddrStream<IO>>,
{
    type Conn = AddrStream<WrapTlsStream<IO>>;
    type Error = I::Error;

    #[inline]
    fn poll_accept(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Option<Result<Self::Conn, Self::Error>>> {
        Poll::Ready(
            match futures::ready!(Pin::new(&mut self.incoming).poll_accept(cx)) {
                Some(Ok(AddrStream {
                    stream,
                    remote_addr,
                })) => {
                    let accept_future = self.acceptor.accept(stream);
                    Some(Ok(AddrStream::new(
                        remote_addr,
                        Handshaking(Box::new(accept_future)),
                    )))
                }
                Some(Err(err)) => Some(Err(err)),
                None => None,
            },
        )
    }
}

/// An app extension.
pub trait TlsListener {
    /// http server
    type Server;

    /// Listen on a socket addr, return a server and the real addr it binds.
    fn bind_tls(
        self,
        addr: impl ToSocketAddrs,
        config: ServerConfig,
    ) -> std::io::Result<(SocketAddr, Self::Server)>;

    /// Listen on a socket addr, return a server, and pass real addr to the callback.
    fn listen_tls(
        self,
        addr: impl ToSocketAddrs,
        config: ServerConfig,
        callback: impl Fn(SocketAddr),
    ) -> std::io::Result<Self::Server>;

    /// Listen on an unused port of 127.0.0.1, return a server and the real addr it binds.
    /// ### Example
    /// ```rust
    /// use roa_core::{App, Context, Error};
    /// use roa_tls::{TlsListener, ServerConfig, NoClientAuth};
    /// use roa_tls::internal::pemfile::{certs, rsa_private_keys};
    /// use roa_core::http::StatusCode;
    /// use async_std::task::spawn;
    /// use std::time::Instant;
    /// use std::fs::File;
    /// use std::io::BufReader;
    ///
    /// async fn end(_ctx: &mut Context<()>) -> Result<(), Error> {
    ///     Ok(())
    /// }
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut config = ServerConfig::new(NoClientAuth::new());
    /// let mut cert_file = BufReader::new(File::open("../assets/cert.pem")?);
    /// let mut key_file = BufReader::new(File::open("../assets/key.pem")?);
    /// let cert_chain = certs(&mut cert_file).unwrap();
    /// let mut keys = rsa_private_keys(&mut key_file).unwrap();
    /// config.set_single_cert(cert_chain, keys.remove(0))?;
    ///
    /// let server = App::new(()).end(end).listen_tls("127.0.0.1:8000", config, |addr| {
    ///     println!("Server is listening on https://localhost:{}", addr.port());
    /// })?;
    /// // server.await
    /// Ok(())
    /// # }
    /// ```
    fn run_tls(
        self,
        config: ServerConfig,
    ) -> std::io::Result<(SocketAddr, Self::Server)>;
}

impl<S, E> TlsListener for App<S, Arc<E>>
where
    S: State,
    E: for<'a> Endpoint<'a, S>,
{
    type Server = Server<TlsIncoming<TcpIncoming>, Self, Executor>;
    fn bind_tls(
        self,
        addr: impl ToSocketAddrs,
        config: ServerConfig,
    ) -> std::io::Result<(SocketAddr, Self::Server)> {
        let incoming = TlsIncoming::bind(addr, config)?;
        let local_addr = incoming.local_addr();
        Ok((local_addr, self.accept(incoming)))
    }

    fn listen_tls(
        self,
        addr: impl ToSocketAddrs,
        config: ServerConfig,
        callback: impl Fn(SocketAddr),
    ) -> std::io::Result<Self::Server> {
        let (addr, server) = self.bind_tls(addr, config)?;
        callback(addr);
        Ok(server)
    }

    fn run_tls(
        self,
        config: ServerConfig,
    ) -> std::io::Result<(SocketAddr, Self::Server)> {
        self.bind_tls("127.0.0.1:0", config)
    }
}

#[cfg(test)]
mod tests {
    use crate::TlsListener;
    use async_std::task::spawn;
    use hyper::client::{Client, HttpConnector};
    use hyper::Body;
    use hyper_tls::native_tls;
    use hyper_tls::HttpsConnector;
    use roa_core::http::StatusCode;
    use roa_core::{App, Context, Error};

    use futures::{AsyncReadExt, TryStreamExt};
    use rustls::internal::pemfile::{certs, rsa_private_keys};
    use rustls::{NoClientAuth, ServerConfig};
    use std::fs::File;
    use std::io::{self, BufReader};
    use tokio_tls::TlsConnector;

    async fn end(ctx: &mut Context<()>) -> Result<(), Error> {
        ctx.resp.write("Hello, World!");
        Ok(())
    }

    #[tokio::test]
    async fn run_tls() -> Result<(), Box<dyn std::error::Error>> {
        let mut config = ServerConfig::new(NoClientAuth::new());
        let mut cert_file = BufReader::new(File::open("../assets/cert.pem")?);
        let mut key_file = BufReader::new(File::open("../assets/key.pem")?);
        let cert_chain = certs(&mut cert_file).unwrap();
        let mut keys = rsa_private_keys(&mut key_file).unwrap();
        config.set_single_cert(cert_chain, keys.remove(0))?;

        let app = App::new(()).end(end);
        let (addr, server) = app.run_tls(config)?;
        spawn(server);

        let native_tls_connector = native_tls::TlsConnector::builder()
            .danger_accept_invalid_hostnames(true)
            .danger_accept_invalid_certs(true)
            .build()?;
        let tls_connector = TlsConnector::from(native_tls_connector);
        let mut http_connector = HttpConnector::new();
        http_connector.enforce_http(false);
        let https_connector = HttpsConnector::from((http_connector, tls_connector));
        let client = Client::builder().build::<_, Body>(https_connector);
        let resp = client
            .get(format!("https://localhost:{}", addr.port()).parse()?)
            .await?;
        assert_eq!(StatusCode::OK, resp.status());
        let mut text = String::new();
        resp.into_body()
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
            .into_async_read()
            .read_to_string(&mut text)
            .await?;
        assert_eq!("Hello, World!", text);
        Ok(())
    }
}
