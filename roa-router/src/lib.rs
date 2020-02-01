mod endpoint;
mod err;
mod path;

pub use endpoint::Endpoint;
pub use err::Conflict;
pub use path::{Path, RegexPath};
use roa_core::{Context, Middleware, Model, Next, Status};

use crate::err::Error;
use roa_query::Variable;
use std::future::Future;

enum Node<M: Model> {
    Router(Router<M>),
    Endpoint(Endpoint<M>),
}

impl<M: Model> Node<M> {
    fn unwrap_router(&mut self) -> &mut Router<M> {
        match self {
            Node::Router(router) => router,
            _ => panic!(
                r"Node is not a router, 
                  This is a bug of roa-router::Router, please report it to https://github.com/Hexilee/roa
            "
            ),
        }
    }

    fn unwrap_endpoint(&mut self) -> &mut Endpoint<M> {
        match self {
            Node::Endpoint(endpoint) => endpoint,
            _ => panic!(
                r"Node is not a endpoint,
                  This is a bug of roa-router::Router, please report it to https://github.com/Hexilee/roa
            "
            ),
        }
    }
}

pub struct Router<M: Model> {
    root: Path,
    middleware: Middleware<M>,
    nodes: Vec<Node<M>>,
}

impl<M: Model> Router<M> {
    pub fn new(path: Path) -> Self {
        Self {
            root: path,
            middleware: Middleware::new(),
            nodes: Vec::new(),
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
        let index = self.nodes.len();
        self.nodes.push(Node::Endpoint(endpoint));
        Ok(self.nodes[index].unwrap_endpoint())
    }

    pub fn route(&mut self, path: &'static str) -> Result<&mut Router<M>, Error> {
        let router = Router::new(self.join_path(path).parse()?);
        let index = self.nodes.len();
        self.nodes.push(Node::Router(router));
        Ok(self.nodes[index].unwrap_router())
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
