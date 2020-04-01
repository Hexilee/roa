use crate::executor::{BlockingObj, FutureObj};
use crate::{App, Spawn};

impl<S> App<S, ()> {
    /// Construct app with default runtime.
    #[cfg_attr(feature = "docs", doc(cfg(feature = "runtime")))]
    #[inline]
    pub fn state(state: S) -> Self {
        Self::with_exec(state, Exec)
    }
}

impl App<(), ()> {
    /// Construct app with default runtime.
    #[cfg_attr(feature = "docs", doc(cfg(feature = "runtime")))]
    #[inline]
    pub fn new() -> Self {
        Self::state(())
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
