use crate::{Context, Executor};
use crossbeam_queue::ArrayQueue;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

pub const DEFAULT_MAX_SIZE: usize = 1 << 20;
pub const DEFAULT_MIN_SIZE: usize = 1 << 8;

const PUSH_BUG: &str = "Context queue is full, push fails, this is a bug of roa.";

pub struct ContextPool<S> {
    pub(crate) exec: Executor,
    state: S,
    counter: AtomicUsize,
    ctx_queue: Arc<ArrayQueue<Context<S>>>,
}

pub struct ContextGuard<S> {
    ctx: Context<S>,
    ctx_queue: Arc<ArrayQueue<Context<S>>>,
}

impl<S: Clone> ContextPool<S> {
    pub fn new(min_size: usize, max_size: usize, state: S, exec: Executor) -> Self {
        debug_assert!(min_size <= max_size);
        let ctx_queue = Arc::new(ArrayQueue::new(max_size));
        let counter = AtomicUsize::new(min_size);
        for _ in 0..min_size {
            ctx_queue
                .push(Context::new(state.clone(), exec.clone()))
                .expect(PUSH_BUG);
        }
        Self {
            counter,
            ctx_queue,
            state,
            exec,
        }
    }

    pub fn get(
        &self,
        addr: SocketAddr,
        req: &mut http::Request<hyper::Body>,
    ) -> Option<ContextGuard<S>> {
        let mut ctx = match self.ctx_queue.pop() {
            Ok(ctx) => ctx,
            Err(_) => {
                if self.counter.fetch_add(1, Ordering::Relaxed)
                    < self.ctx_queue.capacity()
                {
                    Context::new(self.state.clone(), self.exec.clone())
                } else {
                    return None;
                }
            }
        };
        ctx.reload(addr);
        ctx.req_mut().reload(req);
        Some(ContextGuard::new(ctx, self.ctx_queue.clone()))
    }
}

impl<S> ContextGuard<S> {
    fn new(ctx: Context<S>, ctx_queue: Arc<ArrayQueue<Context<S>>>) -> Self {
        Self { ctx, ctx_queue }
    }

    pub unsafe fn get(&self) -> Context<S> {
        self.ctx.unsafe_clone()
    }
}

impl<S> Drop for ContextGuard<S> {
    fn drop(&mut self) {
        self.ctx_queue
            .push(unsafe { self.ctx.unsafe_clone() })
            .expect(PUSH_BUG)
    }
}

unsafe impl<S> Sync for ContextPool<S> where S: Sync + Send {}
unsafe impl<S> Send for ContextPool<S> where S: Sync + Send {}
