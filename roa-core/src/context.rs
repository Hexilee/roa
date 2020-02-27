use crate::{app::Executor, Error, Request, Response};
use http::header::{AsHeaderName, ToStrError};
use http::StatusCode;
use http::{Method, Uri, Version};
use std::any::Any;
use std::any::TypeId;
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::fmt::Display;
use std::future::Future;
use std::net::SocketAddr;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::str::FromStr;
use std::sync::Arc;

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
/// let mut app = App::new(());
/// app.gate_fn(|ctx, next| async move {
///     info!("{} {}", ctx.method(), ctx.uri());
///     next.await
/// });
/// app.end(|mut ctx| async move {
///     ctx.resp_mut().write(File::open("assets/welcome.html").await?);
///     Ok(())
/// });
/// ```
pub struct Context<S>(Rc<UnsafeCell<Inner<S>>>);

struct Inner<S> {
    request: Request,
    response: Response,
    state: S,
    exec: Executor,
    storage: HashMap<TypeId, Bucket>,
    remote_addr: SocketAddr,
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
    pub(crate) fn new(
        request: Request,
        state: S,
        exec: Executor,
        remote_addr: SocketAddr,
    ) -> Self {
        let inner = Inner {
            request,
            response: Response::new(),
            state,
            exec,
            storage: HashMap::new(),
            remote_addr,
        };
        Self(Rc::new(UnsafeCell::new(inner)))
    }

    // clone context is unsafe
    pub(crate) unsafe fn unsafe_clone(&self) -> Self {
        Self(self.0.clone())
    }

    fn inner(&self) -> &Inner<S> {
        unsafe { &*self.0.get() }
    }

    fn inner_mut(&mut self) -> &mut Inner<S> {
        unsafe { &mut *self.0.get() }
    }

    /// Get an immutable reference of request.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::App;
    /// use roa_core::http::Method;
    ///
    /// let mut app = App::new(());
    /// app.end(|ctx| async move {
    ///     assert_eq!(Method::GET, ctx.req().method);
    ///     Ok(())
    /// });
    /// ```
    #[inline]
    pub fn req(&self) -> &Request {
        &self.inner().request
    }

    /// Get an immutable reference of response.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::App;
    /// use roa_core::http::StatusCode;
    ///
    /// let mut app = App::new(());
    /// app.end(|ctx| async move {
    ///     assert_eq!(StatusCode::OK, ctx.resp().status);
    ///     Ok(())
    /// });
    /// ```
    #[inline]
    pub fn resp(&self) -> &Response {
        &self.inner().response
    }

    /// Get an immutable reference of state.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::App;
    ///
    /// #[derive(Clone)]
    /// struct State {
    ///     id: u64,
    /// }
    ///
    /// let mut app = App::new(State { id: 0 });
    /// app.end(|ctx| async move {
    ///     assert_eq!(0, ctx.state().id);
    ///     Ok(())
    /// });
    /// ```
    #[inline]
    pub fn state(&self) -> &S {
        &self.inner().state
    }

    /// Get an immutable reference of storage.
    #[inline]
    fn storage(&self) -> &HashMap<TypeId, Bucket> {
        &self.inner().storage
    }

    /// Get a mutable reference of request.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::App;
    /// use roa_core::http::Method;
    ///
    /// let mut app = App::new(());
    /// app.gate_fn(|mut ctx, next| async move {
    ///     ctx.req_mut().method = Method::POST;
    ///     next.await
    /// });
    /// app.end(|ctx| async move {
    ///     assert_eq!(Method::POST, ctx.req().method);
    ///     Ok(())
    /// });
    /// ```
    #[inline]
    pub fn req_mut(&mut self) -> &mut Request {
        &mut self.inner_mut().request
    }

    /// Get a mutable reference of response.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::App;
    ///
    /// let mut app = App::new(());
    /// app.end(|mut ctx| async move {
    ///     ctx.resp_mut().write(b"Hello, World!".as_ref());
    ///     Ok(())
    /// });
    /// ```
    #[inline]
    pub fn resp_mut(&mut self) -> &mut Response {
        &mut self.inner_mut().response
    }

