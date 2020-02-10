use crate::{Context, Middleware, Model, Next, Result};
use async_trait::async_trait;
use std::sync::Arc;

/// A structure to join middlewares.
///
/// ### Example
/// ```rust
/// use roa_core::{App, join_all, Middleware, Next};
/// use async_std::task::spawn;
/// use futures::lock::Mutex;
/// use http::StatusCode;
/// use std::sync::Arc;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let vector = Arc::new(Mutex::new(Vec::new()));
///     let mut middlewares = Vec::<Arc<dyn Middleware<()>>>::new();
///     for i in 0..100 {
///         let vec = vector.clone();
///         middlewares.push(Arc::new(move |_ctx, next: Next| {
///             let vec = vec.clone();
///             async move {
///                 vec.lock().await.push(i);
///                 next().await?;
///                 vec.lock().await.push(i);
///                 Ok(())
///             }
///         }));
///     }
///     let (addr, server) = App::new(()).gate(join_all(middlewares)).run_local()?;
///     spawn(server);
///     let resp = reqwest::get(&format!("http://{}", addr)).await?;
///     assert_eq!(StatusCode::OK, resp.status());
///     for i in 0..100 {
///         assert_eq!(i, vector.lock().await[i]);
///         assert_eq!(i, vector.lock().await[199 - i]);
///     }
///     Ok(())
/// }
/// ```
struct Join<M: Model>(Vec<Arc<dyn Middleware<M>>>);

impl<M: Model> Join<M> {
    fn new(middlewares: Vec<Arc<dyn Middleware<M>>>) -> Self {
        Self(middlewares)
    }
}

#[async_trait]
impl<M: Model> Middleware<M> for Join<M> {
    async fn handle(self: Arc<Self>, ctx: Context<M>, mut next: Next) -> Result {
        for middleware in self.0.iter().rev() {
            let ctx = ctx.clone();
            let middleware = middleware.clone();
            next = Box::new(move || middleware.handle(ctx, next))
        }
        next().await
    }
}

pub fn join<M: Model>(
    current: Arc<dyn Middleware<M>>,
    next: impl Middleware<M>,
) -> impl Middleware<M> {
    join_all(vec![current, Arc::new(next)])
}

pub fn join_all<M: Model>(middlewares: Vec<Arc<dyn Middleware<M>>>) -> impl Middleware<M> {
    Join::new(middlewares)
}

#[cfg(test)]
mod tests {
    use crate::{join_all, App, Middleware, Next};
    use async_std::task::spawn;
    use futures::lock::Mutex;
    use http::StatusCode;
    use std::sync::Arc;

    #[tokio::test]
    async fn middleware_order() -> Result<(), Box<dyn std::error::Error>> {
        let vector = Arc::new(Mutex::new(Vec::new()));
        let mut middlewares = Vec::<Arc<dyn Middleware<()>>>::new();
        for i in 0..100 {
            let vec = vector.clone();
            middlewares.push(Arc::new(move |_ctx, next: Next| {
                let vec = vec.clone();
                async move {
                    vec.lock().await.push(i);
                    next().await?;
                    vec.lock().await.push(i);
                    Ok(())
                }
            }));
        }
        let (addr, server) = App::new(()).gate(join_all(middlewares)).run_local()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}", addr)).await?;
        assert_eq!(StatusCode::OK, resp.status());
        for i in 0..100 {
            assert_eq!(i, vector.lock().await[i]);
            assert_eq!(i, vector.lock().await[199 - i]);
        }
        Ok(())
    }
}
