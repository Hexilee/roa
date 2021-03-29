use std::future::Future;
use std::net::{SocketAddr, TcpListener as StdListener, ToSocketAddrs};
use std::pin::Pin;
use std::task::{self, Poll};
use std::time::Duration;
use std::{fmt, io};

use futures::FutureExt as _;
use log::{debug, error, trace};
use roa::stream::AsyncStream;
use roa::{Accept, AddrStream};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{delay_for, Delay};

/// A stream of connections from binding to an address.
/// As an implementation of roa_core::Accept.
#[must_use = "streams do nothing unless polled"]
pub struct TcpIncoming {
    addr: SocketAddr,
    listener: TcpListener,
    tcp_keepalive_timeout: Option<Duration>,
    sleep_on_errors: bool,
    tcp_nodelay: bool,
    timeout: Option<Delay>,
}

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
            listener: TcpListener::from_std(listener)?,
            addr,
            tcp_keepalive_timeout: None,
            sleep_on_errors: true,
            tcp_nodelay: false,
            timeout: None,
        })
    }

    /// Get the local address bound to this listener.
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// Set whether TCP keepalive messages are enabled on accepted connections.
    ///
    /// If `None` is specified, keepalive is disabled, otherwise the duration
    /// specified will be the time to remain idle before sending TCP keepalive
    /// probes.
    pub fn set_keepalive(&mut self, keepalive: Option<Duration>) -> &mut Self {
        self.tcp_keepalive_timeout = keepalive;
        self
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
                Poll::Ready(Ok((stream, addr))) => {
                    if let Some(dur) = self.tcp_keepalive_timeout {
                        if let Err(e) = stream.set_keepalive(Some(dur)) {
                            trace!("error trying to set TCP keepalive: {}", e);
                        }
                    }
                    if let Err(e) = stream.set_nodelay(self.tcp_nodelay) {
                        trace!("error trying to set TCP nodelay: {}", e);
                    }
                    return Poll::Ready(Ok((stream, addr)));
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
                        let mut timeout = delay_for(Duration::from_secs(1));

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

impl Accept for TcpIncoming {
    type Conn = AddrStream<AsyncStream<TcpStream>>;
    type Error = io::Error;

    #[inline]
    fn poll_accept(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Option<Result<Self::Conn, Self::Error>>> {
        let (stream, addr) = futures::ready!(self.poll_stream(cx))?;
        let addr_stream = AddrStream::new(addr, AsyncStream(stream));
        Poll::Ready(Some(Ok(addr_stream)))
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
    match e.kind() {
        io::ErrorKind::ConnectionRefused
        | io::ErrorKind::ConnectionAborted
        | io::ErrorKind::ConnectionReset => true,
        _ => false,
    }
}

impl fmt::Debug for TcpIncoming {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TcpIncoming")
            .field("addr", &self.addr)
            .field("tcp_keepalive_timeout", &self.tcp_keepalive_timeout)
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

    use super::TcpIncoming;
    use crate::Exec;

    #[tokio::test]
    async fn incoming() -> Result<(), Box<dyn Error>> {
        let app = App::with_exec((), Exec).end(());
        let incoming = TcpIncoming::bind("127.0.0.1:0")?;
        let addr = incoming.local_addr();
        tokio::spawn(app.accept(incoming));
        let resp = reqwest::get(&format!("http://{}", addr)).await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }
}
