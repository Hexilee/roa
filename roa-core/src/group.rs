use crate::{async_trait, Context, Middleware, Next, Result, State};
use std::sync::Arc;

/// A middleware composing and executing other middlewares in a stack-like manner.
struct Join<S>(Vec<Arc<dyn Middleware<S>>>);

impl<S> Join<S> {
    #[inline]
    fn new(middlewares: Vec<Arc<dyn Middleware<S>>>) -> Self {
        Self(middlewares)
    }
}

#[async_trait(?Send)]
impl<S: State> Middleware<S> for Join<S> {
    #[inline]
    async fn handle(self: Arc<Self>, ctx: Context<S>, mut next: Next) -> Result {
        for middleware in self.0.iter().rev() {
            let ctx = unsafe { ctx.unsafe_clone() };
            let middleware = middleware.clone();
            next = middleware.handle(ctx, next)
        }
        next.await
    }
}

/// Join two middleware.
///
/// ```rust
/// use roa_core::{Middleware, join, Context, Next};
/// use std::sync::Arc;
///
/// let mut middleware: Arc<dyn Middleware<()>> = Arc::new(|_ctx: Context<()>, next: Next| async move {
///     next.await
/// });
///
/// middleware = Arc::new(join(middleware, |_ctx: Context<()>, next: Next| next));
/// ```
#[inline]
pub fn join<S: State>(
    current: Arc<dyn Middleware<S>>,
    next: impl Middleware<S>,
) -> impl Middleware<S> {
    join_all(vec![current, Arc::new(next)])
}

/// Join all middlewares in a vector.
///
/// All middlewares would be composed and executed in a stack-like manner.
#[inline]
pub fn join_all<S: State>(
    middlewares: Vec<Arc<dyn Middleware<S>>>,
) -> impl Middleware<S> {
    Join::new(middlewares)
}

#[cfg(all(test, feature = "runtime"))]
mod tests {
    use crate::{join_all, App, Middleware, Next, Request};
    use futures::lock::Mutex;
    use http::StatusCode;
    use std::sync::Arc;

    #[async_std::test]
    async fn middleware_order() -> Result<(), Box<dyn std::error::Error>> {
        let vector = Arc::new(Mutex::new(Vec::new()));
        let mut middlewares = Vec::<Arc<dyn Middleware<()>>>::new();
        for i in 0..100 {
            let vec = vector.clone();
            middlewares.push(Arc::new(move |_ctx, next: Next| {
                let vec = vec.clone();
                async move {
                    vec.lock().await.push(i);
                    next.await?;
                    vec.lock().await.push(i);
                    Ok(())
                }
            }));
        }
        let service = App::new(()).gate(join_all(middlewares)).fake_service();
        let resp = service.serve(Request::default()).await?;
        assert_eq!(StatusCode::OK, resp.status);
        for i in 0..100 {
            assert_eq!(i, vector.lock().await[i]);
            assert_eq!(i, vector.lock().await[199 - i]);
        }
        Ok(())
    }
}
