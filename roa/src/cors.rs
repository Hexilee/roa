//! The cors module of roa.
//! This module provides a middleware `Cors`.
//!
//! ### Example
//!
//! ```rust
//! use roa::cors::Cors;
//! use roa::App;
//! use roa::preload::*;
//! use roa::http::{StatusCode, header::{ORIGIN, ACCESS_CONTROL_ALLOW_ORIGIN}};
//! use async_std::task::spawn;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     pretty_env_logger::init();
//!     let mut app = App::new(());
//!     app.gate(Cors::new())
//!         .end(|ctx| async move {
//!         Ok(())
//!     });
//!     let (addr, server) = app.run()?;
//!     spawn(server);
//!     let client = reqwest::Client::new();
//!     let resp = client
//!         .get(&format!("http://{}", addr))
//!         .header(ORIGIN, "http://github.com")
//!         .send()
//!         .await?;
//!     assert_eq!(StatusCode::OK, resp.status());
//!     assert_eq!(
//!         "http://github.com",
//!         resp.headers().get(ACCESS_CONTROL_ALLOW_ORIGIN).unwrap().to_str()?
//!     );
//!     Ok(())
//! }
//! ```

use crate::http::header::{HeaderName, HeaderValue, ORIGIN, VARY};

use crate::http::{Method, StatusCode};
use crate::preload::*;
use crate::{async_trait, Context, Middleware, Next, Result, State};
use headers::{
    AccessControlAllowCredentials, AccessControlAllowHeaders, AccessControlAllowMethods,
    AccessControlAllowOrigin, AccessControlExposeHeaders, AccessControlMaxAge,
    AccessControlRequestHeaders, AccessControlRequestMethod, Header, HeaderMapExt,
};
use roa_core::Error;
use std::collections::HashSet;
use std::convert::TryInto;
use std::fmt::Debug;
use std::iter::FromIterator;
use std::sync::Arc;
use std::time::Duration;

/// A middleware to deal with Cross-Origin Resource Sharing (CORS).
///
/// ### Default
///
/// The default Cors middleware works well,
/// it will use "origin" as value of response header "access-control-allow-origin",
///
/// And in preflight request,
/// it will use "access-control-request-method" as value of "access-control-allow-methods"
/// and use "access-control-request-headers" as value of "access-control-allow-headers".
///
/// Build a default Cors middleware:
///
/// ```rust
/// use roa::cors::Cors;
///
/// let default_cors = Cors::new();
/// ```
///
/// ### Config
///
/// You can also configure it:
///
/// ```rust
/// use roa::cors::Cors;
/// use roa::http::header::{CONTENT_DISPOSITION, AUTHORIZATION, WWW_AUTHENTICATE};
/// use roa::http::Method;
///
/// let configured_cors = Cors::builder()
///     .allow_credentials(true)
///     .max_age(86400)
///     .allow_origin("https://github.com")
///     .allow_methods(vec![Method::GET, Method::POST])
///     .allow_method(Method::PUT)
///     .expose_headers(vec![CONTENT_DISPOSITION])
///     .expose_header(WWW_AUTHENTICATE)
///     .allow_headers(vec![AUTHORIZATION])
///     .allow_header(CONTENT_DISPOSITION)
///     .build();
/// ```
#[derive(Debug, Default)]
pub struct Cors {
    allow_origin: Option<AccessControlAllowOrigin>,
    allow_methods: Option<AccessControlAllowMethods>,
    expose_headers: Option<AccessControlExposeHeaders>,
    allow_headers: Option<AccessControlAllowHeaders>,
    max_age: Option<AccessControlMaxAge>,
    credentials: Option<AccessControlAllowCredentials>,
}

/// Builder of Cors.
#[derive(Clone, Debug, Default)]
pub struct Builder {
    credentials: bool,
    allowed_headers: HashSet<HeaderName>,
    exposed_headers: HashSet<HeaderName>,
    max_age: Option<u64>,
    methods: HashSet<Method>,
    origins: Option<HeaderValue>,
}

impl Cors {
    /// Construct default Cors.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get builder.
    pub fn builder() -> Builder {
        Builder::default()
    }
}

