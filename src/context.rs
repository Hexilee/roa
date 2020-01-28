use crate::{App, Model, Request, Response};
use std::cell::UnsafeCell;
use std::net::SocketAddr;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

pub struct Context<M: Model>(Rc<UnsafeCell<Ctx<M>>>);

unsafe impl<M: Model> Send for Context<M> {}
unsafe impl<M: Model> Sync for Context<M> {}

impl<M: Model> Context<M> {
    pub fn new(request: Request, app: App<M>, ip: SocketAddr) -> Self {
        Ctx::new(request, app, ip).into()
    }
}

impl<M: Model> Clone for Context<M> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<M: Model> Deref for Context<M> {
    type Target = Ctx<M>;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0.get() }
    }
}

impl<M: Model> DerefMut for Context<M> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.0.get() }
    }
}


impl<M: Model> Deref for Ctx<M> {
    type Target = M::State;
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl<M: Model> DerefMut for Ctx<M> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.state
    }
}

impl<M: Model> From<Ctx<M>> for Context<M> {
    fn from(ctx: Ctx<M>) -> Self {
        Self(Rc::new(UnsafeCell::new(ctx)))
    }
}

pub struct Ctx<M: Model> {
    pub request: Request,
    pub response: Response,
    pub app: App<M>,
    pub state: M::State,
    pub peer_addr: SocketAddr,
}

impl<M: Model> Ctx<M> {
    fn new(request: Request, app: App<M>, peer_addr: SocketAddr) -> Self {
        let state = app.model.new_state();
        Self {
            request,
            response: Response::new(),
            app,
            state,
            peer_addr,
        }
    }
}

impl Ctx<()> {
    // construct fake Context for test.
    #[cfg(test)]
    pub(crate) fn fake(request: Request) -> Self {
        use std::net::{IpAddr, Ipv4Addr};
        let peer_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        Self {
            request,
            response: Response::new(),
            app: crate::Group::new().app(()),
            state: (),
            peer_addr,
        }
    }
}
