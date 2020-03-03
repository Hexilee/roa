//! The router module of roa.
//! This module provides a middleware `RouteEndpoint` and a context extension `RouterParam`.
//!
//! ### Example
//!
//! ```rust
//! use roa_router::{Router, RouterParam};
//! use roa_core::App;
//! use roa_core::http::StatusCode;
//! use roa_tcp::Listener;
//! use async_std::task::spawn;
//!
//! #[tokio::test]
//! async fn gate() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut router = Router::<()>::new();
//!     router
//!         .gate_fn(|_ctx, next| next)
//!         .get("/", |_ctx| async move {
//!             Ok(())
//!         });
//!     let (addr, server) = App::new(()).gate(router.routes("/route")?).run()?;
//!     spawn(server);
//!     let resp = reqwest::get(&format!("http://{}/route", addr)).await?;
//!     assert_eq!(StatusCode::OK, resp.status());
//!
//!     let resp = reqwest::get(&format!("http://{}/endpoint", addr)).await?;
//!     assert_eq!(StatusCode::NOT_FOUND, resp.status());
//!     Ok(())
//! }
//! ```
//!

#![warn(missing_docs)]

mod err;
mod path;

pub use err::RouterError;

use err::Conflict;
use path::{join_path, standardize_path, Path, RegexPath};

use percent_encoding::percent_decode_str;
use radix_trie::Trie;
use roa_core::http::{Method, StatusCode};
use roa_core::{
    async_trait, join_all, throw, Context, Error, Middleware, Next, Result, State,
    Variable,
};
use std::collections::HashMap;
use std::convert::AsRef;
use std::future::Future;
use std::result::Result as StdResult;
use std::sync::Arc;

const ALL_METHODS: [Method; 9] = [
    Method::GET,
    Method::POST,
    Method::PUT,
    Method::PATCH,
    Method::OPTIONS,
    Method::DELETE,
    Method::HEAD,
    Method::TRACE,
    Method::CONNECT,
];

/// A scope to store and load variables in Context::storage.
struct RouterScope;

/// A context extension.
/// This extension must be used in downstream of middleware `RouteEndpoint`,
/// otherwise you cannot get expected router parameter.
///
/// ### Example
///
/// ```rust
/// use roa_router::{Router, RouterParam};
/// use roa_core::App;
/// use roa_core::http::StatusCode;
/// use roa_tcp::Listener;
/// use async_std::task::spawn;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let mut router = Router::<()>::new();
///     router.get("/:id", |ctx| async move {
///         let id: u64 = ctx.must_param("id")?.parse()?;
///         assert_eq!(0, id);
///         Ok(())
///     });
///     let (addr, server) = App::new(())
///         .gate(router.routes("/user")?)
///         .run()?;
///     spawn(server);
///     let resp = reqwest::get(&format!("http://{}/user/0", addr)).await?;
///     assert_eq!(StatusCode::OK, resp.status());
///     Ok(())
/// }
///
///
/// ```
pub trait RouterParam {
    /// Must get a router parameter, throw 500 INTERNAL SERVER ERROR if it not exists.
    fn must_param<'a>(&self, name: &'a str) -> Result<Variable<'a, String>>;

    /// Try to get a router parameter, return `None` if it not exists.
    /// ### Example
    ///
    /// ```rust
    /// use roa_router::{Router, RouterParam};
    /// use roa_core::App;
    /// use roa_core::http::StatusCode;
    /// use roa_tcp::Listener;
    /// use async_std::task::spawn;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let mut router = Router::<()>::new();
    ///     router.get("/:id", |ctx| async move {
    ///         assert!(ctx.param("name").is_none());
    ///         Ok(())
    ///     });
    ///     let (addr, server) = App::new(())
    ///         .gate(router.routes("/user")?)
    ///         .run()?;
    ///     spawn(server);
    ///     let resp = reqwest::get(&format!("http://{}/user/0", addr)).await?;
    ///     assert_eq!(StatusCode::OK, resp.status());
    ///     Ok(())
    /// }
    ///
    ///
    /// ```
    fn param<'a>(&self, name: &'a str) -> Option<Variable<'a, String>>;
}

/// A builder of `RouteEndpoint`.
pub struct Router<S: State> {
    middlewares: Vec<Arc<dyn Middleware<S>>>,
    endpoints: Vec<(Method, String, Arc<dyn Middleware<S>>)>,
}

struct RouteTable<S: State> {
    static_route: Trie<String, Arc<dyn Middleware<S>>>,
    dynamic_route: Vec<(RegexPath, Arc<dyn Middleware<S>>)>,
}

