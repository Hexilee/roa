use futures::task::{Spawn, SpawnExt};
use hyper::rt;
use log::error;
use std::future::Future;
use std::sync::Arc;

#[derive(Clone)]
pub struct Executor(pub Arc<dyn 'static + Send + Sync + Spawn>);

impl<F> rt::Executor<F> for Executor
where
    F: 'static + Send + Future,
    F::Output: 'static + Send,
{
    fn execute(&self, fut: F) {
        if let Err(err) = self.0.spawn(async move {
            fut.await;
        }) {
            error!("runtime error: {}", err)
        }
    }
}
