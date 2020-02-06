use crate::{AddrStream, App, Model, Request, Response, Status};
use async_std::net::{SocketAddr, TcpStream};
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
    stream: AddrStream,
    pub app: App<M>,
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

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl AsRef<str> for Variable<'_> {
    #[inline]
    fn as_ref(&self) -> &str {
        &self
    }
}

impl<'a> Variable<'a> {
    #[inline]
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

    #[inline]
    pub fn into_value(self) -> String {
        self.value
    }
}

impl Bucket {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    #[inline]
    pub fn insert<'a>(&mut self, name: &'a str, value: String) -> Option<Variable<'a>> {
        self.0
            .insert(name.to_string(), value)
            .map(|value| Variable::new(name, value))
    }

    #[inline]
    pub fn get<'a>(&self, name: &'a str) -> Option<Variable<'a>> {
        self.0.get(name).map(|value| Variable {
            name,
            value: value.to_string(),
        })
    }
}

impl<M: Model> Context<M> {
    pub fn new(request: Request, app: App<M>, stream: AddrStream) -> Self {
        let state = app.model.new_state();
        Self {
            request: Arc::new(RwLock::new(request)),
            response: Arc::new(RwLock::new(Response::new())),
            state: Arc::new(RwLock::new(state)),
            storage: Arc::new(RwLock::new(HashMap::new())),
            app,
            stream,
        }
    }

    #[inline]
    pub async fn req(&self) -> RwLockReadGuard<'_, Request> {
        self.request.read().await
    }

    #[inline]
    pub async fn resp(&self) -> RwLockReadGuard<'_, Response> {
        self.response.read().await
    }

    #[inline]
    pub async fn state(&self) -> RwLockReadGuard<'_, M::State> {
        self.state.read().await
    }

    #[inline]
    pub async fn storage(&self) -> RwLockReadGuard<'_, HashMap<TypeId, Bucket>> {
        self.storage.read().await
    }

    #[inline]
    pub async fn req_mut(&self) -> RwLockWriteGuard<'_, Request> {
        self.request.write().await
    }

    #[inline]
    pub async fn resp_mut(&self) -> RwLockWriteGuard<'_, Response> {
        self.response.write().await
    }

    #[inline]
    pub async fn state_mut(&self) -> RwLockWriteGuard<'_, M::State> {
        self.state.write().await
    }

    #[inline]
    pub async fn storage_mut(&self) -> RwLockWriteGuard<'_, HashMap<TypeId, Bucket>> {
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

    pub async fn status(&self) -> StatusCode {
        self.resp().await.status
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
        let storage = self.storage().await;
        let id = TypeId::of::<T>();
        storage.get(&id).and_then(|bucket| bucket.get(name))
    }

    pub fn remote_addr(&self) -> SocketAddr {
        self.stream.remote_addr()
    }

    pub fn raw_stream(&self) -> &TcpStream {
        self.stream.stream()
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
            stream: self.stream.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{App, Model};
    use async_std::task::spawn;
    use http::{StatusCode, Version};

    #[tokio::test]
    async fn status_and_version() -> Result<(), Box<dyn std::error::Error>> {
        let (addr, server) = App::new(())
            .gate(|ctx, _next| async move {
                assert_eq!(Version::HTTP_11, ctx.version().await);
                assert_eq!(StatusCode::OK, ctx.status().await);
                Ok(())
            })
            .run_local()?;
        spawn(server);
        reqwest::get(&format!("http://{}", addr)).await?;
        Ok(())
    }

    struct AppModel;
    struct AppState {
        data: usize,
    }
    impl Model for AppModel {
        type State = AppState;
        fn new_state(&self) -> Self::State {
            AppState { data: 0 }
        }
    }

    #[tokio::test]
    async fn state_mut() -> Result<(), Box<dyn std::error::Error>> {
        let (addr, server) = App::new(AppModel {})
            .gate(|ctx, next| async move {
                ctx.state_mut().await.data = 1;
                next().await
            })
            .gate(|ctx, _next| async move {
                assert_eq!(1, ctx.state().await.data);
                Ok(())
            })
            .run_local()?;
        spawn(server);
        reqwest::get(&format!("http://{}", addr)).await?;
        Ok(())
    }
}
