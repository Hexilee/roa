use crate::{AddrStream, Error, Request, Response};
use async_std::net::{SocketAddr, TcpStream};
use async_std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use http::header::{AsHeaderName, ToStrError};
use http::StatusCode;
use http::{HeaderValue, Method, Uri, Version};
use std::any::Any;
use std::any::TypeId;
use std::collections::HashMap;
use std::fmt::Display;
use std::ops::Deref;
use std::str::FromStr;

struct PublicScope;

/// A structure to share request, response and other data between middlewares.
///
/// Type of the first parameter in a middleware.
///
/// ### Example
///
/// ```rust
/// use roa_core::App;
/// use log::info;
/// use async_std::fs::File;
///
/// #[async_std::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let server = App::new(())
///         .gate_fn(|ctx, next| async move {
///             info!("{} {}", ctx.method().await, ctx.uri().await);
///             next().await
///         })
///         .end(|mut ctx| async move {
///             ctx.resp_mut().await.write(File::open("assets/welcome.html").await?);
///             Ok(())
///         })
///         .listen("127.0.0.1:8000", |addr| {
///             info!("Server is listening on {}", addr)
///         })?;
///     // server.await;
///     Ok(())
/// }
/// ```
pub struct Context<S> {
    request: Arc<RwLock<Request>>,
    response: Arc<RwLock<Response>>,
    state: Arc<RwLock<S>>,
    storage: Arc<RwLock<HashMap<TypeId, Bucket>>>,
    stream: AddrStream,
}

/// A wrapper of `HashMap<String, Arc<dyn Any + Send + Sync>>`, method `get` return a `Variable`.
#[derive(Debug, Clone)]
struct Bucket(HashMap<String, Arc<dyn Any + Send + Sync>>);

/// A wrapper of Arc<T>.
#[derive(Debug, Clone)]
pub struct Variable<'a, T> {
    name: &'a str,
    value: Arc<T>,
}

impl<T> Deref for Variable<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &*self.value
    }
}

impl<'a, T> Variable<'a, T> {
    /// Construct a variable from name and value.
    #[inline]
    fn new(name: &'a str, value: Arc<T>) -> Self {
        Self { name, value }
    }

    /// Into inner value.
    #[inline]
    pub fn value(&self) -> Arc<T> {
        self.value.clone()
    }
}

impl Variable<'_, String> {
    /// A wrapper of `str::parse`. Converts `T::FromStr::Err` to `Status` automatically.
    pub fn parse<T>(&self) -> Result<T, Error>
    where
        T: FromStr,
        T::Err: Display,
    {
        self.deref().parse().map_err(|err| {
            Error::new(
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
}

impl Bucket {
    /// Construct an empty Bucket.
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    /// Inserts a key-value pair into the bucket.
    ///
    /// If the bucket did not have this key present, [`None`] is returned.
    ///
    /// If the bucket did have this key present, the value is updated, and the old
    /// value is returned.
    #[inline]
    pub fn insert<'a, T: Any + Send + Sync>(
        &mut self,
        name: &'a str,
        value: T,
    ) -> Option<Variable<'a, T>> {
        self.0
            .insert(name.to_string(), Arc::new(value))
            .and_then(|value| value.downcast().ok())
            .map(|value| Variable::new(name, value))
    }

    /// If the bucket did not have this key present, [`None`] is returned.
    ///
    /// If the bucket did have this key present, the key-value pair will be returned as a `Variable`
    #[inline]
    pub fn get<'a, T: Any + Send + Sync>(
        &self,
        name: &'a str,
    ) -> Option<Variable<'a, T>> {
        self.0.get(name).and_then(|value| {
            Some(Variable {
                name,
                value: value.clone().downcast().ok()?,
            })
        })
    }
}

impl Default for Bucket {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> Context<S> {
    /// Construct a context from a request, an app and a addr_stream.  
    pub(crate) fn new(request: Request, state: S, stream: AddrStream) -> Self {
        Self {
            request: Arc::new(RwLock::new(request)),
            response: Arc::new(RwLock::new(Response::new())),
            state: Arc::new(RwLock::new(state)),
            storage: Arc::new(RwLock::new(HashMap::new())),
            stream,
        }
    }

