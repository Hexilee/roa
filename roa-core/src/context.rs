use crate::{Error, Executor, Request, Response};
use http::header::{AsHeaderName, ToStrError};
use http::StatusCode;
use http::{Method, Uri, Version};
use std::any::Any;
use std::any::TypeId;
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::fmt::Display;
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
/// use roa_core::{App, Context, Next, Result, MiddlewareExt};
/// use log::info;
/// use async_std::fs::File;
///
/// let app = App::new((), gate.chain(end));
/// async fn gate(ctx: &mut Context<()>, next: Next<'_>) -> Result {
///     info!("{} {}", ctx.method(), ctx.uri());
///     next.await
/// }
///
/// async fn end(ctx: &mut Context<()>) -> Result {
///     ctx.resp.write_reader(File::open("assets/welcome.html").await?);
///     Ok(())
/// }
/// ```
pub struct Context<S> {
    pub req: Request,
    pub resp: Response,
    pub exec: Executor,

    /// Socket addr of last client or proxy.
    pub remote_addr: SocketAddr,
    state: S,
    storage: HashMap<TypeId, Bucket>,
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
    #[inline]
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
    #[inline]
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
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<S> Context<S> {
    /// Construct a context from a request, an app and a addr_stream.
    #[inline]
    pub(crate) fn new(
        request: Request,
        state: S,
        exec: Executor,
        remote_addr: SocketAddr,
    ) -> Self {
        Self {
            req: request,
            resp: Response::new(),
            state,
            exec,
            storage: HashMap::new(),
            remote_addr,
        }
    }

    /// Clone URI.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::{App, Context, Result};
    ///
    /// let mut app = App::new((), get);
    ///
    /// async fn get(ctx: &mut Context<()>) -> Result {
    ///     assert_eq!("/", ctx.uri().to_string());
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn uri(&self) -> &Uri {
        &self.req.uri
    }

    /// Clone request::method.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::{App, Context, Result};
    /// use roa_core::http::Method;
    ///
    /// let mut app = App::new((), get);
    ///
    /// async fn get(ctx: &mut Context<()>) -> Result {
    ///     assert_eq!(Method::GET, ctx.method());
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn method(&self) -> &Method {
        &self.req.method
    }

    /// Search for a header value and try to get its string copy.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::{App, Context, Result};
    /// use roa_core::http::header::CONTENT_TYPE;
    ///
    /// let mut app = App::new((), get);
    ///
    /// async fn get(ctx: &mut Context<()>) -> Result {
    ///     assert_eq!(
    ///         "text/plain",
    ///         ctx.header(&CONTENT_TYPE).unwrap().unwrap()
    ///     );
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn header(&self, name: impl AsHeaderName) -> Option<Result<&str, ToStrError>> {
        self.req.headers.get(name).map(|value| value.to_str())
    }

    /// Clone response::status.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::{App, Context, Result};
    /// use roa_core::http::StatusCode;
    ///
    /// let mut app = App::new((), get);
    ///
    /// async fn get(ctx: &mut Context<()>) -> Result {
    ///     assert_eq!(StatusCode::OK, ctx.status());
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn status(&self) -> StatusCode {
        self.resp.status
    }

    /// Clone request::version.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::{App, Context, Result};
    /// use roa_core::http::Version;
    ///
    /// let mut app = App::new((), get);
    ///
    /// async fn get(ctx: &mut Context<()>) -> Result {
    ///     assert_eq!(Version::HTTP_11, ctx.version());
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn version(&self) -> Version {
        self.req.version
    }

    /// Store key-value pair in specific scope.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::{App, Context, Next, Result, MiddlewareExt};
    ///
    /// struct Scope;
    /// struct AnotherScope;
    ///
    /// let app = App::new((), gate.chain(end));
    /// async fn gate(ctx: &mut Context<()>, next: Next<'_>) -> Result {
    ///     ctx.store_scoped(Scope, "id", "1".to_string());
    ///     next.await
    /// }
    ///
    /// async fn end(ctx: &mut Context<()>) -> Result {
    ///     assert_eq!(1, ctx.load_scoped::<Scope, String>("id").unwrap().parse::<i32>()?);
    ///     assert!(ctx.load_scoped::<AnotherScope, String>("id").is_none());
    ///     Ok(())
    /// }
    /// ```
    #[inline]
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
        let id = TypeId::of::<SC>();
        match self.storage.get_mut(&id) {
            Some(bucket) => bucket.insert(name, value),
            None => {
                let mut bucket = Bucket::default();
                bucket.insert(name, value);
                self.storage.insert(id, bucket);
                None
            }
        }
    }

