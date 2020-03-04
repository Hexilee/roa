use futures::io::{AsyncRead, AsyncWrite};
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{self, Poll};
use tokio::io::{AsyncRead as TokioRead, AsyncWrite as TokioWrite};

trait InnerStream: 'static + Send + Sync + Unpin + AsyncRead + AsyncWrite {}
impl<T> InnerStream for T where T: 'static + Send + Sync + Unpin + AsyncRead + AsyncWrite {}

/// A transport returned yieled by `AddrIncoming`.
pub struct AddrStream {
    remote_addr: SocketAddr,
    stream: Box<dyn InnerStream>,
}

impl AddrStream {
    /// Construct an AddrStream from an addr and a AsyncReadWriter.
    #[inline]
    pub fn new(
        remote_addr: SocketAddr,
        stream: impl 'static + Send + Sync + Unpin + AsyncRead + AsyncWrite,
    ) -> AddrStream {
        AddrStream {
            remote_addr,
            stream: Box::new(stream),
        }
    }

    /// Returns the remote (peer) address of this connection.
    #[inline]
    pub fn remote_addr(&self) -> SocketAddr {
        self.remote_addr
    }
}

impl TokioRead for AddrStream {
    #[inline]
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut *self.stream).poll_read(cx, buf)
    }
}

impl TokioWrite for AddrStream {
    #[inline]
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut *self.stream).poll_write(cx, buf)
    }

    #[inline]
    fn poll_flush(
        self: Pin<&mut Self>,
        _cx: &mut task::Context<'_>,
    ) -> Poll<io::Result<()>> {
        // TCP flush is a noop
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut *self.stream).poll_close(cx)
    }
}
