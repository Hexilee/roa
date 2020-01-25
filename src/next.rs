use crate::StatusFuture;

pub type Next = Box<dyn FnOnce() -> StatusFuture + Sync + Send>;
pub fn _next() -> StatusFuture {
    Box::pin(async move { Ok(()) })
}
