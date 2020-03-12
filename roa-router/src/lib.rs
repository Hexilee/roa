//! The router module of roa.
//! This module provides an endpoint `RouteEndpoint` and a context extension `RouterParam`.
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
//!     let mut app = App::new(());
//!     app.gate(router.routes("/route")?);
//!     let (addr, server) = app.run()?;
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

mod endpoints;
mod err;
mod path;

pub use endpoints::*;
pub use err::RouterError;

use err::Conflict;
use path::{join_path, standardize_path, Path, RegexPath};

use percent_encoding::percent_decode_str;
use radix_trie::Trie;
use roa_core::http::{Method, StatusCode};
use roa_core::{
    async_trait, throw, Boxed, Context, Endpoint, EndpointExt, Error, Middleware,
    MiddlewareExt, Next, Result, Shared, Variable,
};
use std::collections::HashMap;
use std::convert::AsRef;
use std::future::Future;
use std::result::Result as StdResult;
use std::sync::Arc;

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
///     let mut app = App::new(());
///     app.gate(router.routes("/user")?);
///     let (addr, server) = app.run()?;
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
    ///     let mut app = App::new(());
    ///     app.gate(router.routes("/user")?);
    ///     let (addr, server) = app.run()?;
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

/// A builder of `RouteTable`.
pub struct Router<S, M> {
    middleware: Option<Shared<M>>,
    endpoints: Vec<(String, Boxed<S>)>,
}

pub struct RouteTable<S> {
    static_route: Trie<String, Boxed<S>>,
    dynamic_route: Vec<(RegexPath, Boxed<S>)>,
}

impl<S, M> Router<S, M>
where
    S: 'static,
    M: for<'a> Middleware<'a, S>,
{
    /// Construct a new router.
    pub fn gate(middleware: M) -> Self {
        Self {
            middleware: Some(middleware.shared()),
            endpoints: Vec::new(),
        }
    }

    /// Construct a new router.
    pub fn new() -> Self {
        Self {
            middleware: None,
            endpoints: Vec::new(),
        }
    }

    fn end(&self, endpoint: impl for<'a> Endpoint<'a, S>) -> Boxed<S> {
        match self.middleware.as_ref() {
            Some(middleware) => middleware.clone().end(endpoint).boxed(),
            None => endpoint.boxed(),
        }
    }

    /// Register a new endpoint.
    pub fn on(
        mut self,
        path: &'static str,
        endpoint: impl for<'a> Endpoint<'a, S>,
    ) -> Self {
        self.endpoints.push((path.to_string(), self.end(endpoint)));
        self
    }

    /// Include another router with prefix, only allowing method in parameter methods.
    pub fn include(mut self, prefix: &'static str, router: Router<S, M>) -> Self {
        for (path, endpoint) in router.endpoints {
            self.endpoints
                .push((join_path([prefix, path.as_str()]), self.end(endpoint)))
        }
        self
    }

    /// Build RouteEndpoint with path prefix.
    pub fn routes(self, prefix: &'static str) -> StdResult<RouteTable<S>, RouterError> {
        let mut route_table = RouteTable::default();
        for (raw_path, endpoint) in self.endpoints {
            route_table.insert(join_path([prefix, raw_path.as_str()]), endpoint)?;
        }
        Ok(route_table)
    }
}

impl<S: 'static> RouteTable<S> {
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
        endpoint: Boxed<S>,
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
}

impl<S, M> Default for Router<S, M>
where
    S: 'static,
    M: for<'a> Middleware<'a, S>,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<S: 'static> Default for RouteTable<S> {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait(?Send)]
impl<'a, S> Endpoint<'a, S> for RouteTable<S>
where
    S: 'static,
{
    #[inline]
    async fn call(&'a self, ctx: &'a mut Context<S>) -> Result {
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
        if let Some(end) = self.static_route.get(&path) {
            return end.call(ctx).await;
        }

        // search dynamic routes
        for (regexp_path, end) in self.dynamic_route.iter() {
            if let Some(cap) = regexp_path.re.captures(&path) {
                for var in regexp_path.vars.iter() {
                    ctx.store_scoped(RouterScope, var, cap[var.as_str()].to_string());
                }
                return end.call(ctx).await;
            }
        }

        // 404 NOT FOUND
        throw!(StatusCode::NOT_FOUND)
    }
}

impl<S> RouterParam for Context<S> {
    #[inline]
    fn must_param<'a>(&self, name: &'a str) -> Result<Variable<'a, String>> {
        self.param(name).ok_or_else(|| {
            Error::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("router variable `{}` is required", name),
                false,
            )
        })
    }
    #[inline]
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
        let mut app = App::new(());
        app.gate(router.routes("/route")?);
        let (addr, server) = app.run()?;
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

        let mut app = App::new(());
        app.gate(router.routes("/route")?);
        let (addr, server) = app.run()?;
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
        let mut app = App::new(());
        app.gate(Router::default().routes("/")?);
        let (addr, server) = app.run()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}", addr)).await?;
        assert_eq!(StatusCode::NOT_FOUND, resp.status());
        Ok(())
    }

    #[tokio::test]
    async fn non_utf8_uri() -> Result<(), Box<dyn std::error::Error>> {
        let mut app = App::new(());
        app.gate(Router::default().routes("/")?);
        let (addr, server) = app.run()?;
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
