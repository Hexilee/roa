use crate::{Context, DynMiddleware, MiddlewareStatus, Model, Next, Request, Response, _next};
use hyper::service::{make_service_fn, service_fn};
use hyper::{self, Body, Error, Server};
use std::convert::Infallible;
use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;

pub struct StaticApp<M: Model = ()> {
    handler: Box<dyn DynMiddleware<M>>,
}

impl<M: Model> StaticApp<M> {
    pub fn new() -> Self {
        Self {
            handler: Box::new(|ctx, next| next(ctx)),
        }
    }

    pub fn gate(
        self,
        middleware: impl 'static
            + Sync
            + Send
            + for<'a> Fn(&'a mut Context<M>, Next<M>) -> MiddlewareStatus<'a>,
    ) -> Self {
        let middleware = Arc::new(middleware);
        Self {
            handler: Box::new(move |ctx, next| {
                let middleware_ref = middleware.clone();
                let current: Next<M> = Box::new(move |ctx| middleware_ref.gate(ctx, next));
                (self.handler)(ctx, current)
            }),
        }
    }

    pub async fn serve(&'static self, req: hyper::Request<Body>) -> Result<Response, Infallible> {
        let mut context = Context::new(Request::new(req), self);
        self.handler.gate(&mut context, Box::new(_next)).await?;
        Ok(Response::new())
    }

    pub fn leak(self) -> &'static Self {
        Box::leak(Box::new(self))
    }

    pub fn listen(
        &'static self,
        addr: &SocketAddr,
    ) -> impl 'static + Future<Output = Result<(), Error>> {
        let make_svc = make_service_fn(move |_conn| {
            async move {
                Ok::<_, Infallible>(service_fn(move |req| {
                    async move { Ok::<_, Infallible>(self.serve(req).await?.into_resp()) }
                }))
            }
        });
        Server::bind(addr).serve(make_svc)
    }
}

#[cfg(test)]
mod tests {
    use super::StaticApp;
    use futures::lock::Mutex;
    use hyper::{Body, Request};
    use std::convert::Infallible;
    use std::sync::Arc;
    use std::time::Instant;

    #[tokio::test]
    async fn test_gate_simple() -> Result<(), Infallible> {
        let _resp = StaticApp::<()>::new()
            .gate(|ctx, next| {
                Box::pin(async move {
                    let inbound = Instant::now();
                    next(ctx).await?;
                    println!("time elapsed: {} ms", inbound.elapsed().as_millis());
                    Ok(())
                })
            })
            .leak()
            .serve(Request::new(Body::empty()))
            .await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_middleware_order() -> Result<(), Infallible> {
        let vector = Arc::new(Mutex::new(Vec::new()));
        let mut app = StaticApp::<()>::new();
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
        let _resp = app.leak().serve(Request::new(Body::empty())).await?;
        for i in 0..100 {
            assert_eq!(i, vector.lock().await[i]);
            assert_eq!(i, vector.lock().await[199 - i]);
        }
        Ok(())
    }
}