/// A endpoint to handle request by uri path and http method.
///
/// - Throw 404 NOT FOUND when path is not matched.
/// - Throw 405 METHOD NOT ALLOWED when method is not allowed.
pub struct RouteEndpoint<S: State>(HashMap<Method, RouteTable<S>>);

impl<S: State> Router<S> {
    /// Construct a new router.
    pub fn new() -> Self {
        Self {
            middlewares: Vec::new(),
            endpoints: Vec::new(),
        }
    }

    /// use a middleware.
    pub fn gate(&mut self, middleware: impl Middleware<S>) -> &mut Self {
        self.middlewares.push(Arc::new(middleware));
        self
    }

    /// A sugar to match a lambda as a middleware.
    ///
    /// `Router::gate` cannot match a lambda without parameter type indication.
    ///
    /// ```rust
    /// use roa_core::Next;
    /// use roa_router::Router;
    ///
    /// let mut router = Router::<()>::new();
    /// // router.gate(|_ctx, next| next); compile fails.
    /// router.gate(|_ctx, next: Next| next);
    /// ```
    ///
    /// However, with `Router::gate_fn`, you can match a lambda without type indication.
    /// ```rust
    /// use roa_router::Router;
    ///
    /// let mut router = Router::<()>::new();
    /// router.gate_fn(|_ctx, next| next);
    /// ```
    pub fn gate_fn<F>(
        &mut self,
        middleware: impl 'static + Sync + Send + Fn(Context<S>, Next) -> F,
    ) -> &mut Self
    where
        F: 'static + Future<Output = Result>,
    {
        self.gate(middleware);
        self
    }

    /// Register a new endpoint.
    pub fn end(
        &mut self,
        path: &'static str,
        methods: impl AsRef<[Method]>,
        endpoint: impl Middleware<S>,
    ) -> &mut Self {
        let endpoint_ptr = Arc::new(endpoint);
        for method in methods.as_ref() {
            self.endpoints.push((
                method.clone(),
                path.to_string(),
                endpoint_ptr.clone(),
            ));
        }
        self
    }

    /// A sugar to match a function pointer like `async fn(Context<S>) -> impl Future`
    /// and use register it as an endpoint.
    ///
    /// As the ducument of `Middleware`, an endpoint is defined as a template:
    ///
    /// ```rust
    /// use roa_core::{Context, Result};
    /// use std::future::Future;
    ///
    /// fn endpoint<F>(ctx: Context<()>) -> F
    /// where F: 'static + Send + Future<Output=Result> {
    ///     unimplemented!()
    /// }
    /// ```
    ///
    /// However, an async function is not a template,
    /// it needs a transfer function to suit for `Router::end`.
    ///
    /// ```rust
    /// use roa_core::{Context, Result, State, Middleware};
    /// use roa_router::Router;
    /// use std::future::Future;
    /// use roa_core::http::Method;
    ///
    /// async fn endpoint(ctx: Context<()>) -> Result {
    ///     Ok(())
    /// }
    ///
    /// fn transfer<S, F>(endpoint: fn(Context<S>) -> F) -> impl Middleware<S>
    /// where S: State,
    ///       F: 'static + Future<Output=Result> {
    ///     endpoint
    /// }
    ///
    /// Router::<()>::new().end("/", [Method::GET], transfer(endpoint));
    /// ```
    ///
    /// And `Router::end_fn` is a wrapper of `Router::end` with this transfer function.
    ///
    /// ```rust
    /// use roa_router::Router;
    /// use roa_core::http::Method;
    ///
    /// Router::<()>::new().end_fn("/", [Method::GET], |_ctx| async { Ok(()) });
    /// ```
    pub fn end_fn<F>(
        &mut self,
        path: &'static str,
        methods: impl AsRef<[Method]>,
        endpoint: fn(Context<S>) -> F,
    ) -> &mut Self
    where
        F: 'static + Future<Output = Result>,
    {
        self.end(path, methods, endpoint)
    }

    /// Include another router with prefix, allowing all methods.
    pub fn include(&mut self, prefix: &'static str, router: Router<S>) -> &mut Self {
        self.include_methods(prefix, ALL_METHODS, router)
    }

    /// Include another router with prefix, only allowing method in parameter methods.
    pub fn include_methods(
        &mut self,
        prefix: &'static str,
        methods: impl AsRef<[Method]>,
        router: Router<S>,
    ) -> &mut Self {
        for (method, path, endpoint) in router.on(prefix) {
            if methods.as_ref().contains(&method) {
                self.endpoints.push((method, path, endpoint))
            }
        }
        self
    }

