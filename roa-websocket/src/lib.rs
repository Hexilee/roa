use async_std::task::spawn;
use headers::{
    Connection, HeaderMapExt, SecWebsocketAccept, SecWebsocketKey, SecWebsocketVersion,
    Upgrade,
};
use hyper::upgrade::Upgraded;
use roa_core::header::UPGRADE;
use roa_core::{
    async_trait, throw, Context, Error, Middleware, Next, State, StatusCode,
};
use std::future::Future;
use std::marker::PhantomData;
use std::sync::Arc;
pub use tokio_tungstenite::tungstenite::{
    self,
    protocol::{Message, WebSocketConfig},
};
use tokio_tungstenite::WebSocketStream;

pub type SocketStream = WebSocketStream<Upgraded>;

pub struct Websocket<F, S, Fut>
where
    F: Fn(S, SocketStream) -> Fut,
{
    task: Arc<F>,
    config: Option<WebSocketConfig>,
    _s: PhantomData<S>,
    _fut: PhantomData<Fut>,
}

unsafe impl<F, S, Fut> Send for Websocket<F, S, Fut> where
    F: Sync + Fn(S, SocketStream) -> Fut
{
}
unsafe impl<F, S, Fut> Sync for Websocket<F, S, Fut> where
    F: Sync + Fn(S, SocketStream) -> Fut
{
}

impl<F, S, Fut> Websocket<F, S, Fut>
where
    F: Fn(S, SocketStream) -> Fut,
{
    pub fn new(task: F) -> Self {
        Self::with_config(None, task)
    }

    pub fn with_config(config: Option<WebSocketConfig>, task: F) -> Self {
        Self {
            task: Arc::new(task),
            config,
            _s: PhantomData::default(),
            _fut: PhantomData::default(),
        }
    }
}

#[async_trait(?Send)]
impl<F, S, Fut> Middleware<S> for Websocket<F, S, Fut>
where
    S: State,
    F: 'static + Sync + Send + Fn(S, SocketStream) -> Fut,
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
                // Setup a future that will eventually receive the upgraded
                // connection and talk a new protocol, and spawn the future
                // into the runtime.
                //
                // Note: This can't possibly be fulfilled until the 101 response
                // is returned below, so it's better to spawn this future instead
                // waiting for it to complete to then return a response.
                let body = ctx.req_mut().body_stream();
                let state = ctx.state().clone();
                let task = self.task.clone();
                let config = self.config.clone();
                spawn(async move {
                    match body.on_upgrade().await {
                        Err(err) => log::error!("websocket upgrade error: {}", err),
                        Ok(upgraded) => {
                            let websocket = WebSocketStream::from_raw_socket(
                                upgraded,
                                tungstenite::protocol::Role::Server,
                                config,
                            )
                            .await;
                            task(state, websocket).await
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
