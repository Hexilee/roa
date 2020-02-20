use async_std::sync::{Arc, Mutex, RwLock};
use futures::stream::SplitSink;
use futures::{SinkExt, StreamExt};
use http::Method;
use log::{error, info};
use roa::core::App;
use roa::logger::logger;
use roa::router::Router;
use roa::websocket::{tungstenite::Error as WsError, Message, SocketStream, Websocket};
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

    async fn register(&self, sender: Sender) -> usize {
        self.0.write().await.insert(Mutex::new(sender))
    }

    async fn deregister(&self, index: usize) -> Sender {
        self.0.write().await.remove(index).into_inner()
    }
}

async fn handle_message(
    state: &SyncChannel,
    message: Result<Message, WsError>,
) -> Result<(), WsError> {
    state.broadcast(message?).await
}

#[async_std::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    pretty_env_logger::init();
    let mut app = App::new(SyncChannel::new());
    let mut router = Router::new();
    router.end(
        [Method::GET].as_ref(),
        "/chat",
        Websocket::new(|state: SyncChannel, stream| async move {
            let (sender, mut receiver) = stream.split();
            let index = state.register(sender).await;
            while let Some(message) = receiver.next().await {
                if let Err(err) = handle_message(&state, message).await {
                    error!("websocket error: {}", err);
                }
            }
            let mut sender = state.deregister(index).await;
            sender.send(Message::Close(None)).await.unwrap();
        }),
    );
    app.gate(logger)
        .gate(router.routes("/")?)
        .listen("127.0.0.1:8000", |addr| {
            info!("Server is listening on {}", addr)
        })?
        .await?;
    Ok(())
}
