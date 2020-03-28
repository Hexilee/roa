use super::ServerConfig;
use crate::{Accept, AddrStream};
use async_tls::server::TlsStream;
use async_tls::TlsAcceptor;
use futures::io::{AsyncRead, AsyncWrite, IoSlice, IoSliceMut};
use futures::Future;
use std::io;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{self, Context, Poll};

/// A stream of connections based on another stream.
/// As an implementation of roa_core::Accept.
pub struct TlsIncoming<I> {
    incoming: I,
    acceptor: TlsAcceptor,
}

type AcceptFuture<IO> =
    dyn 'static + Sync + Send + Unpin + Future<Output = io::Result<TlsStream<IO>>>;

/// A finite-state machine to do tls handshake.
pub enum WrapTlsStream<IO> {
    /// Handshaking state.
    Handshaking(Box<AcceptFuture<IO>>),
    /// Streaming state.
    Streaming(Box<TlsStream<IO>>),
}

use WrapTlsStream::*;

impl<IO> WrapTlsStream<IO> {
    #[inline]
    fn poll_handshake(
        handshake: &mut AcceptFuture<IO>,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<Self>> {
        let stream = futures::ready!(Pin::new(handshake).poll(cx))?;
        Poll::Ready(Ok(Streaming(Box::new(stream))))
    }
}

impl<IO> AsyncRead for WrapTlsStream<IO>
where
    IO: 'static + Unpin + AsyncRead + AsyncWrite,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        match &mut *self {
            Streaming(stream) => Pin::new(stream).poll_read(cx, buf),
            Handshaking(handshake) => {
                *self = futures::ready!(Self::poll_handshake(handshake, cx))?;
                self.poll_read(cx, buf)
            }
        }
    }

    fn poll_read_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
    ) -> Poll<io::Result<usize>> {
        match &mut *self {
            Streaming(stream) => Pin::new(stream).poll_read_vectored(cx, bufs),
            Handshaking(handshake) => {
                *self = futures::ready!(Self::poll_handshake(handshake, cx))?;
                self.poll_read_vectored(cx, bufs)
            }
        }
    }
}

impl<IO> AsyncWrite for WrapTlsStream<IO>
where
    IO: 'static + Unpin + AsyncRead + AsyncWrite,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match &mut *self {
            Streaming(stream) => Pin::new(stream).poll_write(cx, buf),
            Handshaking(handshake) => {
                *self = futures::ready!(Self::poll_handshake(handshake, cx))?;
                self.poll_write(cx, buf)
            }
        }
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        match &mut *self {
            Streaming(stream) => Pin::new(stream).poll_write_vectored(cx, bufs),
            Handshaking(handshake) => {
                *self = futures::ready!(Self::poll_handshake(handshake, cx))?;
                self.poll_write_vectored(cx, bufs)
            }
        }
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        match &mut *self {
            Streaming(stream) => Pin::new(stream).poll_flush(cx),
            Handshaking(handshake) => {
                *self = futures::ready!(Self::poll_handshake(handshake, cx))?;
                self.poll_flush(cx)
            }
        }
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        match &mut *self {
            Streaming(stream) => Pin::new(stream).poll_close(cx),
            Handshaking(handshake) => {
                *self = futures::ready!(Self::poll_handshake(handshake, cx))?;
                self.poll_close(cx)
            }
        }
    }
}

impl<I> TlsIncoming<I> {
    /// Construct from inner incoming.
    pub fn new(incoming: I, config: ServerConfig) -> Self {
        Self {
            incoming,
            acceptor: Arc::new(config).into(),
        }
    }
}

impl<I> Deref for TlsIncoming<I> {
    type Target = I;
    fn deref(&self) -> &Self::Target {
        &self.incoming
    }
}

impl<I> DerefMut for TlsIncoming<I> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.incoming
    }
}

impl<I, IO> Accept for TlsIncoming<I>
where
    IO: 'static + Send + Sync + Unpin + AsyncRead + AsyncWrite,
    I: Unpin + Accept<Conn = AddrStream<IO>>,
{
    type Conn = AddrStream<WrapTlsStream<IO>>;
    type Error = I::Error;

    #[inline]
    fn poll_accept(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Option<Result<Self::Conn, Self::Error>>> {
        Poll::Ready(
            match futures::ready!(Pin::new(&mut self.incoming).poll_accept(cx)) {
                Some(Ok(AddrStream {
                    stream,
                    remote_addr,
                })) => {
                    let accept_future = self.acceptor.accept(stream);
                    Some(Ok(AddrStream::new(
                        remote_addr,
                        Handshaking(Box::new(accept_future)),
                    )))
                }
                Some(Err(err)) => Some(Err(err)),
                None => None,
            },
        )
    }
}
