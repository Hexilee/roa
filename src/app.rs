use crate::{Context, DynMiddleware, Model, Next, Request, Response, Status, StatusFuture, _next};

use async_std::net::{TcpListener, ToSocketAddrs};
use async_std::task;
use http_service::HttpService;
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub struct Server<M: Model = ()> {
    middleware: Box<dyn DynMiddleware<M>>,
}

pub struct Service<M: Model = ()> {
    handler: Arc<dyn Fn(&mut Context<M>) -> StatusFuture + Sync + Send>,
}

impl<M: Model> Server<M> {
    pub fn new() -> Self {
        Self {
            middleware: Box::new(|ctx, next| next(ctx)),
        }
    }

    pub fn gate(
        self,
        middleware: impl 'static
            + Sync
            + Send
            + for<'a> Fn(&'a mut Context<M>, Next<M>) -> StatusFuture<'a>,
    ) -> Self {
        let middleware = Arc::new(middleware);
        Self {
            middleware: Box::new(move |ctx, next| {
                let middleware_ref = middleware.clone();
                let current: Next<M> = Box::new(move |ctx| middleware_ref.gate(ctx, next));
                (self.middleware)(ctx, current)
            }),
        }
    }

    pub fn into_service(self) -> Service<M> {
        Service::new(self)
    }
}

impl<M: Model> Service<M> {
    pub fn new(app: Server<M>) -> Self {
        let Server { middleware } = app;
        Self {
            handler: Arc::new(move |ctx| middleware.gate(ctx, Box::new(_next))),
        }
    }

    pub async fn serve(&self, req: http_service::Request) -> Result<Response, Status> {
        let mut context = Context::new(Request::new(req), self.clone());
        let app = self.clone();
        if let Err(err) = (app.handler)(&mut context).await {
            // TODO: change status code by error
            if err.need_throw() {
                return Err(err);
            }
        }
        Ok(context.response)
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

impl<M: Model> HttpService for Service<M> {
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

impl<M: Model> Clone for Service<M> {
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
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
            .gate(|ctx, next| {
                Box::pin(async move {
                    let inbound = Instant::now();
                    next(ctx).await?;
                    println!("time elapsed: {} ms", inbound.elapsed().as_millis());
                    Ok(())
                })
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
            app = app.gate(move |ctx, next| {
                let vec = vec.clone();
                Box::pin(async move {
                    vec.lock().await.push(i);
                    next(ctx).await?;
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