impl Builder {
    /// Sets whether to add the `Access-Control-Allow-Credentials` header.
    pub fn allow_credentials(mut self, allow: bool) -> Self {
        self.credentials = allow;
        self
    }

    /// Adds a method to the existing list of allowed request methods.
    pub fn allow_method(mut self, method: Method) -> Self {
        self.methods.insert(method);
        self
    }

    /// Adds multiple methods to the existing list of allowed request methods.
    pub fn allow_methods(mut self, methods: impl IntoIterator<Item = Method>) -> Self {
        self.methods.extend(methods);
        self
    }

    /// Adds a header to the list of allowed request headers.
    ///
    /// # Panics
    ///
    /// Panics if header is not a valid `http::header::HeaderName`.
    pub fn allow_header<H>(mut self, header: H) -> Self
    where
        H: TryInto<HeaderName>,
        H::Error: Debug,
    {
        self.allowed_headers
            .insert(header.try_into().expect("invalid header"));
        self
    }

    /// Adds multiple headers to the list of allowed request headers.
    ///
    /// # Panics
    ///
    /// Panics if any of the headers are not a valid `http::header::HeaderName`.
    pub fn allow_headers<I>(mut self, headers: I) -> Self
    where
        I: IntoIterator,
        I::Item: TryInto<HeaderName>,
        <I::Item as TryInto<HeaderName>>::Error: Debug,
    {
        let iter = headers
            .into_iter()
            .map(|h| h.try_into().expect("invalid header"));
        self.allowed_headers.extend(iter);
        self
    }

    /// Adds a header to the list of exposed headers.
    ///
    /// # Panics
    ///
    /// Panics if the provided argument is not a valid `http::header::HeaderName`.
    pub fn expose_header<H>(mut self, header: H) -> Self
    where
        H: TryInto<HeaderName>,
        H::Error: Debug,
    {
        self.exposed_headers
            .insert(header.try_into().expect("illegal Header"));
        self
    }

    /// Adds multiple headers to the list of exposed headers.
    ///
    /// # Panics
    ///
    /// Panics if any of the headers are not a valid `http::header::HeaderName`.
    pub fn expose_headers<I>(mut self, headers: I) -> Self
    where
        I: IntoIterator,
        I::Item: TryInto<HeaderName>,
        <I::Item as TryInto<HeaderName>>::Error: Debug,
    {
        let iter = headers
            .into_iter()
            .map(|h| h.try_into().expect("illegal Header"));
        self.exposed_headers.extend(iter);
        self
    }

    /// Add an origin to the existing list of allowed `Origin`s.
    ///
    /// # Panics
    ///
    /// Panics if the provided argument is not a valid `HeaderValue`.
    pub fn allow_origin<H>(mut self, origin: H) -> Self
    where
        H: TryInto<HeaderValue>,
        H::Error: Debug,
    {
        self.origins = Some(origin.try_into().expect("invalid origin"));
        self
    }

    /// Sets the `Access-Control-Max-Age` header.
    pub fn max_age(mut self, seconds: u64) -> Self {
        self.max_age = Some(seconds);
        self
    }

    /// Builds the `Cors` wrapper from the configured settings.
    ///
    /// This step isn't *required*, as the `Builder` itself can be passed
    /// to `Filter::with`. This just allows constructing once, thus not needing
    /// to pay the cost of "building" every time.
    pub fn build(self) -> Cors {
        let Builder {
            allowed_headers,
            credentials,
            exposed_headers,
            max_age,
            origins,
            methods,
        } = self;
        let mut cors = Cors::default();
        if !allowed_headers.is_empty() {
            cors.allow_headers =
                Some(AccessControlAllowHeaders::from_iter(allowed_headers))
        }

        if credentials {
            cors.credentials = Some(AccessControlAllowCredentials)
        }

        if !exposed_headers.is_empty() {
            cors.expose_headers =
                Some(AccessControlExposeHeaders::from_iter(exposed_headers))
        }

        if let Some(age) = max_age {
            cors.max_age = Some(Duration::from_secs(age).into())
        }

        if origins.is_some() {
            cors.allow_origin = Some(
                AccessControlAllowOrigin::decode(&mut origins.iter())
                    .expect("invalid origins"),
            );
        }

        if !methods.is_empty() {
            cors.allow_methods = Some(AccessControlAllowMethods::from_iter(methods))
        }

        cors
    }
}

