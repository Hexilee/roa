mod storage;

use crate::{Executor, Request, Response};
use http::header::{AsHeaderName, ToStrError};
use http::StatusCode;
use http::{Method, Uri, Version};
use std::any::Any;
use std::borrow::Cow;
use std::net::SocketAddr;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

pub use storage::Variable;
use storage::{Storage, Value};

/// A structure to share request, response and other data between middlewares.
///
/// ### Example
///
/// ```rust
/// use roa_core::{App, Context, Next, Result};
/// use log::info;
/// use async_std::fs::File;
///
/// let app = App::new(()).gate(gate).end(end);
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
    /// The request, to read http method, uri, version, headers and body.
    pub req: Request,

    /// The response, to set http status, version, headers and body.
    pub resp: Response,

    /// The executor, to spawn futures or blocking works.
    pub exec: Executor,

    /// Socket addr of last client or proxy.
    pub remote_addr: SocketAddr,

    storage: Storage,
    state: S,
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
            storage: Storage::new(),
            remote_addr,
        }
    }

    /// Clone URI.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::{App, Context, Result};
    ///
    /// let app = App::new(()).end(get);
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
    /// let app = App::new(()).end(get);
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
    /// let app = App::new(()).end(get);
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
    /// let app = App::new(()).end(get);
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
    /// let app = App::new(()).end(get);
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
    /// use roa_core::{App, Context, Result, Next};
    ///
    /// struct Scope;
    /// struct AnotherScope;
    ///
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
    ///
    /// let app = App::new(()).gate(gate).end(end);
    /// ```
    #[inline]
    pub fn store_scoped<SC, K, V>(
        &mut self,
        scope: SC,
        key: K,
        value: V,
    ) -> Option<Arc<V>>
    where
        SC: Any,
        K: Into<Cow<'static, str>>,
        V: Value,
    {
        self.storage.insert(scope, key, value)
    }

    /// Store key-value pair in public scope.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::{App, Context, Result, Next};
    ///
    /// async fn gate(ctx: &mut Context<()>, next: Next<'_>) -> Result {
    ///     ctx.store("id", "1".to_string());
    ///     next.await
    /// }
    ///
    /// async fn end(ctx: &mut Context<()>) -> Result {
    ///     assert_eq!(1, ctx.load::<String>("id").unwrap().parse::<i32>()?);
    ///     Ok(())
    /// }
    ///
    /// let app = App::new(()).gate(gate).end(end);
    /// ```
    #[inline]
    pub fn store<K, V>(&mut self, key: K, value: V) -> Option<Arc<V>>
    where
        K: Into<Cow<'static, str>>,
        V: Value,
    {
        self.store_scoped(PublicScope, key, value)
    }

    /// Search for value by key in specific scope.
    ///
    /// ### Example
    ///
    /// ```rust
    /// use roa_core::{App, Context, Result, Next};
    ///
    /// struct Scope;
    ///
    /// async fn gate(ctx: &mut Context<()>, next: Next<'_>) -> Result {
    ///     ctx.store_scoped(Scope, "id", "1".to_owned());
    ///     next.await
    /// }
    ///
    /// async fn end(ctx: &mut Context<()>) -> Result {
    ///     assert_eq!(1, ctx.load_scoped::<Scope, String>("id").unwrap().parse::<i32>()?);
    ///     Ok(())
    /// }
    ///
    /// let app = App::new(()).gate(gate).end(end);
    /// ```
    #[inline]
    pub fn load_scoped<'a, SC, V>(&self, key: &'a str) -> Option<Variable<'a, V>>
    where
        SC: Any,
        V: Value,
    {
        self.storage.get::<SC, V>(key)
    }

    /// Search for value by key in public scope.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::{App, Context, Result, Next};
    ///
    /// async fn gate(ctx: &mut Context<()>, next: Next<'_>) -> Result {
    ///     ctx.store("id", "1".to_string());
    ///     next.await
    /// }
    ///
    /// async fn end(ctx: &mut Context<()>) -> Result {
    ///     assert_eq!(1, ctx.load::<String>("id").unwrap().parse::<i32>()?);
    ///     Ok(())
    /// }
    ///
    /// let app = App::new(()).gate(gate).end(end);
    /// ```
    #[inline]
    pub fn load<'a, V>(&self, key: &'a str) -> Option<Variable<'a, V>>
    where
        V: Value,
    {
        self.load_scoped::<PublicScope, V>(key)
    }
}

/// Public storage scope.
struct PublicScope;

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
    use crate::{App, Context, Next, Request, Status};
    use http::{StatusCode, Version};

    #[async_std::test]
    async fn status_and_version() -> Result<(), Box<dyn std::error::Error>> {
        async fn test(ctx: &mut Context<()>) -> Result<(), Status> {
            assert_eq!(Version::HTTP_11, ctx.version());
            assert_eq!(StatusCode::OK, ctx.status());
            Ok(())
        }
        let service = App::new(()).end(test).http_service();
        service.serve(Request::default()).await?;
        Ok(())
    }

    #[derive(Clone)]
    struct State {
        data: usize,
    }

    #[async_std::test]
    async fn state_mut() -> Result<(), Box<dyn std::error::Error>> {
        async fn gate(ctx: &mut Context<State>, next: Next<'_>) -> Result<(), Status> {
            ctx.data = 1;
            next.await
        }

        async fn test(ctx: &mut Context<State>) -> Result<(), Status> {
            assert_eq!(1, ctx.data);
            Ok(())
        }
        let service = App::new(State { data: 1 })
            .gate(gate)
            .end(test)
            .http_service();
        service.serve(Request::default()).await?;
        Ok(())
    }
}
