use crate::{Context, MiddlewareStatus, Model};

pub type Next<M> = Box<dyn FnOnce(&mut Context<M>) -> MiddlewareStatus + Sync + Send>;
pub fn _next<M: Model>(_ctx: &mut Context<M>) -> MiddlewareStatus {
    Box::pin(async { Ok(()) })
}
