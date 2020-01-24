use crate::{Context, DynMiddleware, MiddlewareStatus, Model, Next, _next};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Error, Request, Response, Server};
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

    // TODO: replace DynMiddleware with Middleware
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
                let current: Next<M> = Box::new(move |ctx| middleware_ref(ctx, next));
                (self.handler)(ctx, current)
            }),
        }
    }

    //    fn handle<'a, F>(
    //        self,
    //        handler: impl 'static + Sync + Send + Fn(&'a mut Context<M>) -> F + 'a,
    //    ) -> Self
    //    where
    //        F: Future<Output = Result<(), Infallible>> + Sync + Send,
    //    {
    //        let handler = Box::new(move |ctx| Box::pin(handler(ctx)));
    //        Self {
    //            handler: Box::new(move |ctx, _next| handler(ctx)),
    //        }
    //    }

    pub async fn serve(&'static self, req: Request<Body>) -> Result<Response<Body>, Infallible> {
        let mut context = Context::new(req, self);
        (self.handler)(&mut context, Box::new(_next)).await?;
        Ok(Response::new(Body::empty()))
    }

    pub fn leak(self) -> &'static Self {
        Box::leak(Box::new(self))
    }

    pub fn listen(
        &'static self,
        addr: &SocketAddr,
    ) -> impl 'static + Future<Output = Result<(), Error>> {
        let make_svc = make_service_fn(move |_conn| {
            async move { Ok::<_, Infallible>(service_fn(move |req| self.serve(req))) }
        });
        Server::bind(addr).serve(make_svc)
    }
}

#[cfg(test)]
mod tests {
    use super::StaticApp;
    use crate::Next;
    use hyper::{Body, Request};
    use std::convert::Infallible;
    use std::time::Instant;
    use tokio::prelude::*;

    #[tokio::test]
    async fn test_app_simple() -> Result<(), Infallible> {
        let resp = StaticApp::<()>::new()
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
}
