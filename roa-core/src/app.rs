#[cfg(feature = "runtime")]
mod runtime;

mod future;
mod stream;
use crate::{Context, Endpoint, Error, Next, Request, Response, Result, State};
use future::SendFuture;
use http::{Request as HttpRequest, Response as HttpResponse};
use hyper::service::Service;
use hyper::Body as HyperBody;
use hyper::Server;
use std::error::Error as StdError;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::result::Result as StdResult;
use std::sync::Arc;
use std::task::Poll;

use crate::Accept;
use crate::{Executor, Spawn};
pub use stream::AddrStream;

/// The Application of roa.
/// ### Example
/// ```rust,no_run
/// use roa_core::{App, Context, Next, Result, MiddlewareExt};
/// use log::info;
/// use async_std::fs::File;
///
/// let app = App::new((), gate.chain(end));
/// async fn gate(ctx: &mut Context<()>, next: Next<'_>) -> Result {
///     info!("{} {}", ctx.method(), ctx.uri());
///     next.await
/// }
///
/// async fn end(ctx: &mut Context<()>) -> Result {
///     ctx.resp.write_reader(File::open("assets/welcome.html").await?);
///     Ok(())
/// }
/// ```
///
/// ### State
/// The `State` is designed to share data or handler between middlewares.
/// The only one type implemented `State` by this crate is `()`, you can implement your custom state if neccassary.
///
/// ```rust
/// use roa_core::{App, Context, Next, Result, MiddlewareExt};
/// use log::info;
/// use futures::lock::Mutex;
/// use std::sync::Arc;
/// use std::collections::HashMap;
///
/// #[derive(Clone)]
/// struct State {
///     id: u64,
///     database: Arc<Mutex<HashMap<u64, String>>>,
/// }
///
/// impl State {
///     fn new() -> Self {
///         Self {
///             id: 0,
///             database: Arc::new(Mutex::new(HashMap::new()))
///         }
///     }
/// }
///
/// let app = App::new(State::new(), gate.chain(end));
/// async fn gate(ctx: &mut Context<State>, next: Next<'_>) -> Result {
///     ctx.id = 1;
///     next.await
/// }
///
/// async fn end(ctx: &mut Context<State>) -> Result {
///     let id = ctx.id;
///     ctx.database.lock().await.get(&id);
///     Ok(())
/// }
/// ```
///
pub struct App<S> {
    endpoint: Arc<dyn for<'a> Endpoint<'a, S>>,
    exec: Executor,
    pub(crate) state: S,
}

/// An implementation of hyper HttpService.
pub struct HttpService<S> {
    endpoint: Arc<dyn for<'a> Endpoint<'a, S>>,
    remote_addr: SocketAddr,
    exec: Executor,
    pub(crate) state: S,
}

impl<S> App<S> {
    /// Construct an application with custom runtime.
    pub fn with_exec(
        state: S,
        endpoint: impl for<'a> Endpoint<'a, S>,
        exec: impl 'static + Send + Sync + Spawn,
    ) -> Self {
        Self {
            endpoint: Arc::new(endpoint),
            exec: Executor(Arc::new(exec)),
            state,
        }
    }

    /// Construct a hyper server by an incoming.
    pub fn accept<I>(self, incoming: I) -> Server<I, Self, Executor>
    where
        S: State,
        I: Accept<Conn = AddrStream>,
        I::Error: Into<Box<dyn StdError + Send + Sync>>,
    {
        Server::builder(incoming)
            .executor(self.exec.clone())
            .serve(self)
    }

    /// Make a fake http service for test.
    #[cfg(test)]
    pub fn http_service(&self) -> HttpService<S>
    where
        S: Clone,
    {
        let endpoint = self.endpoint.clone();
        let addr = ([127, 0, 0, 1], 0);
        let state = self.state.clone();
        let exec = self.exec.clone();
        HttpService::new(endpoint, addr.into(), exec, state)
    }
}

macro_rules! impl_poll_ready {
    () => {
        #[inline]
        fn poll_ready(&mut self, _cx: &mut std::task::Context<'_>) -> Poll<StdResult<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
    };
}

type AppFuture<S> =
    Pin<Box<dyn 'static + Future<Output = std::io::Result<HttpService<S>>> + Send>>;

impl<S: State> Service<&AddrStream> for App<S> {
    type Response = HttpService<S>;
    type Error = std::io::Error;
    type Future = AppFuture<S>;
    impl_poll_ready!();

    #[inline]
    fn call(&mut self, stream: &AddrStream) -> Self::Future {
        let endpoint = self.endpoint.clone();
        let addr = stream.remote_addr();
        let state = self.state.clone();
        let exec = self.exec.clone();
        Box::pin(async move { Ok(HttpService::new(endpoint, addr, exec, state)) })
    }
}

type HttpFuture =
    Pin<Box<dyn 'static + Future<Output = Result<HttpResponse<HyperBody>>> + Send>>;

impl<S: State> Service<HttpRequest<HyperBody>> for HttpService<S> {
    type Response = HttpResponse<HyperBody>;
    type Error = Error;
    type Future = HttpFuture;
    impl_poll_ready!();

    #[inline]
    fn call(&mut self, req: HttpRequest<HyperBody>) -> Self::Future {
        let service = self.clone();
        Box::pin(async move {
            let serve_future = SendFuture(Box::pin(service.serve(req.into())));
            Ok(serve_future.await?.into())
        })
    }
}

impl<S> HttpService<S> {
    pub fn new(
        endpoint: Arc<dyn for<'a> Endpoint<'a, S>>,
        remote_addr: SocketAddr,
        exec: Executor,
        state: S,
    ) -> Self {
        Self {
            endpoint,
            remote_addr,
            exec,
            state,
        }
    }

    /// Receive a request then return a response.
    /// The entry point of http service.
    pub async fn serve(self, req: Request) -> Result<Response>
    where
        S: 'static,
    {
        let Self {
            endpoint,
            remote_addr,
            exec,
            state,
        } = self;
        let mut context = Context::new(req, state, exec, remote_addr);
        if let Err(err) = endpoint.end(&mut context).await {
            context.resp.status = err.status_code;
            if err.expose && !err.need_throw() {
                context.resp.write(err.message);
            } else if err.expose && err.need_throw() {
                context.resp.write(err.message.clone());
                return Err(err);
            } else if err.need_throw() {
                return Err(err);
            }
        }
        Ok(std::mem::take(&mut context.resp))
    }
}

impl<S: Clone> Clone for HttpService<S> {
    fn clone(&self) -> Self {
        Self {
            endpoint: self.endpoint.clone(),
            state: self.state.clone(),
            exec: self.exec.clone(),
            remote_addr: self.remote_addr,
        }
    }
}

impl<S: Clone> Clone for App<S> {
    fn clone(&self) -> Self {
        Self {
            endpoint: self.endpoint.clone(),
            state: self.state.clone(),
            exec: self.exec.clone(),
        }
    }
}

#[cfg(all(test, feature = "runtime"))]
mod tests {
    use crate::{App, Context, Error, Request};
    use http::StatusCode;

    #[async_std::test]
    async fn gate_simple() -> Result<(), Box<dyn std::error::Error>> {
        async fn test(ctx: &mut Context<()>) -> Result<(), Error> {
            Ok(())
        }
        let service = App::new((), test).http_service();
        let resp = service.serve(Request::default()).await?;
        assert_eq!(StatusCode::OK, resp.status);
        Ok(())
    }
}
