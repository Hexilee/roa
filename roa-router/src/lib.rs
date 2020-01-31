mod endpoint;
mod path;

pub use endpoint::Endpoint;
pub use path::{Path, RegexPath};
use roa_core::{Context, DynHandler, Handler, Middleware, Model, Status};

use http::Method;
use roa_query::Variable;
use std::future::Future;
use std::sync::Arc;

pub struct Router<M: Model> {
    root: &'static str,
    routers: Vec<Router<M>>,
    endpoints: Vec<Endpoint<M>>,
}

impl<M: Model> Router<M> {
    pub fn new(path: &'static str) -> Self {
        Self {
            root: path,
            routers: Vec::new(),
            endpoints: Vec::new(),
        }
    }

    pub fn on(&mut self, path: &'static str) -> &mut Endpoint<M> {
        let endpoint = Endpoint::new(path);
        let index = self.endpoints.len();
        self.endpoints.push(endpoint);
        &mut self.endpoints[index]
    }

    pub fn route(&mut self, path: &'static str) -> &mut Router<M> {
        let router = Router::new(path);
        let index = self.routers.len();
        self.routers.push(router);
        &mut self.routers[index]
    }
}

#[cfg(test)]
mod tests {
    use crate::Router;
    #[test]
    fn handle() {
        //        Router::new("/").on("/id");
    }
}
