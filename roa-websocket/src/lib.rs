use async_std::net::TcpStream;
use async_std::sync::Arc;
pub use async_tungstenite::tungstenite::protocol::WebSocketConfig;
use async_tungstenite::{accept_async, accept_async_with_config, WebSocketStream};
use roa_core::{Context, Error, State};
use std::mem;

pub struct Websocket(WebSocketStream<Arc<TcpStream>>);

impl Websocket {
    pub fn new<S: State>(ctx: &mut Context<S>) -> Self {
        let request = mem::take(ctx.req_mut());
        let stream = ctx.raw_stream();
        unimplemented!()
    }
}
