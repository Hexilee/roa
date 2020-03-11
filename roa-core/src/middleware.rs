use crate::{async_trait, Context, Next, Result, State};
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
pub trait Endpoint<'a, S: 'a>: 'static + Sync + Send {
    #[inline]
    async fn call(&'a self, ctx: &'a mut Context<S>) -> Result;
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

// impl<'a, S, F> Middleware<S> for fn(&'a mut Context<S>, &'a mut dyn Next) -> F
// where
//     F: 'a + Future<Output = Result>,
// {
//     #[inline]
//     fn handle<'life0, 'life1, 'life2, 'async_trait>(
//         &'life0 self,
//         ctx: &'life1 mut Context<S>,
//         next: &'life2 mut dyn Next,
//     ) -> ::core::pin::Pin<Box<dyn ::core::future::Future<Output = Result> + 'async_trait>>
//     where
//         'life0: 'async_trait,
//         'life1: 'async_trait,
//         'life2: 'async_trait,
//         'a: 'async_trait,
//         Self: 'async_trait,
//     {
//         Box::pin((self)(ctx, next))
//     }
// }

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

// impl<'a, S, F> Middleware<S> for fn(&'a mut Context<S>) -> F
// where
//     F: 'a + Future<Output = Result>,
// {
//     #[inline]
//     fn handle<'life0, 'life1, 'life2, 'async_trait>(
//         &'life0 self,
//         ctx: &'life1 mut Context<S>,
//         _next: &'life2 mut dyn Next,
//     ) -> ::core::pin::Pin<Box<dyn ::core::future::Future<Output = Result> + 'async_trait>>
//     where
//         'life0: 'async_trait,
//         'life1: 'async_trait,
//         'life2: 'async_trait,
//         Self: 'async_trait,
//     {
//         Box::pin(self(ctx))
//     }
// }
