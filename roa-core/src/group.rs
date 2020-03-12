use crate::{async_trait, Context, Endpoint, Middleware, Next, Result};
use futures::Future;
use std::sync::Arc;

pub trait MiddlewareExt<S>: Sized + for<'a> Middleware<'a, S> {
    fn chain<M>(self, next: M) -> Chain<Self, M>
    where
        M: for<'a> Middleware<'a, S>,
    {
        Chain(self, next)
    }

    fn end<E>(self, next: E) -> Chain<Self, E>
    where
        E: for<'a> Endpoint<'a, S>,
    {
        Chain(self, next)
    }

    fn shared(self) -> Shared<S>
    where
        S: 'static,
    {
        Shared(Arc::new(self))
    }

    fn boxed(self) -> Boxed<S>
    where
        S: 'static,
    {
        Boxed(Box::new(self))
    }
}

impl<S, T> MiddlewareExt<S> for T where T: for<'a> Middleware<'a, S> {}

/// A middleware composing and executing other middlewares in a stack-like manner.
pub struct Chain<T, U>(T, U);

pub struct Shared<S>(Arc<dyn for<'a> Middleware<'a, S>>);

pub struct Boxed<S>(Box<dyn for<'a> Middleware<'a, S>>);

#[async_trait(?Send)]
impl<'a, S, T, U> Middleware<'a, S> for Chain<T, U>
where
    U: Middleware<'a, S>,
    T: for<'b> Middleware<'b, S>,
{
    #[inline]
    async fn handle(&'a self, ctx: &'a mut Context<S>, next: Next<'a>) -> Result {
        let ptr = ctx as *mut Context<S>;
        let mut next = self.1.handle(unsafe { &mut *ptr }, next);
        self.0.handle(ctx, &mut next).await
    }
}

#[async_trait(?Send)]
impl<'a, S> Middleware<'a, S> for Shared<S>
where
    S: 'static,
{
    #[inline]
    async fn handle(&'a self, ctx: &'a mut Context<S>, next: Next<'a>) -> Result {
        self.0.handle(ctx, next).await
    }
}

impl<S> Clone for Shared<S> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

#[async_trait(?Send)]
impl<'a, S> Middleware<'a, S> for Boxed<S>
where
    S: 'static,
{
    #[inline]
    async fn handle(&'a self, ctx: &'a mut Context<S>, next: Next<'a>) -> Result {
        self.0.handle(ctx, next).await
    }
}

#[async_trait(?Send)]
impl<'a, S, T, U> Endpoint<'a, S> for Chain<T, U>
where
    U: Endpoint<'a, S>,
    T: for<'b> Middleware<'b, S>,
{
    #[inline]
    async fn end(&'a self, ctx: &'a mut Context<S>) -> Result {
        let ptr = ctx as *mut Context<S>;
        let mut next = self.1.end(unsafe { &mut *ptr });
        self.0.handle(ctx, &mut next).await
    }
}

#[cfg(all(test, feature = "runtime"))]
mod tests {
    use crate::{
        async_trait, App, Context, Error, Middleware, MiddlewareExt, Next, Request,
    };
    use futures::lock::Mutex;
    use http::StatusCode;
    use std::sync::Arc;

    struct Pusher {
        data: usize,
        vector: Arc<Mutex<Vec<usize>>>,
    }

    impl Pusher {
        fn new(data: usize, vector: Arc<Mutex<Vec<usize>>>) -> Self {
            Self { data, vector }
        }
    }

    #[async_trait(?Send)]
    impl<'a> Middleware<'a, ()> for Pusher {
        async fn handle(
            &'a self,
            ctx: &'a mut Context<()>,
            next: Next<'a>,
        ) -> Result<(), Error> {
            self.vector.lock().await.push(self.data);
            next.await?;
            self.vector.lock().await.push(self.data);
            Ok(())
        }
    }

    async fn end(_ctx: &mut Context<()>) -> Result<(), Error> {
        Ok(())
    }

    #[async_std::test]
    async fn middleware_order() -> Result<(), Box<dyn std::error::Error>> {
        let vector = Arc::new(Mutex::new(Vec::new()));
        let mut boxed_middleware = Pusher::new(0, vector.clone()).boxed();
        for i in 1..100 {
            boxed_middleware = boxed_middleware
                .chain(Pusher::new(i, vector.clone()))
                .boxed()
        }
        let service = App::new((), boxed_middleware.end(end)).http_service();
        let resp = service.serve(Request::default()).await?;
        assert_eq!(StatusCode::OK, resp.status);
        for i in 0..100 {
            assert_eq!(i, vector.lock().await[i]);
            assert_eq!(i, vector.lock().await[199 - i]);
        }
        Ok(())
    }
}
