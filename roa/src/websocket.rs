//! This module provides a websocket endpoint.
//!
//! ### Example
//! ```
//! use futures::StreamExt;
//! use roa::router::{Router, RouterError};
//! use roa::websocket::Websocket;
//! use roa::{App, Context};
//! use roa::http::Method;
//!
//! # fn main() -> Result<(), RouterError> {
//! let router = Router::new().on("/chat", Websocket::new(|_ctx, stream| async move {
//!     let (write, read) = stream.split();
//!     // echo
//!     if let Err(err) = read.forward(write).await {
//!         println!("forward err: {}", err);
//!     }
//! }));
//! let app = App::new().end(router.routes("/")?);
//! Ok(())
//! # }
//! ```

use std::future::Future;
use std::marker::PhantomData;
use std::sync::Arc;

use headers::{
    Connection, HeaderMapExt, SecWebsocketAccept, SecWebsocketKey, SecWebsocketVersion, Upgrade,
};
use hyper::upgrade::{self, Upgraded};
pub use tokio_tungstenite::tungstenite;
pub use tokio_tungstenite::tungstenite::protocol::{Message, WebSocketConfig};
use tokio_tungstenite::WebSocketStream;

use crate::http::header::UPGRADE;
use crate::http::StatusCode;
use crate::{async_trait, throw, Context, Endpoint, State, Status};

/// An alias for WebSocketStream<Upgraded>.
pub type SocketStream = WebSocketStream<Upgraded>;

/// The Websocket middleware.
///
/// ### Example
/// ```
/// use futures::StreamExt;
/// use roa::router::{Router, RouterError};
/// use roa::websocket::Websocket;
/// use roa::{App, Context};
/// use roa::http::Method;
///
/// # fn main() -> Result<(), RouterError> {
/// let router = Router::new().on("/chat", Websocket::new(|_ctx, stream| async move {
///     let (write, read) = stream.split();
///     // echo
///     if let Err(err) = read.forward(write).await {
///         println!("forward err: {}", err);
///     }
/// }));
/// let app = App::new().end(router.routes("/")?);
/// Ok(())
/// # }
/// ```
///
/// ### Parameter
///
/// - Context<S>
///
/// The context is the same with roa context,
/// however, neither read body from request or write anything to response is unavailing.
///
/// - SocketStream
///
/// The websocket stream, implementing `Stream` and `Sink`.
///
/// ### Return
///
/// Must be `()`, as roa cannot deal with errors occurring in websocket.
pub struct Websocket<F, S, Fut>
where
    F: Fn(Context<S>, SocketStream) -> Fut,
{
    task: Arc<F>,
    config: Option<WebSocketConfig>,
    _s: PhantomData<S>,
    _fut: PhantomData<Fut>,
}

unsafe impl<F, S, Fut> Send for Websocket<F, S, Fut> where
    F: Sync + Send + Fn(Context<S>, SocketStream) -> Fut
{
}
unsafe impl<F, S, Fut> Sync for Websocket<F, S, Fut> where
    F: Sync + Send + Fn(Context<S>, SocketStream) -> Fut
{
}

impl<F, S, Fut> Websocket<F, S, Fut>
where
    F: Fn(Context<S>, SocketStream) -> Fut,
{
    fn config(config: Option<WebSocketConfig>, task: F) -> Self {
        Self {
            task: Arc::new(task),
            config,
            _s: PhantomData::default(),
            _fut: PhantomData::default(),
        }
    }

    /// Construct a websocket middleware by task closure.
    pub fn new(task: F) -> Self {
        Self::config(None, task)
    }

    /// Construct a websocket middleware with config.
    /// ### Example
    /// ```
    /// use futures::StreamExt;
    /// use roa::router::{Router, RouterError};
    /// use roa::websocket::{Websocket, WebSocketConfig};
    /// use roa::{App, Context};
    /// use roa::http::Method;
    ///
    /// # fn main() -> Result<(), RouterError> {
    /// let router = Router::new().on("/chat", Websocket::with_config(
    ///     WebSocketConfig::default(),
    ///     |_ctx, stream| async move {
    ///         let (write, read) = stream.split();
    ///         // echo
    ///         if let Err(err) = read.forward(write).await {
    ///             println!("forward err: {}", err);
    ///         }
    ///     })
    /// );
    /// let app = App::new().end(router.routes("/")?);
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_config(config: WebSocketConfig, task: F) -> Self {
        Self::config(Some(config), task)
    }
}

#[async_trait(?Send)]
impl<'a, F, S, Fut> Endpoint<'a, S> for Websocket<F, S, Fut>
where
    S: State,
    F: 'static + Sync + Send + Fn(Context<S>, SocketStream) -> Fut,
    Fut: 'static + Send + Future<Output = ()>,
{
    #[inline]
    async fn call(&'a self, ctx: &'a mut Context<S>) -> Result<(), Status> {
        let header_map = &ctx.req.headers;
        let key = header_map
            .typed_get::<Upgrade>()
            .filter(|upgrade| upgrade == &Upgrade::websocket())
            .and(header_map.typed_get::<Connection>())
            .filter(|connection| connection.contains(UPGRADE))
            .and(header_map.typed_get::<SecWebsocketVersion>())
            .filter(|version| version == &SecWebsocketVersion::V13)
            .and(header_map.typed_get::<SecWebsocketKey>());

        match key {
            None => throw!(StatusCode::BAD_REQUEST, "invalid websocket upgrade request"),
            Some(key) => {
                let raw_req = ctx.req.take_raw();
                let context = ctx.clone();
                let task = self.task.clone();
                let config = self.config;
                // Setup a future that will eventually receive the upgraded
                // connection and talk a new protocol, and spawn the future
                // into the runtime.
                //
                // Note: This can't possibly be fulfilled until the 101 response
                // is returned below, so it's better to spawn this future instead
                // waiting for it to complete to then return a response.
                ctx.exec.spawn(async move {
                    match upgrade::on(raw_req).await {
                        Err(err) => tracing::error!("websocket upgrade error: {}", err),
                        Ok(upgraded) => {
                            let websocket = WebSocketStream::from_raw_socket(
                                upgraded,
                                tungstenite::protocol::Role::Server,
                                config,
                            )
                            .await;
                            task(context, websocket).await
                        }
                    }
                });
                ctx.resp.status = StatusCode::SWITCHING_PROTOCOLS;
                ctx.resp.headers.typed_insert(Connection::upgrade());
                ctx.resp.headers.typed_insert(Upgrade::websocket());
                ctx.resp.headers.typed_insert(SecWebsocketAccept::from(key));
                Ok(())
            }
        }
    }
}
