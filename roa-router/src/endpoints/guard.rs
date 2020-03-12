use crate::endpoints::method_not_allowed;
use roa_core::http::Method;
use roa_core::{async_trait, Context, Endpoint, Result};
use std::collections::HashSet;
use std::iter::FromIterator;

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

pub struct Guard<E> {
    white_list: HashSet<Method>,
    endpoint: E,
}

fn hash_set(methods: impl AsRef<[Method]>) -> HashSet<Method> {
    HashSet::from_iter(methods.as_ref().to_vec())
}

pub fn allow<E>(methods: impl AsRef<[Method]>, endpoint: E) -> Guard<E> {
    Guard {
        endpoint,
        white_list: hash_set(methods),
    }
}

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
    async fn call(&'a self, ctx: &'a mut Context<S>) -> Result {
        if self.white_list.contains(ctx.method()) {
            self.endpoint.call(ctx).await
        } else {
            method_not_allowed(ctx.method())
        }
    }
}
