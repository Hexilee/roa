use crate::{Context, Model, Next};
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;

pub type MiddlewareStatus<'a> =
    Pin<Box<dyn 'a + Future<Output = Result<(), Infallible>> + Sync + Send>>;

// TODO: constraint F with 'b
pub trait Middleware<M, F>:
    'static + Sync + Send + for<'b> Fn(&'b mut Context<'b, M>, Next<M>) -> F
where
    M: Model,
    F: Future<Output = Result<(), Infallible>> + Sync + Send,
{
}

pub trait DynMiddleware<M: Model>:
    'static + Sync + Send + for<'a> Fn(&'a mut Context<'a, M>, Next<M>) -> MiddlewareStatus<'a>
{
    fn gate<'a>(&self, ctx: &'a mut Context<'a, M>, next: Next<M>) -> MiddlewareStatus<'a>;
}

impl<M, F, T> Middleware<M, F> for T
where
    M: Model,
    F: Future<Output = Result<(), Infallible>> + Sync + Send,
    T: 'static + Sync + Send + Fn(&mut Context<M>, Next<M>) -> F,
{
}

impl<M: Model, T> DynMiddleware<M> for T
where
    T: 'static + Sync + Send + for<'a> Fn(&'a mut Context<'a, M>, Next<M>) -> MiddlewareStatus<'a>,
{
    fn gate<'a>(&self, ctx: &'a mut Context<'a, M>, next: Next<M>) -> MiddlewareStatus<'a> {
        self(ctx, next)
    }
}
