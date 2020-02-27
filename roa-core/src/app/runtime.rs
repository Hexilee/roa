use crate::{App, State};
use futures::future::FutureObj;
use futures::task::{Spawn, SpawnError};

impl<S: State> App<S> {
    /// Construct app with default runtime
    pub fn new(state: S) -> Self {
        Self::with_exec(state, Executor)
    }
}

struct Executor;

impl Spawn for Executor {
    fn spawn_obj(&self, future: FutureObj<'static, ()>) -> Result<(), SpawnError> {
        async_std::task::spawn(future);
        Ok(())
    }
}
