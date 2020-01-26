use crate::{Context, Next, State, Status, StatusFuture};
use std::future::Future;

pub type DynMiddleware<S> = dyn 'static + Sync + Send + Fn(Context<S>, Next) -> StatusFuture;

pub trait Middleware<S: State>: 'static + Sync + Send {
    type StatusFuture: 'static + Future<Output = Result<(), Status>> + Send;
    fn handle(&self, ctx: Context<S>, next: Next) -> Self::StatusFuture;
    fn dynamic(self: Box<Self>) -> Box<DynMiddleware<S>> {
        Box::new(move |ctx, next| Box::pin(self.handle(ctx, next)))
    }
}

impl<S, F, T> Middleware<S> for T
where
    S: State,
    F: 'static + Future<Output = Result<(), Status>> + Send,
    T: 'static + Sync + Send + Fn(Context<S>, Next) -> F,
{
    type StatusFuture = F;
    fn handle(&self, ctx: Context<S>, next: Next) -> Self::StatusFuture {
        (self)(ctx, next)
    }
}

pub async fn first_middleware<S: State>(_ctx: Context<S>, next: Next) -> Result<(), Status> {
    next().await
}
