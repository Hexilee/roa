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

impl Default for App<(), ()> {
    /// Construct app with default runtime.
    fn default() -> Self {
        Self::new()
    }
}

pub struct Exec;

impl Spawn for Exec {
    #[inline]
    fn spawn(&self, fut: FutureObj) {
        tokio::task::spawn(fut);
    }

    #[inline]
    fn spawn_blocking(&self, task: BlockingObj) {
        tokio::task::spawn_blocking(task);
    }
}
