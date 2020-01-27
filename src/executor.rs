use hyper::rt;
use std::future::Future;
use std::pin::Pin;

type ExFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

#[derive(Copy, Clone)]
pub struct Executor;

impl rt::Executor<ExFuture> for Executor {
    fn execute(&self, fut: ExFuture) {
        async_std::task::spawn(Box::pin(fut));
    }
}
impl Executor {
    pub fn new() -> Self {
        Executor
    }
}
