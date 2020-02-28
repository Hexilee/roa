use async_std::net::{SocketAddr, TcpStream};
use async_tls::server::TlsStream;
use async_tls::TlsAcceptor;
use futures::Future;
use roa_core::{Accept, AddrStream};
use roa_tcp::TcpIncoming;
use rustls::ServerConfig;
use std::io;
use std::net::ToSocketAddrs;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{self, Poll};

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
                Some(Ok(AddrStream::new(*addr, stream)))
            }
        })
    }
}
