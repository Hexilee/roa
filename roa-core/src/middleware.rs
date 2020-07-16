use crate::{async_trait, throw, Context, Result, Status};
use http::header::LOCATION;
use http::{StatusCode, Uri};
use std::future::Future;

/// ### Middleware
///
/// #### Build-in middlewares
///
/// - Functional middleware
///
/// A functional middleware is an async function with signature:
/// `async fn(&mut Context, Next<'_>) -> Result`.
///
/// ```rust
/// use roa_core::{App, Context, Next, Result};
///
/// async fn middleware(ctx: &mut Context, next: Next<'_>) -> Result {
///     next.await
/// }
///
/// let app = App::new().gate(middleware);
/// ```
///
/// - Blank middleware
///
/// `()` is a blank middleware, it just calls the next middleware or endpoint.
///
/// ```rust
/// let app = roa_core::App::new().gate(());
/// ```
///
/// #### Custom middleware
///
/// You can implement custom `Middleware` for other types.
///
/// ```rust
/// use roa_core::{App, Middleware, Context, Next, Result, async_trait};
/// use std::sync::Arc;
/// use std::time::Instant;
///
///
/// struct Logger;
///
/// #[async_trait(?Send)]
/// impl <'a> Middleware<'a> for Logger {
///     async fn handle(&'a self, ctx: &'a mut Context, next: Next<'a>) -> Result {
///         let start = Instant::now();
///         let result = next.await;
///         println!("time elapsed: {}ms", start.elapsed().as_millis());
///         result
///     }
/// }
///
/// let app = App::new().gate(Logger);
/// ```
#[cfg_attr(feature = "docs", doc(spotlight))]
#[async_trait(?Send)]
pub trait Middleware<'a, S = ()>: 'static + Sync + Send {
    /// Handle context and next, return status.
    async fn handle(&'a self, ctx: &'a mut Context<S>, next: Next<'a>) -> Result;
}

#[async_trait(?Send)]
impl<'a, S, T, F> Middleware<'a, S> for T
where
    S: 'a,
    T: 'static + Send + Sync + Fn(&'a mut Context<S>, Next<'a>) -> F,
    F: 'a + Future<Output = Result>,
{
    #[inline]
    async fn handle(&'a self, ctx: &'a mut Context<S>, next: Next<'a>) -> Result {
        (self)(ctx, next).await
    }
}

/// ### Endpoint
///
/// An endpoint is a request handler.
///
/// #### Build-in endpoint
///
/// There are some build-in endpoints.
///
/// - Functional endpoint
///
/// A normal functional endpoint is an async function with signature:
/// `async fn(&mut Context) -> Result`.
///
/// ```rust
/// use roa_core::{App, Context, Result};
///
/// async fn endpoint(ctx: &mut Context) -> Result {
///     Ok(())
/// }
///
/// let app = App::new().end(endpoint);
/// ```
/// - Ok endpoint
///
/// `()` is an endpoint always return `Ok(())`
///
/// ```rust
/// let app = roa_core::App::new().end(());
/// ```
///
/// - Status endpoint
///
/// `Status` is an endpoint always return `Err(Status)`
///
/// ```rust
/// use roa_core::{App, status};
/// use roa_core::http::StatusCode;
/// let app = App::new().end(status!(StatusCode::BAD_REQUEST));
/// ```
///
/// - String endpoint
///
/// Write string to body.
///
/// ```rust
/// use roa_core::App;
///
/// let app = App::new().end("Hello, world"); // static slice
/// let app = App::new().end("Hello, world".to_owned()); // string
/// ```
///
/// - Redirect endpoint
///
/// Redirect to an uri.
///
/// ```rust
/// use roa_core::App;
/// use roa_core::http::Uri;
///
/// let app = App::new().end("/target".parse::<Uri>().unwrap());
/// ```
///
/// #### Custom endpoint
///
/// You can implement custom `Endpoint` for your types.
///
/// ```rust
/// use roa_core::{App, Endpoint, Context, Next, Result, async_trait};
/// use async_std::sync::Arc;
/// use std::time::Instant;
///
/// fn is_endpoint(endpoint: impl for<'a> Endpoint<'a>) {
/// }
///
/// struct Logger;
///
/// #[async_trait(?Send)]
/// impl <'a> Endpoint<'a> for Logger {
///     async fn call(&'a self, ctx: &'a mut Context) -> Result {
///         Ok(())
///     }
/// }
///
/// let app = App::new().end(Logger);
/// ```
#[cfg_attr(feature = "docs", doc(spotlight))]
#[async_trait(?Send)]
pub trait Endpoint<'a, S = ()>: 'static + Sync + Send {
    /// Call this endpoint.
    async fn call(&'a self, ctx: &'a mut Context<S>) -> Result;
}

#[async_trait(?Send)]
impl<'a, S, T, F> Endpoint<'a, S> for T
where
    S: 'a,
    T: 'static + Send + Sync + Fn(&'a mut Context<S>) -> F,
    F: 'a + Future<Output = Result>,
{
    #[inline]
    async fn call(&'a self, ctx: &'a mut Context<S>) -> Result {
        (self)(ctx).await
    }
}

/// blank middleware.
#[async_trait(?Send)]
impl<'a, S> Middleware<'a, S> for () {
    #[allow(clippy::trivially_copy_pass_by_ref)]
    #[inline]
    async fn handle(&'a self, _ctx: &'a mut Context<S>, next: Next<'a>) -> Result {
        next.await
    }
}