    /// Get an immutable reference of request.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::App;
    /// use async_std::task::spawn;
    /// use http::{StatusCode, Method};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let (addr, server) = App::new(())
    ///         .end(|ctx| async move {
    ///             assert_eq!(Method::GET, ctx.req().await.method);
    ///             Ok(())
    ///         })
    ///         .run_local()?;
    ///     spawn(server);
    ///     let resp = reqwest::get(&format!("http://{}", addr)).await?;
    ///     assert_eq!(StatusCode::OK, resp.status());
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub async fn req(&self) -> RwLockReadGuard<'_, Request> {
        self.request.read().await
    }

    /// Get an immutable reference of response.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::App;
    /// use async_std::task::spawn;
    /// use http::StatusCode;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let (addr, server) = App::new(())
    ///         .end(|ctx| async move {
    ///             assert_eq!(StatusCode::OK, ctx.resp().await.status);
    ///             Ok(())
    ///         })
    ///         .run_local()?;
    ///     spawn(server);
    ///     let resp = reqwest::get(&format!("http://{}", addr)).await?;
    ///     assert_eq!(StatusCode::OK, resp.status());
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub async fn resp(&self) -> RwLockReadGuard<'_, Response> {
        self.response.read().await
    }

    /// Get an immutable reference of state.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::App;
    /// use log::info;
    /// use async_std::task::spawn;
    /// use http::StatusCode;
    ///
    /// #[derive(Clone)]
    /// struct State {
    ///     id: u64,
    /// }
    ///
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let (addr, server) = App::new(State { id: 0 })
    ///         .gate_fn(|mut ctx, next| async move {
    ///             ctx.state_mut().await.id = 1;
    ///             next().await
    ///         })
    ///         .end(|ctx| async move {
    ///             let id = ctx.state().await.id;
    ///             assert_eq!(1, id);
    ///             Ok(())
    ///         })
    ///         .run_local()?;
    ///     spawn(server);
    ///     let resp = reqwest::get(&format!("http://{}", addr)).await?;
    ///     assert_eq!(StatusCode::OK, resp.status());
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub async fn state(&self) -> RwLockReadGuard<'_, S> {
        self.state.read().await
    }

    /// Get an immutable reference of storage.
    #[inline]
    async fn storage(&self) -> RwLockReadGuard<'_, HashMap<TypeId, Bucket>> {
        self.storage.read().await
    }

    /// Get a mutable reference of request.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::App;
    /// use async_std::task::spawn;
    /// use http::{StatusCode, Method};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let (addr, server) = App::new(())
    ///         .gate_fn(|mut ctx, next| async move {
    ///             ctx.req_mut().await.method = Method::POST;
    ///             next().await
    ///         })
    ///         .end(|ctx| async move {
    ///             assert_eq!(Method::POST, ctx.req().await.method);
    ///             Ok(())
    ///         })
    ///         .run_local()?;
    ///     spawn(server);
    ///     let resp = reqwest::get(&format!("http://{}", addr)).await?;
    ///     assert_eq!(StatusCode::OK, resp.status());
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub async fn req_mut(&mut self) -> RwLockWriteGuard<'_, Request> {
        self.request.write().await
    }

    /// Get a mutable reference of response.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::App;
    /// use async_std::task::spawn;
    /// use http::StatusCode;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let (addr, server) = App::new(())
    ///         .end(|mut ctx| async move {
    ///             ctx.resp_mut().await.write_buf(b"Hello, World!".as_ref());
    ///             Ok(())
    ///         })
    ///         .run_local()?;
    ///     spawn(server);
    ///     let resp = reqwest::get(&format!("http://{}", addr)).await?;
    ///     assert_eq!(StatusCode::OK, resp.status());
    ///     assert_eq!("Hello, World!", resp.text().await?);
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub async fn resp_mut(&mut self) -> RwLockWriteGuard<'_, Response> {
        self.response.write().await
    }

    /// Get a mutable reference of state.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::App;
    /// use log::info;
    /// use async_std::task::spawn;
    /// use http::StatusCode;
    ///
    /// #[derive(Clone)]
    /// struct State {
    ///     id: u64,
    /// }
    ///
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let (addr, server) = App::new(State { id: 0 })
    ///         .gate_fn(|mut ctx, next| async move {
    ///             ctx.state_mut().await.id = 1;
    ///             next().await
    ///         })
    ///         .end(|ctx| async move {
    ///             let id = ctx.state().await.id;
    ///             assert_eq!(1, id);
    ///             Ok(())
    ///         })
    ///         .run_local()?;
    ///     spawn(server);
    ///     let resp = reqwest::get(&format!("http://{}", addr)).await?;
    ///     assert_eq!(StatusCode::OK, resp.status());
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub async fn state_mut(&mut self) -> RwLockWriteGuard<'_, S> {
        self.state.write().await
    }

    /// Get a mutable reference of storage.
    #[inline]
    async fn storage_mut(&mut self) -> RwLockWriteGuard<'_, HashMap<TypeId, Bucket>> {
        self.storage.write().await
    }

    /// Clone URI.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::App;
    /// use async_std::task::spawn;
    /// use http::{StatusCode, Method};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let (addr, server) = App::new(())
    ///         .end(|ctx| async move {
    ///             assert_eq!("/path", ctx.uri().await.to_string());
    ///             Ok(())
    ///         })
    ///         .run_local()?;
    ///     spawn(server);
    ///     let resp = reqwest::get(&format!("http://{}/path", addr)).await?;
    ///     assert_eq!(StatusCode::OK, resp.status());
    ///     Ok(())
    /// }
    /// ```
    pub async fn uri(&self) -> Uri {
        self.req().await.uri.clone()
    }

    /// Clone request::method.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::App;
    /// use async_std::task::spawn;
    /// use http::{StatusCode, Method};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let (addr, server) = App::new(())
    ///         .end(|ctx| async move {
    ///             assert_eq!(Method::GET, ctx.method().await);
    ///             Ok(())
    ///         })
    ///         .run_local()?;
    ///     spawn(server);
    ///     let resp = reqwest::get(&format!("http://{}/path", addr)).await?;
    ///     assert_eq!(StatusCode::OK, resp.status());
    ///     Ok(())
    /// }
    /// ```
    pub async fn method(&self) -> Method {
        self.req().await.method.clone()
    }

    /// Search for a header value and try to get its string copy.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::App;
    /// use async_std::task::spawn;
    /// use http::{StatusCode, Method, header};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let (addr, server) = App::new(())
    ///         .end(|ctx| async move {
    ///             assert_eq!(
    ///                 "text/plain",
    ///                 ctx.header(&header::CONTENT_TYPE).await.unwrap().unwrap()
    ///             );
    ///             Ok(())
    ///         })
    ///         .run_local()?;
    ///     spawn(server);
    ///     let resp = reqwest::Client::new()
    ///         .get(&format!("http://{}", addr))
    ///         .header(&header::CONTENT_TYPE, "text/plain")
    ///         .send()
    ///         .await?;
    ///     assert_eq!(StatusCode::OK, resp.status());
    ///     Ok(())
    /// }
    /// ```
    pub async fn header(
        &self,
        name: impl AsHeaderName,
    ) -> Option<Result<String, ToStrError>> {
        self.req()
            .await
            .headers
            .get(name)
            .map(|value| value.to_str().map(|str| str.to_string()))
    }

    /// Search for a header value and clone it.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::App;
    /// use async_std::task::spawn;
    /// use http::{StatusCode, Method, header};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let (addr, server) = App::new(())
    ///         .end(|ctx| async move {
    ///             assert_eq!(
    ///                 "text/plain",
    ///                 ctx.header_value(&header::CONTENT_TYPE).await.unwrap().to_str().unwrap()
    ///             );
    ///             Ok(())
    ///         })
    ///         .run_local()?;
    ///     spawn(server);
    ///     let resp = reqwest::Client::new()
    ///         .get(&format!("http://{}", addr))
    ///         .header(&header::CONTENT_TYPE, "text/plain")
    ///         .send()
    ///         .await?;
    ///     assert_eq!(StatusCode::OK, resp.status());
    ///     Ok(())
    /// }
    /// ```
    pub async fn header_value(&self, name: impl AsHeaderName) -> Option<HeaderValue> {
        self.req().await.headers.get(name).cloned()
    }

    /// Clone response::status.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::App;
    /// use async_std::task::spawn;
    /// use http::{StatusCode, Method};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let (addr, server) = App::new(())
    ///         .end(|ctx| async move {
    ///             assert_eq!(StatusCode::OK, ctx.status().await);
    ///             Ok(())
    ///         })
    ///         .run_local()?;
    ///     spawn(server);
    ///     let resp = reqwest::get(&format!("http://{}/path", addr)).await?;
    ///     assert_eq!(StatusCode::OK, resp.status());
    ///     Ok(())
    /// }
    /// ```
    pub async fn status(&self) -> StatusCode {
        self.resp().await.status
    }

    /// Clone request::version.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::App;
    /// use async_std::task::spawn;
    /// use http::{StatusCode, Version};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let (addr, server) = App::new(())
    ///         .end(|ctx| async move {
    ///             assert_eq!(Version::HTTP_11, ctx.version().await);
    ///             Ok(())
    ///         })
    ///         .run_local()?;
    ///     spawn(server);
    ///     let resp = reqwest::get(&format!("http://{}/path", addr)).await?;
    ///     assert_eq!(StatusCode::OK, resp.status());
    ///     Ok(())
    /// }
    /// ```
    pub async fn version(&self) -> Version {
        self.req().await.version
    }

    /// Store key-value pair in specific scope.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::App;
    /// use async_std::task::spawn;
    /// use http::{StatusCode, Method};
    ///
    /// struct Scope;
    /// struct AnotherScope;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let (addr, server) = App::new(())
    ///         .gate_fn(|mut ctx, next| async move {
    ///             ctx.store_scoped(Scope, "id", "1".to_string()).await;
    ///             next().await
    ///         })
    ///         .end(|ctx| async move {
    ///             assert_eq!(1, ctx.load_scoped::<Scope, String>("id").await.unwrap().parse::<i32>()?);
    ///             assert!(ctx.load_scoped::<AnotherScope, String>("id").await.is_none());
    ///             Ok(())
    ///         })
    ///         .run_local()?;
    ///     spawn(server);
    ///     let resp = reqwest::get(&format!("http://{}/path", addr)).await?;
    ///     assert_eq!(StatusCode::OK, resp.status());
    ///     Ok(())
    /// }
    /// ```
    #[allow(clippy::needless_lifetimes)]
    pub async fn store_scoped<'a, SC, T>(
        &mut self,
        _scope: SC,
        name: &'a str,
        value: T,
    ) -> Option<Variable<'a, T>>
    where
        SC: Any,
        T: Any + Send + Sync,
    {
        let mut storage = self.storage_mut().await;
        let id = TypeId::of::<SC>();
        match storage.get_mut(&id) {
            Some(bucket) => bucket.insert(name, value),
            None => {
                let mut bucket = Bucket::default();
                bucket.insert(name, value);
                storage.insert(id, bucket);
                None
            }
        }
    }

    /// Store key-value pair in public scope.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::App;
    /// use async_std::task::spawn;
    /// use http::{StatusCode, Method};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let (addr, server) = App::new(())
    ///         .gate_fn(|mut ctx, next| async move {
    ///             ctx.store("id", "1".to_string()).await;
    ///             next().await
    ///         })
    ///         .end(|ctx| async move {
    ///             assert_eq!(1, ctx.load::<String>("id").await.unwrap().parse::<i32>()?);
    ///             Ok(())
    ///         })
    ///         .run_local()?;
    ///     spawn(server);
    ///     let resp = reqwest::get(&format!("http://{}/path", addr)).await?;
    ///     assert_eq!(StatusCode::OK, resp.status());
    ///     Ok(())
    /// }
    /// ```
    #[allow(clippy::needless_lifetimes)]
    pub async fn store<'a, T>(
        &mut self,
        name: &'a str,
        value: T,
    ) -> Option<Variable<'a, T>>
    where
        T: Any + Send + Sync,
    {
        self.store_scoped(PublicScope, name, value).await
    }

    /// Search for value by key in specific scope.
    ///
    /// ### Example
    ///
    /// ```rust
    /// use roa_core::App;
    /// use async_std::task::spawn;
    /// use http::{StatusCode, Method};
    ///
    /// struct Scope;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let (addr, server) = App::new(())
    ///         .gate_fn(|mut ctx, next| async move {
    ///             ctx.store_scoped(Scope, "id", "1".to_owned()).await;
    ///             next().await
    ///         })
    ///         .end(|ctx| async move {
    ///             assert_eq!(1, ctx.load_scoped::<Scope, String>("id").await.unwrap().parse::<i32>()?);
    ///             Ok(())
    ///         })
    ///         .run_local()?;
    ///     spawn(server);
    ///     let resp = reqwest::get(&format!("http://{}/path", addr)).await?;
    ///     assert_eq!(StatusCode::OK, resp.status());
    ///     Ok(())
    /// }
    /// ```
    #[allow(clippy::needless_lifetimes)]
    pub async fn load_scoped<'a, SC, T>(&self, name: &'a str) -> Option<Variable<'a, T>>
    where
        SC: Any,
        T: Any + Send + Sync,
    {
        let storage = self.storage().await;
        let id = TypeId::of::<SC>();
        storage.get(&id).and_then(|bucket| bucket.get(name))
    }

    /// Search for value by key in public scope.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::App;
    /// use async_std::task::spawn;
    /// use http::{StatusCode, Method};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let (addr, server) = App::new(())
    ///         .gate_fn(|mut ctx, next| async move {
    ///             ctx.store("id", "1".to_string()).await;
    ///             next().await
    ///         })
    ///         .end(|ctx| async move {
    ///             assert_eq!(1, ctx.load::<String>("id").await.unwrap().parse::<i32>()?);
    ///             Ok(())
    ///         })
    ///         .run_local()?;
    ///     spawn(server);
    ///     let resp = reqwest::get(&format!("http://{}/path", addr)).await?;
    ///     assert_eq!(StatusCode::OK, resp.status());
    ///     Ok(())
    /// }
    /// ```
    #[allow(clippy::needless_lifetimes)]
    pub async fn load<'a, T>(&self, name: &'a str) -> Option<Variable<'a, T>>
    where
        T: Any + Send + Sync,
    {
        self.load_scoped::<PublicScope, T>(name).await
    }

    /// Get remote socket addr.
    pub fn remote_addr(&self) -> SocketAddr {
        self.stream.remote_addr()
    }

    /// Get reference of raw async_std::net::TcpStream.
    /// This method is dangerous, it's reserved for special scene like websocket.
    pub fn raw_stream(&self) -> Arc<TcpStream> {
        self.stream.stream()
    }
}

