use crate::{App, Model, Request, Response};
use futures::lock::{Mutex, MutexGuard};
use http::header::HeaderName;
use http::{HeaderValue, Method, Uri, Version};
use std::net::SocketAddr;
use std::sync::Arc;

pub struct Context<M: Model> {
    pub request: Arc<Mutex<Request>>,
    pub response: Arc<Mutex<Response>>,
    pub app: App<M>,
    pub state: Arc<Mutex<M::State>>,
    pub peer_addr: SocketAddr,
}

impl<M: Model> Context<M> {
    pub fn new(request: Request, app: App<M>, peer_addr: SocketAddr) -> Self {
        let state = app.model.new_state();
        Self {
            request: Arc::new(Mutex::new(request)),
            response: Arc::new(Mutex::new(Response::new())),
            app,
            state: Arc::new(Mutex::new(state)),
            peer_addr,
        }
    }

    pub async fn req<'a>(&'a self) -> MutexGuard<'a, Request> {
        self.request.lock().await
    }

    pub async fn resp<'a>(&'a self) -> MutexGuard<'a, Response> {
        self.response.lock().await
    }

    pub async fn state<'a>(&'a self) -> MutexGuard<'a, M::State> {
        self.state.lock().await
    }

    pub async fn uri(&self) -> Uri {
        self.req().await.uri.clone()
    }

    pub async fn method(&self) -> Method {
        self.req().await.method.clone()
    }

    pub async fn header(&self, name: &HeaderName) -> Option<HeaderValue> {
        self.req()
            .await
            .headers
            .get(name)
            .map(|value| value.clone())
    }

    pub async fn version(&self) -> Version {
        self.req().await.version
    }
}

impl<M: Model> Clone for Context<M> {
    fn clone(&self) -> Self {
        Self {
            request: self.request.clone(),
            response: self.response.clone(),
            app: self.app.clone(),
            state: self.state.clone(),
            peer_addr: self.peer_addr.clone(),
        }
    }
}

impl Context<()> {
    // construct fake Context for test.
    pub fn fake(request: Request) -> Self {
        use std::net::{IpAddr, Ipv4Addr};
        let peer_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        Self::new(request, App::new(()), peer_addr)
    }
}
