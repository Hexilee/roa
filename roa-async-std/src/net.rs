use std::future::Future;
use std::mem::transmute;
use std::net::{TcpListener as StdListener, ToSocketAddrs};
use std::pin::Pin;
use std::task::{self, Poll};
use std::time::Duration;
use std::{fmt, io, matches};

use async_std::net::{SocketAddr, TcpListener, TcpStream};
use futures_timer::Delay;
use roa::stream::AsyncStream;
use roa::{Accept, AddrStream};
use tracing::{debug, error, trace};

/// A stream of connections from binding to an address.
/// As an implementation of roa_core::Accept.
#[must_use = "streams do nothing unless polled"]
pub struct TcpIncoming {
    addr: SocketAddr,
    listener: Box<TcpListener>,
    sleep_on_errors: bool,
    tcp_nodelay: bool,
    timeout: Option<Pin<Box<Delay>>>,
    accept: Option<Pin<BoxedAccept<'static>>>,
}

type BoxedAccept<'a> =
    Box<dyn 'a + Future<Output = io::Result<(TcpStream, SocketAddr)>> + Send + Sync>;

impl TcpIncoming {
    /// Creates a new `TcpIncoming` binding to provided socket address.
    pub fn bind(addr: impl ToSocketAddrs) -> io::Result<Self> {
        let listener = StdListener::bind(addr)?;
        TcpIncoming::from_std(listener)
    }

    /// Creates a new `TcpIncoming` from std TcpListener.
    pub fn from_std(listener: StdListener) -> io::Result<Self> {
        let addr = listener.local_addr()?;
        Ok(TcpIncoming {
            listener: Box::new(listener.into()),
            addr,
            sleep_on_errors: true,
            tcp_nodelay: false,
            timeout: None,
            accept: None,
        })
    }

    /// Get the local address bound to this listener.
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// Set the value of `TCP_NODELAY` option for accepted connections.
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
    pub fn set_sleep_on_errors(&mut self, val: bool) {
        self.sleep_on_errors = val;
    }

    /// Poll TcpStream.
    fn poll_stream(
        &mut self,
        cx: &mut task::Context<'_>,
    ) -> Poll<io::Result<(TcpStream, SocketAddr)>> {
        // Check if a previous timeout is active that was set by IO errors.
        if let Some(ref mut to) = self.timeout {
            futures::ready!(Pin::new(to).poll(cx));
        }
        self.timeout = None;

        loop {
            if self.accept.is_none() {
                let accept: Pin<BoxedAccept<'_>> = Box::pin(self.listener.accept());
                self.accept = Some(unsafe { transmute(accept) });
            }

            if let Some(f) = &mut self.accept {
                match futures::ready!(f.as_mut().poll(cx)) {
                    Ok((socket, addr)) => {
                        if let Err(e) = socket.set_nodelay(self.tcp_nodelay) {
                            trace!("error trying to set TCP nodelay: {}", e);
                        }
                        self.accept = None;
                        return Poll::Ready(Ok((socket, addr)));
                    }
                    Err(e) => {
                        // Connection errors can be ignored directly, continue by
                        // accepting the next request.
                        if is_connection_error(&e) {
                            debug!("accepted connection already errored: {}", e);
                            continue;
                        }

                        if self.sleep_on_errors {
                            error!("accept error: {}", e);

                            // Sleep 1s.
                            let mut timeout = Box::pin(Delay::new(Duration::from_secs(1)));

                            match timeout.as_mut().poll(cx) {
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
}

impl Accept for TcpIncoming {
    type Conn = AddrStream<AsyncStream<TcpStream>>;
    type Error = io::Error;

    #[inline]
    fn poll_accept(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Option<Result<Self::Conn, Self::Error>>> {
        let (stream, addr) = futures::ready!(self.poll_stream(cx))?;
        Poll::Ready(Some(Ok(AddrStream::new(addr, AsyncStream(stream)))))
    }
}

impl Drop for TcpIncoming {
    fn drop(&mut self) {
        self.accept = None;
    }
}

/// This function defines errors that are per-connection. Which basically
/// means that if we get this error from `accept()` system call it means
/// next connection might be ready to be accepted.
///
/// All other errors will incur a timeout before next `accept()` is performed.
/// The timeout is useful to handle resource exhaustion errors like ENFILE
/// and EMFILE. Otherwise, could enter into tight loop.
fn is_connection_error(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::ConnectionRefused
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::ConnectionReset
    )
}

impl fmt::Debug for TcpIncoming {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TcpIncoming")
            .field("addr", &self.addr)
            .field("sleep_on_errors", &self.sleep_on_errors)
            .field("tcp_nodelay", &self.tcp_nodelay)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use roa::http::StatusCode;
    use roa::App;
    use tracing_subscriber::{fmt, EnvFilter};

    use super::TcpIncoming;
    use crate::Exec;

    #[tokio::test]
    async fn incoming() -> Result<(), Box<dyn Error>> {
        fmt().with_env_filter(EnvFilter::from_default_env()).init();
        let app = App::with_exec((), Exec).end(());
        let incoming = TcpIncoming::bind("127.0.0.1:0")?;
        let addr = incoming.local_addr();
        tokio::task::spawn(app.accept(incoming));
        let resp = reqwest::get(&format!("http://{}", addr)).await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }
}
