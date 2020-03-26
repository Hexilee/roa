use futures::task::{Context, Poll};
use std::future::Future;
use std::pin::Pin;

/// A wrapper to make future `Send`. It's used to wrap future returned by top middleware.
/// So the future returned by each middleware or endpoint can be `?Send`.
///
/// But how to ensure thread safety? Because the middleware and the context must be `Sync + Send`,
/// which means the only factor causing future `!Send` is the variables generated in `Future::poll`.
/// And these variable mustn't be accessed from other threads.
pub struct SendFuture<F>(pub F);

impl<F> Future for SendFuture<F>
where
    F: 'static + Future + Unpin,
{
    type Output = F::Output;
    #[inline]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.0).poll(cx)
    }
}

unsafe impl<F> Send for SendFuture<F> {}
