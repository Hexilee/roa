use crate::{last, App, Context, DynHandler, DynMiddleware, Middleware, Next, State, Status};
use futures::Future;
use std::sync::Arc;

pub struct Group<S: State>(Vec<Arc<DynMiddleware<S>>>);

impl<S: State> Group<S> {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn handle_fn<F>(
        &mut self,
        middleware: impl 'static + Sync + Send + Fn(Context<S>, Next) -> F,
    ) -> &mut Self
    where
        F: 'static + Future<Output = Result<(), Status>> + Send,
    {
        self.handle(middleware)
    }

    pub fn handle<F>(&mut self, middleware: impl Middleware<S, StatusFuture = F>) -> &mut Self
    where
        F: 'static + Future<Output = Result<(), Status>> + Send,
    {
        self.0.push(Arc::from(Box::new(middleware).dynamic()));
        self
    }

    pub fn handler(&self) -> Arc<DynHandler<S>> {
        let mut handler: Arc<DynHandler<S>> = Arc::new(|_ctx| last());
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

    pub fn app(&self, model: S::Model) -> App<S::Model> {
        App::new(self.handler(), Arc::new(model))
    }
}

#[cfg(test)]
mod tests {
    use super::Group;
    use crate::{Ctx, Request};
    use futures::lock::Mutex;
    use std::sync::Arc;

    #[tokio::test]
    async fn middleware_order() -> Result<(), Box<dyn std::error::Error>> {
        let vector = Arc::new(Mutex::new(Vec::new()));
        let mut group = Group::<()>::new();
        for i in 0..100 {
            let vec = vector.clone();
            group.handle_fn(move |_ctx, next| {
                let vec = vec.clone();
                async move {
                    vec.lock().await.push(i);
                    next().await?;
                    vec.lock().await.push(i);
                    Ok(())
                }
            });
        }
        group.handler()(Ctx::fake(Request::new()).into()).await?;
        for i in 0..100 {
            assert_eq!(i, vector.lock().await[i]);
            assert_eq!(i, vector.lock().await[199 - i]);
        }
        Ok(())
    }
}
