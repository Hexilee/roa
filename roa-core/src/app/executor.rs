use hyper::rt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Future Object
pub type FutureObj = Pin<Box<dyn 'static + Send + Future<Output = ()>>>;

/// Blocking task Object
pub type BlockingObj = Box<dyn 'static + Send + FnOnce()>;

/// Executor constraint.
pub trait Spawn {
    /// Spawn a future object
    fn spawn(&self, fut: FutureObj);

    /// Spawn a blocking task object
    fn spawn_blocking(&self, task: BlockingObj);
}

/// A type implementing hyper::rt::Executor
#[derive(Clone)]
pub struct Executor(pub Arc<dyn 'static + Send + Sync + Spawn>);

impl<F> rt::Executor<F> for Executor
where
    F: 'static + Send + Future,
    F::Output: 'static + Send,
{
    fn execute(&self, fut: F) {
        self.0.spawn(Box::pin(async move {
            fut.await;
        }))
    }
}