/// ok endpoint, always return Ok(())
#[async_trait(?Send)]
impl<'a, S> Endpoint<'a, S> for () {
    #[allow(clippy::trivially_copy_pass_by_ref)]
    #[inline]
    async fn call(&'a self, _ctx: &'a mut Context<S>) -> Result {
        Ok(())
    }
}

/// status endpoint.
#[async_trait(?Send)]
impl<'a, S> Endpoint<'a, S> for Status {
    #[inline]
    async fn call(&'a self, _ctx: &'a mut Context<S>) -> Result {
        Err(self.clone())
    }
}

/// String endpoint.
#[async_trait(?Send)]
impl<'a, S> Endpoint<'a, S> for String {
    #[inline]
    #[allow(clippy::ptr_arg)]
    async fn call(&'a self, ctx: &'a mut Context<S>) -> Result {
        ctx.resp.write(self.clone());
        Ok(())
    }
}

/// Static slice endpoint.
#[async_trait(?Send)]
impl<'a, S> Endpoint<'a, S> for &'static str {
    #[inline]
    async fn call(&'a self, ctx: &'a mut Context<S>) -> Result {
        ctx.resp.write(*self);
        Ok(())
    }
}

/// Redirect endpoint.
#[async_trait(?Send)]
impl<'a, S> Endpoint<'a, S> for Uri {
    #[inline]
    async fn call(&'a self, ctx: &'a mut Context<S>) -> Result {
        ctx.resp.headers.insert(LOCATION, self.to_string().parse()?);
        throw!(StatusCode::PERMANENT_REDIRECT)
    }
}

/// Type of the second parameter in a middleware,
/// an alias for `&mut (dyn Unpin + Future<Output = Result>)`
///
/// Developer of middleware can jump to next middleware by calling `next.await`.
///
/// ### Example
///
/// ```rust
/// use roa_core::{App, Context, Result, Status, MiddlewareExt, Next};
/// use roa_core::http::StatusCode;
///
/// let app = App::new()
///     .gate(first)
///     .gate(second)
///     .gate(third)
///     .end(end);
/// async fn first(ctx: &mut Context, next: Next<'_>) -> Result {
///     assert!(ctx.store("id", "1").is_none());
///     next.await?;
///     assert_eq!("5", *ctx.load::<&'static str>("id").unwrap());
///     Ok(())
/// }
/// async fn second(ctx: &mut Context, next: Next<'_>) -> Result {
///     assert_eq!("1", *ctx.load::<&'static str>("id").unwrap());
///     assert_eq!("1", *ctx.store("id", "2").unwrap());
///     next.await?;
///     assert_eq!("4", *ctx.store("id", "5").unwrap());
///     Ok(())
/// }
/// async fn third(ctx: &mut Context, next: Next<'_>) -> Result {
///     assert_eq!("2", *ctx.store("id", "3").unwrap());
///     next.await?; // next is none; do nothing
///     assert_eq!("3", *ctx.store("id", "4").unwrap());
///     Ok(())
/// }
///
/// async fn end(ctx: &mut Context) -> Result {
///     assert_eq!("3", *ctx.load::<&'static str>("id").unwrap());
///     Ok(())
/// }
/// ```
///
/// ### Error Handling
///
/// You can catch or straightly throw a Error returned by next.
///
/// ```rust
/// use roa_core::{App, Context, Result, Status, MiddlewareExt, Next, status};
/// use roa_core::http::StatusCode;
///         
/// let app = App::new()
///     .gate(catch)
///     .gate(gate)
///     .end(status!(StatusCode::IM_A_TEAPOT, "I'm a teapot!"));
///
/// async fn catch(ctx: &mut Context, next: Next<'_>) -> Result {
///     // catch
///     if let Err(err) = next.await {
///         // teapot is ok
///         if err.status_code != StatusCode::IM_A_TEAPOT {
///             return Err(err);
///         }
///     }
///     Ok(())
/// }
/// async fn gate(ctx: &mut Context, next: Next<'_>) -> Result {
///     next.await?; // just throw
///     unreachable!()
/// }
/// ```
///
pub type Next<'a> = &'a mut (dyn Unpin + Future<Output = Result>);

#[cfg(test)]
mod tests {
    use crate::{status, App, Request};
    use futures::{AsyncReadExt, TryStreamExt};
    use http::header::LOCATION;
    use http::{StatusCode, Uri};

    const HELLO: &str = "Hello, world";

    #[async_std::test]
    async fn status_endpoint() {
        let app = App::new().end(status!(StatusCode::BAD_REQUEST));
        let service = app.http_service();
        let resp = service.serve(Request::default()).await;
        assert_eq!(StatusCode::BAD_REQUEST, resp.status);
    }

    #[async_std::test]
    async fn string_endpoint() {
        let app = App::new().end(HELLO.to_owned());
        let service = app.http_service();
        let mut data = String::new();
        service
            .serve(Request::default())
            .await
            .body
            .into_async_read()
            .read_to_string(&mut data)
            .await
            .unwrap();
        assert_eq!(HELLO, data);
    }
    #[async_std::test]
    async fn static_slice_endpoint() {
        let app = App::new().end(HELLO);
        let service = app.http_service();
        let mut data = String::new();
        service
            .serve(Request::default())
            .await
            .body
            .into_async_read()
            .read_to_string(&mut data)
            .await
            .unwrap();
        assert_eq!(HELLO, data);
    }
    #[async_std::test]
    async fn redirect_endpoint() {
        let app = App::new().end("/target".parse::<Uri>().unwrap());
        let service = app.http_service();
        let resp = service.serve(Request::default()).await;
        assert_eq!(StatusCode::PERMANENT_REDIRECT, resp.status);
        assert_eq!("/target", resp.headers[LOCATION].to_str().unwrap())
    }
}
