use super::{Conflict, Path};
use http::{Method, StatusCode};
use roa_core::{
    throw, Context, DynHandler, DynTargetHandler, Handler, Middleware, Model, Next, Status,
};
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

macro_rules! impl_http_method {
    ($fn_name:ident, $method:expr) => {
        pub fn $fn_name<F>(&mut self, handler: impl 'static + Sync + Send + Fn(Context<M>) -> F) -> &mut Self
        where
            F: 'static + Send + Future<Output = Result<(), Status>>,
        {
            self.handle([$method].as_ref(), handler)
        }
    };
}

pub struct Endpoint<M: Model> {
    pub path: Arc<Path>,
    pub(crate) middleware: Middleware<M>,
    handlers: Vec<(Method, Arc<DynHandler<M>>)>,
}

impl<M: Model> Endpoint<M> {
    pub fn new(path: Path) -> Self {
        Self {
            path: Arc::new(path),
            middleware: Middleware::new(),
            handlers: Vec::new(),
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

    pub fn handle<F>(
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

    impl_http_method!(get, Method::GET);
    impl_http_method!(post, Method::POST);
    impl_http_method!(put, Method::PUT);
    impl_http_method!(patch, Method::PATCH);
    impl_http_method!(options, Method::OPTIONS);
    impl_http_method!(delete, Method::DELETE);
    impl_http_method!(head, Method::HEAD);
    impl_http_method!(trace, Method::TRACE);
    impl_http_method!(connect, Method::CONNECT);

    pub fn all<F>(&mut self, handler: impl 'static + Sync + Send + Fn(Context<M>) -> F) -> &mut Self
    where
        F: 'static + Send + Future<Output = Result<(), Status>>,
    {
        self.handle(
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
        let raw_path = path.raw().to_string();
        let mut map = HashMap::new();
        for (method, handler) in handlers {
            if let Some(_) = map.insert(method.clone(), handler) {
                return Err(Conflict::Method(raw_path.clone(), method));
            };
        }

        let map = Arc::new(map);
        middleware.join(move |ctx, _next| {
            let map = map.clone();
            let raw_path = raw_path.clone();
            async move {
                match map.get(&ctx.method().await) {
                    None => throw(
                        StatusCode::METHOD_NOT_ALLOWED,
                        format!(
                            "method {} is not allowed on {}",
                            &ctx.method().await,
                            raw_path
                        ),
                    ),
                    Some(handler) => handler(ctx).await,
                }
            }
        });
        Ok(middleware.handler())
    }
}
