use crate::{Context, MiddlewareStatus, Model};

pub type Next<M> =
    Box<dyn for<'a> FnOnce(&'a mut Context<'a, M>) -> MiddlewareStatus<'a> + Sync + Send>;
pub fn _next<'a, M: Model>(_ctx: &'a mut Context<'a, M>) -> MiddlewareStatus<'a> {
    Box::pin(async { Ok(()) })
}
