//! This module provides a context extension `RouterParam` and
//! many endpoint wrappers like `Router`, `Dispatcher` and `Guard`.
//!
//! ### Example
//!
//! ```rust
//! use roa::router::{Router, RouterParam, get, allow};
//! use roa::{App, Context, Status, MiddlewareExt, Next};
//! use roa::http::{StatusCode, Method};
//! use roa::tcp::Listener;
//! use tokio::task::spawn;
//!
//!
//! async fn gate(_ctx: &mut Context, next: Next<'_>) -> Result<(), Status> {
//!     next.await
//! }
//!
//! async fn query(ctx: &mut Context) -> Result<(), Status> {
//!     Ok(())
//! }
//!
//! async fn create(ctx: &mut Context) -> Result<(), Status> {
//!     Ok(())
//! }
//!
//! async fn graphql(ctx: &mut Context) -> Result<(), Status> {
//!     Ok(())
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let router = Router::new()
//!         .gate(gate)
//!         .on("/restful", get(query).post(create))
//!         .on("/graphql", allow([Method::GET, Method::POST], graphql));
//!     let app = App::new()
//!         .end(router.routes("/api")?);
//!     let (addr, server) = app.run()?;
//!     spawn(server);
//!     let resp = reqwest::get(&format!("http://{}/api/restful", addr)).await?;
//!     assert_eq!(StatusCode::OK, resp.status());
//!
//!     let resp = reqwest::get(&format!("http://{}/restful", addr)).await?;
//!     assert_eq!(StatusCode::NOT_FOUND, resp.status());
//!     Ok(())
//! }
//! ```
//!

mod endpoints;
mod err;
mod path;

use std::convert::AsRef;
use std::result::Result as StdResult;

#[doc(inline)]
pub use endpoints::*;
use err::Conflict;
#[doc(inline)]
pub use err::RouterError;
use path::{join_path, standardize_path, Path, RegexPath};
use percent_encoding::percent_decode_str;
use radix_trie::Trie;

use crate::http::StatusCode;
use crate::{
    async_trait, throw, Boxed, Context, Endpoint, EndpointExt, Middleware, MiddlewareExt, Result,
    Shared, Status, Variable,
};

/// A private scope to store and load variables in Context::storage.
struct RouterScope;

/// A context extension.
/// This extension must be used in `Router`,
/// otherwise you cannot get expected router parameters.
///
/// ### Example
///
/// ```rust
/// use roa::router::{Router, RouterParam};
/// use roa::{App, Context, Status};
/// use roa::http::StatusCode;
/// use roa::tcp::Listener;
/// use tokio::task::spawn;
///
/// async fn test(ctx: &mut Context) -> Result<(), Status> {
///     let id: u64 = ctx.must_param("id")?.parse()?;
///     assert_eq!(0, id);
///     Ok(())
/// }
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let router = Router::new().on("/:id", test);
///     let app = App::new().end(router.routes("/user")?);
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
    /// use roa::router::{Router, RouterParam};
    /// use roa::{App, Context, Status};
    /// use roa::http::StatusCode;
    /// use roa::tcp::Listener;
    /// use tokio::task::spawn;
    ///
    /// async fn test(ctx: &mut Context) -> Result<(), Status> {
    ///     assert!(ctx.param("name").is_none());
    ///     Ok(())
    /// }
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let router = Router::new().on("/:id", test);
    ///     let app = App::new().end(router.routes("/user")?);
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
pub struct Router<S> {
    middleware: Shared<S>,
    endpoints: Vec<(String, Boxed<S>)>,
}

/// An endpoint to route request by uri path.
pub struct RouteTable<S> {
    static_route: Trie<String, Boxed<S>>,
    dynamic_route: Vec<(RegexPath, Boxed<S>)>,
}

