use super::{ServerConfig, TlsIncoming};
use crate::tcp::TcpIncoming;
use crate::{App, Endpoint, Executor, Server, State};
use std::io;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::Arc;

impl TlsIncoming<TcpIncoming> {
    /// Bind a socket addr.
    #[cfg_attr(feature = "docs", doc(cfg(feature = "tcp")))]
    pub fn bind(addr: impl ToSocketAddrs, config: ServerConfig) -> io::Result<Self> {
        Ok(Self::new(TcpIncoming::bind(addr)?, config))
    }
}

/// An app extension.
#[cfg_attr(feature = "docs", doc(cfg(feature = "tcp")))]
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
    /// use roa::{App, Context, Status};
    /// use roa::tls::{TlsIncoming, ServerConfig, NoClientAuth, TlsListener};
    /// use roa::tls::internal::pemfile::{certs, rsa_private_keys};
    /// use roa_core::http::StatusCode;
    /// use async_std::task::spawn;
    /// use std::time::Instant;
    /// use std::fs::File;
    /// use std::io::BufReader;
    ///
    /// async fn end(_ctx: &mut Context) -> Result<(), Status> {
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
    use crate::http::StatusCode;
    use crate::tls::internal::pemfile::{certs, rsa_private_keys};
    use crate::tls::TlsListener;
    use crate::tls::{NoClientAuth, ServerConfig};
    use crate::{App, Context, Status};
    use async_std::task::spawn;
    use futures::{AsyncReadExt, TryStreamExt};
    use hyper::client::{Client, HttpConnector};
    use hyper::Body;
    use hyper_tls::native_tls;
    use hyper_tls::HttpsConnector;
    use std::fs::File;
    use std::io::{self, BufReader};
    use tokio_tls::TlsConnector;

    async fn end(ctx: &mut Context) -> Result<(), Status> {
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
