use futures::channel::oneshot::{channel, Receiver};
use futures::task::{Context, Poll};
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
pub struct Executor(pub(crate) Arc<dyn 'static + Send + Sync + Spawn>);

/// A handle that awaits the result of a task.
pub struct JoinHandle<T>(Receiver<T>);

impl Executor {
    /// Spawn a future by app runtime
    #[inline]
    pub fn spawn<Fut>(&self, fut: Fut) -> JoinHandle<Fut::Output>
    where
        Fut: 'static + Send + Future,
        Fut::Output: 'static + Send,
    {
        let (sender, recv) = channel();
        self.0.spawn(Box::pin(async move {
            if sender.send(fut.await).is_err() {
                // handler is dropped, do nothing.
            };
        }));
        JoinHandle(recv)
    }

    /// Spawn a blocking task by app runtime
    #[inline]
    pub fn spawn_blocking<T, R>(&self, task: T) -> JoinHandle<R>
    where
        T: 'static + Send + FnOnce() -> R,
        R: 'static + Send,
    {
        let (sender, recv) = channel();
        self.0.spawn_blocking(Box::new(|| {
            if sender.send(task()).is_err() {
                // handler is dropped, do nothing.
            };
        }));
        JoinHandle(recv)
    }
}

impl<T> Future for JoinHandle<T> {
    type Output = T;
    #[inline]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let ready = futures::ready!(Pin::new(&mut self.0).poll(cx));
        Poll::Ready(ready.expect("receiver in JoinHandle shouldn't be canceled"))
    }
}

impl<F> rt::Executor<F> for Executor
where
    F: 'static + Send + Future,
    F::Output: 'static + Send,
{
    #[inline]
    fn execute(&self, fut: F) {
        self.0.spawn(Box::pin(async move {
            let _ = fut.await;
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::{BlockingObj, Executor, FutureObj, Spawn};
    use async_std::sync::Arc;

    pub struct Exec;

    impl Spawn for Exec {
        fn spawn(&self, fut: FutureObj) {
            async_std::task::spawn(fut);
        }

        fn spawn_blocking(&self, task: BlockingObj) {
            async_std::task::spawn_blocking(task);
        }
    }

    #[async_std::test]
    async fn spawn() {
        let exec = Executor(Arc::new(Exec));
        assert_eq!(1, exec.spawn(async { 1 }).await);
    }

    #[async_std::test]
    async fn spawn_blocking() {
        let exec = Executor(Arc::new(Exec));
        assert_eq!(1, exec.spawn_blocking(|| 1).await);
    }
}
