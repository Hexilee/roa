use crate::StatusFuture;

pub type Next = Box<dyn FnOnce() -> StatusFuture + Sync + Send>;
pub fn last() -> StatusFuture {
    Box::pin(async move { Ok(()) })
}
