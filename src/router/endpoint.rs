use super::{Conflict, Path};
use http::{Method, StatusCode};
use roa_core::{
    throw, Context, DynHandler, DynTargetHandler, Exception, Group, Handler, Model, Next,
};
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

macro_rules! impl_http_method {
    ($fn_name:ident, $($method:expr),*) => {
        pub fn $fn_name<F>(&mut self, handler: impl 'static + Sync + Send + Fn(Context<M>) -> F) -> &mut Self
        where
            F: 'static + Send + Future<Output = Result<(), Status>>,
        {
            self.handle([$($method, )*].as_ref(), handler)
        }
    };
}

pub struct Endpoint<M: Model> {
    pub path: Arc<Path>,
    pub(crate) middleware: Group<M>,
    handlers: Vec<(Method, Arc<DynHandler<M>>)>,
}

impl<M: Model> Endpoint<M> {
    pub fn new(path: Path) -> Self {
        Self {
            path: Arc::new(path),
            middleware: Group::new(),
            handlers: Vec::new(),
        }
    }

    pub fn gate<F>(
        &mut self,
        middleware: impl 'static + Sync + Send + Fn(Context<M>, Next) -> F,
    ) -> &mut Self
    where
        F: 'static + Future<Output = Result<(), Exception>> + Send,
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
        F: 'static + Send + Future<Output = Result<(), Exception>>,
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
    impl_http_method!(
        all,
        Method::GET,
        Method::POST,
        Method::PUT,
        Method::PATCH,
        Method::OPTIONS,
        Method::DELETE,
        Method::HEAD,
        Method::TRACE,
        Method::CONNECT
    );

    pub fn handler(self) -> Result<Box<DynTargetHandler<M, Next>>, Conflict> {
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

#[cfg(test)]
mod tests {
    use super::Endpoint;
    use crate::router::err::Conflict;
    use crate::App;
    use async_std::task::spawn;
    use http::StatusCode;

    #[test]
    fn conflict_method() {
        let mut endpoint = Endpoint::<()>::new("/".parse().unwrap());
        endpoint
            .get(|_ctx| async { Ok(()) })
            .all(|_ctx| async { Ok(()) });
        let ret = endpoint.handler();
        assert!(ret.is_err());
        if let Err(conflict) = ret {
            assert_eq!(
                Conflict::Method("//".to_string(), http::Method::GET),
                conflict
            );
        }
    }

    #[tokio::test]
    async fn gate() -> Result<(), Box<dyn std::error::Error>> {
        struct TestSymbol;
        let mut endpoint = Endpoint::<()>::new("/endpoint".parse()?);
        endpoint
            .gate(|ctx, next| async move {
                ctx.store::<TestSymbol>("id", "0".to_string()).await;
                next().await
            })
            .all(|ctx| async move {
                let id: u64 = ctx.load::<TestSymbol>("id").await.unwrap().parse()?;
                assert_eq!(0, id);
                Ok(())
            });
        let (addr, server) = App::new(()).gate(endpoint.handler()?).run_local()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}/endpoint", addr)).await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }

    #[tokio::test]
    async fn method_not_allowed() -> Result<(), Box<dyn std::error::Error>> {
        let mut endpoint = Endpoint::<()>::new("/endpoint".parse()?);
        endpoint.post(|_ctx| async { Ok(()) });
        let (addr, server) = App::new(()).gate(endpoint.handler()?).run_local()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}/endpoint", addr)).await?;
        assert_eq!(StatusCode::METHOD_NOT_ALLOWED, resp.status());
        Ok(())
    }
}
