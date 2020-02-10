use crate::{AddrStream, App, Error, Model, Request, Response};
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
///         .gate(|ctx, next| async move {
///             info!("{} {}", ctx.method().await, ctx.uri().await);
///             next().await
///         })
///         .end(|ctx| async move {
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
pub struct Context<M: Model> {
    request: Arc<RwLock<Request>>,
    response: Arc<RwLock<Response>>,
    state: Arc<RwLock<M::State>>,
    storage: Arc<RwLock<HashMap<TypeId, Bucket>>>,
    stream: AddrStream,

    /// The global Application.
    pub app: App<M>,
}

/// A wrapper of `HashMap<String, String>`, method `get` return a `Variable`.
///
/// ### Example
/// ```rust
/// use roa_core::{Bucket, Variable};
/// let mut bucket = Bucket::new();
/// assert!(bucket.get("id").is_none());
/// assert!(bucket.insert("id", "1".to_string()).is_none());
/// assert_eq!(1, bucket.get("id").unwrap().parse().unwrap());
/// assert_eq!(1, bucket.insert("id", "2".to_string()).unwrap().parse().unwrap());
/// ```
#[derive(Debug, Clone)]
pub struct Bucket(HashMap<String, String>);

/// A wrapper of String.
///
/// ### Example
/// ```rust
/// use roa_core::Variable;
/// use http::StatusCode;
/// assert_eq!(1, Variable::new("id", "1".to_string()).parse().unwrap());
/// let result = Variable::new("id", "x".to_string()).parse::<usize>();
/// assert!(result.is_err());
/// let status = result.unwrap_err();
/// assert_eq!(StatusCode::BAD_REQUEST, status.status_code);
/// assert!(status.message.ends_with("type of variable `id` should be usize"));
/// ```
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
    /// Construct a variable from name and value.
    #[inline]
    pub fn new(name: &'a str, value: String) -> Self {
        Self { name, value }
    }

    /// A wrapper of `str::parse`. Converts `T::FromStr::Err` to `Status` automatically.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::Variable;
    /// use http::StatusCode;
    /// let result = Variable::new("id", "x".to_string()).parse::<usize>();
    /// assert!(result.is_err());
    /// let status = result.unwrap_err();
    /// assert_eq!(StatusCode::BAD_REQUEST, status.status_code);
    /// assert!(status.message.ends_with("type of variable `id` should be usize"));
    /// ```
    pub fn parse<T>(&self) -> Result<T, Error>
    where
        T: FromStr,
        T::Err: Display,
    {
        self.as_ref().parse().map_err(|err| {
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

    /// Into inner value.
    #[inline]
    pub fn into_value(self) -> String {
        self.value
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
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::{Bucket, Variable};
    /// let mut bucket = Bucket::new();
    /// assert!(bucket.insert("id", "1".to_string()).is_none());
    /// assert_eq!(1, bucket.insert("id", "2".to_string()).unwrap().parse().unwrap());
    /// ```
    #[inline]
    pub fn insert<'a>(&mut self, name: &'a str, value: String) -> Option<Variable<'a>> {
        self.0
            .insert(name.to_string(), value)
            .map(|value| Variable::new(name, value))
    }

    /// If the bucket did not have this key present, [`None`] is returned.
    ///
    /// If the bucket did have this key present, the key-value pair will be returned as a `Variable`
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::{Bucket, Variable};
    /// let mut bucket = Bucket::new();
    /// assert!(bucket.get("id").is_none());
    /// bucket.insert("id", "1".to_string());
    /// assert_eq!(1, bucket.get("id").unwrap().parse().unwrap());
    /// ```
    #[inline]
    pub fn get<'a>(&self, name: &'a str) -> Option<Variable<'a>> {
        self.0.get(name).map(|value| Variable {
            name,
            value: value.to_string(),
        })
    }
}

impl Default for Bucket {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: Model> Context<M> {
    /// Construct a context from a request, an app and a addr_stream.  
    pub(crate) fn new(request: Request, app: App<M>, stream: AddrStream) -> Self {
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
    /// use roa_core::{App, Model};
    /// use log::info;
    /// use async_std::task::spawn;
    /// use http::StatusCode;
    ///
    /// struct AppModel {
    ///     default_id: u64,
    /// }
    ///
    /// struct AppState {
    ///     id: u64,
    /// }
    ///
    /// impl AppModel {
    ///     fn new() -> Self {
    ///         Self {
    ///             default_id: 0,
    ///         }
    ///     }
    /// }
    ///
    /// impl Model for AppModel {
    ///     type State = AppState;
    ///     fn new_state(&self) -> Self::State {
    ///         AppState {
    ///             id: self.default_id,
    ///         }
    ///     }
    /// }
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let (addr, server) = App::new(AppModel::new())
    ///         .gate(|ctx, next| async move {
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
    pub async fn state(&self) -> RwLockReadGuard<'_, M::State> {
        self.state.read().await
    }

