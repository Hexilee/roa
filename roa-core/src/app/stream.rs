use std::fmt::Debug;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{self, Poll};

use futures::ready;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tracing::{instrument, trace};

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

impl<IO> AsyncRead for AddrStream<IO>
where
    IO: Unpin + AsyncRead,
{
    #[inline]
    #[instrument(skip(cx, buf))]
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let poll = Pin::new(&mut self.stream).poll_read(cx, buf);
        trace!("poll read: {:?}", poll);
        ready!(poll)?;
        trace!("read {} bytes", buf.filled().len());
        Poll::Ready(Ok(()))
    }
}

impl<IO> AsyncWrite for AddrStream<IO>
where
    IO: Unpin + AsyncWrite,
{
    #[inline]
    #[instrument(skip(cx, buf))]
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let write_size = ready!(Pin::new(&mut self.stream).poll_write(cx, buf))?;
        trace!("wrote {} bytes", write_size);
        Poll::Ready(Ok(write_size))
    }

    #[inline]
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    #[inline]
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

impl<IO> Debug for AddrStream<IO> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AddrStream")
            .field("remote_addr", &self.remote_addr)
            .finish()
    }
}
