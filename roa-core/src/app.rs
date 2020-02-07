mod executor;
mod tcp;

use crate::{
    default_error_handler, last, Context, DynTargetHandler, Error, Group, Model, Next, Request,
    Response, Result, TargetHandler,
};
use executor::Executor;
use http::{Request as HttpRequest, Response as HttpResponse};
use hyper::service::Service;
use hyper::Body as HyperBody;
use hyper::Server as HyperServer;
use std::future::Future;
use std::net::{SocketAddr, ToSocketAddrs};
use std::pin::Pin;
use std::result::Result as StdResult;
use std::sync::Arc;
use std::task::Poll;
pub use tcp::{AddrIncoming, AddrStream};

/// The Application of roa.
/// ### Example
/// ```rust
/// use roa_core::App;
/// use log::info;
/// use async_std::fs::File;
///
/// #[async_std::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let server = App::new(())
///         .gate(|ctx, next| async move {
///             info!("{} {}", ctx.method().await, ctx.uri().await);
///             next().await
///         })
///         .gate(|ctx, _next| async move {
///             ctx.resp_mut().await.write(File::open("assets/welcome.html").await?);
///             Ok(())
///         })
///         .listen("127.0.0.1:8000", |addr| {
///             info!("Server is listening on {}", addr)
///         })?;
///     // server.await;
///     Ok(())
/// }
/// ```
///
/// ### Model
/// The `Model` and its `State` is designed to share data or handler between middlewares.
/// The only one type implemented `Model` by this crate is `()`, you should implement your custom Model if neccassary.
///
/// ### Example
/// ```rust
/// use roa_core::{App, Model};
/// use log::info;
/// use futures::lock::Mutex;
/// use std::sync::Arc;
/// use std::collections::HashMap;
///
/// struct AppModel {
///     default_id: u64,
///     database: Arc<Mutex<HashMap<u64, String>>>,
/// }
///
/// struct AppState {
///     id: u64,
///     database: Arc<Mutex<HashMap<u64, String>>>,
/// }
///
/// impl AppModel {
///     fn new() -> Self {
///         Self {
///             default_id: 0,
///             database: Arc::new(Mutex::new(HashMap::new()))
///         }
///     }
/// }
///
/// impl Model for AppModel {
///     type State = AppState;
///     fn new_state(&self) -> Self::State {
///         AppState {
///             id: self.default_id,
///             database: self.database.clone(),
///         }
///     }
/// }
///
/// #[async_std::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let server = App::new(AppModel::new())
///         .gate(|ctx, next| async move {
///             ctx.state_mut().await.id = 1;
///             next().await
///         })
///         .gate(|ctx, _next| async move {
///             let id = ctx.state().await.id;
///             ctx.state().await.database.lock().await.get(&id);
///             Ok(())
///         })
///         .listen("127.0.0.1:8000", |addr| {
///             info!("Server is listening on {}", addr)
///         })?;
///     // server.await;
///     Ok(())
/// }
/// ```
///
/// ### Graceful Shutdown
///
/// `App::listen` returns a hyper::Server, which supports graceful shutdown.
///
/// ```rust
/// use roa_core::App;
/// use log::info;
/// use futures::channel::oneshot;
///
/// #[async_std::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Prepare some signal for when the server should start shutting down...
///     let (tx, rx) = oneshot::channel::<()>();
///     let server = App::new(())
///         .listen("127.0.0.1:8000", |addr| {
///             info!("Server is listening on {}", addr)
///         })?
///         .with_graceful_shutdown(async {
///             rx.await.ok();
///         });
///     // Await the `server` receiving the signal...
///     // server.await;
///     
///     // And later, trigger the signal by calling `tx.send(())`.
///     let _ = tx.send(());
///     Ok(())
/// }
/// ```
pub struct App<M: Model> {
    middleware: Group<M>,
    error_handler: Arc<DynTargetHandler<M, Error>>,
    pub(crate) model: Arc<M>,
}

/// An implementation of hyper HttpService.
pub struct HttpService<M: Model> {
    app: App<M>,
    stream: AddrStream,
}

/// Alias of `HyperServer<AddrIncoming, App<M>, Executor>`.
pub type Server<M> = HyperServer<AddrIncoming, App<M>, Executor>;

impl<M: Model> App<M> {
    /// Construct an Application from a Model.
    pub fn new(model: M) -> Self {
        Self {
            middleware: Group::new(),
            error_handler: Arc::from(Box::new(default_error_handler).dynamic()),
            model: Arc::new(model),
        }
    }

    /// Use a middleware.
    pub fn gate<F>(
        &mut self,
        middleware: impl 'static + Sync + Send + Fn(Context<M>, Next) -> F,
    ) -> &mut Self
    where
        F: 'static + Future<Output = Result> + Send,
    {
        self.middleware.join(middleware);
        self
    }

