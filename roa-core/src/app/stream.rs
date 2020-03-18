use bytes::{Buf, BufMut};
use futures::io::Error;
use futures::task::Context;
use std::io;
use std::mem::MaybeUninit;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{self, Poll};
use tokio::io::{AsyncRead, AsyncWrite};

/// A transport returned yieled by `AddrIncoming`.
pub struct AddrStream<IO> {
    remote_addr: SocketAddr,
    stream: IO,
}

impl<IO> AddrStream<IO> {
    /// Construct an AddrStream from an addr and a AsyncReadWriter.
    #[inline]
    pub fn new(remote_addr: SocketAddr, stream: IO) -> Self {
        AddrStream {
            remote_addr,
            stream,
        }
    }

    /// Returns the remote (peer) address of this connection.
    #[inline]
    pub fn remote_addr(&self) -> SocketAddr {
        self.remote_addr
    }
}

impl<IO> AsyncRead for AddrStream<IO>
where
    IO: Unpin + AsyncRead,
{
    #[inline]
    unsafe fn prepare_uninitialized_buffer(&self, buf: &mut [MaybeUninit<u8>]) -> bool {
        self.stream.prepare_uninitialized_buffer(buf)
    }

    #[inline]
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }

    #[inline]
    fn poll_read_buf<B: BufMut>(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut B,
    ) -> Poll<io::Result<usize>>
    where
        Self: Sized,
    {
        Pin::new(&mut self.stream).poll_read_buf(cx, buf)
    }
}

impl<IO> AsyncWrite for AddrStream<IO>
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
    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    #[inline]
    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }

    #[inline]
    fn poll_write_buf<B: Buf>(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut B,
    ) -> Poll<Result<usize, Error>>
    where
        Self: Sized,
    {
        Pin::new(&mut self.stream).poll_write_buf(cx, buf)
    }
}
