use crate::{last, App, Context, DynHandler, DynTargetHandler, Model, Next, Status, TargetHandler};
use futures::Future;
use std::sync::Arc;

#[derive(Clone)]
pub struct Middleware<M: Model>(Vec<Arc<DynTargetHandler<M, Next>>>);

impl<M: Model> Middleware<M> {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn join<F>(
        &mut self,
        middleware: impl 'static + Sync + Send + Fn(Context<M>, Next) -> F,
    ) -> &mut Self
    where
        F: 'static + Future<Output = Result<(), Status>> + Send,
    {
        self.0.push(Arc::from(Box::new(middleware).dynamic()));
        self
    }

    pub fn handler(&self) -> Arc<DynHandler<M>> {
        let mut handler: Arc<DynHandler<M>> = Arc::new(|_ctx| last());
        for middleware in self.0.iter().rev() {
            let current = middleware.clone();
            handler = Arc::new(move |ctx| {
                let ctx_cloned = ctx.clone();
                let handler = handler.clone();
                let next = Box::new(move || handler(ctx_cloned));
                current(ctx, next)
            })
        }
        handler
    }

    pub fn app(&self, model: M) -> App<M> {
        App::new(self.handler(), Arc::new(model))
    }
}

#[cfg(test)]
mod tests {
    use super::Middleware;
    use crate::{Ctx, Request};
    use futures::lock::Mutex;
    use std::sync::Arc;

    #[tokio::test]
    async fn middleware_order() -> Result<(), Box<dyn std::error::Error>> {
        let vector = Arc::new(Mutex::new(Vec::new()));
        let mut middleware = Middleware::<()>::new();
        for i in 0..100 {
            let vec = vector.clone();
            middleware.join(move |_ctx, next| {
                let vec = vec.clone();
                async move {
                    vec.lock().await.push(i);
                    next().await?;
                    vec.lock().await.push(i);
                    Ok(())
                }
            });
        }
        middleware.handler()(Ctx::fake(Request::new()).into()).await?;
        for i in 0..100 {
            assert_eq!(i, vector.lock().await[i]);
            assert_eq!(i, vector.lock().await[199 - i]);
        }
        Ok(())
    }
}
