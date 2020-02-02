use crate::{App, Model, Request, Response, Status};
use async_std::net::SocketAddr;
use async_std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use http::header::{HeaderName, ToStrError};
use http::StatusCode;
use http::{HeaderValue, Method, Uri, Version};
use std::any::TypeId;
use std::collections::HashMap;
use std::convert::AsRef;
use std::fmt::Display;
use std::ops::Deref;
use std::str::FromStr;

pub struct Context<M: Model> {
    request: Arc<RwLock<Request>>,
    response: Arc<RwLock<Response>>,
    state: Arc<RwLock<M::State>>,
    storage: Arc<RwLock<HashMap<TypeId, Bucket>>>,
    pub app: App<M>,
    pub peer_addr: SocketAddr,
}

#[derive(Debug, Clone)]
pub struct Bucket(HashMap<String, String>);

#[derive(Debug, Clone)]
pub struct Variable<'a> {
    name: &'a str,
    value: String,
}

impl Deref for Variable<'_> {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl AsRef<str> for Variable<'_> {
    fn as_ref(&self) -> &str {
        &self
    }
}

impl<'a> Variable<'a> {
    pub fn new(name: &'a str, value: String) -> Self {
        Self { name, value }
    }

    pub fn parse<T>(&self) -> Result<T, Status>
    where
        T: FromStr,
        T::Err: Display,
    {
        self.as_ref().parse().map_err(|err| {
            Status::new(
                StatusCode::BAD_REQUEST,
                format!(
                    "{}\ntype of variable `{}` should be {}",
                    err,
                    self.name,
                    std::any::type_name::<T>()
                ),
                true,
            )
        })
    }

    pub fn into_value(self) -> String {
        self.value
    }
}

impl Bucket {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn insert<'a>(&mut self, name: &'a str, value: String) -> Option<Variable<'a>> {
        self.0
            .insert(name.to_string(), value)
            .map(|value| Variable::new(name, value))
    }

    pub fn get<'a>(&mut self, name: &'a str) -> Option<Variable<'a>> {
        self.0.get(name).map(|value| Variable {
            name,
            value: value.to_string(),
        })
    }
}

impl<M: Model> Context<M> {
    pub fn new(request: Request, app: App<M>, peer_addr: SocketAddr) -> Self {
        let state = app.model.new_state();
        Self {
            request: Arc::new(RwLock::new(request)),
            response: Arc::new(RwLock::new(Response::new())),
            state: Arc::new(RwLock::new(state)),
            storage: Arc::new(RwLock::new(HashMap::new())),
            app,
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

    pub async fn storage<'a>(&'a self) -> RwLockReadGuard<'a, HashMap<TypeId, Bucket>> {
        self.storage.read().await
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

    pub async fn storage_mut<'a>(&'a self) -> RwLockWriteGuard<'a, HashMap<TypeId, Bucket>> {
        self.storage.write().await
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

    pub async fn store<'a, T: 'static>(
        &self,
        name: &'a str,
        value: String,
    ) -> Option<Variable<'a>> {
        let mut storage = self.storage_mut().await;
        let id = TypeId::of::<T>();
        match storage.get_mut(&id) {
            Some(bucket) => bucket.insert(name, value),
            None => {
                let mut bucket = Bucket::new();
                bucket.insert(name, value);
                storage.insert(id, bucket);
                None
            }
        }
    }

    pub async fn load<'a, T: 'static>(&self, name: &'a str) -> Option<Variable<'a>> {
        let mut storage = self.storage_mut().await;
        let id = TypeId::of::<T>();
        storage.get_mut(&id).and_then(|bucket| bucket.get(name))
    }
}

impl<M: Model> Clone for Context<M> {
    fn clone(&self) -> Self {
        Self {
            request: self.request.clone(),
            response: self.response.clone(),
            state: self.state.clone(),
            storage: self.storage.clone(),
            app: self.app.clone(),
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
