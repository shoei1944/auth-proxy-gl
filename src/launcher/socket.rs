use crate::launcher::types::{request, response};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt, TryStreamExt};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::task::JoinHandle;
use tokio::time;
use tokio_tungstenite::{tungstenite, MaybeTlsStream, WebSocketStream};
use tracing::{error, info};
use uuid::Uuid;

pub struct Socket {
    sender: ActorSender,
    handle: JoinHandle<()>,
}

impl Socket {
    pub async fn new(addr: impl Into<String>) -> Result<Socket, tungstenite::Error> {
        let (ws_stream, _) = tokio_tungstenite::connect_async(addr.into()).await?;
        let (ws_sender, ws_receiver) = ws_stream.split();

        let (sender, receiver) = mpsc::channel::<ActorMessage>(64);
        let mut inner = SocketActor::new(ws_sender, ws_receiver, receiver);

        let handle = tokio::spawn(async move {
            while let Some(msg) = inner.receiver.recv().await {
                inner.handle_message(msg).await;
            }
        });

        Ok(Socket { sender, handle })
    }

    pub async fn restore_token(
        &self,
        pair: request::restore_token::Pair,
        user_info: bool,
    ) -> Option<response::restore_token::RestoreToken> {
        let (tx, rx) = oneshot::channel();

        let _ = self
            .sender
            .send(ActorMessage {
                sender: tx,
                request: request::Request {
                    id: Uuid::new_v4(),
                    body: request::any::Kind::RestoreToken(request::restore_token::RestoreToken {
                        extended: HashMap::from([(pair.name, pair.value)]),
                        need_user_info: user_info,
                    }),
                },
            })
            .await;

        rx.await.unwrap().and_then(|v| {
            if let response::any::Kind::RestoreToken(check_server) = v {
                Some(check_server)
            } else {
                None
            }
        })
    }

    pub async fn get_public_key(&self) -> Option<response::get_public_key::GetPublicKey> {
        let (tx, rx) = oneshot::channel();

        let _ = self
            .sender
            .send(ActorMessage {
                sender: tx,
                request: request::Request {
                    id: Uuid::new_v4(),
                    body: request::any::Kind::GetPublicKey(
                        request::get_public_key::GetPublicKey {},
                    ),
                },
            })
            .await;

        rx.await.unwrap().and_then(|v| {
            if let response::any::Kind::GetPublicKey(check_server) = v {
                Some(check_server)
            } else {
                None
            }
        })
    }

    pub async fn check_server(
        &self,
        username: impl Into<String>,
        server_id: impl Into<String>,
        need_hardware: bool,
        need_properties: bool,
    ) -> Option<response::check_server::CheckServer> {
        let (tx, rx) = oneshot::channel();

        let _ = self
            .sender
            .send(ActorMessage {
                sender: tx,
                request: request::Request {
                    id: Uuid::new_v4(),
                    body: request::any::Kind::CheckServer(request::check_server::CheckServer {
                        username: username.into(),
                        server_id: server_id.into(),
                        need_hardware,
                        need_properties,
                    }),
                },
            })
            .await;

        rx.await.unwrap().and_then(|v| {
            if let response::any::Kind::CheckServer(check_server) = v {
                Some(check_server)
            } else {
                None
            }
        })
    }

    pub async fn get_profile_by_uuid(
        &self,
        uuid: Uuid,
    ) -> Option<response::get_profile_by_uuid::GetProfileByUuid> {
        let (tx, rx) = oneshot::channel();

        let _ = self
            .sender
            .send(ActorMessage {
                sender: tx,
                request: request::Request {
                    id: Uuid::new_v4(),
                    body: request::any::Kind::GetProfileByUuid(
                        request::get_profile_by_uuid::GetProfileByUuid { uuid },
                    ),
                },
            })
            .await;

        rx.await.unwrap().and_then(|v| {
            if let response::any::Kind::GetProfileByUuid(check_server) = v {
                Some(check_server)
            } else {
                None
            }
        })
    }
}

struct ActorMessage {
    sender: oneshot::Sender<Option<response::any::Kind>>,
    request: request::any::Any,
}

type ActorSender = mpsc::Sender<ActorMessage>;
type ActorReceiver = mpsc::Receiver<ActorMessage>;

type WebSocketSender = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, tungstenite::Message>;
type WebSocketReceiver = SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;

struct SocketActor {
    callbacks: Arc<Mutex<HashMap<Uuid, oneshot::Sender<response::any::Kind>>>>,
    ws_sender: WebSocketSender,
    receiver: ActorReceiver,
    handle: JoinHandle<()>,
}

impl SocketActor {
    pub fn new(
        ws_sender: WebSocketSender,
        ws_receiver: WebSocketReceiver,
        receiver: ActorReceiver,
    ) -> SocketActor {
        let callbacks = Arc::new(Mutex::new(HashMap::new()));
        let handle = tokio::spawn(start_receiver_handle(ws_receiver, callbacks.clone()));

        SocketActor {
            callbacks,
            ws_sender,
            receiver,
            handle,
        }
    }

    pub async fn handle_message(&mut self, message: ActorMessage) {
        let (tx, rx) = oneshot::channel();
        let request_id = message.request.id;
        {
            let mut callbacks = self.callbacks.lock().await;
            callbacks.insert(request_id, tx);
        }

        if let Ok(json_request) = serde_json::to_string(&message.request) {
            info!("Request: {}", json_request);

            let _ = self
                .ws_sender
                .send(tungstenite::Message::Text(json_request))
                .await;

            if let Ok(Ok(v)) = time::timeout(Duration::from_secs(10), rx).await {
                let _ = message.sender.send(Some(v));
            }
        }

        {
            let mut callbacks = self.callbacks.lock().await;
            callbacks.remove(&request_id);
        }
    }
}

async fn start_receiver_handle(
    receiver: WebSocketReceiver,
    callbacks: Arc<Mutex<HashMap<Uuid, oneshot::Sender<response::any::Kind>>>>,
) {
    let _ = receiver
        .try_for_each_concurrent(64, |message| {
            let callbacks = callbacks.clone();

            async move {
                let tungstenite::Message::Text(body) = message else {
                    return Ok(());
                };

                info!("Response: {}", body);
                let response = match serde_json::from_str::<response::any::Any>(&body) {
                    Ok(v) => v,
                    Err(err) => {
                        error!("Error with deserialize from injector: {}", err);

                        return Ok(());
                    }
                };

                info!("Response id: {}", response.id);
                let mut callbacks = callbacks.lock().await;
                let _ = callbacks
                    .remove(&response.id)
                    .map(|v| v.send(response.body));

                Ok(())
            }
        })
        .await;
}