impl<S> Clone for Context<S> {
    fn clone(&self) -> Self {
        Self {
            request: self.request.clone(),
            response: self.response.clone(),
            state: self.state.clone(),
            storage: self.storage.clone(),
            stream: self.stream.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Bucket, Variable};
    use crate::{App, Context};
    use async_std::task::spawn;
    use http::{StatusCode, Version};

    #[tokio::test]
    async fn status_and_version() -> Result<(), Box<dyn std::error::Error>> {
        let (addr, server) = App::new(())
            .end(|ctx| async move {
                assert_eq!(Version::HTTP_11, ctx.version().await);
                assert_eq!(StatusCode::OK, ctx.status().await);
                Ok(())
            })
            .run_local()?;
        spawn(server);
        reqwest::get(&format!("http://{}", addr)).await?;
        Ok(())
    }
    #[derive(Clone)]
    struct State {
        data: usize,
    }

    #[tokio::test]
    async fn state_mut() -> Result<(), Box<dyn std::error::Error>> {
        let (addr, server) = App::new(State { data: 1 })
            .gate_fn(|mut ctx, next| async move {
                ctx.state_mut().await.data = 1;
                next().await
            })
            .end(|ctx: Context<State>| async move {
                assert_eq!(1, ctx.state().await.data);
                Ok(())
            })
            .run_local()?;
        spawn(server);
        reqwest::get(&format!("http://{}", addr)).await?;
        Ok(())
    }

    #[test]
    fn bucket() {
        let mut bucket = Bucket::new();
        assert!(bucket.get::<String>("id").is_none());
        assert!(bucket.insert("id", "1".to_string()).is_none());
        let id: i32 = bucket.get::<String>("id").unwrap().parse().unwrap();
        assert_eq!(1, id);
        assert_eq!(
            1,
            bucket
                .insert("id", "2".to_string())
                .unwrap()
                .parse::<i32>()
                .unwrap()
        );
    }

    #[test]
    fn variable() {
        use std::sync::Arc;
        assert_eq!(
            1,
            Variable::new("id", Arc::new("1".to_string()))
                .parse::<i32>()
                .unwrap()
        );
        let result = Variable::new("id", Arc::new("x".to_string())).parse::<usize>();
        assert!(result.is_err());
        let status = result.unwrap_err();
        assert_eq!(StatusCode::BAD_REQUEST, status.status_code);
        assert!(status
            .message
            .ends_with("type of variable `id` should be usize"));
    }
}