impl<S> Router<S>
where
    S: 'static,
{
    /// Construct a new router.
    pub fn new() -> Self {
        Self {
            middleware: ().shared(),
            endpoints: Vec::new(),
        }
    }

    /// Register a new endpoint.
    pub fn on(mut self, path: &'static str, endpoint: impl for<'a> Endpoint<'a, S>) -> Self {
        self.endpoints
            .push((path.to_string(), self.register(endpoint)));
        self
    }

    /// Chain an endpoint to Router::middleware.
    fn register(&self, endpoint: impl for<'a> Endpoint<'a, S>) -> Boxed<S> {
        self.middleware.clone().end(endpoint).boxed()
    }

    /// Include another router with prefix.
    pub fn include(mut self, prefix: &'static str, router: Router<S>) -> Self {
        for (path, endpoint) in router.endpoints {
            self.endpoints
                .push((join_path([prefix, path.as_str()]), self.register(endpoint)))
        }
        self
    }

    /// Chain a middleware to Router::middleware.
    pub fn gate(self, next: impl for<'a> Middleware<'a, S>) -> Router<S> {
        let Self {
            middleware,
            endpoints,
        } = self;
        Self {
            middleware: middleware.chain(next).shared(),
            endpoints,
        }
    }

    /// Build RouteTable with path prefix.
    pub fn routes(self, prefix: &'static str) -> StdResult<RouteTable<S>, RouterError> {
        let mut route_table = RouteTable::default();
        for (raw_path, endpoint) in self.endpoints {
            route_table.insert(join_path([prefix, raw_path.as_str()]), endpoint)?;
        }
        Ok(route_table)
    }
}

impl<S> RouteTable<S>
where
    S: 'static,
{
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
                if self.static_route.insert(path.clone(), endpoint).is_some() {
                    return Err(Conflict::Path(path).into());
                }
            }
            Path::Dynamic(regex_path) => self.dynamic_route.push((regex_path, endpoint)),
        }
        Ok(())
    }
}

impl<S> Default for Router<S>
where
    S: 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<S> Default for RouteTable<S>
where
    S: 'static,
{
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
        let path = standardize_path(&percent_decode_str(uri.path()).decode_utf8().map_err(
            |err| {
                Status::new(
                    StatusCode::BAD_REQUEST,
                    format!("{}\npath `{}` is not a valid utf-8 string", err, uri.path()),
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
                    ctx.store_scoped(RouterScope, var.to_string(), cap[var.as_str()].to_string());
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
            Status::new(
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

#[cfg(all(test, feature = "tcp"))]
mod tests {
    use encoding::EncoderTrap;
    use percent_encoding::NON_ALPHANUMERIC;
    use tokio::task::spawn;

    use super::Router;
    use crate::http::StatusCode;
    use crate::tcp::Listener;
    use crate::{App, Context, Next, Status};

    async fn gate(ctx: &mut Context, next: Next<'_>) -> Result<(), Status> {
        ctx.store("id", "0".to_string());
        next.await
    }

    async fn test(ctx: &mut Context) -> Result<(), Status> {
        let id: u64 = ctx.load::<String>("id").unwrap().parse()?;
        assert_eq!(0, id);
        Ok(())
    }

    #[tokio::test]
    async fn gate_test() -> Result<(), Box<dyn std::error::Error>> {
        let router = Router::new().gate(gate).on("/", test);
        let app = App::new().end(router.routes("/route")?);
        let (addr, server) = app.run()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}/route", addr)).await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }

    #[tokio::test]
    async fn route() -> Result<(), Box<dyn std::error::Error>> {
        let user_router = Router::new().on("/", test);
        let router = Router::new().gate(gate).include("/user", user_router);
        let app = App::new().end(router.routes("/route")?);
        let (addr, server) = app.run()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}/route/user", addr)).await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }

    #[test]
    fn conflict_path() -> Result<(), Box<dyn std::error::Error>> {
        let evil_router = Router::new().on("/endpoint", test);
        let router = Router::new()
            .on("/route/endpoint", test)
            .include("/route", evil_router);
        let ret = router.routes("/");
        assert!(ret.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn route_not_found() -> Result<(), Box<dyn std::error::Error>> {
        let app = App::new().end(Router::default().routes("/")?);
        let (addr, server) = app.run()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}", addr)).await?;
        assert_eq!(StatusCode::NOT_FOUND, resp.status());
        Ok(())
    }

    #[tokio::test]
    async fn non_utf8_uri() -> Result<(), Box<dyn std::error::Error>> {
        let app = App::new().end(Router::default().routes("/")?);
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
