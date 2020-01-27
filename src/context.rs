use crate::{App, Model, Request, Response, State};
use std::cell::UnsafeCell;
use std::net::SocketAddr;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

pub struct Context<S: State> {
    inner: Rc<UnsafeCell<Ctx<S>>>,
}

unsafe impl<S: State> Send for Context<S> {}
unsafe impl<S: State> Sync for Context<S> {}

impl<S: State> Context<S> {
    pub fn new(request: Request, app: App<S::Model>, ip: SocketAddr) -> Self {
        let inner = Ctx::new(request, app, ip);
        Self {
            inner: Rc::new(UnsafeCell::new(inner)),
        }
    }
}

impl<S: State> Clone for Context<S> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<S: State> Deref for Context<S> {
    type Target = Ctx<S>;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.inner.get() }
    }
}

impl<S: State> DerefMut for Context<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.inner.get() }
    }
}

pub struct Ctx<S: State> {
    pub request: Request,
    pub response: Response,
    pub app: App<S::Model>,
    pub state: S,
    pub ip: SocketAddr,
}

impl<S: State> Ctx<S> {
    fn new(request: Request, app: App<S::Model>, ip: SocketAddr) -> Self {
        let state = app.model.new_state();
        Self {
            request,
            response: Response::new(),
            app,
            state,
            ip,
        }
    }
}
