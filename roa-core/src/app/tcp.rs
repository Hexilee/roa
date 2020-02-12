pub use self::addr_stream::AddrStream;
use async_std::net::{SocketAddr, TcpListener};
use futures::FutureExt as _;
use futures_timer::Delay;
use hyper::server::accept::Accept;
use log::{debug, error, trace};
use std::fmt;
use std::future::Future;
use std::io;
use std::net::{TcpListener as StdListener, ToSocketAddrs};
use std::pin::Pin;
use std::task::{self, Poll};
use std::time::Duration;

/// A stream of connections from binding to an address.
/// As an implementation of hyper::server::accept::Accept.
#[must_use = "streams do nothing unless polled"]
pub struct AddrIncoming {
    addr: SocketAddr,
    listener: TcpListener,
    sleep_on_errors: bool,
    tcp_nodelay: bool,
    timeout: Option<Delay>,
}

impl AddrIncoming {
    pub(super) fn new(addr: impl ToSocketAddrs) -> io::Result<Self> {
        let listener = StdListener::bind(addr)?;
        AddrIncoming::from_std(listener)
    }

    pub(super) fn from_std(listener: StdListener) -> io::Result<Self> {
        let addr = listener.local_addr()?;
        Ok(AddrIncoming {
            listener: listener.into(),
            addr,
            sleep_on_errors: true,
            tcp_nodelay: false,
            timeout: None,
        })
    }

    /// Creates a new `AddrIncoming` binding to provided socket address.
    pub fn bind(addr: impl ToSocketAddrs) -> io::Result<Self> {
        AddrIncoming::new(addr)
    }

    /// Get the local address bound to this listener.
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// Set the value of `TCP_NODELAY` option for accepted connections.
    #[cfg_attr(tarpaulin, skip)]
    pub fn set_nodelay(&mut self, enabled: bool) -> &mut Self {
        self.tcp_nodelay = enabled;
        self
    }

    /// Set whether to sleep on accept errors.
    ///
    /// A possible scenario is that the process has hit the max open files
    /// allowed, and so trying to accept a new connection will fail with
    /// `EMFILE`. In some cases, it's preferable to just wait for some time, if
    /// the application will likely close some files (or connections), and try
    /// to accept the connection again. If this option is `true`, the error
    /// will be logged at the `error` level, since it is still a big deal,
    /// and then the listener will sleep for 1 second.
    ///
    /// In other cases, hitting the max open files should be treat similarly
    /// to being out-of-memory, and simply error (and shutdown). Setting
    /// this option to `false` will allow that.
    ///
    /// Default is `true`.
    #[cfg_attr(tarpaulin, skip)]
    pub fn set_sleep_on_errors(&mut self, val: bool) {
        self.sleep_on_errors = val;
    }

    fn poll_next_(&mut self, cx: &mut task::Context<'_>) -> Poll<io::Result<AddrStream>> {
        // Check if a previous timeout is active that was set by IO errors.
        if let Some(ref mut to) = self.timeout {
            match Pin::new(to).poll(cx) {
                Poll::Ready(()) => {}
                Poll::Pending => return Poll::Pending,
            }
        }
        self.timeout = None;

        let accept = self.listener.accept();
        futures::pin_mut!(accept);

        loop {
            match accept.poll_unpin(cx) {
                Poll::Ready(Ok((socket, addr))) => {
                    if let Err(e) = socket.set_nodelay(self.tcp_nodelay) {
                        trace!("error trying to set TCP nodelay: {}", e);
                    }
                    return Poll::Ready(Ok(AddrStream::new(socket, addr)));
                }
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Err(e)) => {
                    // Connection errors can be ignored directly, continue by
                    // accepting the next request.
                    if is_connection_error(&e) {
                        debug!("accepted connection already errored: {}", e);
                        continue;
                    }

                    if self.sleep_on_errors {
                        error!("accept error: {}", e);

                        // Sleep 1s.
                        let mut timeout = Delay::new(Duration::from_secs(1));

                        match Pin::new(&mut timeout).poll(cx) {
                            Poll::Ready(()) => {
                                // Wow, it's been a second already? Ok then...
                                continue;
                            }
                            Poll::Pending => {
                                self.timeout = Some(timeout);
                                return Poll::Pending;
                            }
                        }
                    } else {
                        return Poll::Ready(Err(e));
                    }
                }
            }
        }
    }
}

impl Accept for AddrIncoming {
    type Conn = AddrStream;
    type Error = io::Error;

    fn poll_accept(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Option<Result<Self::Conn, Self::Error>>> {
        let result = futures::ready!(self.poll_next_(cx));
        Poll::Ready(Some(result))
    }
}

/// This function defines errors that are per-connection. Which basically
/// means that if we get this error from `accept()` system call it means
/// next connection might be ready to be accepted.
///
/// All other errors will incur a timeout before next `accept()` is performed.
/// The timeout is useful to handle resource exhaustion errors like ENFILE
/// and EMFILE. Otherwise, could enter into tight loop.
#[cfg_attr(tarpaulin, skip)]
fn is_connection_error(e: &io::Error) -> bool {
    match e.kind() {
        io::ErrorKind::ConnectionRefused
        | io::ErrorKind::ConnectionAborted
        | io::ErrorKind::ConnectionReset => true,
        _ => false,
    }
}

#[cfg_attr(tarpaulin, skip)]
impl fmt::Debug for AddrIncoming {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AddrIncoming")
            .field("addr", &self.addr)
            .field("sleep_on_errors", &self.sleep_on_errors)
            .field("tcp_nodelay", &self.tcp_nodelay)
            .finish()
    }
}

mod addr_stream {
    use async_std::net::TcpStream;
    use async_std::sync::Arc;
    use std::io;
    use std::net::SocketAddr;
    use std::pin::Pin;
    use std::task::{self, Poll};
    use tokio::io::{AsyncRead, AsyncWrite};

    /// A transport returned yieled by `AddrIncoming`.
    #[derive(Debug, Clone)]
    pub struct AddrStream {
        inner: Arc<TcpStream>,
        pub(super) remote_addr: SocketAddr,
    }

    impl AddrStream {
        pub(super) fn new(tcp: TcpStream, addr: SocketAddr) -> AddrStream {
            AddrStream {
                inner: Arc::new(tcp),
                remote_addr: addr,
            }
        }

        /// Returns the remote (peer) address of this connection.
        #[inline]
        pub fn remote_addr(&self) -> SocketAddr {
            self.remote_addr
        }

        /// Consumes the AddrStream and returns the underlying IO object
        #[inline]
        pub fn stream(&self) -> &TcpStream {
            &*self.inner
        }
    }

    impl AsyncRead for AddrStream {
        #[inline]
        fn poll_read(
            self: Pin<&mut Self>,
            cx: &mut task::Context<'_>,
            buf: &mut [u8],
        ) -> Poll<io::Result<usize>> {
            futures::AsyncRead::poll_read(Pin::new(&mut self.stream()), cx, buf)
        }
    }

    impl AsyncWrite for AddrStream {
        #[inline]
        fn poll_write(
            self: Pin<&mut Self>,
            cx: &mut task::Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            futures::AsyncWrite::poll_write(Pin::new(&mut self.stream()), cx, buf)
        }

        #[inline]
        fn poll_flush(self: Pin<&mut Self>, _cx: &mut task::Context<'_>) -> Poll<io::Result<()>> {
            // TCP flush is a noop
            Poll::Ready(Ok(()))
        }

        #[inline]
        fn poll_shutdown(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<io::Result<()>> {
            futures::AsyncWrite::poll_close(Pin::new(&mut self.stream()), cx)
        }
    }
}
