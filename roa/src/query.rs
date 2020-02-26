//! The query module of roa.
//! This module provides a middleware `query_parser` and a context extension `Query`.
//!
//! ### Example
//!
//! ```rust
//! use roa::query::query_parser;
//! use roa::App;
//! use roa::http::StatusCode;
//! use roa::preload::*;
//! use async_std::task::spawn;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let (addr, server) = App::new(())
//!         .gate(query_parser)
//!         .end(|ctx| async move {
//!             assert_eq!("Hexilee", &*ctx.must_query("name")?);
//!             Ok(())
//!         })
//!         .run_local()?;
//!     spawn(server);
//!     let resp = reqwest::get(&format!("http://{}?name=Hexilee", addr)).await?;
//!     assert_eq!(StatusCode::OK, resp.status());
//!     Ok(())
//! }
//! ```

use crate::http::StatusCode;
use crate::{Context, Error, Next, Result, State, Variable};
use url::form_urlencoded::parse;

/// A scope to store and load variables in Context::storage.
struct QueryScope;

/// A context extension.
/// This extension must be used in downstream of middleware `query_parser`,
/// otherwise you cannot get expected query variable.
///
/// ### Example
///
/// ```rust
/// use roa::query::query_parser;
/// use roa::App;
/// use roa::http::StatusCode;
/// use roa::preload::*;
/// use async_std::task::spawn;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // downstream of `query_parser`
///     let (addr, server) = App::new(())
///         .gate(query_parser)
///         .end( |ctx| async move {
///             assert_eq!("Hexilee", &*ctx.must_query("name")?);
///             Ok(())
///         })
///         .run_local()?;
///     spawn(server);
///     let resp = reqwest::get(&format!("http://{}?name=Hexilee", addr)).await?;
///     assert_eq!(StatusCode::OK, resp.status());
///
///     // miss `query_parser`
///     let (addr, server) = App::new(())
///         .end( |ctx| async move {
///             assert!(ctx.query("name").is_none());
///             Ok(())
///         })
///         .run_local()?;
///     spawn(server);
///     let resp = reqwest::get(&format!("http://{}?name=Hexilee", addr)).await?;
///     assert_eq!(StatusCode::OK, resp.status());
///     Ok(())
/// }
/// ```
pub trait Query {
    /// Must get a variable, throw 400 BAD_REQUEST if it not exists.
    /// ### Example
    ///
    /// ```rust
    /// use roa::query::query_parser;
    /// use roa::App;
    /// use roa::http::StatusCode;
    /// use roa::preload::*;
    /// use async_std::task::spawn;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     // downstream of `query_parser`
    ///     let (addr, server) = App::new(())
    ///         .gate(query_parser)
    ///         .end( |ctx| async move {
    ///             assert_eq!("Hexilee", &*ctx.must_query("name")?);
    ///             Ok(())
    ///         })
    ///         .run_local()?;
    ///     spawn(server);
    ///     let resp = reqwest::get(&format!("http://{}", addr)).await?;
    ///     assert_eq!(StatusCode::BAD_REQUEST, resp.status());
    ///     Ok(())
    /// }
    /// ```
    fn must_query<'a>(&self, name: &'a str) -> Result<Variable<'a, String>>;

    /// Query a variable, return `None` if it not exists.
    /// ### Example
    ///
    /// ```rust
    /// use roa::query::query_parser;
    /// use roa::App;
    /// use roa::http::StatusCode;
    /// use roa::preload::*;
    /// use async_std::task::spawn;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     // downstream of `query_parser`
    ///     let (addr, server) = App::new(())
    ///         .gate(query_parser)
    ///         .end( |ctx| async move {
    ///             assert!(ctx.query("name").is_none());
    ///             Ok(())
    ///         })
    ///         .run_local()?;
    ///     spawn(server);
    ///     let resp = reqwest::get(&format!("http://{}", addr)).await?;
    ///     assert_eq!(StatusCode::OK, resp.status());
    ///     Ok(())
    /// }
    /// ```
    fn query<'a>(&self, name: &'a str) -> Option<Variable<'a, String>>;
}

/// A middleware to parse query.
pub async fn query_parser<S: State>(mut ctx: Context<S>, next: Next) -> Result {
    let uri = ctx.uri();
    let query_string = uri.query().unwrap_or("");
    for (key, value) in parse(query_string.as_bytes()) {
        ctx.store_scoped(QueryScope, &key, value.to_string());
    }
    next.await
}

impl<S: State> Query for Context<S> {
    fn must_query<'a>(&self, name: &'a str) -> Result<Variable<'a, String>> {
        self.query(name).ok_or_else(|| {
            Error::new(
                StatusCode::BAD_REQUEST,
                format!("query `{}` is required", name),
                true,
            )
        })
    }
    fn query<'a>(&self, name: &'a str) -> Option<Variable<'a, String>> {
        self.load_scoped::<QueryScope, String>(name)
    }
}

#[cfg(test)]
mod tests {
    use crate::http::StatusCode;
    use crate::preload::*;
    use crate::query::query_parser;
    use crate::App;
    use async_std::task::spawn;

    #[tokio::test]
    async fn query() -> Result<(), Box<dyn std::error::Error>> {
        // miss key
        let (addr, server) = App::new(())
            .gate(query_parser)
            .end(|ctx| async move {
                assert!(ctx.query("name").is_none());
                Ok(())
            })
            .run_local()?;
        spawn(server);
        reqwest::get(&format!("http://{}/", addr)).await?;

        // string value
        let (addr, server) = App::new(())
            .gate(query_parser)
            .end(|ctx| async move {
                assert_eq!("Hexilee", &*ctx.must_query("name")?);
                Ok(())
            })
            .run_local()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}?name=Hexilee", addr)).await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }

    #[tokio::test]
    async fn query_parse() -> Result<(), Box<dyn std::error::Error>> {
        // invalid int value
        let (addr, server) = App::new(())
            .gate(query_parser)
            .end(|ctx| async move {
                assert!(ctx.must_query("age")?.parse::<u64>().is_err());
                Ok(())
            })
            .run_local()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}?age=Hexilee", addr)).await?;
        assert_eq!(StatusCode::OK, resp.status());

        let (addr, server) = App::new(())
            .gate(query_parser)
            .end(|ctx| async move {
                assert_eq!(120, ctx.must_query("age")?.parse::<u64>()?);
                Ok(())
            })
            .run_local()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}?age=120", addr)).await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }

    #[tokio::test]
    async fn query_action() -> Result<(), Box<dyn std::error::Error>> {
        let (addr, server) = App::new(())
            .gate(query_parser)
            .end(|ctx| async move {
                assert_eq!("Hexilee", &*ctx.must_query("name")?);
                assert_eq!("rust", &*ctx.must_query("lang")?);
                Ok(())
            })
            .run_local()?;
        spawn(server);
        let resp =
            reqwest::get(&format!("http://{}?name=Hexilee&lang=rust", addr)).await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }
}
