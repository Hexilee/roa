use futures::task::{Context, Poll};
use std::future::Future;
use std::pin::Pin;

pub struct SendFuture<F>(pub F);

impl<F> Future for SendFuture<F>
where
    F: 'static + Future + Unpin,
{
    type Output = F::Output;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.0).poll(cx)
    }
}

unsafe impl<F> Send for SendFuture<F> {}