    /// Get a mutable reference of state.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::App;
    ///
    /// #[derive(Clone)]
    /// struct State {
    ///     id: u64,
    /// }
    ///
    /// let mut app = App::new(State { id: 0 });
    /// app.gate_fn(|mut ctx, next| async move {
    ///     ctx.state_mut().id = 1;
    ///     next.await
    /// });
    /// app.end(|ctx| async move {
    ///     assert_eq!(1, ctx.state().id);
    ///     Ok(())
    /// });
    /// ```
    #[inline]
    pub fn state_mut(&mut self) -> &mut S {
        &mut self.inner_mut().state
    }

    /// Get a mutable reference of storage.
    #[inline]
    fn storage_mut(&mut self) -> &mut HashMap<TypeId, Bucket> {
        &mut self.inner_mut().storage
    }

    /// Clone URI.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::App;
    ///
    /// let mut app = App::new(());
    /// app.end(|ctx| async move {
    ///     assert_eq!("/", ctx.uri().to_string());
    ///     Ok(())
    /// });
    /// ```
    pub fn uri(&self) -> &Uri {
        &self.req().uri
    }

    /// Clone request::method.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::App;
    /// use http::Method;
    ///
    /// let mut app = App::new(());
    /// app.end(|ctx| async move {
    ///     assert_eq!(Method::GET, ctx.method());
    ///     Ok(())
    /// });
    /// ```
    pub fn method(&self) -> &Method {
        &self.req().method
    }

    /// Search for a header value and try to get its string copy.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::App;
    /// use roa_core::http::{StatusCode, header};
    ///
    /// let mut app = App::new(());
    /// app.end(|ctx| async move {
    ///     assert_eq!(
    ///         "text/plain",
    ///         ctx.header(&header::CONTENT_TYPE).unwrap().unwrap()
    ///     );
    ///     Ok(())
    /// });
    /// ```
    pub fn header(&self, name: impl AsHeaderName) -> Option<Result<&str, ToStrError>> {
        self.req().headers.get(name).map(|value| value.to_str())
    }

    /// Clone response::status.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::{App, http::StatusCode};
    ///
    /// let mut app = App::new(());
    /// app.end(|ctx| async move {
    ///     assert_eq!(StatusCode::OK, ctx.status());
    ///     Ok(())
    /// });
    /// ```
    pub fn status(&self) -> StatusCode {
        self.resp().status
    }

    /// Clone request::version.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::App;
    /// use roa_core::http::{StatusCode, Version};
    ///
    /// let mut app = App::new(());
    /// app.end(|ctx| async move {
    ///     assert_eq!(Version::HTTP_11, ctx.version());
    ///     Ok(())
    /// });
    /// ```
    pub fn version(&self) -> Version {
        self.req().version
    }