    /// Set a custom error handler. error_handler will be called when middlewares return an `Err(Error)`.
    /// The error thrown by error_handler will be thrown to hyper.
    /// ```rust
    /// use roa_core::{Model, Context, Error, ErrorKind, Result};
    ///
    /// pub async fn default_error_handler<M: Model>(
    ///     context: Context<M>,
    ///     err: Error,
    /// ) -> Result {
    ///     context.resp_mut().await.status = err.status_code;
    ///     if err.expose {
    ///         context.resp_mut().await.write_str(&err.message);
    ///     }
    ///     if err.kind == ErrorKind::ServerError {
    ///         Err(err)
    ///     } else {
    ///         Ok(())
    ///     }
    /// }
    /// ```
    pub fn handle_err<F>(
        &mut self,
        handler: impl 'static + Sync + Send + Fn(Context<M>, Error) -> F,
    ) -> &mut Self
    where
        F: 'static + Future<Output = Result> + Send,
    {
        self.error_handler = Arc::from(Box::new(handler).dynamic());
        self
    }

    /// Listen on a socket addr, return a server and the real addr it binds.
    fn listen_on(&self, addr: impl ToSocketAddrs) -> std::io::Result<(SocketAddr, Server<M>)> {
        let incoming = AddrIncoming::bind(addr)?;
        let local_addr = incoming.local_addr();
        let server = HyperServer::builder(incoming)
            .executor(Executor {})
            .serve(self.clone());
        Ok((local_addr, server))
    }

    /// Listen on a socket addr, return a server, and pass real addr to the callback.
    pub fn listen(
        &self,
        addr: impl ToSocketAddrs,
        callback: impl Fn(SocketAddr),
    ) -> std::io::Result<Server<M>> {
        let (addr, server) = self.listen_on(addr)?;
        callback(addr);
        Ok(server)
    }

    /// Listen on an unused port of 0.0.0.0, return a server and the real addr it binds.
    pub fn run(&self) -> std::io::Result<(SocketAddr, Server<M>)> {
        self.listen_on("0.0.0.0:0")
    }

    /// Listen on an unused port of 127.0.0.1, return a server and the real addr it binds.
    /// ### Example
    /// ```rust
    /// use roa_core::App;
    /// use async_std::task::spawn;
    /// use http::StatusCode;
    /// use std::time::Instant;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let (addr, server) = App::new(())
    ///         .gate(|_ctx, next| async move {
    ///             let inbound = Instant::now();
    ///             next().await?;
    ///             println!("time elapsed: {} ms", inbound.elapsed().as_millis());
    ///             Ok(())
    ///         })
    ///         .run_local()?;
    ///     spawn(server);
    ///     let resp = reqwest::get(&format!("http://{}", addr)).await?;
    ///     assert_eq!(StatusCode::OK, resp.status());
    ///     Ok(())
    /// }
    /// ```
    pub fn run_local(&self) -> std::io::Result<(SocketAddr, Server<M>)> {
        self.listen_on("127.0.0.1:0")
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

type AppFuture<M> = Pin<Box<dyn 'static + Future<Output = std::io::Result<HttpService<M>>> + Send>>;

impl<M: Model> Service<&AddrStream> for App<M> {
    type Response = HttpService<M>;
    type Error = std::io::Error;
    type Future = AppFuture<M>;
    impl_poll_ready!();

    #[inline]
    fn call(&mut self, stream: &AddrStream) -> Self::Future {
        let app = self.clone();
        let stream = stream.clone();
        Box::pin(async move { Ok(HttpService::new(app, stream)) })
    }
}

type HttpFuture = Pin<Box<dyn 'static + Future<Output = Result<HttpResponse<HyperBody>>> + Send>>;

impl<M: Model> Service<HttpRequest<HyperBody>> for HttpService<M> {
    type Response = HttpResponse<HyperBody>;
    type Error = Error;
    type Future = HttpFuture;
    impl_poll_ready!();

    #[inline]
    fn call(&mut self, req: HttpRequest<HyperBody>) -> Self::Future {
        let service = self.clone();
        Box::pin(async move { Ok(service.serve(req.into()).await?.into()) })
    }
}

impl<M: Model> HttpService<M> {
    pub fn new(app: App<M>, stream: AddrStream) -> Self {
        Self { app, stream }
    }

    pub async fn serve(&self, req: Request) -> Result<Response> {
        let context = Context::new(req, self.app.clone(), self.stream.clone());
        let app = self.app.clone();
        if let Err(status) = (app.middleware.handler())(context.clone(), Box::new(last)).await {
            (app.error_handler)(context.clone(), status).await?;
        }
        let mut response = context.resp_mut().await;
        Ok(std::mem::take(&mut *response))
    }
}

impl<M: Model> Clone for App<M> {
    fn clone(&self) -> Self {
        Self {
            middleware: self.middleware.clone(),
            error_handler: self.error_handler.clone(),
            model: self.model.clone(),
        }
    }
}

impl<M: Model> Clone for HttpService<M> {
    fn clone(&self) -> Self {
        Self {
            app: self.app.clone(),
            stream: self.stream.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::App;
    use async_std::task::spawn;
    use http::StatusCode;
    use std::time::Instant;

    #[tokio::test]
    async fn gate_simple() -> Result<(), Box<dyn std::error::Error>> {
        let (addr, server) = App::new(())
            .gate(|_ctx, next| async move {
                let inbound = Instant::now();
                next().await?;
                println!("time elapsed: {} ms", inbound.elapsed().as_millis());
                Ok(())
            })
            .run_local()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}", addr)).await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }
}
