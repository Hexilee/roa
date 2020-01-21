use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Error, Request, Response, Server};
use std::convert::Infallible;
use std::future::Future;
use std::marker::PhantomData;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;

pub type MiddlewareStatus<'a> =
    Pin<Box<dyn 'a + Future<Output = Result<(), Infallible>> + Sync + Send>>;

pub type Next<'a, D> =
    Box<dyn 'a + for<'b> FnOnce(&'b mut Context<D>) -> MiddlewareStatus<'b> + Sync + Send>;

pub trait Middleware<D>:
    Sync + Send + for<'a> Fn(&'a mut Context<D>, Next<'a, D>) -> MiddlewareStatus<'a>
{
}
impl<D, T> Middleware<D> for T where
    T: Sync + Send + for<'a> Fn(&'a mut Context<D>, Next<'a, D>) -> MiddlewareStatus<'a>
{
}

pub struct Roa<D: Default = ()> {
    _data: PhantomData<D>,
    middlewares: Vec<Box<dyn Middleware<D>>>,
}

pub struct Context<D: Default = ()> {
    request: Request<Body>,
    data: D,
}

impl<D: Default + Sync + Send + 'static> Roa<D> {
    pub fn new() -> Self {
        Self {
            _data: PhantomData::<D>,
            middlewares: Vec::new(),
        }
    }

    async fn handle(self: Arc<Self>, req: Request<Body>) -> Result<Response<Body>, Infallible> {
        let mut context = Context::new(req);
        let mut handler: Next<D> = Box::new(|_ctx: &mut Context<D>| Box::pin(async { Ok(()) }));
        for middleware in self.middlewares.iter().rev() {
            handler = Box::new(move |ctx| Box::pin(async move { middleware(ctx, handler).await }));
        }
        handler(&mut context).await?;
        Ok(Response::new(Body::empty()))
    }

    pub async fn listen(&self, addr: &SocketAddr) -> Result<(), Error> {
        let origin = Arc::new(self.clone());
        let make_svc = make_service_fn(move |_conn| {
            let app = origin.clone();
            let handler = move |req| {
                let copied = app.clone();
                async move { copied.handle(req).await }
            };
            async move { Ok::<_, Infallible>(service_fn(handler)) }
        });
        Server::bind(addr).serve(make_svc).await
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