    /// Store key-value pair in public scope.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::{App, Context, Next, Result, MiddlewareExt};
    ///
    /// let app = App::new((), gate.chain(end));
    /// async fn gate(ctx: &mut Context<()>, next: Next<'_>) -> Result {
    ///     ctx.store("id", "1".to_string());
    ///     next.await
    /// }
    ///
    /// async fn end(ctx: &mut Context<()>) -> Result {
    ///     assert_eq!(1, ctx.load::<String>("id").unwrap().parse::<i32>()?);
    ///     Ok(())
    /// }
    /// ```
    #[inline]
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
    /// use roa_core::{App, Context, Next, Result, MiddlewareExt};
    ///
    /// struct Scope;
    ///
    /// let app = App::new((), gate.chain(end));
    /// async fn gate(ctx: &mut Context<()>, next: Next<'_>) -> Result {
    ///     ctx.store_scoped(Scope, "id", "1".to_owned());
    ///     next.await
    /// }
    ///
    /// async fn end(ctx: &mut Context<()>) -> Result {
    ///     assert_eq!(1, ctx.load_scoped::<Scope, String>("id").unwrap().parse::<i32>()?);
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn load_scoped<'a, SC, T>(&self, name: &'a str) -> Option<Variable<'a, T>>
    where
        SC: Any,
        T: Any + Send + Sync,
    {
        let id = TypeId::of::<SC>();
        self.storage.get(&id).and_then(|bucket| bucket.get(name))
    }

    /// Search for value by key in public scope.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::{App, Context, Next, Result, MiddlewareExt};
    ///
    /// let app = App::new((), gate.chain(end));
    /// async fn gate(ctx: &mut Context<()>, next: Next<'_>) -> Result {
    ///     ctx.store("id", "1".to_string());
    ///     next.await
    /// }
    ///
    /// async fn end(ctx: &mut Context<()>) -> Result {
    ///     assert_eq!(1, ctx.load::<String>("id").unwrap().parse::<i32>()?);
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn load<'a, T>(&self, name: &'a str) -> Option<Variable<'a, T>>
    where
        T: Any + Send + Sync,
    {
        self.load_scoped::<PublicScope, T>(name)
    }
}

impl<S> Deref for Context<S> {
    type Target = S;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl<S> DerefMut for Context<S> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.state
    }
}

impl<S: Clone> Clone for Context<S> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            req: Request::default(),
            resp: Response::new(),
            state: self.state.clone(),
            exec: self.exec.clone(),
            storage: self.storage.clone(),
            remote_addr: self.remote_addr,
        }
    }
}

#[cfg(all(test, feature = "runtime"))]
mod tests_with_runtime {
    use crate::{App, Context, Error, MiddlewareExt, Next, Request};
    use http::{StatusCode, Version};
    use std::io::Read;

    #[async_std::test]
    async fn status_and_version() -> Result<(), Box<dyn std::error::Error>> {
        async fn test(ctx: &mut Context<()>) -> Result<(), Error> {
            assert_eq!(Version::HTTP_11, ctx.version());
            assert_eq!(StatusCode::OK, ctx.status());
            Ok(())
        }
        let service = App::new((), test).http_service();
        service.serve(Request::default()).await?;
        Ok(())
    }

    #[derive(Clone)]
    struct State {
        data: usize,
    }

    #[async_std::test]
    async fn state_mut() -> Result<(), Box<dyn std::error::Error>> {
        async fn gate(ctx: &mut Context<State>, next: Next<'_>) -> Result<(), Error> {
            ctx.data = 1;
            next.await
        }

        async fn test(ctx: &mut Context<State>) -> Result<(), Error> {
            assert_eq!(1, ctx.data);
            Ok(())
        }
        let service = App::new(State { data: 1 }, gate.chain(test)).http_service();
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
