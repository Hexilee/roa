use crate::{async_trait, Context, Result, State};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// ### Middleware
///
/// There are two kinds of middlewares,
/// the one is functional middlewares, the another is trait middlewares.
///
/// #### Normal Functional Middlewares
///
/// A normal functional middleware is an object implements `Fn` trait:
///
/// ```rust
/// use roa_core::{Context, Next, Result, State, Middleware};
/// use std::future::Future;
///
/// fn is_normal<S, F>(
///     middleware: impl 'static + Send + Sync + Fn(Context<S>, Next) -> F
/// ) -> impl Middleware<S>
/// where S: State,
///       F: 'static + Future<Output=Result> {
///     middleware
/// }
///
/// is_normal(|_ctx: Context<()>, next| next);
/// ```
///
/// Both of function pointers and closures are middlewares:
///
/// ```rust
/// use roa_core::{Middleware, Context, Next, Result, State};
///
/// fn is_middleware<S: State>(_: impl Middleware<S>) {}
///
/// async fn function_ptr(_ctx: Context<()>, next: Next) -> Result {
///     next.await
/// }
///
/// // capture a variable to avoid matching lambda as function pointer.
/// let x = 0;
/// // moving is necessary to confirm closure is static.
/// let closure = move |ctx: Context<()>, next: Next| async move {
///     let x = x;
///     next.await
/// };
///
/// is_middleware(function_ptr);
/// is_middleware(closure);
/// ```
///
/// #### Endpoints
///
/// Another kind of functional middlewares is endpoints,
/// whose type is `fn<S, F>(Context<S>) -> F where F: 'static + Send + Future<Output=Result>`.
///
/// Endpoints never invoke next middleware.
///
/// ```rust
/// use roa_core::{Middleware, Context, Result, State};
/// use std::future::Future;
///
/// fn is_middleware<S: State>(_: impl Middleware<S>) {}
///
/// async fn endpoint(_: Context<()>) -> Result {
///     Ok(())
/// }
/// // `fn(Context<()>) -> impl 'static + Send + Future<Output=Result>` is a function pointer
/// // which returns value of a existential type.
/// //
/// // `fn<F>(Context<()>) -> F where F: 'static + Send + Future<Output=Result>` is a template.
/// //
/// // They are different!
/// //
/// // is_middleware(endpoint); compile fails!
///
/// fn to_middleware<F>(middleware: fn(Context<()>) -> F) -> impl Middleware<()>
/// where F: 'static + Future<Output=Result> {
///     middleware
/// }
///
/// is_middleware(to_middleware(endpoint))
/// ```
///
/// #### Trait Middlewares
///
/// A trait middleware is an object implementing trait `Middleware`.
///
/// ```rust
/// use roa_core::{State, Middleware, Context, Next, Result, async_trait};
/// use async_std::sync::Arc;
/// use std::time::Instant;
///
/// fn is_middleware(_: impl Middleware<()>) {}
///
/// struct Logger;
///
/// #[async_trait(?Send)]
/// impl <S: State> Middleware<S> for Logger {
///     async fn handle(self:Arc<Self>, ctx: Context<S>, next: Next) -> Result {
///         let start = Instant::now();
///         let result = next.await;
///         println!("time elapsed: {}ms", start.elapsed().as_millis());
///         result
///     }
/// }
///
/// is_middleware(Logger);
/// ```
#[async_trait(?Send)]
pub trait Middleware<'a, S: 'a>: 'static + Sync + Send {
    /// Handle context and next, then return a future to get status.
    async fn handle(&'a self, ctx: &'a mut Context<S>, next: &'a mut dyn Next)
        -> Result;
}

#[async_trait(?Send)]
impl<'a, S, T, F> Middleware<'a, S> for T
where
    S: 'a,
    T: 'static + Send + Sync + Fn(&'a mut Context<S>, &'a mut dyn Next) -> F,
    F: 'a + Future<Output = Result>,
{
    #[inline]
    async fn handle(
        &'a self,
        ctx: &'a mut Context<S>,
        next: &'a mut dyn Next,
    ) -> Result {
        (self)(ctx, next).await
    }
}

#[async_trait(?Send)]
pub trait Endpoint<'a, S: 'a>: 'static + Sync + Send {
    #[inline]
    async fn end(&'a self, ctx: &'a mut Context<S>) -> Result;
}

#[async_trait(?Send)]
impl<'a, S, T, F> Endpoint<'a, S> for T
where
    S: 'a,
    T: 'static + Send + Sync + Fn(&'a mut Context<S>) -> F,
    F: 'a + Future<Output = Result>,
{
    #[inline]
    async fn end(&'a self, ctx: &'a mut Context<S>) -> Result {
        (self)(ctx).await
    }
}

/// Type of the second parameter in a middleware.
///
/// `Next` is usually a closure capturing the next middleware, context and the next `Next`.
///
/// Developer of middleware can jump to next middleware by calling `next.await`.
///
/// ### Example
///
/// ```rust
/// use roa_core::App;
/// use roa_core::http::StatusCode;
///
/// let mut app = App::new(());
/// app.gate_fn(|mut ctx, next| async move {
///     ctx.store("id", "1".to_string());
///     next.await?;
///     assert_eq!("5", &*ctx.load::<String>("id").unwrap());
///     Ok(())
/// });
/// app.gate_fn(|mut ctx, next| async move {
///     assert_eq!("1", &*ctx.load::<String>("id").unwrap());
///     ctx.store("id", "2".to_string());
///     next.await?;
///     assert_eq!("4", &*ctx.load::<String>("id").unwrap());
///     ctx.store("id", "5".to_string());
///     Ok(())
/// });
/// app.gate_fn(|mut ctx, next| async move {
///     assert_eq!("2", &*ctx.load::<String>("id").unwrap());
///     ctx.store("id", "3".to_string());
///     next.await?; // next is none; do nothing
///     assert_eq!("3", &*ctx.load::<String>("id").unwrap());
///     ctx.store("id", "4".to_string());
///     Ok(())
/// });
/// ```
///
/// ### Error Handling
///
/// You can catch or straightly throw a Error returned by next.
///
/// ```rust
/// use roa_core::{App, throw};
/// use roa_core::http::StatusCode;
///
/// let mut app = App::new(());
/// app.gate_fn(|ctx, next| async move {
///     // catch
///     if let Err(err) = next.await {
///         // teapot is ok
///         if err.status_code != StatusCode::IM_A_TEAPOT {
///             return Err(err);
///         }
///     }
///     Ok(())
/// });
/// app.gate_fn(|ctx, next| async move {
///     next.await?; // just throw
///     unreachable!()
/// });
/// app.gate_fn(|_ctx, _next| async move {
///     throw!(StatusCode::IM_A_TEAPOT, "I'm a teapot!")
/// });
/// ```
///
pub trait Next: Future<Output = Result<()>> {}
impl<T> Next for T where T: Future<Output = Result<()>> {}
