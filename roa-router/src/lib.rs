mod endpoint;
mod err;
mod parse;
mod path;

pub use endpoint::Endpoint;
pub use err::{Conflict, Error};
pub use path::{Path, RegexPath};
use roa_core::{Context, DynTargetHandler, Middleware, Model, Next, Status};

use roa_query::Variable;
use std::future::Future;
use std::sync::Arc;

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
    root: String,
    middleware: Middleware<M>,
    nodes: Vec<Node<M>>,
}

impl<M: Model> Router<M> {
    pub fn new(path: impl ToString) -> Self {
        Self {
            root: path.to_string(),
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
        vec![self.root.as_str(), path.trim_matches('/')].join("/")
    }

    pub fn on(&mut self, path: &'static str) -> Result<&mut Endpoint<M>, Error> {
        let endpoint = Endpoint::new(self.join_path(path).parse()?);
        let index = self.nodes.len();
        self.nodes.push(Node::Endpoint(endpoint));
        Ok(self.nodes[index].unwrap_endpoint())
    }

    pub fn route(&mut self, path: &'static str) -> &mut Router<M> {
        let router = Router::new(self.join_path(path));
        let index = self.nodes.len();
        self.nodes.push(Node::Router(router));
        self.nodes[index].unwrap_router()
    }

    //    pub fn handler(self) -> Result<Arc<DynTargetHandler<M, Next>>, Conflict> {
    //        let Self {
    //            root,
    //            mut middleware,
    //            nodes,
    //        } = self;
    //        let raw_path = path.raw().to_string();
    //        let mut map = HashMap::new();
    //        for (method, handler) in handlers {
    //            if let Some(_) = map.insert(method.clone(), handler) {
    //                return Err(Conflict::Method(raw_path.clone(), method));
    //            };
    //        }
    //
    //        let map = Arc::new(map);
    //        middleware.join(move |ctx, _next| {
    //            let map = map.clone();
    //            let raw_path = raw_path.clone();
    //            async move {
    //                match map.get(&ctx.request.method) {
    //                    None => throw(
    //                        StatusCode::METHOD_NOT_ALLOWED,
    //                        format!(
    //                            "method {} is not allowed on {}",
    //                            &ctx.request.method, raw_path
    //                        ),
    //                    ),
    //                    Some(handler) => handler(ctx).await,
    //                }
    //            }
    //        });
    //        Ok(middleware.handler())
    //    }
}

#[cfg(test)]
mod tests {
    use crate::Router;
    use roa_body::PowerBody;
    #[test]
    fn handle() -> Result<(), Box<dyn std::error::Error>> {
        let mut router = Router::new("/");
        router
            .on("/file/:filename")?
            .join(|_ctx, next| next())
            .get(|mut ctx| ctx.write_file("filename"));
    }
}
