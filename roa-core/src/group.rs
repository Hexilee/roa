use crate::{Context, DynTargetHandler, Error, Model, Next, Result, TargetHandler};
use std::future::Future;
use std::sync::Arc;

pub struct Group<M: Model>(Arc<DynTargetHandler<M, Next>>);

impl<M: Model> Group<M> {
    pub fn new() -> Self {
        Self(Arc::new(|_ctx, next| next()))
    }

    pub fn join<F>(
        &mut self,
        middleware: impl 'static + Sync + Send + Fn(Context<M>, Next) -> F,
    ) -> &mut Self
    where
        F: 'static + Future<Output = Result> + Send,
    {
        let current = self.0.clone();
        let next_middleware: Arc<DynTargetHandler<M, Next>> =
            Arc::from(Box::new(middleware).dynamic());
        self.0 = Arc::new(move |ctx, next| {
            let next_middleware = next_middleware.clone();
            let ctx_cloned = ctx.clone();
            let next = Box::new(move || next_middleware(ctx_cloned, next));
            current(ctx, next)
        });
        self
    }

    pub fn handler(&self) -> Box<DynTargetHandler<M, Next>> {
        let handler = self.0.clone();
        Box::new(move |ctx, next| handler(ctx, next))
    }
}

impl<M: Model> Clone for Group<M> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::Group;
    use crate::App;
    use async_std::task::spawn;
    use futures::lock::Mutex;
    use http::StatusCode;
    use std::sync::Arc;

    #[tokio::test]
    async fn middleware_order() -> Result<(), Box<dyn std::error::Error>> {
        let vector = Arc::new(Mutex::new(Vec::new()));
        let mut middleware = Group::<()>::new();
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
        let (addr, server) = App::new(()).gate(middleware.handler()).run_local()?;
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