    /// Return endpoints with prefix.
    fn on(
        &self,
        prefix: &'static str,
    ) -> impl '_ + Iterator<Item = (Method, String, Arc<dyn Middleware<S>>)> {
        self.endpoints.iter().map(move |(method, path, endpoint)| {
            let mut middlewares = self.middlewares.clone();
            middlewares.push(endpoint.clone());
            let new_endpoint: Arc<dyn Middleware<S>> = Arc::new(join_all(middlewares));
            let new_path = join_path(&vec![prefix, path.as_str()]);
            (method.clone(), new_path, new_endpoint)
        })
    }

    /// Build RouteEndpoint with path prefix.
    pub fn routes(
        self,
        prefix: &'static str,
    ) -> StdResult<RouteEndpoint<S>, RouterError> {
        let mut route_endpoint = RouteEndpoint::default();
        for (method, raw_path, endpoint) in self.on(prefix) {
            route_endpoint.insert(method, raw_path, endpoint)?;
        }
        Ok(route_endpoint)
    }
}

impl<S: State> Default for Router<S> {
    fn default() -> Self {
        Self::new()
    }
}

macro_rules! impl_http_method {
    ($end:ident, $($method:expr),*) => {
        #[allow(missing_docs)]
        pub fn $end<F>(&mut self, path: &'static str, endpoint: fn(Context<S>) -> F) -> &mut Self
        where
            F: 'static + Future<Output = Result>,
        {
            self.end(path, [$($method, )*], endpoint)
        }
    };
}

impl<S: State> Router<S> {
    impl_http_method!(get, Method::GET);
    impl_http_method!(post, Method::POST);
    impl_http_method!(put, Method::PUT);
    impl_http_method!(patch, Method::PATCH);
    impl_http_method!(options, Method::OPTIONS);
    impl_http_method!(delete, Method::DELETE);
    impl_http_method!(head, Method::HEAD);
    impl_http_method!(trace, Method::TRACE);
    impl_http_method!(connect, Method::CONNECT);
    impl_http_method!(
        all,
        Method::GET,
        Method::POST,
        Method::PUT,
        Method::PATCH,
        Method::OPTIONS,
        Method::DELETE,
        Method::HEAD,
        Method::TRACE,
        Method::CONNECT
    );
}

impl<S: State> Default for RouteEndpoint<S> {
    fn default() -> Self {
        let mut map = HashMap::new();
        for method in ALL_METHODS.as_ref() {
            map.insert(method.clone(), RouteTable::new());
        }
        Self(map)
    }
}

impl<S: State> RouteEndpoint<S> {
    /// Insert endpoint to route table by method.
    fn insert(
        &mut self,
        method: Method,
        raw_path: impl AsRef<str>,
        endpoint: Arc<dyn Middleware<S>>,
    ) -> StdResult<(), RouterError> {
        match self.0.get_mut(&method) {
            Some(route_table) => route_table.insert(raw_path, endpoint),
            None => {
                self.0.insert(method.clone(), RouteTable::new());
                self.insert(method, raw_path, endpoint)
            }
        }
    }
}

impl<S: State> RouteTable<S> {
    fn new() -> Self {
        Self {
            static_route: Trie::new(),
            dynamic_route: Vec::new(),
        }
    }

    /// Insert endpoint to table.
    fn insert(
        &mut self,
        raw_path: impl AsRef<str>,
        endpoint: Arc<dyn Middleware<S>>,
    ) -> StdResult<(), RouterError> {
        match raw_path.as_ref().parse()? {
            Path::Static(path) => {
                if self
                    .static_route
                    .insert(path.to_string(), endpoint)
                    .is_some()
                {
                    return Err(Conflict::Path(path).into());
                }
            }
            Path::Dynamic(regex_path) => self.dynamic_route.push((regex_path, endpoint)),
        }
        Ok(())
    }

    /// Handle request.
    async fn end(&self, mut ctx: Context<S>) -> Result {
        let uri = ctx.uri();
        // standardize path
        let path =
            standardize_path(&percent_decode_str(uri.path()).decode_utf8().map_err(
                |err| {
                    Error::new(
                        StatusCode::BAD_REQUEST,
                        format!(
                            "{}\npath `{}` is not a valid utf-8 string",
                            err,
                            uri.path()
                        ),
                        true,
                    )
                },
            )?);

        // search static routes
        if let Some(handler) = self.static_route.get(&path) {
            return handler.clone().end(ctx).await;
        }

        // search dynamic routes
        for (regexp_path, handler) in self.dynamic_route.iter() {
            if let Some(cap) = regexp_path.re.captures(&path) {
                for var in regexp_path.vars.iter() {
                    ctx.store_scoped(RouterScope, var, cap[var.as_str()].to_string());
                }
                return handler.clone().end(ctx).await;
            }
        }

        // 404 NOT FOUND
        throw!(StatusCode::NOT_FOUND)
    }
}

#[async_trait(?Send)]
impl<S: State> Middleware<S> for RouteEndpoint<S> {
    async fn handle(self: Arc<Self>, ctx: Context<S>, _next: Next) -> Result {
        match self.0.get(&ctx.method()) {
            None => throw!(
                StatusCode::METHOD_NOT_ALLOWED,
                format!("method {} is not allowed", &ctx.method())
            ),
            Some(handler) => handler.end(ctx).await,
        }
    }
}

impl<S: State> RouterParam for Context<S> {
    fn must_param<'a>(&self, name: &'a str) -> Result<Variable<'a, String>> {
        self.param(name).ok_or_else(|| {
            Error::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("router variable `{}` is required", name),
                false,
            )
        })
    }
    fn param<'a>(&self, name: &'a str) -> Option<Variable<'a, String>> {
        self.load_scoped::<RouterScope, String>(name)
    }
}

