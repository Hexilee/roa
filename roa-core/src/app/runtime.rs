use crate::{App, BlockingObj, Endpoint, FutureObj, Spawn, State};

impl<S> App<S> {
    /// Construct app with default runtime.
    ///
    /// Feature `runtime` is required.
    #[inline]
    pub fn new(state: S, endpoint: impl for<'a> Endpoint<'a, S>) -> Self {
        Self::with_exec(state, endpoint, Exec)
    }
}

pub struct Exec;

impl Spawn for Exec {
    #[inline]
    fn spawn(&self, fut: FutureObj) {
        async_std::task::spawn(fut);
    }

    #[inline]
    fn spawn_blocking(&self, task: BlockingObj) {
        async_std::task::spawn_blocking(task);
    }
}
