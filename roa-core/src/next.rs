use crate::ResultFuture;

pub type Next = Box<dyn FnOnce() -> ResultFuture + Sync + Send>;
pub fn last() -> ResultFuture {
    Box::pin(async move { Ok(()) })
}
