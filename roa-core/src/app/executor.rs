use hyper::rt;
use std::future::Future;

#[derive(Copy, Clone)]
pub struct Executor;

impl<F> rt::Executor<F> for Executor
where
    F: 'static + Send + Future,
    F::Output: 'static + Send,
{
    fn execute(&self, fut: F) {
        async_std::task::spawn(fut);
    }
}
