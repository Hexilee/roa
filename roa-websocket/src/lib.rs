//! This crate provides a websocket middleware.
//!
//! ### Example
//! ```
//! use futures::StreamExt;
//! use roa_router::{Router, RouterError};
//! use roa_websocket::Websocket;
//! use roa_core::{App, SyncContext};
//! use roa_core::http::Method;
//!
//! # fn main() -> Result<(), RouterError> {
//! let mut app = App::new(());
//! let mut router = Router::new();
//! router.end(
//!     "/chat",
//!     [Method::GET],
//!     Websocket::new(|_ctx: SyncContext<()>, stream| async move {
//!         let (write, read) = stream.split();
//!         // echo
//!         if let Err(err) = read.forward(write).await {
//!             println!("forward err: {}", err);
//!         }
//!     }),
//! );
//! app.gate(router.routes("/")?);
//! # Ok(())
//! # }
//! ```
#![warn(missing_docs)]

use headers::{
    Connection, HeaderMapExt, SecWebsocketAccept, SecWebsocketKey, SecWebsocketVersion,
    Upgrade,
};
use hyper::upgrade::Upgraded;
use roa_core::http::header::UPGRADE;
use roa_core::http::StatusCode;
use roa_core::{
    async_trait, throw, Context, Error, Middleware, Next, State, SyncContext,
};
use std::future::Future;
use std::marker::PhantomData;
use std::sync::Arc;
pub use tokio_tungstenite::tungstenite::{
    self,
    protocol::{Message, WebSocketConfig},
};
use tokio_tungstenite::WebSocketStream;

/// An alias for WebSocketStream<Upgraded>.
pub type SocketStream = WebSocketStream<Upgraded>;

/// The Websocket middleware.
///
/// ### Example
/// ```
/// use futures::StreamExt;
/// use roa_router::{Router, RouterError};
/// use roa_websocket::Websocket;
/// use roa_core::{App, SyncContext};
/// use roa_core::http::Method;
///
/// # fn main() -> Result<(), RouterError> {
/// let mut app = App::new(());
/// let mut router = Router::new();
/// router.end(
///     "/chat",
///     [Method::GET],
///     Websocket::new(|_ctx: SyncContext<()>, stream| async move {
///         let (write, read) = stream.split();
///         // echo
///         if let Err(err) = read.forward(write).await {
///             println!("forward err: {}", err);
///         }
///     }),
/// );
/// app.gate(router.routes("/")?);
/// # Ok(())
/// # }
/// ```
pub struct Websocket<F, S, Fut>
where
    F: Fn(SyncContext<S>, SocketStream) -> Fut,
{
    task: Arc<F>,
    config: Option<WebSocketConfig>,
    _s: PhantomData<S>,
    _fut: PhantomData<Fut>,
}

unsafe impl<F, S, Fut> Send for Websocket<F, S, Fut> where
    F: Sync + Send + Fn(SyncContext<S>, SocketStream) -> Fut
{
}
unsafe impl<F, S, Fut> Sync for Websocket<F, S, Fut> where
    F: Sync + Send + Fn(SyncContext<S>, SocketStream) -> Fut
{
}

impl<F, S, Fut> Websocket<F, S, Fut>
where
    F: Fn(SyncContext<S>, SocketStream) -> Fut,
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
    /// use roa_router::{Router, RouterError};
    /// use roa_websocket::{Websocket, WebSocketConfig};
    /// use roa_core::{App, SyncContext};
    /// use roa_core::http::Method;
    ///
    /// # fn main() -> Result<(), RouterError> {
    /// let mut app = App::new(());
    /// let mut router = Router::new();
    /// router.end(
    ///     "/chat",
    ///     [Method::GET],
    ///     Websocket::with_config(
    ///         WebSocketConfig::default(),
    ///         |_ctx: SyncContext<()>, stream| async move {
    ///             let (write, read) = stream.split();
    ///             // echo
    ///             if let Err(err) = read.forward(write).await {
    ///                 println!("forward err: {}", err);
    ///             }
    ///         }
    ///     ),
    /// );
    /// app.gate(router.routes("/")?);
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_config(config: WebSocketConfig, task: F) -> Self {
        Self::config(Some(config), task)
    }
}

#[async_trait(?Send)]
impl<F, S, Fut> Middleware<S> for Websocket<F, S, Fut>
where
    S: State,
    F: 'static + Sync + Send + Fn(SyncContext<S>, SocketStream) -> Fut,
    Fut: 'static + Send + Future<Output = ()>,
{
    async fn handle(
        self: Arc<Self>,
        mut ctx: Context<S>,
        _next: Next,
    ) -> Result<(), Error> {
        let header_map = &ctx.req().headers;
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
                let body = ctx.req_mut().body_stream();
                let sync_context = ctx.clone();
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
                    match body.on_upgrade().await {
                        Err(err) => log::error!("websocket upgrade error: {}", err),
                        Ok(upgraded) => {
                            let websocket = WebSocketStream::from_raw_socket(
                                upgraded,
                                tungstenite::protocol::Role::Server,
                                config,
                            )
                            .await;
                            task(sync_context, websocket).await
                        }
                    }
                });
                ctx.resp_mut().status = StatusCode::SWITCHING_PROTOCOLS;
                ctx.resp_mut().headers.typed_insert(Connection::upgrade());
                ctx.resp_mut().headers.typed_insert(Upgrade::websocket());
                ctx.resp_mut()
                    .headers
                    .typed_insert(SecWebsocketAccept::from(key));
                Ok(())
            }
        }
    }
}
