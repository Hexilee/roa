use crate::{last, Context, Next, Result, State};
use async_std::sync::Arc;
use async_trait::async_trait;
use std::future::Future;

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
///       F: 'static + Send + Future<Output=Result> {
///     middleware
/// }
///
/// is_normal(|_ctx: Context<()>, next| next());
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
///     next().await
/// }
///
/// // capture a variable to avoid matching lambda as function pointer.
/// let x = 0;
/// // moving is necessary to confirm closure is static.
/// let closure = move |ctx: Context<()>, next: Next| async move {
///     let x = x;
///     next().await
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
/// where F: 'static + Send + Future<Output=Result> {
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
/// use roa_core::{State, Middleware, async_trait, Context, Next, Result};
/// use async_std::sync::Arc;
/// use std::time::Instant;
///
/// fn is_middleware(_: impl Middleware<()>) {}
///
/// struct Logger;
///
/// #[async_trait]
/// impl <S: State> Middleware<S> for Logger {
///     async fn handle(self:Arc<Self>, ctx: Context<S>, next: Next) -> Result {
///         let start = Instant::now();
///         let result = next().await;
///         println!("time elapsed: {}ms", start.elapsed().as_millis());
///         result
///     }
/// }
///
/// is_middleware(Logger);
/// ```
#[async_trait]
pub trait Middleware<S: State>: 'static + Sync + Send {
    /// Handle context and next, then return a future to get status.
    async fn handle(self: Arc<Self>, ctx: Context<S>, next: Next) -> Result;

    /// Handle context as an endpoint.
    async fn end(self: Arc<Self>, ctx: Context<S>) -> Result {
        self.handle(ctx, Box::new(last)).await
    }
}

#[async_trait]
impl<S, F, T> Middleware<S> for T
where
    S: State,
    T: 'static + Sync + Send + Fn(Context<S>, Next) -> F,
    F: 'static + Future<Output = Result> + Send,
{
    #[inline]
    async fn handle(self: Arc<Self>, ctx: Context<S>, next: Next) -> Result {
        (self)(ctx, next).await
    }
}

#[async_trait]
impl<S, F> Middleware<S> for fn(Context<S>) -> F
where
    S: State,
    F: 'static + Future<Output = Result> + Send,
{
    #[inline]
    async fn handle(self: Arc<Self>, ctx: Context<S>, _next: Next) -> Result {
        (self)(ctx).await
    }
}
