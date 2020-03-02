use async_std::sync::{Arc, Mutex, RwLock};
use futures::stream::SplitSink;
use futures::{stream::SplitStream, SinkExt, StreamExt};
use http::Method;
use log::{error, info, warn};
use roa::logger::logger;
use roa::preload::*;
use roa::router::{RouteEndpoint, Router, RouterError};
use roa::websocket::{tungstenite::Error as WsError, Message, SocketStream, Websocket};
use roa::{App, SyncContext};
use slab::Slab;
use std::error::Error as StdError;

type Sender = SplitSink<SocketStream, Message>;
type Channel = Slab<Mutex<Sender>>;
#[derive(Clone)]
struct SyncChannel(Arc<RwLock<Channel>>);

impl SyncChannel {
    fn new() -> Self {
        Self(Arc::new(RwLock::new(Slab::new())))
    }

    async fn broadcast(&self, message: Message) -> Result<(), WsError> {
        let channel = self.0.read().await;
        for (_, sender) in channel.iter() {
            sender.lock().await.send(message.clone()).await?;
        }
        Ok(())
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
    ctx: &SyncContext<SyncChannel>,
    index: usize,
    mut receiver: SplitStream<SocketStream>,
) -> Result<(), WsError> {
    while let Some(message) = receiver.next().await {
        let message = message?;
        match message {
            Message::Close(frame) => {
                info!("websocket connection close: {:?}", frame);
                break;
            }
            Message::Ping(data) => ctx.send(index, Message::Pong(data)).await,
            Message::Pong(data) => warn!("ignored pong: {:?}", data),
            msg => ctx.broadcast(msg).await?,
        }
    }
    Ok(())
}

fn route(prefix: &'static str) -> Result<RouteEndpoint<SyncChannel>, RouterError> {
    let mut router = Router::new();
    router.end(
        [Method::GET].as_ref(),
        "/chat",
        Websocket::new(|ctx: SyncContext<SyncChannel>, stream| async move {
            let (sender, receiver) = stream.split();
            let index = ctx.register(sender).await;
            if let Err(err) = handle_message(&ctx, index, receiver).await {
                error!("websocket error: {}", err);
            }
            let _ = ctx.deregister(index).await;
        }),
    );
    router.routes(prefix)
}

#[async_std::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    pretty_env_logger::init();
    let mut app = App::new(SyncChannel::new());
    app.gate(logger)
        .gate(route("/")?)
        .listen("127.0.0.1:8000", |addr| {
            info!("Server is listening on {}", addr)
        })?
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{route, App, Message, SinkExt, StdError, StreamExt, SyncChannel};
    use async_tungstenite::async_std::connect_async;
    use roa::preload::*;
    use std::time::Duration;

    #[async_std::test]
    async fn echo() -> Result<(), Box<dyn StdError>> {
        let channel = SyncChannel::new();
        let mut app = App::new(channel.clone());
        let (addr, server) = app.gate(route("/")?).run_local()?;
        async_std::task::spawn(server);
        let (ws_stream, _) = connect_async(format!("ws://{}/chat", addr)).await?;
        let (mut sender, mut recv) = ws_stream.split();
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
}
