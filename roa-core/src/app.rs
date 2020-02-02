use crate::{
    default_status_handler, last, Context, DynTargetHandler, Middleware, Model, Next, Request,
    Response, Status, TargetHandler,
};
use http::{Request as HttpRequest, Response as HttpResponse};
use hyper::server::conn::{AddrIncoming, AddrStream};
use hyper::service::Service;
use hyper::Body as HyperBody;
use hyper::Server;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Poll;

pub struct App<M: Model> {
    middleware: Middleware<M>,
    status_handler: Arc<DynTargetHandler<M, Status>>,
    pub(crate) model: Arc<M>,
}

pub struct HttpService<M: Model> {
    app: App<M>,
    addr: SocketAddr,
}

impl<M: Model> App<M> {
    pub fn new(model: M) -> Self {
        Self {
            middleware: Middleware::new(),
            status_handler: Arc::from(Box::new(default_status_handler).dynamic()),
            model: Arc::new(model),
        }
    }

    pub fn join<F>(
        &mut self,
        middleware: impl 'static + Sync + Send + Fn(Context<M>, Next) -> F,
    ) -> &mut Self
    where
        F: 'static + Future<Output = Result<(), Status>> + Send,
    {
        self.middleware.join(middleware);
        self
    }

    pub fn handle_status<F>(
        &mut self,
        handler: impl 'static + Sync + Send + Fn(Context<M>, Status) -> F,
    ) -> &mut Self
    where
        F: 'static + Future<Output = Result<(), Status>> + Send,
    {
        self.status_handler = Arc::from(Box::new(handler).dynamic());
        self
    }

    pub async fn serve(&self, req: Request, peer_addr: SocketAddr) -> Result<Response, Status> {
        let context = Context::new(req, self.clone(), peer_addr);
        let app = self.clone();
        if let Err(status) = (app.middleware.handler())(context.clone(), Box::new(last)).await {
            (app.status_handler)(context.clone(), status).await?;
        }
        let mut response = context.resp_mut().await;
        Ok(std::mem::take(&mut *response))
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
        Box::pin(async move { Ok(service.app.serve(req.into(), service.addr).await?.into()) })
    }
}

impl<M: Model> HttpService<M> {
    pub fn new(app: App<M>, addr: SocketAddr) -> Self {
        Self { app, addr }
    }
}

impl<M: Model> Clone for App<M> {
    fn clone(&self) -> Self {
        Self {
            middleware: self.middleware.clone(),
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
    use crate::{App, Request};
    use std::time::Instant;

    #[tokio::test]
    async fn gate_simple() -> Result<(), Box<dyn std::error::Error>> {
        App::new(())
            .join(|_ctx, next| async move {
                let inbound = Instant::now();
                next().await?;
                println!("time elapsed: {} ms", inbound.elapsed().as_millis());
                Ok(())
            })
            .serve(Request::new(), "127.0.0.1:8080".parse()?)
            .await?;
        Ok(())
    }
}
