use std::borrow::Cow;
use std::error::Error as StdError;

use async_std::sync::{Arc, Mutex, RwLock};
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use http::Method;
use log::{debug, error, info, warn};
use roa::logger::logger;
use roa::preload::*;
use roa::router::{allow, RouteTable, Router, RouterError};
use roa::websocket::tungstenite::protocol::frame::coding::CloseCode;
use roa::websocket::tungstenite::protocol::frame::CloseFrame;
use roa::websocket::tungstenite::Error as WsError;
use roa::websocket::{Message, SocketStream, Websocket};
use roa::{App, Context};
use slab::Slab;

type Sender = SplitSink<SocketStream, Message>;
type Channel = Slab<Mutex<Sender>>;
#[derive(Clone)]
struct SyncChannel(Arc<RwLock<Channel>>);

impl SyncChannel {
    fn new() -> Self {
        Self(Arc::new(RwLock::new(Slab::new())))
    }

    async fn broadcast(&self, message: Message) {
        let channel = self.0.read().await;
        for (_, sender) in channel.iter() {
            if let Err(err) = sender.lock().await.send(message.clone()).await {
                error!("broadcast error: {}", err);
            }
        }
    }

    async fn send(&self, index: usize, message: Message) {
        if let Err(err) = self.0.read().await[index].lock().await.send(message).await {
            error!("message send error: {}", err)
        }
    }

    async fn register(&self, sender: Sender) -> usize {
        self.0.write().await.insert(Mutex::new(sender))
    }

    async fn deregister(&self, index: usize) -> Sender {
        self.0.write().await.remove(index).into_inner()
    }
}

async fn handle_message(
    ctx: &Context<SyncChannel>,
    index: usize,
    mut receiver: SplitStream<SocketStream>,
) -> Result<(), WsError> {
    while let Some(message) = receiver.next().await {
        let message = message?;
        match message {
            Message::Close(frame) => {
                debug!("websocket connection close: {:?}", frame);
                break;
            }
            Message::Ping(data) => ctx.send(index, Message::Pong(data)).await,
            Message::Pong(data) => warn!("ignored pong: {:?}", data),
            msg => ctx.broadcast(msg).await,
        }
    }
    Ok(())
}

fn route(prefix: &'static str) -> Result<RouteTable<SyncChannel>, RouterError> {
    Router::new()
        .on(
            "/chat",
            allow(
                [Method::GET],
                Websocket::new(|ctx: Context<SyncChannel>, stream| async move {
                    let (sender, receiver) = stream.split();
                    let index = ctx.register(sender).await;
                    let result = handle_message(&ctx, index, receiver).await;
                    let mut sender = ctx.deregister(index).await;
                    if let Err(err) = result {
                        let result = sender
                            .send(Message::Close(Some(CloseFrame {
                                code: CloseCode::Invalid,
                                reason: Cow::Owned(err.to_string()),
                            })))
                            .await;
                        if let Err(err) = result {
                            error!("send close message error: {}", err)
                        }
                    }
                }),
            ),
        )
        .routes(prefix)
}

#[async_std::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    pretty_env_logger::init();
    let app = App::state(SyncChannel::new()).gate(logger).end(route("/")?);
    app.listen("127.0.0.1:8000", |addr| {
        info!("Server is listening on {}", addr)
    })?
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use async_tungstenite::async_std::connect_async;
    use roa::preload::*;

    use super::{route, App, Message, SinkExt, StdError, StreamExt, SyncChannel};

    #[async_std::test]
    async fn echo() -> Result<(), Box<dyn StdError>> {
        let channel = SyncChannel::new();
        let app = App::state(channel.clone()).end(route("/")?);
        let (addr, server) = app.run()?;
        async_std::task::spawn(server);
        let (ws_stream, _) = connect_async(format!("ws://{}/chat", addr)).await?;
        let (mut sender, mut recv) = ws_stream.split();
        async_std::task::sleep(Duration::from_secs(1)).await;
        assert_eq!(1, channel.0.read().await.len());

        // ping
        sender
            .send(Message::Ping(b"Hello, World!".to_vec()))
            .await?;
        let msg = recv.next().await.unwrap()?;
        assert!(msg.is_pong());
        assert_eq!(b"Hello, World!".as_ref(), msg.into_data().as_slice());

        // close
        sender.send(Message::Close(None)).await?;
        async_std::task::sleep(Duration::from_secs(1)).await;
        assert_eq!(0, channel.0.read().await.len());
        Ok(())
    }

    #[async_std::test]
    async fn broadcast() -> Result<(), Box<dyn StdError>> {
        let channel = SyncChannel::new();
        let app = App::state(channel.clone()).end(route("/")?);
        let (addr, server) = app.run()?;
        async_std::task::spawn(server);
        let url = format!("ws://{}/chat", addr);
        for _ in 0..100 {
            let url = url.clone();
            async_std::task::spawn(async move {
                if let Ok((ws_stream, _)) = connect_async(url).await {
                    let (mut sender, mut recv) = ws_stream.split();
                    if let Some(Ok(message)) = recv.next().await {
                        assert!(sender.send(message).await.is_ok());
                    }
                    async_std::task::sleep(Duration::from_secs(1)).await;
                    assert!(sender.send(Message::Close(None)).await.is_ok());
                }
            });
        }
        async_std::task::sleep(Duration::from_secs(1)).await;
        assert_eq!(100, channel.0.read().await.len());

        let (ws_stream, _) = connect_async(url).await?;
        let (mut sender, mut recv) = ws_stream.split();
        assert!(sender
            .send(Message::Text("Hello, World!".to_string()))
            .await
            .is_ok());
        async_std::task::sleep(Duration::from_secs(2)).await;
        assert_eq!(1, channel.0.read().await.len());

        let mut counter = 0i32;
        while let Some(item) = recv.next().await {
            if let Ok(Message::Text(message)) = item {
                assert_eq!("Hello, World!", message);
            }
            counter += 1;
            if counter == 101 {
                break;
            }
        }
        Ok(())
    }
}
