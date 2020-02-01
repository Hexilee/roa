use crate::Conflict;
use http::{Method, StatusCode};
use roa_core::{
    throw, Context, DynHandler, DynTargetHandler, Handler, Middleware, Model, Next, Status,
    TargetHandler,
};
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

pub struct Endpoint<M: Model> {
    pub path: &'static str,
    middleware: Middleware<M>,
    handlers: Vec<(Method, Arc<DynHandler<M>>)>,
}

impl<M: Model> Endpoint<M> {
    pub fn new(path: &'static str) -> Self {
        Self {
            path,
            middleware: Middleware::new(),
            handlers: Vec::new(),
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

    pub fn gate<F>(
        &mut self,
        methods: &[Method],
        handler: impl 'static + Sync + Send + Fn(Context<M>) -> F,
    ) -> &mut Self
    where
        F: 'static + Send + Future<Output = Result<(), Status>>,
    {
        let dyn_handler: Arc<DynHandler<M>> = Arc::from(Box::new(handler).dynamic());
        for method in methods {
            self.handlers.push((method.clone(), dyn_handler.clone()));
        }
        self
    }

    pub fn get<F>(&mut self, handler: impl 'static + Sync + Send + Fn(Context<M>) -> F) -> &mut Self
    where
        F: 'static + Send + Future<Output = Result<(), Status>>,
    {
        self.gate([Method::GET].as_ref(), handler)
    }

    pub fn all<F>(&mut self, handler: impl 'static + Sync + Send + Fn(Context<M>) -> F) -> &mut Self
    where
        F: 'static + Send + Future<Output = Result<(), Status>>,
    {
        self.gate(
            [
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::PATCH,
                Method::OPTIONS,
                Method::DELETE,
                Method::HEAD,
                Method::TRACE,
                Method::CONNECT,
            ]
            .as_ref(),
            handler,
        )
    }

    pub fn handler(self) -> Result<Arc<DynTargetHandler<M, Next>>, Conflict> {
        let Self {
            path,
            mut middleware,
            handlers,
        } = self;
        let mut map = HashMap::new();
        for (method, handler) in handlers {
            if let Some(_) = map.insert(method.clone(), handler) {
                return Err(Conflict::Method(path.to_string(), method));
            };
        }
        let map = Arc::new(map);
        middleware.join(move |ctx, _next| {
            let map = map.clone();
            async move {
                match map.get(&ctx.request.method) {
                    None => throw(
                        StatusCode::METHOD_NOT_ALLOWED,
                        format!("method {} is not allowed on {}", &ctx.request.method, path),
                    ),
                    Some(handler) => handler(ctx).await,
                }
            }
        });
        Ok(middleware.handler())
    }
}