#[cfg(test)]
mod tests {
    use super::Router;
    use async_std::task::spawn;
    use encoding::EncoderTrap;
    use percent_encoding::NON_ALPHANUMERIC;
    use roa_core::http::StatusCode;
    use roa_core::App;
    use roa_tcp::Listener;

    #[tokio::test]
    async fn gate() -> Result<(), Box<dyn std::error::Error>> {
        let mut router = Router::<()>::new();
        router
            .gate_fn(|mut ctx, next| async move {
                ctx.store("id", "0".to_string());
                next.await
            })
            .get("/", |ctx| async move {
                let id: u64 = ctx.load::<String>("id").unwrap().parse()?;
                assert_eq!(0, id);
                Ok(())
            });
        let (addr, server) = App::new(()).gate(router.routes("/route")?).run()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}/route", addr)).await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }

    #[tokio::test]
    async fn route() -> Result<(), Box<dyn std::error::Error>> {
        let mut router = Router::<()>::new();
        let mut user_router = Router::<()>::new();
        router.gate_fn(|mut ctx, next| async move {
            ctx.store("id", "0".to_string());
            next.await
        });
        user_router.get("/", |ctx| async move {
            let id: u64 = ctx.load::<String>("id").unwrap().parse()?;
            assert_eq!(0, id);
            Ok(())
        });
        router.include("/user", user_router);

        let (addr, server) = App::new(()).gate(router.routes("/route")?).run()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}/route/user", addr)).await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }

    #[test]
    fn conflict_path() -> Result<(), Box<dyn std::error::Error>> {
        let mut router = Router::<()>::new();
        let mut evil_router = Router::<()>::new();
        router.get("/route/endpoint", |_ctx| async { Ok(()) });
        evil_router.get("/endpoint", |_ctx| async { Ok(()) });
        router.include("/route", evil_router);
        let ret = router.routes("/");
        assert!(ret.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn route_not_found() -> Result<(), Box<dyn std::error::Error>> {
        let (addr, server) = App::new(()).gate(Router::default().routes("/")?).run()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}", addr)).await?;
        assert_eq!(StatusCode::NOT_FOUND, resp.status());
        Ok(())
    }

    #[tokio::test]
    async fn non_utf8_uri() -> Result<(), Box<dyn std::error::Error>> {
        let (addr, server) = App::new(()).gate(Router::new().routes("/")?).run()?;
        spawn(server);
        let gbk_path = encoding::label::encoding_from_whatwg_label("gbk")
            .unwrap()
            .encode("路由", EncoderTrap::Strict)
            .unwrap();
        let encoded_path =
            percent_encoding::percent_encode(&gbk_path, NON_ALPHANUMERIC).to_string();
        let uri = format!("http://{}/{}", addr, encoded_path);
        let resp = reqwest::get(&uri).await?;
        assert_eq!(StatusCode::BAD_REQUEST, resp.status());
        assert!(resp
            .text()
            .await?
            .ends_with("path `/%C2%B7%D3%C9` is not a valid utf-8 string"));
        Ok(())
    }
}
