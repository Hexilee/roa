use crate::{App, Model, Request, Response};
use async_std::net::SocketAddr;
use async_std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use http::header::{HeaderName, ToStrError};
use http::{HeaderValue, Method, Uri, Version};

pub struct Context<M: Model> {
    pub request: Arc<RwLock<Request>>,
    pub response: Arc<RwLock<Response>>,
    pub app: App<M>,
    pub state: Arc<RwLock<M::State>>,
    pub peer_addr: SocketAddr,
}

impl<M: Model> Context<M> {
    pub fn new(request: Request, app: App<M>, peer_addr: SocketAddr) -> Self {
        let state = app.model.new_state();
        Self {
            request: Arc::new(RwLock::new(request)),
            response: Arc::new(RwLock::new(Response::new())),
            app,
            state: Arc::new(RwLock::new(state)),
            peer_addr,
        }
    }

    pub async fn req<'a>(&'a self) -> RwLockReadGuard<'a, Request> {
        self.request.read().await
    }

    pub async fn resp<'a>(&'a self) -> RwLockReadGuard<'a, Response> {
        self.response.read().await
    }

    pub async fn state<'a>(&'a self) -> RwLockReadGuard<'a, M::State> {
        self.state.read().await
    }

    pub async fn req_mut<'a>(&'a self) -> RwLockWriteGuard<'a, Request> {
        self.request.write().await
    }

    pub async fn resp_mut<'a>(&'a self) -> RwLockWriteGuard<'a, Response> {
        self.response.write().await
    }

    pub async fn state_mut<'a>(&'a self) -> RwLockWriteGuard<'a, M::State> {
        self.state.write().await
    }

    pub async fn uri(&self) -> Uri {
        self.req().await.uri.clone()
    }

    pub async fn method(&self) -> Method {
        self.req().await.method.clone()
    }

    pub async fn header(&self, name: &HeaderName) -> Option<Result<String, ToStrError>> {
        self.req()
            .await
            .headers
            .get(name)
            .map(|value| value.to_str().map(|str| str.to_string()))
    }

    pub async fn header_value(&self, name: &HeaderName) -> Option<HeaderValue> {
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

#[cfg(test)]
impl Context<()> {
    // construct fake Context for test.
    pub fn fake(request: Request) -> Self {
        use std::net::{IpAddr, Ipv4Addr};
        let peer_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        Self::new(request, App::new(()), peer_addr)
    }
}
