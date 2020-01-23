use crate::{Context, MiddlewareStatus, Model};

pub type Next<'a, M> = &'a (dyn Fn(&'a mut Context<'a, M>) -> MiddlewareStatus<'a> + Sync + Send);
pub fn _next<'a, M: Model>(_ctx: &'a mut Context<'a, M>) -> MiddlewareStatus<'a> {
    Box::pin(async { Ok(()) })
}
