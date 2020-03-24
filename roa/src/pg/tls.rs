use super::*;
use crate::tls::ClientConfig;
use bytes::{Buf, BufMut};
use std::future::Future;
use std::mem::MaybeUninit;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::macros::support::Pin;
use tokio_postgres::tls::{self, ChannelBinding};
use tokio_rustls::client;
use tokio_rustls::TlsConnector;
use webpki::DNSNameRef;

/// A TLS connector.
pub struct Connector<'a> {
    connector: TlsConnector,
    dns_name_ref: DNSNameRef<'a>,
}

impl<'a> Connector<'a> {
    /// Construct a TLS connector.
    #[inline]
    pub fn new(connector: TlsConnector, dns_name_ref: DNSNameRef<'a>) -> Self {
        Self {
            connector,
            dns_name_ref,
        }
    }
}

/// A wrapper for tokio_rustls::Connect.
pub struct Connect<IO>(tokio_rustls::Connect<IO>);

/// A wrapper for tokio_rustls::client::TlsStream.
pub struct TlsStream<IO>(client::TlsStream<IO>);

impl<IO> Future for Connect<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin,
{
    type Output = io::Result<TlsStream<IO>>;

    #[inline]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let stream = futures::ready!(Pin::new(&mut self.0).poll(cx))?;
        Poll::Ready(Ok(TlsStream(stream)))
    }
}

impl<IO> AsyncRead for TlsStream<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin,
{
    #[inline]
    unsafe fn prepare_uninitialized_buffer(&self, buf: &mut [MaybeUninit<u8>]) -> bool {
        self.0.prepare_uninitialized_buffer(buf)
    }

    #[inline]
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
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
        Pin::new(&mut self.0).poll_read_buf(cx, buf)
    }
}

impl<IO> AsyncWrite for TlsStream<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin,
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
    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    #[inline]
    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_shutdown(cx)
    }

    #[inline]
    fn poll_write_buf<B: Buf>(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut B,
    ) -> Poll<io::Result<usize>>
    where
        Self: Sized,
    {
        Pin::new(&mut self.0).poll_write_buf(cx, buf)
    }
}

impl<IO> tls::TlsStream for TlsStream<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin,
{
    #[inline]
    fn channel_binding(&self) -> ChannelBinding {
        ChannelBinding::none()
    }
}

impl<IO> TlsConnect<IO> for Connector<'_>
where
    IO: AsyncRead + AsyncWrite + Unpin,
{
    type Stream = TlsStream<IO>;
    type Error = io::Error;
    type Future = Connect<IO>;

    #[inline]
    fn connect(self, stream: IO) -> Self::Future {
        let Connector {
            connector,
            dns_name_ref,
        } = self;
        Connect(connector.connect(dns_name_ref, stream))
    }
}

/// Connect to postgres server with tls.
#[inline]
pub async fn connect_tls(
    config: &Config,
    tls_config: ClientConfig,
) -> io::Result<(
    Client,
    Connection<WrapStream<TcpStream>, TlsStream<WrapStream<TcpStream>>>,
)> {
    let stream = connect_stream(config).await?;
    let dns_name_ref = DNSNameRef::try_from_ascii_str(try_tcp_host(config)?)
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
    let connector = TlsConnector::from(Arc::new(tls_config));

    let (client, conn) = config
        .connect_raw(WrapStream(stream), Connector::new(connector, dns_name_ref))
        .await
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
    Ok((Client(client), conn))
}
