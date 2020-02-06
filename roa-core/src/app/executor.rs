use hyper::rt;
use std::future::Future;

/// An implementation of hyper::rt::Executor based on async-std
#[derive(Copy, Clone)]
pub struct Executor;

impl<F> rt::Executor<F> for Executor
where
    F: 'static + Send + Future,
    F::Output: 'static + Send,
{
    #[inline]
    fn execute(&self, fut: F) {
        async_std::task::spawn(fut);
    }
}
