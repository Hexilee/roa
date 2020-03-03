#[cfg(feature = "runtime")]
mod runtime;

mod future;
mod stream;
use crate::{
    join, join_all, Context, Error, Middleware, Next, Request, Response, Result, State,
};
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
/// use roa_core::App;
/// use log::info;
/// use async_std::fs::File;
///
/// let mut app = App::new(());
/// app.gate_fn(|ctx, next| async move {
///     info!("{} {}", ctx.method(), ctx.uri());
///     next.await
/// });
/// app.end(|mut ctx| async move {
///     ctx.resp_mut().write_reader(File::open("assets/welcome.html").await?);
///     Ok(())
/// });
/// ```
///
/// ### State
/// The `State` is designed to share data or handler between middlewares.
/// The only one type implemented `State` by this crate is `()`, you can implement your custom state if neccassary.
///
/// ```rust
/// use roa_core::App;
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
/// let mut app = App::new(State::new());
/// app.gate_fn(|mut ctx, next| async move {
///     ctx.id = 1;
///     next.await
/// });
/// app.end(|ctx| async move {
///     let id = ctx.id;
///     ctx.database.lock().await.get(&id);
///     Ok(())
/// });
/// ```
///
pub struct App<S> {
    middleware: Arc<dyn Middleware<S>>,
    exec: Executor,
    pub(crate) state: S,
}

/// An implementation of hyper HttpService.
pub struct HttpService<S> {
    middleware: Arc<dyn Middleware<S>>,
    remote_addr: SocketAddr,
    exec: Executor,
    pub(crate) state: S,
}

impl<S: State> App<S> {
    /// Construct an application with custom runtime.
    pub fn with_exec(state: S, exec: impl 'static + Send + Sync + Spawn) -> Self {
        Self {
            middleware: Arc::new(join_all(Vec::new())),
            exec: Executor(Arc::new(exec)),
            state,
        }
    }

    /// Use a middleware.
    pub fn gate(&mut self, middleware: impl Middleware<S>) -> &mut Self {
        self.middleware = Arc::new(join(self.middleware.clone(), middleware));
        self
    }

    /// A sugar to match a lambda as a middleware.
    ///
    /// `App::gate` cannot match a lambda without parameter type indication.
    ///
    /// ```rust
    /// use roa_core::{App, Next};
    ///
    /// let mut app = App::new(());
    /// // app.gate(|_ctx, next| async move { next.await }); compile fails.
    /// app.gate(|_ctx, next: Next| async move { next.await });
    /// ```
    ///
    /// However, with `App::gate_fn`, you can match a lambda without type indication.
    /// ```rust
    /// use roa_core::{App, Next};
    ///
    /// let mut app = App::new(());
    /// app.gate_fn(|_ctx, next| async move { next.await });
    /// ```
    pub fn gate_fn<F>(
        &mut self,
        middleware: impl 'static + Sync + Send + Fn(Context<S>, Next) -> F,
    ) -> &mut Self
    where
        F: 'static + Future<Output = Result>,
    {
        self.gate(middleware)
    }

    /// A sugar to match a function pointer like `async fn(Context<S>) -> impl Future`
    /// and use it as a middleware(endpoint).
    ///
    /// As the ducument of `Middleware`, an endpoint is defined as a template:
    ///
    /// ```rust
    /// use roa_core::{App, Context, Result};
    /// use std::future::Future;
    ///
    /// fn endpoint<F>(ctx: Context<()>) -> F
    /// where F: 'static + Send + Future<Output=Result> {
    ///     unimplemented!()
    /// }
    /// ```
    ///
    /// However, an async function is not a template,
    /// it needs a transfer function to suit for `App::gate`.
    ///
    /// ```rust
    /// use roa_core::{App, Context, Result, State, Middleware};
    /// use std::future::Future;
    ///
    /// async fn endpoint(ctx: Context<()>) -> Result {
    ///     Ok(())
    /// }
    ///
    /// fn transfer<S, F>(endpoint: fn(Context<S>) -> F) -> impl Middleware<S>
    /// where S: State,
    ///       F: 'static + Future<Output=Result> {
    ///     endpoint
    /// }
    ///
    /// App::new(()).gate(transfer(endpoint));
    /// ```
    ///
    /// And `App::end` is a wrapper of `App::gate` with this transfer function.
    ///
    /// ```rust
    /// use roa_core::App;
    /// App::new(()).end(|_ctx| async { Ok(()) });
    /// ```
    pub fn end<F>(&mut self, endpoint: fn(Context<S>) -> F) -> &mut Self
    where
        F: 'static + Future<Output = Result>,
    {
        self.gate(endpoint)
    }

