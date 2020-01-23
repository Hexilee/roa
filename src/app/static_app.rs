use super::Application;
use crate::{Context, Middleware, Model, _next};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Error, Request, Response, Server};
use std::convert::Infallible;
use std::future::Future;
use std::net::SocketAddr;

pub struct StaticApp<M: Model = ()> {
    handler: Box<dyn Middleware<M>>,
}

impl<M: Model + Sync + Send + 'static> StaticApp<M> {
    pub fn new() -> Self {
        Self {
            handler: Box::new(|ctx, next| next(ctx)),
        }
    }

    pub fn leak(self) -> &'static Self {
        Box::leak(Box::new(self))
    }

    //    pub fn register(self, middleware: impl Middleware<M>) -> Self {
    //        Self {
    //            handler: Box::new(move |ctx, next| {
    //                let current = &|ctx| middleware(ctx, next);
    //                (self.handler)(ctx, current)
    //            }),
    //        }
    //    }

    async fn handle(&self, req: Request<Body>) -> Result<Response<Body>, Infallible> {
        let mut context = Context::new(req, self);
        (self.handler)(&mut context, &_next).await?;
        Ok(Response::new(Body::empty()))
    }

    pub fn listen_static(
        &'static self,
        addr: &SocketAddr,
    ) -> impl 'static + Future<Output = Result<(), Error>> {
        let make_svc = make_service_fn(move |_conn| {
            async move { Ok::<_, Infallible>(service_fn(move |req| self.handle(req))) }
        });
        Server::bind(addr).serve(make_svc)
    }
}

impl<M: Model> Application for StaticApp<M> {}

#[cfg(test)]
mod tests {
    use super::StaticApp;
    #[test]
    fn test() {
        let x = 1;
        let a: Box<dyn Fn() -> i32> = Box::new(|| x);
    }
}
