//! The query module of roa.
//! This module provides a middleware `query_parser` and a context extension `Query`.
//!
//! ### Example
//!
//! ```rust
//! use roa::query::{query_parser, Query};
//! use roa::core::{App, StatusCode};
//! use async_std::task::spawn;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let (addr, server) = App::new(())
//!         .gate(query_parser)
//!         .end(|ctx| async move {
//!             assert_eq!("Hexilee", ctx.must_query("name").await?.as_ref());
//!             Ok(())
//!         })
//!         .run_local()?;
//!     spawn(server);
//!     let resp = reqwest::get(&format!("http://{}?name=Hexilee", addr)).await?;
//!     assert_eq!(StatusCode::OK, resp.status());
//!     Ok(())
//! }
//! ```

use crate::core::{
    async_trait, Context, Error, Next, Result, State, StatusCode, Variable,
};
use url::form_urlencoded::parse;

/// A unique symbol to store and load variables in Context::storage.
struct QuerySymbol;

/// A context extension.
/// This extension must be used in downstream of middleware `query_parser`,
/// otherwise you cannot get expected query variable.
///
/// ### Example
///
/// ```rust
/// use roa::query::{query_parser, Query};
/// use roa::core::{App, StatusCode};
/// use async_std::task::spawn;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // downstream of `query_parser`
///     let (addr, server) = App::new(())
///         .gate(query_parser)
///         .end( |ctx| async move {
///             assert_eq!("Hexilee", ctx.must_query("name").await?.as_ref());
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
///             assert!(ctx.query("name").await.is_none());
///             Ok(())
///         })
///         .run_local()?;
///     spawn(server);
///     let resp = reqwest::get(&format!("http://{}?name=Hexilee", addr)).await?;
///     assert_eq!(StatusCode::OK, resp.status());
///     Ok(())
/// }
/// ```
#[async_trait]
pub trait Query {
    /// Must get a variable, throw 400 BAD_REQUEST if it not exists.
    /// ### Example
    ///
    /// ```rust
    /// use roa::query::{query_parser, Query};
    /// use roa::core::{App, StatusCode};
    /// use async_std::task::spawn;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     // downstream of `query_parser`
    ///     let (addr, server) = App::new(())
    ///         .gate(query_parser)
    ///         .end( |ctx| async move {
    ///             assert_eq!("Hexilee", ctx.must_query("name").await?.as_ref());
    ///             Ok(())
    ///         })
    ///         .run_local()?;
    ///     spawn(server);
    ///     let resp = reqwest::get(&format!("http://{}", addr)).await?;
    ///     assert_eq!(StatusCode::BAD_REQUEST, resp.status());
    ///     Ok(())
    /// }
    /// ```
    async fn must_query<'a>(&self, name: &'a str) -> Result<Variable<'a>>;

    /// Query a variable, return `None` if it not exists.
    /// ### Example
    ///
    /// ```rust
    /// use roa::query::{query_parser, Query};
    /// use roa::core::{App, StatusCode};
    /// use async_std::task::spawn;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     // downstream of `query_parser`
    ///     let (addr, server) = App::new(())
    ///         .gate(query_parser)
    ///         .end( |ctx| async move {
    ///             assert!(ctx.query("name").await.is_none());
    ///             Ok(())
    ///         })
    ///         .run_local()?;
    ///     spawn(server);
    ///     let resp = reqwest::get(&format!("http://{}", addr)).await?;
    ///     assert_eq!(StatusCode::OK, resp.status());
    ///     Ok(())
    /// }
    /// ```
    async fn query<'a>(&self, name: &'a str) -> Option<Variable<'a>>;
}

/// A middleware to parse query.
pub async fn query_parser<S: State>(ctx: Context<S>, next: Next) -> Result {
    let uri = ctx.uri().await;
    let query_string = uri.query().unwrap_or("");
    for (key, value) in parse(query_string.as_bytes()) {
        ctx.store::<QuerySymbol>(&key, value.to_string()).await;
    }
    next().await
}

#[async_trait]
impl<S: State> Query for Context<S> {
    async fn must_query<'a>(&self, name: &'a str) -> Result<Variable<'a>> {
        self.query(name).await.ok_or_else(|| {
            Error::new(
                StatusCode::BAD_REQUEST,
                format!("query `{}` is required", name),
                true,
            )
        })
    }
    async fn query<'a>(&self, name: &'a str) -> Option<Variable<'a>> {
        self.load::<QuerySymbol>(name).await
    }
}

#[cfg(test)]
mod tests {
    use super::{query_parser, Query};
    use crate::core::App;
    use async_std::task::spawn;
    use http::StatusCode;

    #[tokio::test]
    async fn query() -> Result<(), Box<dyn std::error::Error>> {
        // miss key
        let (addr, server) = App::new(())
            .gate(query_parser)
            .end(|ctx| async move {
                assert!(ctx.query("name").await.is_none());
                Ok(())
            })
            .run_local()?;
        spawn(server);
        reqwest::get(&format!("http://{}/", addr)).await?;

        // string value
        let (addr, server) = App::new(())
            .gate(query_parser)
            .end(|ctx| async move {
                assert_eq!("Hexilee", ctx.must_query("name").await?.as_ref());
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
                assert!(ctx.must_query("age").await?.parse::<u64>().is_err());
                Ok(())
            })
            .run_local()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}?age=Hexilee", addr)).await?;
        assert_eq!(StatusCode::OK, resp.status());

        let (addr, server) = App::new(())
            .gate(query_parser)
            .end(|ctx| async move {
                assert_eq!(120, ctx.must_query("age").await?.parse::<u64>()?);
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
                assert_eq!("Hexilee", ctx.must_query("name").await?.as_ref());
                assert_eq!("rust", ctx.must_query("lang").await?.as_ref());
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
