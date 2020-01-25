use crate::{
    Context, DynMiddleware, Next, Request, Response, State, Status, StatusFuture, _next,
    default_status_handler, make_dyn, make_dyn_middleware, make_status_handler, Middleware,
    StatusHandler,
};

use async_std::net::{TcpListener, ToSocketAddrs};
use async_std::task;
use http_service::HttpService;
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub struct Server<S: State = ()> {
    middleware: Arc<dyn DynMiddleware<S>>,
    status_handler: Box<dyn StatusHandler<S>>,
}

pub struct Service<S: State> {
    handler: Arc<dyn Fn(Context<S>) -> StatusFuture + Sync + Send>,
    status_handler: Arc<dyn StatusHandler<S>>,
}

impl<S: State> Server<S> {
    pub fn new() -> Self {
        Self {
            middleware: Arc::from(make_dyn_middleware(|_ctx, next| next())),
            status_handler: make_status_handler(make_dyn(default_status_handler)),
        }
    }

    pub fn handle_fn<F>(self, next: impl 'static + Sync + Send + Fn(Context<S>, Next) -> F) -> Self
    where
        F: 'static + Future<Output = Result<(), Status>> + Send,
    {
        let current_middleware = self.middleware.clone();
        let next_middleware = Arc::new(make_dyn(next));
        Self {
            middleware: Arc::from(make_dyn_middleware(move |ctx, next| {
                let next_ref = next_middleware.clone();
                let ctx_cloned = ctx.clone();
                let current = Box::new(move || next_ref(ctx_cloned, next));
                current_middleware.handle(ctx, current)
            })),
            ..self
        }
    }

    pub fn handle<F>(self, middleware: impl Middleware<S, StatusFuture = F>) -> Self
    where
        F: 'static + Future<Output = Result<(), Status>> + Send,
    {
        self.handle_fn(move |ctx, next| middleware.handle(ctx, next))
    }

    pub fn into_service(self) -> Service<S> {
        Service::new(self)
    }

    pub async fn listen(self, addr: impl ToSocketAddrs) -> Result<(), std::io::Error> {
        self.into_service().listen(addr).await
    }
}

impl<S: State> Service<S> {
    pub fn new(app: Server<S>) -> Self {
        let Server {
            middleware,
            status_handler,
        } = app;
        Self {
            handler: Arc::new(move |ctx| middleware.handle(ctx, Box::new(_next))),
            status_handler: Arc::from(status_handler),
        }
    }

    pub async fn serve(&self, req: http_service::Request) -> Result<Response, Status> {
        let mut context = Context::new(Request::new(req), self.clone());
        let app = self.clone();
        if let Err(status) = (app.handler)(context.clone()).await {
            app.status_handler.handle(context.clone(), status).await?;
        }
        Ok(std::mem::take(&mut context.response))
    }

    pub async fn listen(&self, addr: impl ToSocketAddrs) -> Result<(), std::io::Error> {
        let http_service = self.clone();
        #[derive(Copy, Clone)]
        struct Spawner;

        impl futures::task::Spawn for &Spawner {
            fn spawn_obj(
                &self,
                future: futures::future::FutureObj<'static, ()>,
            ) -> Result<(), futures::task::SpawnError> {
                task::spawn(Box::pin(future));
                Ok(())
            }
        }

        let listener = TcpListener::bind(addr).await?;
        log::info!("Server is listening on: http://{}", listener.local_addr()?);
        let res = http_service_hyper::Server::builder(listener.incoming())
            .with_spawner(Spawner {})
            .serve(http_service)
            .await;

        res.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        Ok(())
    }
}

impl<S: State> HttpService for Service<S> {
    type Connection = ();
    type ConnectionFuture =
        Pin<Box<dyn 'static + Future<Output = Result<(), Infallible>> + Sync + Send>>;
    fn connect(&self) -> Self::ConnectionFuture {
        Box::pin(async { Ok(()) })
    }

    type ResponseFuture =
        Pin<Box<dyn 'static + Future<Output = Result<http_service::Response, Status>> + Send>>;

    fn respond(&self, _conn: &mut (), req: http_service::Request) -> Self::ResponseFuture {
        let service = self.clone();
        Box::pin(async move { Ok(service.serve(req).await?.into_resp()?) })
    }
}

impl<S: State> Clone for Service<S> {
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
            status_handler: self.status_handler.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Server;
    use futures::lock::Mutex;
    use http_service::{Body, Request};
    use std::convert::Infallible;
    use std::sync::Arc;
    use std::time::Instant;

    #[async_std::test]
    async fn gate_simple() -> Result<(), Infallible> {
        let _resp = Server::<()>::new()
            .handle_fn(|_ctx, next| {
                async move {
                    let inbound = Instant::now();
                    next().await?;
                    println!("time elapsed: {} ms", inbound.elapsed().as_millis());
                    Ok(())
                }
            })
            .into_service()
            .serve(Request::new(Body::empty()))
            .await;
        Ok(())
    }

    #[async_std::test]
    async fn middleware_order() -> Result<(), Infallible> {
        let vector = Arc::new(Mutex::new(Vec::new()));
        let mut app = Server::<()>::new();
        for i in 0..100 {
            let vec = vector.clone();
            app = app.handle_fn(move |_ctx, next| {
                let vec = vec.clone();
                Box::pin(async move {
                    vec.lock().await.push(i);
                    next().await?;
                    vec.lock().await.push(i);
                    Ok(())
                })
            });
        }
        let _resp = app.into_service().serve(Request::new(Body::empty())).await;
        for i in 0..100 {
            assert_eq!(i, vector.lock().await[i]);
            assert_eq!(i, vector.lock().await[199 - i]);
        }
        Ok(())
    }
}
