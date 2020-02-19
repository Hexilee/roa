use async_std::net::TcpStream;
use async_std::sync::Arc;
pub use async_tungstenite::tungstenite::protocol::WebSocketConfig;
use async_tungstenite::{accept_async, accept_async_with_config, WebSocketStream};
use hyper::upgrade::Upgraded;
use roa_core::{Context, Error, State};

pub struct Websocket<S, F, Fut>(F)
where
    F: Fn(S, Result<Upgraded, hyper::Error>) -> Fut;

impl<S, F, Fut> Websocket<S, F, Fut>
where
    F: Fn(S, Result<Upgraded, hyper::Error>) -> Fut,
{
    pub fn new(task: F) -> Self {
        Self(task)
    }
}
