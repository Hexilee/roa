use crate::{
    default_status_handler, first_middleware, last, Context, DynHandler, DynMiddleware,
    DynStatusHandler, Middleware, Model, Next, Request, Response, Status, StatusHandler,
    TargetHandler,
};
use futures::task::Poll;
use http::{Request as HttpRequest, Response as HttpResponse};
use hyper::server::conn::{AddrIncoming, AddrStream};
use hyper::service::Service;
use hyper::Body as HyperBody;
use hyper::Server;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;

pub struct Builder<M: Model = ()> {
    middleware: Arc<DynMiddleware<M::State>>,
    status_handler: Box<DynStatusHandler<M::State>>,
}

pub struct App<M: Model> {
    handler: Arc<DynHandler<M::State>>,
    status_handler: Arc<DynStatusHandler<M::State>>,
    pub(crate) model: Arc<M>,
}

pub struct HttpService<M: Model> {
    app: App<M>,
    addr: SocketAddr,
}

impl<M: Model> Builder<M> {
    pub fn new() -> Self {
        Self {
            middleware: Arc::from(Box::new(first_middleware).dynamic()),
            status_handler: Box::new(default_status_handler).dynamic(),
        }
    }

    pub fn handle_fn<F>(
        self,
        middleware: impl 'static + Sync + Send + Fn(Context<M::State>, Next) -> F,
    ) -> Self
    where
        F: 'static + Future<Output = Result<(), Status>> + Send,
    {
        self.handle(middleware)
    }

    pub fn handle<F>(self, middleware: impl Middleware<M::State, StatusFuture = F>) -> Self
    where
        F: 'static + Future<Output = Result<(), Status>> + Send,
    {
        let current_middleware = self.middleware.clone();
        let next_middleware: Arc<DynMiddleware<M::State>> =
            Arc::from(Box::new(middleware).dynamic());
        Self {
            middleware: Arc::from(move |ctx: Context<M::State>, next| {
                let next_ref = next_middleware.clone();
                let ctx_cloned = ctx.clone();
                let current = Box::new(move || next_ref(ctx_cloned, next));
                current_middleware(ctx, current)
            }),
            ..self
        }
    }

    pub fn handle_status<F>(self, handler: impl StatusHandler<M::State, StatusFuture = F>) -> Self
    where
        F: 'static + Future<Output = Result<(), Status>> + Send,
    {
        Self {
            status_handler: Box::new(handler).dynamic(),
            ..self
        }
    }

    pub fn handle_status_fn<F>(
        self,
        handler: impl 'static + Sync + Send + Fn(Context<M::State>, Status) -> F,
    ) -> Self
    where
        F: 'static + Future<Output = Result<(), Status>> + Send,
    {
        self.handle_status(handler)
    }

    pub fn model(self, model: M) -> App<M> {
        let Builder {
            middleware,
            status_handler,
        } = self;
        App::new(
            Arc::new(move |ctx| middleware(ctx, Box::new(last))),
            Arc::from(status_handler),
            Arc::new(model),
        )
    }
}

impl<M: Model> Default for Builder<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: Model> App<M> {
    pub fn builder() -> Builder<M> {
        Builder::default()
    }

    pub fn new(
        handler: Arc<DynHandler<M::State>>,
        status_handler: Arc<DynStatusHandler<M::State>>,
        model: Arc<M>,
    ) -> Self {
        Self {
            handler,
            status_handler,
            model,
        }
    }

    pub fn listen(&self, addr: SocketAddr) -> hyper::Server<AddrIncoming, App<M>> {
        log::info!("Server is listening on: http://{}", &addr);
        Server::bind(&addr).serve(self.clone())
    }
}

macro_rules! impl_poll_ready {
    () => {
        fn poll_ready(&mut self, _cx: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
    };
}

type AppFuture<M> =
    Pin<Box<dyn 'static + Future<Output = Result<HttpService<M>, std::io::Error>> + Send>>;

impl<M: Model> Service<&AddrStream> for App<M> {
    type Response = HttpService<M>;
    type Error = std::io::Error;
    type Future = AppFuture<M>;
    impl_poll_ready!();
    fn call(&mut self, stream: &AddrStream) -> Self::Future {
        let addr = stream.remote_addr();
        let app = self.clone();
        Box::pin(async move { Ok(HttpService::new(app, addr)) })
    }
}

type HttpFuture =
    Pin<Box<dyn 'static + Future<Output = Result<HttpResponse<HyperBody>, Status>> + Send>>;

impl<M: Model> Service<HttpRequest<HyperBody>> for HttpService<M> {
    type Response = HttpResponse<HyperBody>;
    type Error = Status;
    type Future = HttpFuture;
    impl_poll_ready!();
    fn call(&mut self, req: HttpRequest<HyperBody>) -> Self::Future {
        let service = self.clone();
        Box::pin(async move { Ok(service.serve(req.into()).await?.into()) })
    }
}

impl<M: Model> HttpService<M> {
    pub fn new(app: App<M>, addr: SocketAddr) -> Self {
        Self { app, addr }
    }

    pub async fn serve(&self, req: Request) -> Result<Response, Status> {
        let mut context = Context::new(req, self.app.clone(), self.addr);
        let app = self.app.clone();
        if let Err(status) = (app.handler)(context.clone()).await {
            (app.status_handler)(context.clone(), status).await?;
        }
        Ok(std::mem::take(&mut context.response))
    }
}

impl<M: Model> Clone for App<M> {
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
            status_handler: self.status_handler.clone(),
            model: self.model.clone(),
        }
    }
}

impl<M: Model> Clone for HttpService<M> {
    fn clone(&self) -> Self {
        Self {
            app: self.app.clone(),
            addr: self.addr,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{App, HttpService};
    use crate::Request;
    use std::time::Instant;

    #[tokio::test]
    async fn gate_simple() -> Result<(), Box<dyn std::error::Error>> {
        let app = App::builder()
            .handle_fn(|_ctx, next| {
                async move {
                    let inbound = Instant::now();
                    next().await?;
                    println!("time elapsed: {} ms", inbound.elapsed().as_millis());
                    Ok(())
                }
            })
            .model(());
        let _resp = HttpService::new(app, "127.0.0.1:8080".parse()?)
            .serve(Request::new())
            .await?;
        Ok(())
    }
}
