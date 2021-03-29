use std::collections::HashSet;
use std::iter::FromIterator;

use super::method_not_allowed;
use crate::http::Method;
use crate::{async_trait, Context, Endpoint, Result};

/// Methods allowed in `Guard`.
const ALL_METHODS: [Method; 9] = [
    Method::GET,
    Method::POST,
    Method::PUT,
    Method::PATCH,
    Method::OPTIONS,
    Method::DELETE,
    Method::HEAD,
    Method::TRACE,
    Method::CONNECT,
];

/// An endpoint wrapper to guard endpoint by http method.
pub struct Guard<E> {
    white_list: HashSet<Method>,
    endpoint: E,
}

/// Initialize hash set.
fn hash_set(methods: impl AsRef<[Method]>) -> HashSet<Method> {
    HashSet::from_iter(methods.as_ref().to_vec())
}

/// A function to construct guard by white list.
///
/// Only requests with http method in list can access this endpoint, otherwise will get a 405 METHOD NOT ALLOWED.
///
/// ```
/// use roa::{App, Context, Result};
/// use roa::http::Method;
/// use roa::router::allow;
///
/// async fn foo(ctx: &mut Context) -> Result {
///     Ok(())
/// }
///
/// let app = App::new().end(allow([Method::GET, Method::POST], foo));
/// ```
pub fn allow<E>(methods: impl AsRef<[Method]>, endpoint: E) -> Guard<E> {
    Guard {
        endpoint,
        white_list: hash_set(methods),
    }
}

/// A function to construct guard by black list.
///
/// Only requests with http method not in list can access this endpoint, otherwise will get a 405 METHOD NOT ALLOWED.
///
/// ```
/// use roa::{App, Context, Result};
/// use roa::http::Method;
/// use roa::router::deny;
///
/// async fn foo(ctx: &mut Context) -> Result {
///     Ok(())
/// }
///
/// let app = App::new().end(deny([Method::PUT, Method::DELETE], foo));
/// ```
pub fn deny<E>(methods: impl AsRef<[Method]>, endpoint: E) -> Guard<E> {
    let white_list = hash_set(ALL_METHODS);
    let black_list = &white_list & &hash_set(methods);
    Guard {
        endpoint,
        white_list: &white_list ^ &black_list,
    }
}

#[async_trait(?Send)]
impl<'a, S, E> Endpoint<'a, S> for Guard<E>
where
    E: Endpoint<'a, S>,
{
    #[inline]
    async fn call(&'a self, ctx: &'a mut Context<S>) -> Result {
        if self.white_list.contains(ctx.method()) {
            self.endpoint.call(ctx).await
        } else {
            method_not_allowed(ctx.method())
        }
    }
}
