//! this module provides a stream adaptor `AsyncStream`

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::io::{AsyncRead as Read, AsyncWrite as Write};
use futures::ready;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// A adaptor between futures::io::{AsyncRead, AsyncWrite} and tokio::io::{AsyncRead, AsyncWrite}.
pub struct AsyncStream<IO>(pub IO);

impl<IO> AsyncRead for AsyncStream<IO>
where
    IO: Unpin + Read,
{
    #[inline]
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let read_size = ready!(Pin::new(&mut self.0).poll_read(cx, buf.initialize_unfilled()))?;
        buf.advance(read_size);
        Poll::Ready(Ok(()))
    }
}

impl<IO> AsyncWrite for AsyncStream<IO>
where
    IO: Unpin + Write,
{
    #[inline]
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    #[inline]
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    #[inline]
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_close(cx)
    }
}

impl<IO> Read for AsyncStream<IO>
where
    IO: Unpin + AsyncRead,
{
    #[inline]
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let mut read_buf = ReadBuf::new(buf);
        ready!(Pin::new(&mut self.0).poll_read(cx, &mut read_buf))?;
        Poll::Ready(Ok(read_buf.filled().len()))
    }
}

impl<IO> Write for AsyncStream<IO>
where
    IO: Unpin + AsyncWrite,
{
    #[inline]
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    #[inline]
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    #[inline]
    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_shutdown(cx)
    }
}