    /// Construct a hyper server by an incoming.
    pub fn accept<I>(&self, incoming: I) -> Server<I, Self, Executor>
    where
        I: Accept<Conn = AddrStream>,
        I::Error: Into<Box<dyn StdError + Send + Sync>>,
    {
        Server::builder(incoming)
            .executor(self.exec.clone())
            .serve(self.clone())
    }

    /// Make a fake http service for test.
    #[cfg(test)]
    pub fn fake_service(&self) -> HttpService<S> {
        let middleware = self.middleware.clone();
        let addr = ([127, 0, 0, 1], 0);
        let state = self.state.clone();
        let exec = self.exec.clone();
        HttpService::new(middleware, addr.into(), exec, state)
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
        let middleware = self.middleware.clone();
        let addr = stream.remote_addr();
        let state = self.state.clone();
        let exec = self.exec.clone();
        Box::pin(async move { Ok(HttpService::new(middleware, addr, exec, state)) })
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

impl<S: State> HttpService<S> {
    pub fn new(
        middleware: Arc<dyn Middleware<S>>,
        remote_addr: SocketAddr,
        exec: Executor,
        state: S,
    ) -> Self {
        Self {
            middleware,
            remote_addr,
            exec,
            state,
        }
    }

    /// Receive a request then return a response.
    /// The entry point of middlewares.
    pub async fn serve(self, req: Request) -> Result<Response> {
        let Self {
            middleware,
            remote_addr,
            exec,
            state,
        } = self;
        let mut context = Context::new(req, state, exec, remote_addr);
        if let Err(err) = middleware.end(unsafe { context.unsafe_clone() }).await {
            context.resp_mut().status = err.status_code;
            if err.expose && !err.need_throw() {
                context.resp_mut().write(err.message);
            } else if err.expose && err.need_throw() {
                context.resp_mut().write(err.message.clone());
                return Err(err);
            } else if err.need_throw() {
                return Err(err);
            }
        }
        Ok(std::mem::take(&mut *context.resp_mut()))
    }
}

impl<S: State> Clone for App<S> {
    fn clone(&self) -> Self {
        Self {
            middleware: self.middleware.clone(),
            exec: self.exec.clone(),
            state: self.state.clone(),
        }
    }
}

impl<S: State> Clone for HttpService<S> {
    fn clone(&self) -> Self {
        Self {
            middleware: self.middleware.clone(),
            state: self.state.clone(),
            exec: self.exec.clone(),
            remote_addr: self.remote_addr,
        }
    }
}

#[cfg(all(test, feature = "runtime"))]
mod tests {
    use crate::{App, Request};
    use http::StatusCode;
    use std::time::Instant;

    #[async_std::test]
    async fn gate_simple() -> Result<(), Box<dyn std::error::Error>> {
        let service = App::new(())
            .gate_fn(|_ctx, next| async move {
                let inbound = Instant::now();
                next.await?;
                println!("time elapsed: {} ms", inbound.elapsed().as_millis());
                Ok(())
            })
            .fake_service();
        let resp = service.serve(Request::default()).await?;
        assert_eq!(StatusCode::OK, resp.status);
        Ok(())
    }
}
