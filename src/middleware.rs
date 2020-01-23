use crate::{Context, Model, Next};
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;

pub type MiddlewareStatus<'a> =
    Pin<Box<dyn 'a + Future<Output = Result<(), Infallible>> + Sync + Send>>;

pub trait Middleware<M: Model>:
    'static + Sync + Send + for<'a> Fn(&'a mut Context<'a, M>, Next<M>) -> MiddlewareStatus<'a>
{
    fn gate<'a>(&self, ctx: &'a mut Context<'a, M>, next: Next<M>) -> MiddlewareStatus<'a>;
}
impl<M: Model, T> Middleware<M> for T
where
    T: 'static + Sync + Send + for<'a> Fn(&'a mut Context<'a, M>, Next<M>) -> MiddlewareStatus<'a>,
{
    fn gate<'a>(&self, ctx: &'a mut Context<'a, M>, next: Next<M>) -> MiddlewareStatus<'a> {
        self(ctx, next)
    }
}
