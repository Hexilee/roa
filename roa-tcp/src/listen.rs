use crate::TcpIncoming;
use roa_core::{App, Endpoint, Executor, Server, State};
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::Arc;

/// An app extension.
pub trait Listener {
    /// http server
    type Server;

    /// Listen on a socket addr, return a server and the real addr it binds.
    fn bind(
        self,
        addr: impl ToSocketAddrs,
    ) -> std::io::Result<(SocketAddr, Self::Server)>;

    /// Listen on a socket addr, return a server, and pass real addr to the callback.
    fn listen(
        self,
        addr: impl ToSocketAddrs,
        callback: impl Fn(SocketAddr),
    ) -> std::io::Result<Self::Server>;

    /// Listen on an unused port of 127.0.0.1, return a server and the real addr it binds.
    /// ### Example
    /// ```rust
    /// use roa_core::{App, Context, Error};
    /// use roa_tcp::Listener;
    /// use roa_core::http::StatusCode;
    /// use tokio::spawn;
    /// use std::time::Instant;
    ///
    /// async fn end(_ctx: &mut Context<()>) -> Result<(), Error> {
    ///     Ok(())
    /// }
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let (addr, server) = App::new(()).end(end).run()?;
    ///     spawn(server);
    ///     let resp = reqwest::get(&format!("http://{}", addr)).await?;
    ///     assert_eq!(StatusCode::OK, resp.status());
    ///     Ok(())
    /// }
    /// ```
    fn run(self) -> std::io::Result<(SocketAddr, Self::Server)>;
}

impl<S, E> Listener for App<S, Arc<E>>
where
    S: State,
    E: for<'a> Endpoint<'a, S>,
{
    type Server = Server<TcpIncoming, Self, Executor>;
    fn bind(
        self,
        addr: impl ToSocketAddrs,
    ) -> std::io::Result<(SocketAddr, Self::Server)> {
        let incoming = TcpIncoming::bind(addr)?;
        let local_addr = incoming.local_addr();
        Ok((local_addr, self.accept(incoming)))
    }

    fn listen(
        self,
        addr: impl ToSocketAddrs,
        callback: impl Fn(SocketAddr),
    ) -> std::io::Result<Self::Server> {
        let (addr, server) = self.bind(addr)?;
        callback(addr);
        Ok(server)
    }

    fn run(self) -> std::io::Result<(SocketAddr, Self::Server)> {
        self.bind("127.0.0.1:0")
    }
}
