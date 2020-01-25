use crate::{Context, Next, State, Status};
use std::future::Future;
use std::pin::Pin;

pub type StatusFuture<'a> = Pin<Box<dyn 'a + Future<Output = Result<(), Status>> + Send>>;

// TODO: constraint F with 'b
pub trait Middleware<'a, S, F>:
    'static + Sync + Send + Fn(&'a mut Context<S>, Next<S>) -> F
where
    S: State,
    F: 'a + Future<Output = Result<(), Status>> + Send,
{
}

pub trait DynMiddleware<S: State>:
    'static + Sync + Send + for<'a> Fn(&'a mut Context<S>, Next<S>) -> StatusFuture<'a>
{
    fn gate<'a>(&self, ctx: &'a mut Context<S>, next: Next<S>) -> StatusFuture<'a>;
}

impl<'a, S, F, T> Middleware<'a, S, F> for T
where
    S: State,
    F: 'a + Future<Output = Result<(), Status>> + Send,
    T: 'static + Sync + Send + Fn(&'a mut Context<S>, Next<S>) -> F,
{
}

impl<S, T> DynMiddleware<S> for T
where
    S: State,
    T: 'static + Sync + Send + for<'a> Fn(&'a mut Context<S>, Next<S>) -> StatusFuture<'a>,
{
    fn gate<'a>(&self, ctx: &'a mut Context<S>, next: Next<S>) -> StatusFuture<'a> {
        (self)(ctx, next)
    }
}
