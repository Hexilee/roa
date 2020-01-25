use crate::Error;
use crate::{Context, Model, Next};
use std::future::Future;
use std::pin::Pin;

pub type MiddlewareStatus<'a> = Pin<Box<dyn 'a + Future<Output = Result<(), Error>> + Send>>;

// TODO: constraint F with 'b
pub trait Middleware<'a, M, F>:
    'static + Sync + Send + Fn(&'a mut Context<M>, Next<M>) -> F
where
    M: Model,
    F: 'a + Future<Output = Result<(), Error>> + Send,
{
}

pub trait DynMiddleware<M: Model>:
    'static + Sync + Send + for<'a> Fn(&'a mut Context<M>, Next<M>) -> MiddlewareStatus<'a>
{
    fn gate<'a>(&self, ctx: &'a mut Context<M>, next: Next<M>) -> MiddlewareStatus<'a>;
}

impl<'a, M, F, T> Middleware<'a, M, F> for T
where
    M: Model,
    F: 'a + Future<Output = Result<(), Error>> + Send,
    T: 'static + Sync + Send + Fn(&'a mut Context<M>, Next<M>) -> F,
{
}

impl<M: Model, T> DynMiddleware<M> for T
where
    T: 'static + Sync + Send + for<'a> Fn(&'a mut Context<M>, Next<M>) -> MiddlewareStatus<'a>,
{
    fn gate<'a>(&self, ctx: &'a mut Context<M>, next: Next<M>) -> MiddlewareStatus<'a> {
        (self)(ctx, next)
    }
}
