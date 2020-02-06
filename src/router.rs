mod endpoint;
mod err;
mod path;

use endpoint::Endpoint;
use err::{Conflict, Error};
use path::{join_path, standardize_path, Path};

use crate::{
    throw, Context, DynTargetHandler, Middleware, Model, Next, Status, TargetHandler, Variable,
};
use async_trait::async_trait;
use http::StatusCode;
use percent_encoding::percent_decode_str;
use radix_trie::Trie;
use std::future::Future;
use std::sync::Arc;

struct RouterSymbol;

#[async_trait]
pub trait RouterParam {
    async fn param<'a>(&self, name: &'a str) -> Result<Variable<'a>, Status>;
    async fn try_param<'a>(&self, name: &'a str) -> Option<Variable<'a>>;
}

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
            new_middleware.join(middleware.handler());
            new_middleware.join(endpoint.middleware.handler());
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
                    if let Some(cap) = regexp_path.re.captures(&path) {
                        for var in regexp_path.vars.iter() {
                            ctx.store::<RouterSymbol>(var, cap[var.as_str()].to_string())
                                .await;
                        }
                        return handler(ctx, next).await;
                    }
                }
                throw(StatusCode::NOT_FOUND, "")
            }
        };
        Ok(Box::new(handler).dynamic())
    }
}

#[async_trait]
impl<M: Model> RouterParam for Context<M> {
    async fn param<'a>(&self, name: &'a str) -> Result<Variable<'a>, Status> {
        self.try_param(name).await.ok_or(Status::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("router variable `{}` is required", name),
            false,
        ))
    }
    async fn try_param<'a>(&self, name: &'a str) -> Option<Variable<'a>> {
        self.load::<RouterSymbol>(name).await
    }
}

#[cfg(test)]
mod tests {
    use super::{endpoint::Endpoint, Node, Router};
    use crate::App;
    use async_std::task::spawn;
    use encoding::EncoderTrap;
    use http::StatusCode;
    use percent_encoding::NON_ALPHANUMERIC;

    #[should_panic]
    #[test]
    fn node_unwrap_router_fails() {
        let mut node = Node::Endpoint(Endpoint::<()>::new("/".parse().unwrap()));
        node.unwrap_router();
    }

    #[should_panic]
    #[test]
    fn node_unwrap_endpoint_fails() {
        let mut node = Node::Router(Router::<()>::new("/"));
        node.unwrap_endpoint();
    }

    #[tokio::test]
    async fn gate() -> Result<(), Box<dyn std::error::Error>> {
        struct TestSymbol;
        let mut router = Router::<()>::new("/route");
        router
            .gate(|ctx, next| async move {
                ctx.store::<TestSymbol>("id", "0".to_string()).await;
                next().await
            })
            .on("/")?
            .get(|ctx| async move {
                let id: u64 = ctx.load::<TestSymbol>("id").await.unwrap().parse()?;
                assert_eq!(0, id);
                Ok(())
            });
        let (addr, server) = App::new(()).gate(router.handler()?).run_local()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}/route", addr)).await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }

    #[tokio::test]
    async fn route() -> Result<(), Box<dyn std::error::Error>> {
        struct TestSymbol;
        let mut router = Router::<()>::new("/route");
        router
            .gate(|ctx, next| async move {
                ctx.store::<TestSymbol>("id", "0".to_string()).await;
                next().await
            })
            .route("/user")
            .on("/")?
            .get(|ctx| async move {
                let id: u64 = ctx.load::<TestSymbol>("id").await.unwrap().parse()?;
                assert_eq!(0, id);
                Ok(())
            });
        let (addr, server) = App::new(()).gate(router.handler()?).run_local()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}/route/user", addr)).await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }

    #[test]
    fn conflict_path() -> Result<(), Box<dyn std::error::Error>> {
        let mut router = Router::<()>::new("/");
        router.on("/route/endpoint")?.get(|_ctx| async { Ok(()) });
        router
            .route("/route")
            .on("/endpoint")?
            .get(|_ctx| async { Ok(()) });
        let ret = router.handler();
        assert!(ret.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn route_not_found() -> Result<(), Box<dyn std::error::Error>> {
        let (addr, server) = App::new(())
            .gate(Router::<()>::new("/").handler()?)
            .run_local()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}", addr)).await?;
        assert_eq!(StatusCode::NOT_FOUND, resp.status());
        Ok(())
    }

    #[tokio::test]
    async fn non_utf8_uri() -> Result<(), Box<dyn std::error::Error>> {
        let (addr, server) = App::new(())
            .gate(Router::<()>::new("/").handler()?)
            .run_local()?;
        spawn(server);
        let gbk_path = encoding::label::encoding_from_whatwg_label("gbk")
            .unwrap()
            .encode("路由", EncoderTrap::Strict)
            .unwrap();
        let encoded_path =
            percent_encoding::percent_encode(&gbk_path, NON_ALPHANUMERIC).to_string();
        let uri = format!("http://{}/{}", addr, encoded_path);
        let resp = reqwest::get(&uri).await?;
        assert_eq!(StatusCode::BAD_REQUEST, resp.status());
        assert!(resp
            .text()
            .await?
            .ends_with("path `/%C2%B7%D3%C9` is not a valid utf-8 string"));
        Ok(())
    }
}