    /// Store key-value pair in specific scope.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::App;
    /// use roa_core::http::{StatusCode, Method};
    ///
    /// struct Scope;
    /// struct AnotherScope;
    ///
    /// let mut app = App::new(());
    /// app.gate_fn(|mut ctx, next| async move {
    ///     ctx.store_scoped(Scope, "id", "1".to_string());
    ///     next.await
    /// });
    /// app.end(|ctx| async move {
    ///     assert_eq!(1, ctx.load_scoped::<Scope, String>("id").unwrap().parse::<i32>()?);
    ///     assert!(ctx.load_scoped::<AnotherScope, String>("id").is_none());
    ///     Ok(())
    /// });
    /// ```
    pub fn store_scoped<'a, SC, T>(
        &mut self,
        _scope: SC,
        name: &'a str,
        value: T,
    ) -> Option<Variable<'a, T>>
    where
        SC: Any,
        T: Any + Send + Sync,
    {
        let storage = self.storage_mut();
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
    /// use roa_core::http::{StatusCode, Method};
    ///
    /// let mut app = App::new(());
    /// app.gate_fn(|mut ctx, next| async move {
    ///     ctx.store("id", "1".to_string());
    ///     next.await
    /// });
    /// app.end(|ctx| async move {
    ///     assert_eq!(1, ctx.load::<String>("id").unwrap().parse::<i32>()?);
    ///     Ok(())
    /// });
    /// ```
    pub fn store<'a, T>(&mut self, name: &'a str, value: T) -> Option<Variable<'a, T>>
    where
        T: Any + Send + Sync,
    {
        self.store_scoped(PublicScope, name, value)
    }

    /// Search for value by key in specific scope.
    ///
    /// ### Example
    ///
    /// ```rust
    /// use roa_core::App;
    ///
    /// struct Scope;
    ///
    /// let mut app = App::new(());
    /// app.gate_fn(|mut ctx, next| async move {
    ///     ctx.store_scoped(Scope, "id", "1".to_owned());
    ///     next.await
    /// });
    /// app.end(|ctx| async move {
    ///     assert_eq!(1, ctx.load_scoped::<Scope, String>("id").unwrap().parse::<i32>()?);
    ///     Ok(())
    /// });
    /// ```
    pub fn load_scoped<'a, SC, T>(&self, name: &'a str) -> Option<Variable<'a, T>>
    where
        SC: Any,
        T: Any + Send + Sync,
    {
        let storage = self.storage();
        let id = TypeId::of::<SC>();
        storage.get(&id).and_then(|bucket| bucket.get(name))
    }

    /// Search for value by key in public scope.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::App;
    ///
    /// let mut app = App::new(());
    /// app.gate_fn(|mut ctx, next| async move {
    ///     ctx.store("id", "1".to_string());
    ///     next.await
    /// });
    /// app.end(|ctx| async move {
    ///     assert_eq!(1, ctx.load::<String>("id").unwrap().parse::<i32>()?);
    ///     Ok(())
    /// });
    /// ```
    pub fn load<'a, T>(&self, name: &'a str) -> Option<Variable<'a, T>>
    where
        T: Any + Send + Sync,
    {
        self.load_scoped::<PublicScope, T>(name)
    }

    /// Get remote socket addr.
    pub fn remote_addr(&self) -> SocketAddr {
        self.inner().remote_addr
    }

    /// Spawn a future by app runtime
    pub fn spawn(&self, fut: impl 'static + Send + Future) {
        self.inner().exec.0.spawn(Box::pin(async move {
            fut.await;
        }))
    }

    /// Spawn a blocking task by app runtime
    pub fn spawn_blocking(&self, task: impl 'static + Send + FnOnce()) {
        self.inner().exec.0.spawn_blocking(Box::new(task))
    }
}

impl<S> Deref for Context<S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        self.state()
    }
}

impl<S> DerefMut for Context<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.state_mut()
    }
}

#[cfg(all(test, feature = "runtime"))]
mod tests_with_runtime {
    use crate::{App, Context, Request};
    use http::{StatusCode, Version};

    #[async_std::test]
    async fn status_and_version() -> Result<(), Box<dyn std::error::Error>> {
        let service = App::new(())
            .end(|ctx| async move {
                assert_eq!(Version::HTTP_11, ctx.version());
                assert_eq!(StatusCode::OK, ctx.status());
                Ok(())
            })
            .fake_service();
        service.serve(Request::default()).await?;
        Ok(())
    }

    #[derive(Clone)]
    struct State {
        data: usize,
    }

    #[async_std::test]
    async fn state_mut() -> Result<(), Box<dyn std::error::Error>> {
        let service = App::new(State { data: 1 })
            .gate_fn(|mut ctx, next| async move {
                ctx.data = 1;
                next.await
            })
            .end(|ctx: Context<State>| async move {
                assert_eq!(1, ctx.state().data);
                Ok(())
            })
            .fake_service();
        service.serve(Request::default()).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Bucket, Variable};
    use http::StatusCode;
    use std::sync::Arc;

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
