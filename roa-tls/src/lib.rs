use async_std::net::TcpStream;
use async_tls::server::TlsStream;
use async_tls::TlsAcceptor;
use futures::Future;
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
    accept_future: Option<(
        SocketAddr,
        Box<
            dyn 'static
                + Sync
                + Send
                + Unpin
                + Future<Output = io::Result<TlsStream<TcpStream>>>,
        >,
    )>,
}

impl TlsIncoming {
    pub fn new(incoming: TcpIncoming, config: ServerConfig) -> Self {
        Self {
            incoming,
            acceptor: Arc::new(config).into(),
            accept_future: None,
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
        Poll::Ready(match self.accept_future.as_mut() {
            None => {
                let stream =
                    futures::ready!(Pin::new(&mut self.incoming).poll_stream(cx))?;
                let addr = stream.peer_addr()?;
                self.accept_future =
                    Some((addr, Box::new(self.acceptor.accept(stream))));
                return self.poll_accept(cx);
            }
            Some((addr, fut)) => {
                let stream = futures::ready!(Pin::new(fut).poll(cx))?;
                let addr_stream = AddrStream::new(*addr, stream);
                self.accept_future = None;
                Some(Ok(addr_stream))
            }
        })
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
    /// ```rust
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
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let mut config = ServerConfig::new(NoClientAuth::new());
    ///     let mut cert_file = BufReader::new(File::open("../assets/cert.pem")?);
    ///     let mut key_file = BufReader::new(File::open("../assets/key.pem")?);
    ///     let cert_chain = certs(&mut cert_file).unwrap();
    ///     let mut keys = rsa_private_keys(&mut key_file).unwrap();
    ///     config.set_single_cert(cert_chain, keys.remove(0))?;
    ///
    ///     let (addr, server) = App::new(())
    ///         .gate_fn(|_ctx, next| async move {
    ///             let inbound = Instant::now();
    ///             next.await?;
    ///             println!("time elapsed: {} ms", inbound.elapsed().as_millis());
    ///             Ok(())
    ///         })
    ///         .run_tls(config)?;
    ///     spawn(server);
    ///     let resp = reqwest::get(&format!("https://{}", addr)).await?;
    ///     assert_eq!(StatusCode::OK, resp.status());
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
    use roa_core::http::StatusCode;
    use roa_core::App;
    use rustls::internal::pemfile::{certs, rsa_private_keys};
    use rustls::{NoClientAuth, ServerConfig};
    use std::fs::File;
    use std::io::BufReader;
    use std::time::Instant;

    #[tokio::test]
    async fn run_tls() -> Result<(), Box<dyn std::error::Error>> {
        let mut config = ServerConfig::new(NoClientAuth::new());
        let mut cert_file = BufReader::new(File::open("../assets/cert.pem")?);
        let mut key_file = BufReader::new(File::open("../assets/key.pem")?);
        let cert_chain = certs(&mut cert_file).unwrap();
        let mut keys = rsa_private_keys(&mut key_file).unwrap();
        config.set_single_cert(cert_chain, keys.remove(0))?;
        let (addr, server) = App::new(())
            .gate_fn(|_ctx, next| async move {
                let inbound = Instant::now();
                next.await?;
                println!("time elapsed: {} ms", inbound.elapsed().as_millis());
                Ok(())
            })
            .run_tls(config)?;
        spawn(server);
        let client = reqwest::ClientBuilder::new().use_rustls_tls().build()?;
        let resp = client
            .get(&format!("https://localhost:{}", addr.port()))
            .send()
            .await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }
}
