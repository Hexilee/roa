use crate::{Context, Next, State, Status};
use std::future::Future;
use std::pin::Pin;

pub type StatusFuture = Pin<Box<dyn 'static + Future<Output = Result<(), Status>> + Send>>;

pub trait DynMiddleware<S: State>: 'static + Sync + Send {
    fn handle(&self, ctx: Context<S>, next: Next) -> StatusFuture;
}

impl<S, T> DynMiddleware<S> for T
where
    S: State,
    T: 'static + Sync + Send + Fn(Context<S>, Next) -> StatusFuture,
{
    fn handle(&self, ctx: Context<S>, next: Next) -> StatusFuture {
        (self)(ctx, next)
    }
}

pub trait Middleware<S: State>: 'static + Sync + Send {
    type StatusFuture: 'static + Future<Output = Result<(), Status>> + Send;
    fn handle(&self, ctx: Context<S>, next: Next) -> Self::StatusFuture;
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

pub fn make_dyn<S, F, T>(
    handler: impl 'static + Sync + Send + Fn(Context<S>, T) -> F,
) -> impl 'static + Sync + Send + Fn(Context<S>, T) -> StatusFuture
where
    S: State,
    F: 'static + Future<Output = Result<(), Status>> + Send,
{
    move |ctx, next| Box::pin(handler(ctx, next))
}

pub fn make_dyn_middleware<S: State>(
    handler: impl 'static + Sync + Send + Fn(Context<S>, Next) -> StatusFuture,
) -> Box<dyn DynMiddleware<S>> {
    Box::new(handler)
}
