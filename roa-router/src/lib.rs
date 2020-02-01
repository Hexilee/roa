mod endpoint;
mod err;
mod path;

pub use endpoint::Endpoint;
pub use err::Conflict;
pub use path::{Path, RegexPath};
use roa_core::{Context, Middleware, Model, Next, Status};

use http::Method;
use roa_query::Variable;
use std::future::Future;

pub struct Router<M: Model> {
    root: &'static str,
    middleware: Middleware<M>,
    routers: Vec<Router<M>>,
    endpoints: Vec<Endpoint<M>>,
}

impl<M: Model> Router<M> {
    pub fn new(path: &'static str) -> Self {
        Self {
            root: path,
            middleware: Middleware::new(),
            routers: Vec::new(),
            endpoints: Vec::new(),
        }
    }

    pub fn join<F>(
        &mut self,
        middleware: impl 'static + Sync + Send + Fn(Context<M>, Next) -> F,
    ) -> &mut Self
    where
        F: 'static + Future<Output = Result<(), Status>> + Send,
    {
        self.middleware.join(middleware);
        self
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
    use roa_body::PowerBody;
    #[test]
    fn handle() {
        let mut router = Router::new("/");
        router
            .on("/file/:filename")
            .join(|_ctx, next| next())
            .get(|mut ctx| ctx.write_file("filename"));
    }
}
