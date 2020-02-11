mod controller;
mod err;
mod path;

use controller::Controller;
use err::{Conflict, RouterError};
use path::{join_path, standardize_path, Path, RegexPath};

use crate::core::{
    join, join_all, throw, Context, Endpoint, Error, Middleware, Next, Result, State, Variable,
};
use async_trait::async_trait;
use http::StatusCode;
use percent_encoding::percent_decode_str;
use radix_trie::Trie;
use std::future::Future;
use std::result::Result as StdResult;
use std::sync::Arc;

struct RouterSymbol;

#[async_trait]
pub trait RouterParam {
    async fn param<'a>(&self, name: &'a str) -> Result<Variable<'a>>;
    async fn try_param<'a>(&self, name: &'a str) -> Option<Variable<'a>>;
}

enum Node<S> {
    Router(Router<S>),
    Controller(Controller<S>),
}

impl<S> Node<S> {
    fn unwrap_router(&mut self) -> &mut Router<S> {
        match self {
            Node::Router(router) => router,
            _ => panic!(
                r"Node is not a router, 
                  This is a bug of roa-router::Router, please report it to https://github.com/Hexilee/roa
            "
            ),
        }
    }

    fn unwrap_controller(&mut self) -> &mut Controller<S> {
        match self {
            Node::Controller(controller) => controller,
            _ => panic!(
                r"Node is not a controller,
                  This is a bug of roa-router::Router, please report it to https://github.com/Hexilee/roa
            "
            ),
        }
    }
}

pub struct Router<S> {
    root: String,
    middlewares: Vec<Arc<dyn Middleware<S>>>,
    nodes: Vec<Node<S>>,
}

pub struct RouteEndpoint<S> {
    static_route: Trie<String, Arc<dyn Endpoint<S>>>,
    dynamic_route: Vec<(RegexPath, Arc<dyn Endpoint<S>>)>,
}

impl<S> Router<S> {
    pub fn new(path: impl ToString) -> Self {
        Self {
            root: path.to_string(),
            middlewares: Vec::new(),
            nodes: Vec::new(),
        }
    }
}

impl<S: State> Router<S> {
    pub fn gate(&mut self, middleware: impl Middleware<S>) -> &mut Self {
        self.middlewares.push(Arc::new(middleware));
        self
    }

    pub fn gate_fn<F>(
        &mut self,
        middleware: impl 'static + Sync + Send + Fn(Context<S>, Next) -> F,
    ) -> &mut Self
    where
        F: 'static + Future<Output = Result> + Send,
    {
        self.gate(middleware);
        self
    }

    pub fn on(&mut self, path: &'static str) -> StdResult<&mut Controller<S>, RouterError> {
        let controller = Controller::new(join_path([self.root.as_str(), path].as_ref()).parse()?);
        let index = self.nodes.len();
        self.nodes.push(Node::Controller(controller));
        Ok(self.nodes[index].unwrap_controller())
    }

    pub fn route(&mut self, path: &'static str) -> &mut Router<S> {
        let router = Router::new(join_path([self.root.as_str(), path].as_ref()));
        let index = self.nodes.len();
        self.nodes.push(Node::Router(router));
        self.nodes[index].unwrap_router()
    }

    fn controllers(self) -> Vec<Controller<S>> {
        let Self {
            middlewares, nodes, ..
        } = self;
        let mut controllers = Vec::new();
        for node in nodes {
            match node {
                Node::Controller(controller) => {
                    controllers.push(controller);
                }
                Node::Router(router) => controllers.extend(router.controllers().into_iter()),
            };
        }

        // join middlewares
        for controller in controllers.iter_mut() {
            let mut new_middlewares = middlewares.clone();
            new_middlewares.extend(controller.middlewares.clone());
            controller.middlewares = middlewares;
        }
        controllers
    }

    pub fn endpoint(self) -> StdResult<RouteEndpoint<S>, Conflict> {
        let controllers = self.controllers();
        let mut static_route = Trie::new();
        let mut dynamic_route = Vec::new();
        for controller in controllers {
            match &*controller.path.clone() {
                Path::Static(path) => {
                    if static_route
                        .insert(path.to_string(), controller.endpoint()?.into())
                        .is_some()
                    {
                        return Err(Conflict::Path(path.to_string()));
                    }
                }
                Path::Dynamic(regex_path) => {
                    dynamic_route.push((regex_path.clone(), controller.endpoint()?.into()))
                }
            }
        }

        Ok(RouteEndpoint {
            static_route,
            dynamic_route,
        })
    }
}

#[async_trait]
impl<S: State> Endpoint<S> for RouteEndpoint<S> {
    async fn handle(self: Arc<Self>, ctx: Context<S>) -> Result {
        let uri = ctx.uri().await;
        let path = standardize_path(&percent_decode_str(uri.path()).decode_utf8().map_err(
            |err| {
                Error::new(
                    StatusCode::BAD_REQUEST,
                    format!("{}\npath `{}` is not a valid utf-8 string", err, uri.path()),
                    true,
                )
            },
        )?);
        if let Some(handler) = self.static_route.get(&path) {
            return handler.clone().handle(ctx).await;
        }

        for (regexp_path, handler) in self.dynamic_route.iter() {
            if let Some(cap) = regexp_path.re.captures(&path) {
                for var in regexp_path.vars.iter() {
                    ctx.store::<RouterSymbol>(var, cap[var.as_str()].to_string())
                        .await;
                }
                return handler.clone().handle(ctx).await;
            }
        }
        throw(StatusCode::NOT_FOUND, "")
    }
}

#[async_trait]
impl<S: State> RouterParam for Context<S> {
    async fn param<'a>(&self, name: &'a str) -> Result<Variable<'a>> {
        self.try_param(name).await.ok_or_else(|| {
            Error::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("router variable `{}` is required", name),
                false,
            )
        })
    }
    async fn try_param<'a>(&self, name: &'a str) -> Option<Variable<'a>> {
        self.load::<RouterSymbol>(name).await
    }
}

#[cfg(test)]
mod tests {
    use super::{endpoint::Endpoint, Node, Router};
    use crate::core::App;
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
        let (addr, server) = App::new(()).end_fn(router.handler()?).run_local()?;
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
            .gate_fn(|ctx, next| async move {
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
        let (addr, server) = App::new(()).end(router.endpoint()?).run_local()?;
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
            .end_fn(Router::<()>::new("/").handler()?)
            .run_local()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}", addr)).await?;
        assert_eq!(StatusCode::NOT_FOUND, resp.status());
        Ok(())
    }

    #[tokio::test]
    async fn non_utf8_uri() -> Result<(), Box<dyn std::error::Error>> {
        let (addr, server) = App::new(())
            .end_fn(Router::<()>::new("/").handler()?)
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
