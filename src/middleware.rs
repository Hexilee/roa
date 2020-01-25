use crate::Status;
use crate::{Context, Model, Next};
use std::future::Future;
use std::pin::Pin;

pub type StatusFuture<'a> = Pin<Box<dyn 'a + Future<Output = Result<(), Status>> + Send>>;

// TODO: constraint F with 'b
pub trait Middleware<'a, M, F>:
    'static + Sync + Send + Fn(&'a mut Context<M>, Next<M>) -> F
where
    M: Model,
    F: 'a + Future<Output = Result<(), Status>> + Send,
{
}

pub trait DynMiddleware<M: Model>:
    'static + Sync + Send + for<'a> Fn(&'a mut Context<M>, Next<M>) -> StatusFuture<'a>
{
    fn gate<'a>(&self, ctx: &'a mut Context<M>, next: Next<M>) -> StatusFuture<'a>;
}

impl<'a, M, F, T> Middleware<'a, M, F> for T
where
    M: Model,
    F: 'a + Future<Output = Result<(), Status>> + Send,
    T: 'static + Sync + Send + Fn(&'a mut Context<M>, Next<M>) -> F,
{
}

impl<M: Model, T> DynMiddleware<M> for T
where
    T: 'static + Sync + Send + for<'a> Fn(&'a mut Context<M>, Next<M>) -> StatusFuture<'a>,
{
    fn gate<'a>(&self, ctx: &'a mut Context<M>, next: Next<M>) -> StatusFuture<'a> {
        (self)(ctx, next)
    }
}
