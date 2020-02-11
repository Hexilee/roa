// use super::{Conflict, Path};
// use crate::core::{join_all, throw, Context, Middleware, Next, Result, State, ResultFuture};
// use http::{Method, StatusCode};
// use std::collections::HashMap;
// use std::future::Future;
// use std::result::Result as StdResult;
// use std::sync::Arc;
// use async_trait::async_trait;

// macro_rules! impl_http_method {
//     ($end:ident, $end_fn:ident, $($method:expr),*) => {
//         pub fn $end(&mut self, endpoint: impl Endpoint<S>) -> &mut Self {
//             self.handle([$($method, )*].as_ref(), endpoint)
//         }

//         pub fn $end_fn<F>(&mut self, endpoint: impl 'static + Sync + Send + Fn(Context<S>) -> F) -> &mut Self
//         where
//             F: 'static + Send + Future<Output = Result>,
//         {
//             self.handle([$($method, )*].as_ref(), endpoint)
//         }
//     };
// }

// pub struct Controller<S> {
//     pub path: Arc<Path>,
//     pub(crate) middlewares: Vec<Arc<dyn Middleware<S>>>,
//     handlers: Vec<(Method, Arc<dyn Middleware<S>>)>,
// }

// pub struct CtrlEndpoint<S> {
//     inner: Arc<dyn Middleware<S>>
// }

// impl<S: State> Controller<S> {
//     pub fn new(path: Path) -> Self {
//         Self {
//             path: Arc::new(path),
//             middlewares: Vec::new(),
//             handlers: Vec::new(),
//         }
//     }

//     /// Use a middleware.
//     pub fn gate(&mut self, middleware: impl Middleware<S>) -> &mut Self {
//         self.middlewares.push(Arc::new(middleware));
//         self
//     }

//     pub fn gate_fn<F>(
//         &mut self,
//         middleware: impl 'static + Sync + Send + Fn(Context<S>, Next) -> F,
//     ) -> &mut Self
//     where
//         F: 'static + Future<Output = Result> + Send,
//     {
//         self.gate(middleware);
//         self
//     }

//     pub fn handle(&mut self, methods: &[Method], endpoint: impl Endpoint<S>) -> &mut Self {
//         let dyn_handler = Arc::new(endpoint);
//         for method in methods {
//             self.handlers.push((method.clone(), dyn_handler.clone()));
//         }
//         self
//     }

//     pub fn handle_fn<F>(
//         &mut self,
//         methods: &[Method],
//         endpoint: impl 'static + Sync + Send + Fn(Context<S>) -> F,
//     ) -> &mut Self
//     where
//         F: 'static + Future<Output = Result> + Send,
//     {
//         self.handle(methods, endpoint)
//     }

//     impl_http_method!(get, get_fn, Method::GET);
//     impl_http_method!(post, post_fn, Method::POST);
//     impl_http_method!(put, put_fn, Method::PUT);
//     impl_http_method!(patch, patch_fn, Method::PATCH);
//     impl_http_method!(options, options_fn, Method::OPTIONS);
//     impl_http_method!(delete, delete_fn, Method::DELETE);
//     impl_http_method!(head, head_fn, Method::HEAD);
//     impl_http_method!(trace, trace_fn, Method::TRACE);
//     impl_http_method!(connect, connect_fn, Method::CONNECT);
//     impl_http_method!(
//         all,
//         all_fn,
//         Method::GET,
//         Method::POST,
//         Method::PUT,
//         Method::PATCH,
//         Method::OPTIONS,
//         Method::DELETE,
//         Method::HEAD,
//         Method::TRACE,
//         Method::CONNECT
//     );

//     pub fn endpoint(self) -> StdResult<CtrlEndpoint<S>, Conflict> {
//         let Self {
//             path,
//             mut middlewares,
//             handlers,
//         } = self;
//         let raw_path = path.raw().to_string();
//         let mut map = HashMap::new();
//         for (method, handler) in handlers {
//             if map.insert(method.clone(), handler).is_some() {
//                 return Err(Conflict::Method(raw_path, method));
//             };
//         }

//         let map = Arc::new(map);
//         middlewares.push(Arc::new(move |ctx: Context<S>, _next| {
//             let map = map.clone();
//             let raw_path = raw_path.clone();
//             async move {
//                 match map.get(&ctx.method().await) {
//                     None => throw(
//                         StatusCode::METHOD_NOT_ALLOWED,
//                         format!(
//                             "method {} is not allowed on {}",
//                             &ctx.method().await,
//                             raw_path
//                         ),
//                     ),
//                     Some(handler) => handler.clone().handle(ctx).await,
//                 }
//             }
//         }));

//         let middleware = Arc::new(join_all(middlewares));
//         Ok(Box::new(move |ctx: Context<S>| middleware.clone().handle(ctx, Box::new(last))))
//     }
// }

// #[async_trait]
// impl<S: State> Endpoint<S> for CtrlEndpoint<S> {
//     async fn handle(self: Arc<Self>, ctx: Context<S>) -> Result {
//         self.inner.handle(ctx).await
//     }
// }

// pub fn last() -> ResultFuture {
//     Box::pin(async move { Ok(()) })
// }

// #[cfg(test)]
// mod tests {
//     use super::Controller;
//     use crate::core::App;
//     use crate::router::err::Conflict;
//     use async_std::task::spawn;
//     use http::StatusCode;

//     #[test]
//     fn conflict_method() {
//         let mut controller = Controller::<()>::new("/".parse().unwrap());
//         controller
//             .get_fn(|_ctx| async { Ok(()) })
//             .all_fn(|_ctx| async { Ok(()) });
//         let ret = controller.endpoint();
//         assert!(ret.is_err());
//         if let Err(conflict) = ret {
//             assert_eq!(
//                 Conflict::Method("//".to_string(), http::Method::GET),
//                 conflict
//             );
//         }
//     }

//     #[tokio::test]
//     async fn gate() -> Result<(), Box<dyn std::error::Error>> {
//         struct TestSymbol;
//         let mut controller = Controller::<()>::new("/endpoint".parse()?);
//         controller
//             .gate_fn(|ctx, next| async move {
//                 ctx.store::<TestSymbol>("id", "0".to_string()).await;
//                 next().await
//             })
//             .all_fn(|ctx| async move {
//                 let id: u64 = ctx.load::<TestSymbol>("id").await.unwrap().parse()?;
//                 assert_eq!(0, id);
//                 Ok(())
//             });
//         let (addr, server) = App::new(()).end(controller.endpoint()?).run_local()?;
//         spawn(server);
//         let resp = reqwest::get(&format!("http://{}/endpoint", addr)).await?;
//         assert_eq!(StatusCode::OK, resp.status());
//         Ok(())
//     }

//     #[tokio::test]
//     async fn method_not_allowed() -> Result<(), Box<dyn std::error::Error>> {
//         let mut controller = Controller::<()>::new("/endpoint".parse()?);
//         controller.post_fn(|_ctx| async { Ok(()) });
//         let (addr, server) = App::new(()).end(controller.endpoint()?).run_local()?;
//         spawn(server);
//         let resp = reqwest::get(&format!("http://{}/endpoint", addr)).await?;
//         assert_eq!(StatusCode::METHOD_NOT_ALLOWED, resp.status());
//         Ok(())
//     }
// }
