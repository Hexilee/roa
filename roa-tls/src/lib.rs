use async_std::net::TcpStream;
use async_tls::server::TlsStream;
use async_tls::TlsAcceptor;
use futures::io::Error;
use futures::task::Context;
use futures::{AsyncRead, AsyncWrite, Future};
use roa_core::{Accept, AddrStream, App, Executor, Server, State};
use roa_tcp::TcpIncoming;
use rustls::ServerConfig;
use std::io;
use std::net::{SocketAddr, ToSocketAddrs};
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{self, Poll};

pub use rustls;

pub struct TlsIncoming {
    incoming: TcpIncoming,
    acceptor: TlsAcceptor,
}

type AcceptFuture = dyn 'static
    + Sync
    + Send
    + Unpin
    + Future<Output = io::Result<TlsStream<TcpStream>>>;

enum WrapStream {
    Handshaking(Box<AcceptFuture>),
    Streaming(Box<TlsStream<TcpStream>>),
}

use WrapStream::*;

impl WrapStream {
    #[inline]
    fn poll_handshake(
        handshake: &mut AcceptFuture,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<Self>> {
        let stream = futures::ready!(Pin::new(handshake).poll(cx))?;
        Poll::Ready(Ok(Streaming(Box::new(stream))))
    }
}

impl AsyncRead for WrapStream {
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
}

impl AsyncWrite for WrapStream {
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
    ) -> Poll<Result<(), Error>> {
        match &mut *self {
            Streaming(stream) => Pin::new(stream).poll_flush(cx),
            Handshaking(handshake) => {
                *self = futures::ready!(Self::poll_handshake(handshake, cx))?;
                self.poll_flush(cx)
            }
        }
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Error>> {
        match &mut *self {
            Streaming(stream) => Pin::new(stream).poll_close(cx),
            Handshaking(handshake) => {
                *self = futures::ready!(Self::poll_handshake(handshake, cx))?;
                self.poll_close(cx)
            }
        }
    }
}

impl TlsIncoming {
    pub fn new(incoming: TcpIncoming, config: ServerConfig) -> Self {
        Self {
            incoming,
            acceptor: Arc::new(config).into(),
        }
    }

    pub fn bind(addr: impl ToSocketAddrs, config: ServerConfig) -> io::Result<Self> {
        let incoming = TcpIncoming::bind(addr)?;
        Ok(Self::new(incoming, config))
    }
}

impl Deref for TlsIncoming {
    type Target = TcpIncoming;
    fn deref(&self) -> &Self::Target {
        &self.incoming
    }
}

impl DerefMut for TlsIncoming {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.incoming
    }
}

impl Accept for TlsIncoming {
    type Conn = AddrStream;
    type Error = io::Error;

    fn poll_accept(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Option<Result<Self::Conn, Self::Error>>> {
        let stream = futures::ready!(Pin::new(&mut self.incoming).poll_stream(cx))?;
        let addr = stream.peer_addr()?;
        let accept_future = self.acceptor.accept(stream);
        Poll::Ready(Some(Ok(AddrStream::new(
            addr,
            Handshaking(Box::new(accept_future)),
        ))))
    }
}

pub trait TlsListener {
    /// tcp server
    type Server;

    /// Listen on a socket addr, return a server and the real addr it binds.
    fn listen_tls_on(
        &self,
        addr: impl ToSocketAddrs,
        config: ServerConfig,
    ) -> std::io::Result<(SocketAddr, Self::Server)>;

    /// Listen on a socket addr, return a server, and pass real addr to the callback.
    fn listen_tls(
        &self,
        addr: impl ToSocketAddrs,
        config: ServerConfig,
        callback: impl Fn(SocketAddr),
    ) -> std::io::Result<Self::Server>;

    /// Listen on an unused port of 127.0.0.1, return a server and the real addr it binds.
    /// ### Example
    /// ```rust,no_run
    /// use roa_core::App;
    /// use roa_tls::TlsListener;
    /// use roa_tls::rustls::{ServerConfig, NoClientAuth};
    /// use roa_tls::rustls::internal::pemfile::{certs, rsa_private_keys};
    /// use roa_core::http::StatusCode;
    /// use async_std::task::spawn;
    /// use std::time::Instant;
    /// use std::fs::File;
    /// use std::io::BufReader;
    ///
    /// #[async_std::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let mut config = ServerConfig::new(NoClientAuth::new());
    ///     let mut cert_file = BufReader::new(File::open("../assets/cert.pem")?);
    ///     let mut key_file = BufReader::new(File::open("../assets/key.pem")?);
    ///     let cert_chain = certs(&mut cert_file).unwrap();
    ///     let mut keys = rsa_private_keys(&mut key_file).unwrap();
    ///     config.set_single_cert(cert_chain, keys.remove(0))?;
    ///
    ///     let server = App::new(())
    ///         .gate_fn(|_ctx, next| async move {
    ///             let inbound = Instant::now();
    ///             next.await?;
    ///             println!("time elapsed: {} ms", inbound.elapsed().as_millis());
    ///             Ok(())
    ///         })
    ///         .listen_tls("127.0.0.1:8000", config, |addr| {
    ///             println!("Server is listening on https://localhost:{}", addr.port());
    ///         })?;
    ///     server.await?;
    ///     Ok(())
    /// }
    /// ```
    fn run_tls(
        &self,
        config: ServerConfig,
    ) -> std::io::Result<(SocketAddr, Self::Server)>;
}

impl<S: State> TlsListener for App<S> {
    type Server = Server<TlsIncoming, Self, Executor>;
    fn listen_tls_on(
        &self,
        addr: impl ToSocketAddrs,
        config: ServerConfig,
    ) -> std::io::Result<(SocketAddr, Self::Server)> {
        let incoming = TlsIncoming::bind(addr, config)?;
        let local_addr = incoming.local_addr();
        Ok((local_addr, self.accept(incoming)))
    }

    fn listen_tls(
        &self,
        addr: impl ToSocketAddrs,
        config: ServerConfig,
        callback: impl Fn(SocketAddr),
    ) -> std::io::Result<Self::Server> {
        let (addr, server) = self.listen_tls_on(addr, config)?;
        callback(addr);
        Ok(server)
    }

    fn run_tls(
        &self,
        config: ServerConfig,
    ) -> std::io::Result<(SocketAddr, Self::Server)> {
        self.listen_tls_on("127.0.0.1:0", config)
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
    use roa_body::PowerBody;
    use roa_core::http::StatusCode;
    use roa_core::App;

    use futures::{AsyncReadExt, TryStreamExt};
    use rustls::internal::pemfile::{certs, rsa_private_keys};
    use rustls::{NoClientAuth, ServerConfig};
    use std::fs::File;
    use std::io::{self, BufReader};
    use tokio_tls::TlsConnector;

    #[tokio::test]
    async fn run_tls() -> Result<(), Box<dyn std::error::Error>> {
        let mut config = ServerConfig::new(NoClientAuth::new());
        let mut cert_file = BufReader::new(File::open("../assets/cert.pem")?);
        let mut key_file = BufReader::new(File::open("../assets/key.pem")?);
        let cert_chain = certs(&mut cert_file).unwrap();
        let mut keys = rsa_private_keys(&mut key_file).unwrap();
        config.set_single_cert(cert_chain, keys.remove(0))?;
        let (addr, server) = App::new(())
            .end(|mut ctx| async move { ctx.write_text("Hello, World!") })
            .run_tls(config)?;
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
