use futures::future;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Error, Request, Response, Server};
use std::convert::Infallible;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;

pub trait Model: Sync + Send + Sized {
    fn init(app: &Roa<Self>) -> Self;
}

impl Model for () {
    fn init(app: &Roa<Self>) -> Self {
        ()
    }
}

pub type MiddlewareStatus<'a> =
    Pin<Box<dyn 'a + Future<Output = Result<(), Infallible>> + Sync + Send>>;

pub type Next<'a, M> = &'a (dyn Fn(&'a mut Context<'a, M>) -> MiddlewareStatus<'a> + Sync + Send);

pub trait Middleware<M: Model>:
    Sync + Send + for<'a> Fn(&'a mut Context<'a, M>, Next<'a, M>) -> MiddlewareStatus<'a>
{
    fn gate<'a>(&'a self, ctx: &'a mut Context<'a, M>, next: Next<'a, M>) -> MiddlewareStatus<'a>;
}
impl<M: Model, T> Middleware<M> for T
where
    T: Sync + Send + for<'a> Fn(&'a mut Context<'a, M>, Next<'a, M>) -> MiddlewareStatus<'a>,
{
    fn gate<'a>(&'a self, ctx: &'a mut Context<'a, M>, next: Next<'a, M>) -> MiddlewareStatus<'a> {
        self(ctx, next)
    }
}

pub struct Roa<M: Model = ()> {
    handler: Box<dyn Middleware<M>>,
}

fn _next<'a, M: Model>(_ctx: &'a mut Context<'a, M>) -> MiddlewareStatus<'a> {
    Box::pin(async { Ok(()) })
}

pub struct Context<'a, M: Model = ()> {
    request: Request<Body>,
    app: &'a Roa<M>,
    model: M,
}

impl<M: Model + Sync + Send + 'static> Roa<M> {
    //    pub fn new() -> Self {
    //        Self {
    //            handler: Box::new(|ctx, next| next(ctx)),
    //        }
    //    }
    //
    //    pub fn register(self, middleware: impl Middleware<M> + 'static) -> Self {
    //        Self {
    //            handler: Box::new(move |ctx, next| {
    //                let current: Next<M> = &|ctx| middleware(ctx, next);
    //                (self.handler)(ctx, current)
    //            }),
    //        }
    //    }

    async fn handle(&self, req: Request<Body>) -> Result<Response<Body>, Infallible> {
        let mut context = Context::new(req, self);
        (self.handler)(&mut context, &_next).await?;
        Ok(Response::new(Body::empty()))
    }

    pub fn listen(
        &'static self,
        addr: &SocketAddr,
    ) -> impl 'static + Future<Output = Result<(), Error>> {
        let make_svc = make_service_fn(move |_conn| {
            future::ok::<_, Infallible>(service_fn(move |req| self.handle(req)))
            //            future::ok::<_, Infallible>(service_fn(|req| {
            //                future::ok::<_, Infallible>(Response::new(Body::empty()))
            //            }))
        });
        Server::bind(addr).serve(make_svc)
    }
}

impl<'a, M: Model> Context<'a, M> {
    pub fn new(request: Request<Body>, app: &'a Roa<M>) -> Self {
        Self {
            request,
            app,
            model: Model::init(app),
        }
    }
}
