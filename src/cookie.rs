//! The cookie module of roa.
//! This module provides a middleware `cookie_parser` and a context extension `Cookier`.
//!
//! ### Example
//!
//! ```rust
//! use roa::cookie::{cookie_parser, Cookier};
//! use roa::core::{App, StatusCode};
//! use roa::core::header::COOKIE;
//! use async_std::task::spawn;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let (addr, server) = App::new(())
//!         .gate(cookie_parser)
//!         .end(|mut ctx| async move {
//!             assert_eq!("Hexilee", ctx.must_cookie("name").await?);
//!             Ok(())
//!         })
//!         .run_local()?;
//!     spawn(server);
//!     let client = reqwest::Client::new();
//!     let resp = client
//!         .get(&format!("http://{}", addr))
//!         .header(COOKIE, "name=Hexilee")
//!         .send()
//!         .await?;
//!     assert_eq!(StatusCode::OK, resp.status());
//!     Ok(())
//! }
//! ```

use crate::core::{
    async_trait, header, throw, Context, Next, Result, State, StatusCode,
};
use crate::header::FriendlyHeaders;
pub use cookie::Cookie;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

const WWW_AUTHENTICATE_BUG_HELP: &str = "
Invalid WWW_AUTHENTICATE value, this is a bug of roa::cookie.
Please report it to https://github.com/Hexilee/roa.
";

/// A unique symbol to store and load variables in Context::storage.
struct CookieSymbol;

/// A context extension.
/// The `cookie` and `must_cookie` method of this extension
/// must be used in downstream of middleware `cookier_parser`,
/// otherwise you cannot get expected cookie.
///
/// ### Example
///
/// ```rust
/// use roa::cookie::{cookie_parser, Cookier};
/// use roa::core::{App, StatusCode};
/// use roa::core::header::COOKIE;
/// use async_std::task::spawn;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // downstream of `query_parser`
///     let (addr, server) = App::new(())
///         .gate(cookie_parser)
///         .end( |mut ctx| async move {
///             assert_eq!("Hexilee", ctx.must_cookie("name").await?);
///             Ok(())
///         })
///         .run_local()?;
///     spawn(server);
///     let client = reqwest::Client::new();
///     let resp = client
///         .get(&format!("http://{}", addr))
///         .header(COOKIE, "name=Hexilee")
///         .send()
///         .await?;
///     assert_eq!(StatusCode::OK, resp.status());
///
///     // miss `cookie_parser`
///     let (addr, server) = App::new(())
///         .end( |ctx| async move {
///             assert!(ctx.cookie("name").await.is_none());
///             Ok(())
///         })
///         .run_local()?;
///     spawn(server);
///     let resp = client
///         .get(&format!("http://{}", addr))
///         .header(COOKIE, "name=Hexilee")
///         .send()
///         .await?;
///     assert_eq!(StatusCode::OK, resp.status());
///     Ok(())
/// }
/// ```
#[async_trait]
pub trait Cookier {
    /// Must get a cookie, throw 401 UNAUTHORIZED if it not exists.
    /// ### Example
    ///
    /// ```rust
    /// use roa::cookie::{cookie_parser, Cookier};
    /// use roa::core::{App, StatusCode};
    /// use async_std::task::spawn;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     // downstream of `query_parser`
    ///     let (addr, server) = App::new(())
    ///         .gate(cookie_parser)
    ///         .end( |mut ctx| async move {
    ///             assert_eq!("Hexilee", ctx.must_cookie("name").await?);
    ///             Ok(())
    ///         })
    ///         .run_local()?;
    ///     spawn(server);
    ///     let resp = reqwest::get(&format!("http://{}", addr)).await?;
    ///     assert_eq!(StatusCode::UNAUTHORIZED, resp.status());
    ///     Ok(())
    /// }
    /// ```
    async fn must_cookie(&mut self, name: &str) -> Result<String>;

    /// Try to get a cookie, return `None` if it not exists.
    /// ### Example
    ///
    /// ```rust
    /// use roa::cookie::{cookie_parser, Cookier};
    /// use roa::core::{App, StatusCode};
    /// use async_std::task::spawn;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     // downstream of `query_parser`
    ///     let (addr, server) = App::new(())
    ///         .gate(cookie_parser)
    ///         .end( |ctx| async move {
    ///             assert!(ctx.cookie("name").await.is_none());
    ///             Ok(())
    ///         })
    ///         .run_local()?;
    ///     spawn(server);
    ///     let resp = reqwest::get(&format!("http://{}", addr)).await?;
    ///     assert_eq!(StatusCode::OK, resp.status());
    ///     Ok(())
    /// }
    /// ```
    async fn cookie(&self, name: &str) -> Option<String>;

