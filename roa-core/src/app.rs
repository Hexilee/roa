#[cfg(feature = "runtime")]
mod runtime;

mod future;
mod pool;
mod stream;
use crate::{join_all, Context, Error, Middleware, Next, Result, State};
use futures::future::err;
use http::{Request as HttpRequest, Response as HttpResponse, StatusCode};
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
use future::SendFuture;
use pool::{ContextPool, DEFAULT_MAX_SIZE, DEFAULT_MIN_SIZE};
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
    middlewares: Vec<Arc<dyn Middleware<S>>>,
    state: S,
    exec: Executor,
    ctx_pool_min: usize,
    ctx_pool_max: usize,
}

/// An implementation of hyper MakeService.
pub struct AppService<S> {
    middleware: Arc<dyn Middleware<S>>,
    pool: Arc<ContextPool<S>>,
}

/// An implementation of hyper HttpService.
pub struct HttpService<S> {
    middleware: Arc<dyn Middleware<S>>,
    pool: Arc<ContextPool<S>>,
    remote_addr: SocketAddr,
}

impl<S: State> App<S> {
    /// Construct an application with custom runtime.
    pub fn with_exec(state: S, exec: impl 'static + Send + Sync + Spawn) -> Self {
        Self {
            middlewares: Vec::new(),
            exec: Executor(Arc::new(exec)),
            state,
            ctx_pool_min: DEFAULT_MIN_SIZE,
            ctx_pool_max: DEFAULT_MAX_SIZE,
        }
    }

    /// Use a middleware.
    pub fn gate(&mut self, middleware: impl Middleware<S>) -> &mut Self {
        self.middlewares.push(Arc::new(middleware));
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

    /// Set min size of context pool,
    /// default value is 1 << 8.
    pub fn ctx_pool_min(&mut self, min: usize) -> &mut Self {
        self.ctx_pool_min = min;
        self
    }

    /// Set max size of context pool,
    /// default value is 1 << 20.
    pub fn ctx_pool_max(&mut self, max: usize) -> &mut Self {
        self.ctx_pool_max = max;
        self
    }

    /// Build an app service.
    fn service(&self) -> AppService<S> {
        let pool = ContextPool::new(
            self.ctx_pool_min,
            self.ctx_pool_max,
            self.state.clone(),
            self.exec.clone(),
        );
        AppService {
            pool: Arc::new(pool),
            middleware: Arc::new(join_all(self.middlewares.clone())),
        }
    }

    /// Construct a hyper server by an incoming.
    pub fn accept<I>(&self, incoming: I) -> Server<I, AppService<S>, Executor>
    where
        I: Accept<Conn = AddrStream>,
        I::Error: Into<Box<dyn StdError + Send + Sync>>,
    {
        Server::builder(incoming)
            .executor(self.exec.clone())
            .serve(self.service())
    }

    /// Make a fake http service for test.
    #[cfg(test)]
    pub fn http_service(&self) -> HttpService<S> {
        let middleware = Arc::new(join_all(self.middlewares.clone()));
        let addr = ([127, 0, 0, 1], 0);
        let state = self.state.clone();
        let exec = self.exec.clone();
        let pool = ContextPool::new(self.ctx_pool_min, self.ctx_pool_max, state, exec);
        HttpService::new(middleware, addr.into(), Arc::new(pool))
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

impl<S: State> Service<&AddrStream> for AppService<S> {
    type Response = HttpService<S>;
    type Error = std::io::Error;
    type Future = AppFuture<S>;

    impl_poll_ready! {}

    #[inline]
    fn call(&mut self, stream: &AddrStream) -> Self::Future {
        let middleware = self.middleware.clone();
        let addr = stream.remote_addr();
        let pool = self.pool.clone();
        Box::pin(async move { Ok(HttpService::new(middleware, addr, pool)) })
    }
}

type HttpFuture =
    Pin<Box<dyn 'static + Future<Output = Result<HttpResponse<HyperBody>>> + Send>>;

impl<S: State> Service<HttpRequest<HyperBody>> for HttpService<S> {
    type Response = HttpResponse<HyperBody>;
    type Error = Error;
    type Future = HttpFuture;

    impl_poll_ready! {}

    #[inline]
    fn call(&mut self, mut req: HttpRequest<HyperBody>) -> Self::Future {
        let ctx_guard = match self.pool.get(self.remote_addr, &mut req) {
            Some(guard) => guard,
            None => {
                return Box::pin(err(Error::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Resources busy: context pool is empty",
                    false,
                )));
            }
        };
        let middleware = self.middleware.clone();
        let serve_future = SendFuture(Box::pin(async move {
            Self::serve(middleware, unsafe { ctx_guard.get() }).await?;
            Ok(unsafe { ctx_guard.get() }.resp_mut().load_resp())
        }));
        Box::pin(serve_future)
    }
}

impl<S: State> HttpService<S> {
    pub fn new(
        middleware: Arc<dyn Middleware<S>>,
        remote_addr: SocketAddr,
        pool: Arc<ContextPool<S>>,
    ) -> Self {
        Self {
            middleware,
            remote_addr,
            pool,
        }
    }

    /// Process a new request.
    /// The entry point of middlewares.
    pub async fn serve(
        middleware: Arc<dyn Middleware<S>>,
        mut ctx: Context<S>,
    ) -> Result {
        if let Err(err) = middleware.end(unsafe { ctx.unsafe_clone() }).await {
            ctx.resp_mut().status = err.status_code;
            if err.expose && !err.need_throw() {
                ctx.resp_mut().write(err.message);
            } else if err.expose && err.need_throw() {
                ctx.resp_mut().write(err.message.clone());
                return Err(err);
            } else if err.need_throw() {
                return Err(err);
            }
        }
        Ok(())
    }
}

#[cfg(all(test, feature = "runtime"))]
mod tests {
    use crate::App;
    use http::{Request, StatusCode};
    use hyper::service::Service;
    use hyper::Body;
    use std::time::Instant;

    #[async_std::test]
    async fn gate_simple() -> Result<(), Box<dyn std::error::Error>> {
        let mut service = App::new(())
            .gate_fn(|_ctx, next| async move {
                let inbound = Instant::now();
                next.await?;
                println!("time elapsed: {} ms", inbound.elapsed().as_millis());
                Ok(())
            })
            .http_service();
        let resp = service.call(Request::new(Body::empty())).await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }
}