    /// Get an immutable reference of storage.
    #[inline]
    pub(crate) async fn storage(&self) -> RwLockReadGuard<'_, HashMap<TypeId, Bucket>> {
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
    ///         .gate(|ctx, next| async move {
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
    pub async fn req_mut(&self) -> RwLockWriteGuard<'_, Request> {
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
    ///         .end(|ctx| async move {
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
    pub async fn resp_mut(&self) -> RwLockWriteGuard<'_, Response> {
        self.response.write().await
    }

    /// Get a mutable reference of state.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::{App, Model};
    /// use log::info;
    /// use async_std::task::spawn;
    /// use http::StatusCode;
    ///
    /// struct AppModel {
    ///     default_id: u64,
    /// }
    ///
    /// struct AppState {
    ///     id: u64,
    /// }
    ///
    /// impl AppModel {
    ///     fn new() -> Self {
    ///         Self {
    ///             default_id: 0,
    ///         }
    ///     }
    /// }
    ///
    /// impl Model for AppModel {
    ///     type State = AppState;
    ///     fn new_state(&self) -> Self::State {
    ///         AppState {
    ///             id: self.default_id,
    ///         }
    ///     }
    /// }
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let (addr, server) = App::new(AppModel::new())
    ///         .gate(|ctx, next| async move {
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
    pub async fn state_mut(&self) -> RwLockWriteGuard<'_, M::State> {
        self.state.write().await
    }

    /// Get a mutable reference of storage.
    #[inline]
    pub(crate) async fn storage_mut(&self) -> RwLockWriteGuard<'_, HashMap<TypeId, Bucket>> {
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
    pub async fn header(&self, name: &HeaderName) -> Option<Result<String, ToStrError>> {
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
    pub async fn header_value(&self, name: &HeaderName) -> Option<HeaderValue> {
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

    /// Store key-value pair. Each type has its namespace.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::App;
    /// use async_std::task::spawn;
    /// use http::{StatusCode, Method};
    ///
    /// struct Symbol;
    /// struct AnotherSymbol;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let (addr, server) = App::new(())
    ///         .gate(|ctx, next| async move {
    ///             ctx.store::<Symbol>("id", "1".to_string()).await;
    ///             next().await
    ///         })
    ///         .end(|ctx| async move {
    ///             assert_eq!(1, ctx.load::<Symbol>("id").await.unwrap().parse::<i32>()?);
    ///             assert!(ctx.load::<AnotherSymbol>("id").await.is_none());
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
                let mut bucket = Bucket::default();
                bucket.insert(name, value);
                storage.insert(id, bucket);
                None
            }
        }
    }

    /// Search for value by key.
    ///
    /// ### Example
    ///
    /// ```rust
    /// use roa_core::App;
    /// use async_std::task::spawn;
    /// use http::{StatusCode, Method};
    ///
    /// struct Symbol;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let (addr, server) = App::new(())
    ///         .gate(|ctx, next| async move {
    ///             ctx.store::<Symbol>("id", "1".to_string()).await;
    ///             next().await
    ///         })
    ///         .end(|ctx| async move {
    ///             assert_eq!(1, ctx.load::<Symbol>("id").await.unwrap().parse::<i32>()?);
    ///             Ok(())
    ///         })
    ///         .run_local()?;
    ///     spawn(server);
    ///     let resp = reqwest::get(&format!("http://{}/path", addr)).await?;
    ///     assert_eq!(StatusCode::OK, resp.status());
    ///     Ok(())
    /// }
    /// ```
    ///
    /// ### Parse fails
    ///
    /// The loaded value can be parsed as str, and return a 400 BAD REQUEST Error if fails.
    ///
    /// ```rust
    /// use roa_core::App;
    /// use async_std::task::spawn;
    /// use http::{StatusCode, Method};
    ///
    /// struct Symbol;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let (addr, server) = App::new(())
    ///         .gate(|ctx, next| async move {
    ///             ctx.store::<Symbol>("id", "x".to_string()).await;
    ///             next().await
    ///         })
    ///         .end(|ctx| async move {
    ///             assert_eq!(1, ctx.load::<Symbol>("id").await.unwrap().parse::<i32>()?);
    ///             Ok(())
    ///         })
    ///         .run_local()?;
    ///     spawn(server);
    ///     let resp = reqwest::get(&format!("http://{}/path", addr)).await?;
    ///     assert_eq!(StatusCode::BAD_REQUEST, resp.status());
    ///     Ok(())
    /// }
    /// ```
    #[allow(clippy::needless_lifetimes)]
    pub async fn load<'a, T: 'static>(&self, name: &'a str) -> Option<Variable<'a>> {
        let storage = self.storage().await;
        let id = TypeId::of::<T>();
        storage.get(&id).and_then(|bucket| bucket.get(name))
    }

    /// Get remote socket addr.
    pub fn remote_addr(&self) -> SocketAddr {
        self.stream.remote_addr()
    }

    /// Get reference of raw async_std::net::TcpStream.
    /// This method is dangerous, it's reserved for special scene like websocket.
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
            .end(|ctx| async move {
                assert_eq!(1, ctx.state().await.data);
                Ok(())
            })
            .run_local()?;
        spawn(server);
        reqwest::get(&format!("http://{}", addr)).await?;
        Ok(())
    }
}
