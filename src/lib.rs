use futures::future;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Error, Request, Response, Server};
use std::convert::Infallible;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;

pub type MiddlewareStatus<'a> =
    Pin<Box<dyn 'a + Future<Output = Result<(), Infallible>> + Sync + Send>>;

pub type Next<D> = dyn for<'b> Fn(&'b mut Context<D>) -> MiddlewareStatus<'b> + Sync + Send;

pub trait Middleware<D: Default>:
    Sync + Send + for<'a> Fn(&'a mut Context<D>, &'a Next<D>) -> MiddlewareStatus<'a>
{
    fn gate<'a>(&'a self, ctx: &'a mut Context<D>, next: &'a Next<D>) -> MiddlewareStatus<'a>;
}
impl<D: Default, T> Middleware<D> for T
where
    T: Sync + Send + for<'a> Fn(&'a mut Context<D>, &'a Next<D>) -> MiddlewareStatus<'a>,
{
    fn gate<'a>(&'a self, ctx: &'a mut Context<D>, next: &'a Next<D>) -> MiddlewareStatus<'a> {
        self(ctx, next)
    }
}

pub struct Roa<D: Default = ()> {
    handler: Box<dyn Middleware<D>>,
}

fn _next<D: Default>(_ctx: &mut Context<D>) -> MiddlewareStatus {
    Box::pin(async { Ok(()) })
}

pub struct Context<D: Default = ()> {
    request: Request<Body>,
    data: D,
}

impl<D: Default + Sync + Send + 'static> Roa<D> {
    pub fn new() -> Self {
        Self {
            handler: Box::new(|ctx, next| Box::pin(async { next(ctx).await })),
        }
    }

    pub fn register(&mut self, middleware: impl Middleware<D>) {
        unimplemented!()
    }

    async fn handle(&self, req: Request<Body>) -> Result<Response<Body>, Infallible> {
        let mut context = Context::new(req);
        (self.handler)(&mut context, &_next).await?;
        Ok(Response::new(Body::empty()))
    }

    pub fn listen<'a>(
        &'a self,
        addr: &'a SocketAddr,
    ) -> impl 'a + Future<Output = Result<(), Error>> {
        let make_svc = make_service_fn(|_conn| {
            future::ok::<_, Infallible>(service_fn(|req| self.handle(req)))
        });
        Server::bind(addr).serve(make_svc)
    }
}

impl<D: Default> Context<D> {
    pub fn new(request: Request<Body>) -> Self {
        Self {
            request,
            data: Default::default(),
        }
    }
}

impl<D: Default> Clone for Roa<D> {
    fn clone(&self) -> Self {
        unimplemented!()
    }
}
