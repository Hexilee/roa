#[cfg(feature = "runtime")]
mod runtime;

mod future;
mod stream;
use crate::{
    Chain, Context, Endpoint, Middleware, MiddlewareExt, Request, Response, State,
};
use future::SendFuture;
use http::{Request as HttpRequest, Response as HttpResponse};
use hyper::service::Service;
use hyper::Body as HyperBody;
use hyper::Server;
use std::error::Error;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Poll;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::Accept;
use crate::{Executor, Spawn};
use std::convert::Infallible;
pub use stream::AddrStream;

/// The Application of roa.
/// ### Example
/// ```rust,no_run
/// use roa_core::{App, Context, Next, Result, MiddlewareExt};
/// use log::info;
/// use async_std::fs::File;
///
/// let app = App::new(()).gate(gate).end(end);
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
/// let app = App::new(State::new()).gate(gate).end(end);
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
pub struct App<S, T> {
    service: T,
    exec: Executor,
    state: S,
}

/// An implementation of hyper HttpService.
pub struct HttpService<S, E> {
    endpoint: Arc<E>,
    remote_addr: SocketAddr,
    exec: Executor,
    pub(crate) state: S,
}

impl<S, T> App<S, T> {
    /// Map app::service
    fn map_service<U>(self, mapper: impl FnOnce(T) -> U) -> App<S, U> {
        let Self {
            exec,
            state,
            service,
        } = self;
        App {
            service: mapper(service),
            exec,
            state,
        }
    }
}

impl<S> App<S, ()> {
    /// Construct an application with custom runtime.
    pub fn with_exec(state: S, exec: impl 'static + Send + Sync + Spawn) -> Self {
        Self {
            service: (),
            exec: Executor(Arc::new(exec)),
            state,
        }
    }
}

impl<S, T> App<S, T>
where
    T: for<'a> Middleware<'a, S>,
{
    /// Use a middleware.
    pub fn gate<M>(self, middleware: M) -> App<S, Chain<T, M>>
    where
        M: for<'a> Middleware<'a, S>,
    {
        self.map_service(move |service| service.chain(middleware))
    }

    /// Set endpoint, then app can only be used to serve http request.
    pub fn end<E>(self, endpoint: E) -> App<S, Arc<Chain<T, E>>>
    where
        E: for<'a> Endpoint<'a, S>,
    {
        self.map_service(move |service| Arc::new(service.end(endpoint)))
    }
}

impl<S, E> App<S, Arc<E>>
where
    E: for<'a> Endpoint<'a, S>,
{
    /// Construct a hyper server by an incoming.
    pub fn accept<I, IO>(self, incoming: I) -> Server<I, Self, Executor>
    where
        S: State,
        IO: 'static + Send + Sync + Unpin + AsyncRead + AsyncWrite,
        I: Accept<Conn = AddrStream<IO>>,
        I::Error: Into<Box<dyn Error + Send + Sync>>,
    {
        Server::builder(incoming)
            .executor(self.exec.clone())
            .serve(self)
    }

    /// Make a fake http service for test.
    #[cfg(test)]
    pub fn http_service(&self) -> HttpService<S, E>
    where
        S: Clone,
    {
        let endpoint = self.service.clone();
        let addr = ([127, 0, 0, 1], 0);
        let state = self.state.clone();
        let exec = self.exec.clone();
        HttpService::new(endpoint, addr.into(), exec, state)
    }
}

macro_rules! impl_poll_ready {
    () => {
        #[inline]
        fn poll_ready(&mut self, _cx: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
    };
}

type AppFuture<S, E> =
    Pin<Box<dyn 'static + Future<Output = std::io::Result<HttpService<S, E>>> + Send>>;

impl<S, E, IO> Service<&AddrStream<IO>> for App<S, Arc<E>>
where
    S: State,
    E: for<'a> Endpoint<'a, S>,
    IO: 'static + Send + Sync + Unpin + AsyncRead + AsyncWrite,
{
    type Response = HttpService<S, E>;
    type Error = std::io::Error;
    type Future = AppFuture<S, E>;
    impl_poll_ready!();

    #[inline]
    fn call(&mut self, stream: &AddrStream<IO>) -> Self::Future {
        let endpoint = self.service.clone();
        let addr = stream.remote_addr;
        let state = self.state.clone();
        let exec = self.exec.clone();
        Box::pin(async move { Ok(HttpService::new(endpoint, addr, exec, state)) })
    }
}

type HttpFuture = Pin<
    Box<
        dyn 'static
            + Future<Output = Result<HttpResponse<HyperBody>, Infallible>>
            + Send,
    >,
>;

impl<S, E> Service<HttpRequest<HyperBody>> for HttpService<S, E>
where
    S: State,
    E: for<'a> Endpoint<'a, S>,
{
    type Response = HttpResponse<HyperBody>;
    type Error = Infallible;
    type Future = HttpFuture;
    impl_poll_ready!();

    #[inline]
    fn call(&mut self, req: HttpRequest<HyperBody>) -> Self::Future {
        let service = self.clone();
        Box::pin(async move {
            let serve_future = SendFuture(Box::pin(service.serve(req.into())));
            Ok(serve_future.await.into())
        })
    }
}

impl<S, E> HttpService<S, E> {
    pub fn new(
        endpoint: Arc<E>,
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
    pub async fn serve(self, req: Request) -> Response
    where
        S: 'static,
        E: for<'a> Endpoint<'a, S>,
    {
        let Self {
            endpoint,
            remote_addr,
            exec,
            state,
        } = self;
        let mut ctx = Context::new(req, state, exec, remote_addr);
        if let Err(status) = endpoint.call(&mut ctx).await {
            ctx.resp.status = status.status_code;
            if status.expose {
                ctx.resp.write(status.message);
            } else {
                ctx.exec
                    .spawn_blocking(move || log::error!("Uncaught status: {}", status))
                    .await;
            }
        }
        ctx.resp
    }
}

impl<S: Clone, E> Clone for HttpService<S, E> {
    fn clone(&self) -> Self {
        Self {
            endpoint: self.endpoint.clone(),
            state: self.state.clone(),
            exec: self.exec.clone(),
            remote_addr: self.remote_addr,
        }
    }
}

impl<S: Clone> Clone for App<S, Arc<dyn for<'a> Endpoint<'a, S>>> {
    fn clone(&self) -> Self {
        Self {
            service: self.service.clone(),
            state: self.state.clone(),
            exec: self.exec.clone(),
        }
    }
}

#[cfg(all(test, feature = "runtime"))]
mod tests {
    use crate::{App, Request};
    use http::StatusCode;

    #[async_std::test]
    async fn gate_simple() -> Result<(), Box<dyn std::error::Error>> {
        let service = App::new(()).end(()).http_service();
        let resp = service.serve(Request::default()).await;
        assert_eq!(StatusCode::OK, resp.status);
        Ok(())
    }
}
