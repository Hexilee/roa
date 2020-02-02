mod endpoint;
mod err;
mod path;

pub use endpoint::Endpoint;
pub use err::{Conflict, Error};
pub use path::{join_path, standardize_path, Path, RegexPath};

use crate::{throw, Context, DynTargetHandler, Middleware, Model, Next, Status, TargetHandler};
use http::StatusCode;
use percent_encoding::percent_decode_str;
use radix_trie::Trie;
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

    pub fn gate<F>(
        &mut self,
        middleware: impl 'static + Sync + Send + Fn(Context<M>, Next) -> F,
    ) -> &mut Self
    where
        F: 'static + Future<Output = Result<(), Status>> + Send,
    {
        self.middleware.join(middleware);
        self
    }

    pub fn on(&mut self, path: &'static str) -> Result<&mut Endpoint<M>, Error> {
        let endpoint = Endpoint::new(join_path([self.root.as_str(), path].as_ref()).parse()?);
        let index = self.nodes.len();
        self.nodes.push(Node::Endpoint(endpoint));
        Ok(self.nodes[index].unwrap_endpoint())
    }

    pub fn route(&mut self, path: &'static str) -> &mut Router<M> {
        let router = Router::new(join_path([self.root.as_str(), path].as_ref()));
        let index = self.nodes.len();
        self.nodes.push(Node::Router(router));
        self.nodes[index].unwrap_router()
    }

    fn endpoints(self) -> Vec<Endpoint<M>> {
        let Self {
            root: _,
            middleware,
            nodes,
        } = self;
        let mut endpoints = Vec::new();
        for node in nodes {
            match node {
                Node::Endpoint(endpoint) => {
                    endpoints.push(endpoint);
                }
                Node::Router(router) => endpoints.extend(router.endpoints().into_iter()),
            };
        }

        for endpoint in endpoints.iter_mut() {
            let mut new_middleware = Middleware::new();
            let root_middleware = middleware.handler();
            let current_middleware = endpoint.middleware.handler();
            new_middleware.join(move |ctx, next| root_middleware(ctx, next));
            new_middleware.join(move |ctx, next| current_middleware(ctx, next));
            endpoint.middleware = new_middleware;
        }
        endpoints
    }

    pub fn handler(self) -> Result<Box<DynTargetHandler<M, Next>>, Conflict> {
        let endpoints = self.endpoints();
        let mut static_route = Trie::new();
        let mut dynamic_route = Vec::new();
        for endpoint in endpoints {
            match &*endpoint.path.clone() {
                Path::Static(path) => {
                    if let Some(_) = static_route.insert(path.to_string(), endpoint.handler()?) {
                        return Err(Conflict::Path(path.to_string()));
                    }
                }
                Path::Dynamic(regex_path) => {
                    dynamic_route.push((regex_path.clone(), endpoint.handler()?))
                }
            }
        }

        let static_route = Arc::new(static_route);
        let dynamic_route = Arc::new(dynamic_route);

        let handler = move |ctx: Context<M>, next| {
            let static_route = static_route.clone();
            let dynamic_route = dynamic_route.clone();
            async move {
                let uri = ctx.uri().await;
                let path =
                    standardize_path(&percent_decode_str(uri.path()).decode_utf8().map_err(
                        |err| {
                            Status::new(
                                StatusCode::BAD_REQUEST,
                                format!(
                                    "{}\npath `{}` is not a valid utf-8 string",
                                    err,
                                    uri.path()
                                ),
                                true,
                            )
                        },
                    )?);
                if let Some(handler) = static_route.get(&path) {
                    return handler(ctx, next).await;
                }

                for (regexp_path, handler) in dynamic_route.iter() {
                    if regexp_path.re.is_match(&path) {
                        // TODO: store variable in ctx
                        return handler(ctx, next).await;
                    }
                }
                throw(StatusCode::NOT_FOUND, "")
            }
        };
        Ok(Box::new(handler).dynamic())
    }
}

//#[cfg(test)]
//mod tests {
//    use crate::Router;
//    use roa_body::PowerBody;
//    #[test]
//    fn handle() -> Result<(), Box<dyn std::error::Error>> {
//        let mut router = Router::new("/");
//        router
//            .on("/file/:filename")?
//            .join(|_ctx, next| next())
//            .get(|mut ctx| ctx.write_file("filename"));
//    }
//}
