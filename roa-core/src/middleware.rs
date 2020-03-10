use crate::{async_trait, last, Context, Next, Result, State};
use std::future::Future;
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
pub trait Middleware<S>: 'static + Sync + Send {
    /// Handle context and next, then return a future to get status.
    async fn handle(&self, ctx: &mut Context<S>, next: &mut dyn Next) -> Result;

    /// Handle context as an endpoint.
    #[inline]
    async fn end(&self, ctx: &mut Context<S>) -> Result {
        self.handle(ctx, &mut last()).await
    }
}

// #[async_trait(?Send)]
// impl<'a, S, F> Middleware<S> for fn(&'a mut Context<S>, &'a mut dyn Next) -> F
// where
//     F: 'a + Future<Output = Result>,
// {
//     #[inline]
//     async fn handle(&self, ctx: &mut Context<S>, next: &mut dyn Next) -> Result {
//         (self)(ctx, next).await
//     }
// }

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
//         Self: 'async_trait,
//     {
//         #[allow(
//             clippy::missing_docs_in_private_items,
//             clippy::type_repetition_in_bounds,
//             clippy::used_underscore_binding
//         )]
//         async fn __handle<'a, S, F>(
//             _self: &fn(&'a mut Context<S>, &'a mut dyn Next) -> F,
//             ctx: &mut Context<S>,
//             next: &mut dyn Next,
//         ) -> Result
//         where
//             (): Sized,
//             F: 'a + Future<Output = Result>,
//         {
//             (_self)(ctx, next).await
//         }
//         Box::pin(__handle::<S, F>(self, ctx, next))
//     }
// }

// #[async_trait(?Send)]
// impl<'a, S, F> Middleware<S> for fn(&'a mut Context<S>) -> F
// where
//     F: 'a + Future<Output = Result>,
// {
//     #[inline]
//     async fn handle(&self, ctx: &mut Context<S>, _next: &mut dyn Next) -> Result {
//         (self)(ctx).await
//     }
// }
