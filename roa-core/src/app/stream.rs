use futures::io::{AsyncRead, AsyncWrite};
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{self, Poll};
use tokio::io::{AsyncRead as TokioRead, AsyncWrite as TokioWrite};

trait InnerStream: 'static + Send + Sync + AsyncRead + AsyncWrite {}
impl<T> InnerStream for T where T: 'static + Send + Sync + AsyncRead + AsyncWrite {}

/// A transport returned yieled by `AddrIncoming`.
pub struct AddrStream {
    remote_addr: SocketAddr,
    stream: Pin<Box<dyn InnerStream>>,
}

impl AddrStream {
    /// Construct an AddrStream from an addr and a AsyncReadWriter.
    pub fn new(
        remote_addr: SocketAddr,
        stream: impl 'static + Send + Sync + AsyncRead + AsyncWrite,
    ) -> AddrStream {
        AddrStream {
            remote_addr,
            stream: Box::pin(stream),
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
        self.stream.as_mut().poll_read(cx, buf)
    }
}

impl TokioWrite for AddrStream {
    #[inline]
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.stream.as_mut().poll_write(cx, buf)
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
        self.stream.as_mut().poll_close(cx)
    }
}
