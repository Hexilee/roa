use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{self, Poll};

use futures::io::{AsyncRead, AsyncWrite};
use tokio::io::{AsyncRead as TokioRead, AsyncWrite as TokioWrite, ReadBuf};

/// A transport returned yieled by `AddrIncoming`.
pub struct AddrStream<IO> {
    /// The remote address of this stream.
    pub remote_addr: SocketAddr,

    /// The inner stream.
    pub stream: IO,
}

impl<IO> AddrStream<IO> {
    /// Construct an AddrStream from an addr and a AsyncReadWriter.
    #[inline]
    pub fn new(remote_addr: SocketAddr, stream: IO) -> AddrStream<IO> {
        AddrStream {
            remote_addr,
            stream,
        }
    }
}

impl<IO> TokioRead for AddrStream<IO>
where
    IO: Unpin + AsyncRead,
{
    #[inline]
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let bytes =
            futures::ready!(Pin::new(&mut self.stream).poll_read(cx, buf.initialized_mut()))?;
        buf.set_filled(bytes);
        Poll::Ready(Ok(()))
    }
}

impl<IO> TokioWrite for AddrStream<IO>
where
    IO: Unpin + AsyncWrite,
{
    #[inline]
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.stream).poll_write(cx, buf)
    }

    #[inline]
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    #[inline]
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_close(cx)
    }
}
