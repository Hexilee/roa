use crate::{App, BlockingObj, FutureObj, Spawn, State};

impl<S: State> App<S> {
    /// Construct app with default runtime.
    pub fn new(state: S) -> Self {
        Self::with_exec(state, Exec)
    }
}

pub struct Exec;

impl Spawn for Exec {
    fn spawn(&self, fut: FutureObj) {
        async_std::task::spawn(fut);
    }

    fn spawn_blocking(&self, task: BlockingObj) {
        async_std::task::spawn_blocking(task);
    }
}
