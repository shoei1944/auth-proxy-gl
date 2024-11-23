use crate::launcher::error;
use crate::launcher::types::{request, response};
use dashmap::DashMap;
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
    actor_handle: JoinHandle<()>,
}

impl Socket {
    pub async fn new(addr: impl Into<String>) -> Result<Socket, tungstenite::Error> {
        let (ws_stream, _) = tokio_tungstenite::connect_async(addr.into()).await?;
        let (ws_sender, ws_receiver) = ws_stream.split();

        let (sender, mut receiver) = mpsc::channel::<ActorMessage>(64);
        let actor = Arc::new(SocketActor::new(ws_sender, ws_receiver));

        let actor_handle = tokio::spawn(async move {
            while let Some(msg) = receiver.recv().await {
                let actor = actor.clone();

                tokio::spawn(async move {
                    let (tx, rx) = oneshot::channel();
                    let request_id = msg.request.id;

                    actor.callbacks.insert(request_id, tx);

                    if let Err(err) = actor.handle_message(msg, rx).await {
                        error!("Error with handle actor message: {:?}", err);
                    };

                    actor.callbacks.remove(&request_id);
                });
            }
        });

        Ok(Socket {
            sender,
            actor_handle,
        })
    }

    pub async fn restore_token(
        &self,
        pair: request::restore_token::Pair,
        user_info: bool,
    ) -> Result<response::restore_token::RestoreToken, error::Error> {
        let response = self
            .send_to_actor(request::Request {
                id: Uuid::new_v4(),
                body: request::any::Kind::RestoreToken(request::restore_token::RestoreToken {
                    extended: HashMap::from([(pair.name, pair.value)]),
                    need_user_info: user_info,
                }),
            })
            .await?;

        if let response::any::Kind::RestoreToken(restore_token) = response {
            Ok(restore_token)
        } else {
            Err(error::Error::UnexpectedResponse(response))
        }
    }

    pub async fn get_public_key(
        &self,
    ) -> Result<response::get_public_key::GetPublicKey, error::Error> {
        let response = self
            .send_to_actor(request::Request {
                id: Uuid::new_v4(),
                body: request::any::Kind::GetPublicKey(request::get_public_key::GetPublicKey {}),
            })
            .await?;

        if let response::any::Kind::GetPublicKey(get_public_key) = response {
            Ok(get_public_key)
        } else {
            Err(error::Error::UnexpectedResponse(response))
        }
    }

    pub async fn check_server(
        &self,
        username: impl Into<String>,
        server_id: impl Into<String>,
        need_hardware: bool,
        need_properties: bool,
    ) -> Result<response::check_server::CheckServer, error::Error> {
        let response = self
            .send_to_actor(request::Request {
                id: Uuid::new_v4(),
                body: request::any::Kind::CheckServer(request::check_server::CheckServer {
                    username: username.into(),
                    server_id: server_id.into(),
                    need_hardware,
                    need_properties,
                }),
            })
            .await?;

        if let response::any::Kind::CheckServer(check_server) = response {
            Ok(check_server)
        } else {
            Err(error::Error::UnexpectedResponse(response))
        }
    }

    pub async fn get_profile_by_uuid(
        &self,
        uuid: Uuid,
    ) -> Result<response::get_profile_by_uuid::GetProfileByUuid, error::Error> {
        let response = self
            .send_to_actor(request::Request {
                id: Uuid::new_v4(),
                body: request::any::Kind::GetProfileByUuid(
                    request::get_profile_by_uuid::GetProfileByUuid { uuid },
                ),
            })
            .await?;

        if let response::any::Kind::GetProfileByUuid(get_profile_by_uuid) = response {
            Ok(get_profile_by_uuid)
        } else {
            Err(error::Error::UnexpectedResponse(response))
        }
    }

    async fn send_to_actor(
        &self,
        request: request::any::Any,
    ) -> Result<response::any::Kind, error::Error> {
        let (tx, rx) = oneshot::channel();

        self.sender
            .send(ActorMessage {
                sender: tx,
                request,
            })
            .await
            .map_err(error::ActorError::from)?;

        rx.await
            .map_err(error::ActorError::from)
            .map_err(|err| err.into())
    }
}

struct ActorMessage {
    sender: oneshot::Sender<response::any::Kind>,
    request: request::any::Any,
}

type Callbacks = DashMap<Uuid, oneshot::Sender<response::any::Kind>>;

type ActorSender = mpsc::Sender<ActorMessage>;
type ActorReceiver = mpsc::Receiver<ActorMessage>;

type WebSocketSender = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, tungstenite::Message>;
type WebSocketReceiver = SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;

struct SocketActor {
    callbacks: Arc<Callbacks>,
    ws_sender: Mutex<WebSocketSender>,
    receiver_handle: JoinHandle<()>,
}

impl SocketActor {
    pub fn new(ws_sender: WebSocketSender, ws_receiver: WebSocketReceiver) -> SocketActor {
        let callbacks = Arc::new(DashMap::new());
        let receiver_handle = tokio::spawn(start_receiver_handle(ws_receiver, callbacks.clone()));

        SocketActor {
            callbacks,
            ws_sender: Mutex::new(ws_sender),
            receiver_handle,
        }
    }

    pub async fn handle_message(
        &self,
        message: ActorMessage,
        receiver: oneshot::Receiver<response::any::Kind>,
    ) -> Result<(), error::ActorError> {
        let json_request = serde_json::to_string(&message.request)?;
        info!("Request: {}", json_request);

        let _ = {
            let mut sender = self.ws_sender.lock().await;

            sender.send(tungstenite::Message::Text(json_request)).await
        };
        let response = time::timeout(Duration::from_secs(10), receiver).await??;

        message
            .sender
            .send(response)
            .map_err(|_| error::ActorError::Send)
    }
}

async fn start_receiver_handle(receiver: WebSocketReceiver, callbacks: Arc<Callbacks>) {
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

                let _ = callbacks
                    .remove(&response.id)
                    .map(|(_, sender)| sender.send(response.body));

                Ok(())
            }
        })
        .await;
}
