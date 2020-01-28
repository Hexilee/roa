use crate::{App, Model, Request, Response, State};
use std::cell::UnsafeCell;
use std::net::SocketAddr;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

pub struct Context<S: State>(Rc<UnsafeCell<Ctx<S>>>);

unsafe impl<S: State> Send for Context<S> {}
unsafe impl<S: State> Sync for Context<S> {}

impl<S: State> Context<S> {
    pub fn new(request: Request, app: App<S::Model>, ip: SocketAddr) -> Self {
        Ctx::new(request, app, ip).into()
    }
}

impl<S: State> Clone for Context<S> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<S: State> Deref for Context<S> {
    type Target = Ctx<S>;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0.get() }
    }
}

impl<S: State> DerefMut for Context<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.0.get() }
    }
}

impl<S: State> From<Ctx<S>> for Context<S> {
    fn from(ctx: Ctx<S>) -> Self {
        Self(Rc::new(UnsafeCell::new(ctx)))
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

impl Ctx<()> {
    // construct fake Context for test.
    #[cfg(test)]
    pub(crate) fn fake(request: Request) -> Self {
        use std::net::{IpAddr, Ipv4Addr};
        let ip = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        Self {
            request,
            response: Response::new(),
            app: App::builder().build(()),
            state: (),
            ip,
        }
    }
}
