use crate::{async_trait, Context, Endpoint, Middleware, Next, Result};

pub trait MiddlewareExt<S>: for<'a> Middleware<'a, S> {
    fn chain<M>(self, next: M) -> Chain<Self, M>
    where
        Self: Sized,
    {
        Chain(self, next)
    }
}

impl<S, T> MiddlewareExt<S> for T where T: for<'a> Middleware<'a, S> {}

/// A middleware composing and executing other middlewares in a stack-like manner.
pub struct Chain<T, U>(T, U);

#[async_trait(?Send)]
impl<'a, S, T, U> Middleware<'a, S> for Chain<T, U>
where
    S: 'a,
    T: for<'b> Middleware<'b, S>,
    U: for<'c> Middleware<'c, S>,
{
    #[inline]
    async fn handle(
        &'a self,
        ctx: &'a mut Context<S>,
        next: &'a mut dyn Next,
    ) -> Result {
        let ptr = ctx as *mut Context<S>;
        let mut next = self.1.handle(unsafe { &mut *ptr }, next);
        self.0.handle(ctx, &mut next).await
    }
}

#[async_trait(?Send)]
impl<'a, S, T, U> Endpoint<'a, S> for Chain<T, U>
where
    S: 'a,
    T: for<'b> Middleware<'b, S>,
    U: for<'c> Endpoint<'c, S>,
{
    #[inline]
    async fn end(&'a self, ctx: &'a mut Context<S>) -> Result {
        let ptr = ctx as *mut Context<S>;
        let mut next = self.1.end(unsafe { &mut *ptr });
        self.0.handle(ctx, &mut next).await
    }
}

// /// Join two middleware.
// ///
// /// ```rust
// /// use roa_core::{Middleware, join, Context, Next};
// /// use std::sync::Arc;
// ///
// /// let mut middleware: Arc<dyn Middleware<()>> = Arc::new(|_ctx: Context<()>, next: Next| async move {
// ///     next.await
// /// });
// ///
// /// middleware = Arc::new(join(middleware, |_ctx: Context<()>, next: Next| next));
// /// ```
// #[inline]
// pub fn join<S: State>(
//     current: Arc<dyn Middleware<S>>,
//     next: impl Middleware<S>,
// ) -> impl Middleware<S> {
//     join_all(vec![current, Arc::new(next)])
// }
//
// /// Join all middlewares in a vector.
// ///
// /// All middlewares would be composed and executed in a stack-like manner.
// #[inline]
// pub fn join_all<S: State>(
//     middlewares: Vec<Arc<dyn Middleware<S>>>,
// ) -> impl Middleware<S> {
//     Chain::new(middlewares)
// }

#[cfg(all(test, feature = "runtime"))]
mod tests {
    use crate::{App, Middleware, MiddlewareExt, Next, Request};
    use futures::lock::Mutex;
    use http::StatusCode;
    use std::sync::Arc;

    fn push_middleware(
        data: i32,
        vector: Arc<Mutex<Vec<i32>>>,
    ) -> impl for<'a> Middleware<'a, ()> {
        move |ctx, next| {
            let vector = vector.clone();
            async move {
                vector.lock().await.push(data);
                next.await?;
                vector.lock().await.push(data);
                Ok(())
            }
        }
    }

    #[async_std::test]
    async fn middleware_order() -> Result<(), Box<dyn std::error::Error>> {
        let vector = Arc::new(Mutex::new(Vec::new()));
        let endpoint = push_middleware(0, vector.clone())
            .chain(push_middleware(1, vector.clone()))
            .chain(push_middleware(2, vector.clone()))
            .chain(push_middleware(3, vector.clone()))
            .chain(push_middleware(4, vector.clone()))
            .chain(push_middleware(5, vector.clone()))
            .chain(push_middleware(6, vector.clone()))
            .chain(push_middleware(7, vector.clone()))
            .chain(push_middleware(8, vector.clone()))
            .chain(push_middleware(9, vector.clone()))
            .chain(|_ctx| async { Ok(()) });
        let service = App::new((), endpoint).http_service();
        let resp = service.serve(Request::default()).await?;
        assert_eq!(StatusCode::OK, resp.status);
        for i in 0..9 {
            assert_eq!(i, vector.lock().await[i]);
            assert_eq!(i, vector.lock().await[19 - i]);
        }
        Ok(())
    }
}
