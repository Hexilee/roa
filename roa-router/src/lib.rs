mod endpoint;
mod err;
mod path;

pub use endpoint::Endpoint;
pub use err::Conflict;
pub use path::{Path, RegexPath};
use roa_core::{Context, Middleware, Model, Next, Status};

use crate::err::Error;
use http::Method;
use roa_query::Variable;
use std::future::Future;

pub struct Router<M: Model> {
    root: Path,
    middleware: Middleware<M>,
    routers: Vec<Router<M>>,
    endpoints: Vec<Endpoint<M>>,
}

impl<M: Model> Router<M> {
    pub fn new(path: Path) -> Self {
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

    fn join_path(&self, path: &str) -> String {
        vec![self.root.raw(), path.trim_matches('/')].join("/")
    }

    pub fn on(&mut self, path: &'static str) -> Result<&mut Endpoint<M>, Error> {
        let endpoint = Endpoint::new(self.join_path(path).parse()?);
        let index = self.endpoints.len();
        self.endpoints.push(endpoint);
        Ok(&mut self.endpoints[index])
    }

    pub fn route(&mut self, path: &'static str) -> Result<&mut Router<M>, Error> {
        let router = Router::new(self.join_path(path).parse()?);
        let index = self.routers.len();
        self.routers.push(router);
        Ok(&mut self.routers[index])
    }
}

#[cfg(test)]
mod tests {
    use crate::Router;
    use roa_body::PowerBody;
    #[test]
    fn handle() -> Result<(), Box<dyn std::error::Error>> {
        let mut router = Router::new("/".parse()?);
        router
            .on("/file/:filename")?
            .join(|_ctx, next| next())
            .get(|mut ctx| ctx.write_file("filename"));
    }
}
