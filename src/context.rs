use crate::{Request, Response, Service};
use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

pub struct Context<S: State> {
    inner: Rc<UnsafeCell<Ctx<S>>>,
}

unsafe impl<S: State> Send for Context<S> {}
unsafe impl<S: State> Sync for Context<S> {}

impl<S: State> Context<S> {
    pub fn new(request: Request, app: Service<S>) -> Self {
        let inner = Ctx::new(request, app);
        Self {
            inner: Rc::new(UnsafeCell::new(inner)),
        }
    }

    //    pub fn request(&self) -> &Request {
    //        &self.inner.borrow().deref().request
    //    }
    //
    //    pub fn request_mut(&mut self) -> &mut Request {
    //        &mut self.inner.borrow_mut().request
    //    }
    //
    //    pub fn response(&self) -> &Response {
    //        &self.inner.borrow().response
    //    }
    //
    //    pub fn response_mut(&mut self) -> &mut Response {
    //        &mut self.inner.borrow_mut().response
    //    }
    //
    //    pub fn app(&self) -> &Service<S> {
    //        &self.inner.borrow().app
    //    }
    //
    //    pub fn state(&self) -> &S {
    //        &self.inner.borrow().state
    //    }
    //
    //    pub fn state_mut(&mut self) -> &mut S {
    //        &mut self.inner.borrow_mut().state
    //    }
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
    pub app: Service<S>,
    pub state: S,
}

impl<S: State> Ctx<S> {
    fn new(request: Request, app: Service<S>) -> Self {
        Self {
            request,
            response: Response::new(),
            app,
            state: Default::default(),
        }
    }
}

pub trait State: 'static + Send + Default {}
impl<T> State for T where T: 'static + Send + Default {}