    /// Set a cookie in pecent encoding, should not return Err.
    /// ### Example
    ///
    /// ```rust
    /// use roa::cookie::{Cookier, Cookie};
    /// use roa::core::{App, StatusCode};
    /// use async_std::task::spawn;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     // downstream of `query_parser`
    ///     let (addr, server) = App::new(())
    ///         .end( |mut ctx| async move {
    ///             ctx.set_cookie(Cookie::new("name", "Hexi Lee")).await?;
    ///             Ok(())
    ///         })
    ///         .run_local()?;
    ///     spawn(server);
    ///     let resp = reqwest::get(&format!("http://{}", addr)).await?;
    ///     assert_eq!(StatusCode::OK, resp.status());
    ///     let cookie = resp.cookies().find(|cookie| cookie.name() == "name");
    ///     assert!(cookie.is_some());
    ///     assert_eq!("Hexi%20Lee", cookie.unwrap().value());
    ///     Ok(())
    /// }
    /// ```
    async fn set_cookie(&mut self, cookie: Cookie<'_>) -> Result;
}

/// A middleware to parse cookie.
pub async fn cookie_parser<S: State>(mut ctx: Context<S>, next: Next) -> Result {
    if let Some(Ok(cookies)) = ctx.header(header::COOKIE).await {
        for cookie in cookies
            .split(';')
            .map(|cookie| cookie.trim())
            .map(Cookie::parse_encoded)
            .filter_map(|cookie| cookie.ok())
        {
            ctx.store::<CookieSymbol>(cookie.name(), cookie.value().to_string())
                .await;
        }
    }
    next().await
}

#[async_trait]
impl<S: State> Cookier for Context<S> {
    async fn must_cookie(&mut self, name: &str) -> Result<String> {
        match self.cookie(name).await {
            Some(value) => Ok(value),
            None => {
                let www_authenticate = format!(
                    r#"Cookie name="{}""#,
                    utf8_percent_encode(name, NON_ALPHANUMERIC).to_string()
                );
                self.resp_mut().await.headers.insert(
                    header::WWW_AUTHENTICATE,
                    www_authenticate.parse().expect(WWW_AUTHENTICATE_BUG_HELP),
                );
                throw!(StatusCode::UNAUTHORIZED)
            }
        }
    }
    async fn cookie(&self, name: &str) -> Option<String> {
        self.load::<CookieSymbol>(name)
            .await
            .map(|var| var.into_value())
    }
    async fn set_cookie(&mut self, cookie: Cookie<'_>) -> Result {
        let cookie_value = cookie.encoded().to_string();
        self.resp_mut()
            .await
            .append(header::SET_COOKIE, cookie_value)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{cookie_parser, Cookie, Cookier};
    use crate::core::App;
    use async_std::task::spawn;
    use http::{header, StatusCode};

    #[tokio::test]
    async fn cookie() -> Result<(), Box<dyn std::error::Error>> {
        // miss cookie
        let (addr, server) = App::new(())
            .gate(cookie_parser)
            .end(move |ctx| async move {
                assert!(ctx.cookie("name").await.is_none());
                Ok(())
            })
            .run_local()?;
        spawn(server);
        reqwest::get(&format!("http://{}", addr)).await?;

        let (addr, server) = App::new(())
            .gate(cookie_parser)
            .end(move |mut ctx| async move {
                ctx.must_cookie("nick name").await?;
                Ok(())
            })
            .run_local()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}", addr)).await?;
        assert_eq!(StatusCode::UNAUTHORIZED, resp.status());
        assert_eq!(
            r#"Cookie name="nick%20name""#,
            resp.headers()
                .get(header::WWW_AUTHENTICATE)
                .unwrap()
                .to_str()
                .unwrap()
        );
        // string value
        let (addr, server) = App::new(())
            .gate(cookie_parser)
            .end(move |mut ctx| async move {
                assert_eq!("Hexilee", ctx.must_cookie("name").await?);
                Ok(())
            })
            .run_local()?;
        spawn(server);
        let client = reqwest::Client::new();
        let resp = client
            .get(&format!("http://{}", addr))
            .header(header::COOKIE, "name=Hexilee")
            .send()
            .await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }

    #[tokio::test]
    async fn cookie_decode() -> Result<(), Box<dyn std::error::Error>> {
        let (addr, server) = App::new(())
            .gate(cookie_parser)
            .end(move |mut ctx| async move {
                assert_eq!("bar baz", ctx.must_cookie("bar baz").await?);
                Ok(())
            })
            .run_local()?;
        spawn(server);
        let client = reqwest::Client::new();
        let resp = client
            .get(&format!("http://{}", addr))
            .header(header::COOKIE, "bar%20baz=bar%20baz")
            .send()
            .await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }

    #[tokio::test]
    async fn cookie_action() -> Result<(), Box<dyn std::error::Error>> {
        let (addr, server) = App::new(())
            .gate(cookie_parser)
            .end(move |mut ctx| async move {
                assert_eq!("bar baz", ctx.must_cookie("bar baz").await?);
                assert_eq!("bar foo", ctx.must_cookie("foo baz").await?);
                Ok(())
            })
            .run_local()?;
        spawn(server);
        let client = reqwest::Client::new();
        let resp = client
            .get(&format!("http://{}", addr))
            .header(header::COOKIE, "bar%20baz=bar%20baz; foo%20baz=bar%20foo")
            .send()
            .await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }

    #[tokio::test]
    async fn set_cookie() -> Result<(), Box<dyn std::error::Error>> {
        let (addr, server) = App::new(())
            .end(move |mut ctx| async move {
                ctx.set_cookie(Cookie::new("bar baz", "bar baz")).await?;
                ctx.set_cookie(Cookie::new("bar foo", "foo baz")).await?;
                Ok(())
            })
            .run_local()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}", addr)).await?;
        assert_eq!(StatusCode::OK, resp.status());
        let cookies: Vec<reqwest::cookie::Cookie> = resp.cookies().collect();
        assert_eq!(2, cookies.len());
        assert_eq!(("bar%20baz"), cookies[0].name());
        assert_eq!(("bar%20baz"), cookies[0].value());
        assert_eq!(("bar%20foo"), cookies[1].name());
        assert_eq!(("foo%20baz"), cookies[1].value());
        Ok(())
    }
}
