use crate::{Context, Next, State, Status};
use std::future::Future;
use std::pin::Pin;

pub type StatusFuture<'a> = Pin<Box<dyn 'a + Future<Output = Result<(), Status>> + Send>>;

pub trait DynMiddleware<S: State>: 'static + Sync + Send {
    fn handle<'a>(&self, ctx: &'a mut Context<S>, next: Next<S>) -> StatusFuture<'a>;
}

impl<S, T> DynMiddleware<S> for T
where
    S: State,
    T: 'static + Sync + Send + for<'a> Fn(&'a mut Context<S>, Next<S>) -> StatusFuture<'a>,
{
    fn handle<'a>(&self, ctx: &'a mut Context<S>, next: Next<S>) -> StatusFuture<'a> {
        (self)(ctx, next)
    }
}

pub fn make_dyn_middleware<S: State>(
    handler: impl 'static + Sync + Send + for<'a> Fn(&'a mut Context<S>, Next<S>) -> StatusFuture<'a>,
) -> Box<dyn DynMiddleware<S>> {
    Box::new(handler)
}