#[async_trait(?Send)]
impl<'a, S> Middleware<'a, S> for Cors {
    async fn handle(&'a self, ctx: &'a mut Context<S>, next: Next<'a>) -> Result {
        // Always set Vary header
        // https://github.com/rs/cors/issues/10
        ctx.resp.append(VARY, ORIGIN)?;

        let origin = match ctx.req.headers.get(ORIGIN) {
            // If there is no Origin header, skip this middleware.
            None => return next.await,
            Some(origin) => AccessControlAllowOrigin::decode(
                &mut Some(origin).into_iter(),
            )
            .map_err(|err| {
                Error::new(
                    StatusCode::BAD_REQUEST,
                    format!("invalid origin: {}", err),
                    true,
                )
            })?,
        };

        // If Options::allow_origin is None, `Access-Control-Allow-Origin` will be set to `Origin`.
        let allow_origin = self.allow_origin.clone().unwrap_or(origin);

        let credentials = self.credentials.clone();
        let insert_origin_and_credentials = move |ctx: &mut Context<S>| {
            // Set "Access-Control-Allow-Origin"
            ctx.resp.headers.typed_insert(allow_origin);

            // Try to set "Access-Control-Allow-Credentials"
            if let Some(credentials) = credentials {
                ctx.resp.headers.typed_insert(credentials);
            }
        };

        if ctx.method() != Method::OPTIONS {
            // Simple Request

            insert_origin_and_credentials(ctx);

            // Set "Access-Control-Expose-Headers"
            if let Some(ref exposed_headers) = self.expose_headers {
                ctx.resp.headers.typed_insert(exposed_headers.clone());
            }
            next.await
        } else {
            // Preflight Request

            let request_method =
                match ctx.req.headers.typed_get::<AccessControlRequestMethod>() {
                    // If there is no Origin header or if parsing failed, skip this middleware.
                    None => return next.await,
                    Some(request_method) => request_method,
                };

            // If Options::allow_methods is None, `Access-Control-Allow-Methods` will be set to `Access-Control-Request-Method`.
            let allow_methods = match self.allow_methods {
                Some(ref origin) => origin.clone(),
                None => {
                    AccessControlAllowMethods::from_iter(Some(request_method.into()))
                }
            };

            // Try to set "Access-Control-Allow-Methods"
            ctx.resp.headers.typed_insert(allow_methods);

            insert_origin_and_credentials(ctx);

            // Set "Access-Control-Max-Age"
            if let Some(ref max_age) = self.max_age {
                ctx.resp.headers.typed_insert(max_age.clone());
            }

            // If allow_headers is None, try to assign `Access-Control-Request-Headers` to `Access-Control-Allow-Headers`.
            let allow_headers = self.allow_headers.clone().or_else(|| {
                ctx.req
                    .headers
                    .typed_get::<AccessControlRequestHeaders>()
                    .map(|headers| AccessControlAllowHeaders::from_iter(headers.iter()))
            });
            if let Some(headers) = allow_headers {
                ctx.resp.headers.typed_insert(headers);
            };

            ctx.resp.status = StatusCode::NO_CONTENT;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Cors;
    use crate::http::header::{
        ACCESS_CONTROL_ALLOW_CREDENTIALS, ACCESS_CONTROL_ALLOW_HEADERS,
        ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_ORIGIN,
        ACCESS_CONTROL_MAX_AGE, ACCESS_CONTROL_REQUEST_HEADERS,
        ACCESS_CONTROL_REQUEST_METHOD, AUTHORIZATION, CONTENT_DISPOSITION, CONTENT_TYPE,
        ORIGIN, VARY, WWW_AUTHENTICATE,
    };
    use crate::http::{HeaderValue, Method, StatusCode};
    use crate::preload::*;
    use crate::App;
    use async_std::task::spawn;
    use headers::{
        AccessControlAllowCredentials, AccessControlAllowOrigin,
        AccessControlExposeHeaders, HeaderMapExt, HeaderName,
    };

    #[tokio::test]
    async fn default_cors() -> Result<(), Box<dyn std::error::Error>> {
        let mut app = App::new(());
        app.gate(Cors::new()).end(|mut ctx| async move {
            ctx.resp.write("Hello, World");
            Ok(())
        });
        let (addr, server) = app.run()?;
        spawn(server);
        let client = reqwest::Client::new();

        // No origin
        let resp = client.get(&format!("http://{}", addr)).send().await?;
        assert_eq!(StatusCode::OK, resp.status());
        assert!(resp
            .headers()
            .typed_get::<AccessControlAllowOrigin>()
            .is_none());
        assert_eq!(
            HeaderValue::from_name(ORIGIN),
            resp.headers().get(VARY).unwrap()
        );
        assert_eq!("Hello, World", resp.text().await?);

        // invalid origin
        let resp = client
            .get(&format!("http://{}", addr))
            .header(ORIGIN, "github.com")
            .send()
            .await?;
        assert_eq!(StatusCode::BAD_REQUEST, resp.status());

        // simple request
        let resp = client
            .get(&format!("http://{}", addr))
            .header(ORIGIN, "http://github.com")
            .send()
            .await?;
        assert_eq!(StatusCode::OK, resp.status());

        let allow_origin = resp
            .headers()
            .typed_get::<AccessControlAllowOrigin>()
            .unwrap();
        let origin = allow_origin.origin().unwrap();
        assert_eq!("http", origin.scheme());
        assert_eq!("github.com", origin.hostname());
        assert!(origin.port().is_none());
        assert!(resp
            .headers()
            .typed_get::<AccessControlAllowCredentials>()
            .is_none());

        assert!(resp
            .headers()
            .typed_get::<AccessControlExposeHeaders>()
            .is_none());

        assert_eq!("Hello, World", resp.text().await?);

        // options, no Access-Control-Request-Method
        let resp = client
            .request(Method::OPTIONS, &format!("http://{}", addr))
            .header(ORIGIN, "http://github.com")
            .send()
            .await?;
        assert_eq!(StatusCode::OK, resp.status());
        assert!(resp.headers().get(ACCESS_CONTROL_ALLOW_ORIGIN).is_none());
        assert_eq!(
            HeaderValue::from_name(ORIGIN),
            resp.headers().get(VARY).unwrap()
        );
        assert_eq!("Hello, World", resp.text().await?);

        // options, contains Access-Control-Request-Method
        let resp = client
            .request(Method::OPTIONS, &format!("http://{}", addr))
            .header(ORIGIN, "http://github.com")
            .header(ACCESS_CONTROL_REQUEST_METHOD, "POST")
            .header(
                ACCESS_CONTROL_REQUEST_HEADERS,
                HeaderValue::from_name(CONTENT_TYPE),
            )
            .send()
            .await?;
        assert_eq!(StatusCode::NO_CONTENT, resp.status());
        assert_eq!(
            "http://github.com",
            resp.headers()
                .get(ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap()
                .to_str()?
        );
        assert!(resp
            .headers()
            .get(ACCESS_CONTROL_ALLOW_CREDENTIALS)
            .is_none());

        assert!(resp.headers().get(ACCESS_CONTROL_MAX_AGE).is_none());

        assert_eq!(
            "POST",
            resp.headers()
                .get(ACCESS_CONTROL_ALLOW_METHODS)
                .unwrap()
                .to_str()?
        );

        assert_eq!(
            HeaderValue::from_name(CONTENT_TYPE),
            resp.headers().get(ACCESS_CONTROL_ALLOW_HEADERS).unwrap()
        );
        assert_eq!("", resp.text().await?);
        //
        Ok(())
    }

    #[tokio::test]
    async fn configured_cors() -> Result<(), Box<dyn std::error::Error>> {
        let mut app = App::new(());
        let configured_cors = Cors::builder()
            .allow_credentials(true)
            .max_age(86400)
            .allow_origin("https://github.com")
            .allow_methods(vec![Method::GET, Method::POST])
            .allow_method(Method::PUT)
            .expose_headers(vec![CONTENT_DISPOSITION])
            .expose_header(WWW_AUTHENTICATE)
            .allow_headers(vec![AUTHORIZATION])
            .allow_header(CONTENT_TYPE)
            .build();
        app.gate(configured_cors).end(|mut ctx| async move {
            ctx.resp.write("Hello, World");
            Ok(())
        });
        let (addr, server) = app.run()?;
        spawn(server);
        let client = reqwest::Client::new();

        // No origin
        let resp = client.get(&format!("http://{}", addr)).send().await?;
        assert_eq!(StatusCode::OK, resp.status());
        assert!(resp
            .headers()
            .typed_get::<AccessControlAllowOrigin>()
            .is_none());
        assert_eq!(
            HeaderValue::from_name(ORIGIN),
            resp.headers().get(VARY).unwrap()
        );
        assert_eq!("Hello, World", resp.text().await?);

        // invalid origin
        let resp = client
            .get(&format!("http://{}", addr))
            .header(ORIGIN, "github.com")
            .send()
            .await?;
        assert_eq!(StatusCode::BAD_REQUEST, resp.status());

        // simple request
        let resp = client
            .get(&format!("http://{}", addr))
            .header(ORIGIN, "http://github.io")
            .send()
            .await?;
        assert_eq!(StatusCode::OK, resp.status());

        let allow_origin = resp
            .headers()
            .typed_get::<AccessControlAllowOrigin>()
            .unwrap();
        let origin = allow_origin.origin().unwrap();
        assert_eq!("https", origin.scheme());
        assert_eq!("github.com", origin.hostname());
        assert!(origin.port().is_none());
        assert!(resp
            .headers()
            .typed_get::<AccessControlAllowCredentials>()
            .is_some());

        let expose_headers = resp
            .headers()
            .typed_get::<AccessControlExposeHeaders>()
            .unwrap();

        let headers = expose_headers.iter().collect::<Vec<HeaderName>>();
        assert!(headers.contains(&CONTENT_DISPOSITION));
        assert!(headers.contains(&WWW_AUTHENTICATE));

        assert_eq!("Hello, World", resp.text().await?);

        // options, no Access-Control-Request-Method
        let resp = client
            .request(Method::OPTIONS, &format!("http://{}", addr))
            .header(ORIGIN, "http://github.com")
            .send()
            .await?;
        assert_eq!(StatusCode::OK, resp.status());
        assert!(resp.headers().get(ACCESS_CONTROL_ALLOW_ORIGIN).is_none());
        assert_eq!(
            HeaderValue::from_name(ORIGIN),
            resp.headers().get(VARY).unwrap()
        );
        assert_eq!("Hello, World", resp.text().await?);

        // options, contains Access-Control-Request-Method
        let resp = client
            .request(Method::OPTIONS, &format!("http://{}", addr))
            .header(ORIGIN, "http://github.io")
            .header(ACCESS_CONTROL_REQUEST_METHOD, "POST")
            .header(
                ACCESS_CONTROL_REQUEST_HEADERS,
                HeaderValue::from_name(CONTENT_TYPE),
            )
            .send()
            .await?;
        assert_eq!(StatusCode::NO_CONTENT, resp.status());
        assert_eq!(
            "https://github.com",
            resp.headers()
                .get(ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap()
                .to_str()?
        );
        assert_eq!(
            "true",
            resp.headers()
                .get(ACCESS_CONTROL_ALLOW_CREDENTIALS)
                .unwrap()
                .to_str()?
        );

        assert_eq!("86400", resp.headers().get(ACCESS_CONTROL_MAX_AGE).unwrap());

        let allow_methods = resp
            .headers()
            .get(ACCESS_CONTROL_ALLOW_METHODS)
            .unwrap()
            .to_str()?;
        assert!(allow_methods.contains("POST"));
        assert!(allow_methods.contains("GET"));
        assert!(allow_methods.contains("PUT"));

        let allow_headers = resp
            .headers()
            .get(ACCESS_CONTROL_ALLOW_HEADERS)
            .unwrap()
            .to_str()?;
        assert!(allow_headers.contains(CONTENT_TYPE.as_str()));
        assert!(allow_headers.contains(AUTHORIZATION.as_str()));
        assert_eq!("", resp.text().await?);
        //
        Ok(())
    }
}
