use crate::{
    config,
    launcher::{
        error,
        types::{request, response},
    },
};
use dashmap::DashMap;
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use std::{
    collections::{HashMap, VecDeque},
    future::Future,
    sync::Arc,
    time::Duration,
};
use tokio::{
    net::TcpStream,
    sync::{mpsc, oneshot, Mutex},
    task::JoinHandle,
};
use tokio_tungstenite::{tungstenite, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error};
use uuid::Uuid;

macro_rules! extract_response {
    ($response:expr, $kind:path) => {
        if let $kind(value) = $response {
            Ok(value)
        } else {
            Err(error::Error::UnexpectedResponse($response))
        }
    };
}

const CONCURRENCY: usize = 128;

struct ActorMessage {
    sender: oneshot::Sender<response::any::Kind>,
    request: request::any::Any,
}

type WsRequestsCallbacks = DashMap<Uuid, oneshot::Sender<response::any::Kind>>;

type ActorSender = mpsc::Sender<ActorMessage>;
type ActorReceiver = mpsc::Receiver<ActorMessage>;

type WebSocketSender = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, tungstenite::Message>;
type WebSocketReceiver = SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;

pub struct Socket {
    actor_token: tokio_util::sync::CancellationToken,
    actor_sender: ActorSender,
    actor_handle: JoinHandle<()>,
}

impl Socket {
    pub fn new(addr: impl Into<url::Url>) -> Socket {
        let actor_token = tokio_util::sync::CancellationToken::new();
        let (actor_sender, actor_receiver) = mpsc::channel::<ActorMessage>(CONCURRENCY);
        let actor_handle = tokio::spawn(start_handle_loop(
            addr.into(),
            actor_receiver,
            actor_token.clone(),
        ));

        Socket {
            actor_token,
            actor_sender,
            actor_handle,
        }
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

        extract_response!(response, response::any::Kind::RestoreToken)
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

        extract_response!(response, response::any::Kind::CheckServer)
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

        extract_response!(response, response::any::Kind::GetProfileByUuid)
    }

    pub async fn get_profile_by_username(
        &self,
        username: impl Into<String>,
    ) -> Result<response::get_profile_by_username::GetProfileByUsername, error::Error> {
        let response = self
            .send_to_actor(request::Request {
                id: Uuid::new_v4(),
                body: request::any::Kind::GetProfileByUsername(
                    request::get_profile_by_username::GetProfileByUsername {
                        username: username.into(),
                    },
                ),
            })
            .await?;

        extract_response!(response, response::any::Kind::GetProfileByUsername)
    }

    pub async fn batch_profiles_by_usernames(
        &self,
        usernames: Vec<impl Into<String>>,
    ) -> Result<response::batch_profiles_by_usernames::BatchProfilesByUsernames, error::Error> {
        let response = self
            .send_to_actor(request::Request {
                id: Uuid::new_v4(),
                body: request::any::Kind::BatchProfilesByUsernames(
                    request::batch_profiles_by_usernames::BatchProfilesByUsernames {
                        list: usernames
                            .into_iter()
                            .map(|username| request::batch_profiles_by_usernames::Entry {
                                username: username.into(),
                            })
                            .collect::<Vec<_>>(),
                    },
                ),
            })
            .await?;

        extract_response!(response, response::any::Kind::BatchProfilesByUsernames)
    }

    async fn send_to_actor(
        &self,
        request: request::any::Any,
    ) -> Result<response::any::Kind, error::Error> {
        let (tx, rx) = oneshot::channel();

        self.actor_sender
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

    pub fn shutdown(&self) {
        self.actor_token.cancel();
    }
}

pub async fn execute_with_token_restore<T, F, Fut>(
    socket: Arc<Socket>,
    server: &config::Server,
    action: F,
) -> Result<T, error::Error>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T, error::Error>>,
{
    match action().await {
        Ok(result) => Ok(result),
        Err(err) => match err {
            error::Error::UnexpectedResponse(response::any::Kind::Error(
                response::error::Error {
                    kind: response::error::Kind::PermissionsDenied,
                },
            )) => {
                let res = socket
                    .restore_token(
                        request::restore_token::Pair {
                            name: "checkServer".to_string(),
                            value: server.token.clone(),
                        },
                        false,
                    )
                    .await;

                if res.map(|v| v.invalid_tokens.is_empty()).unwrap_or(false) {
                    action().await
                } else {
                    Err(err)
                }
            }
            _ => Err(err),
        },
    }
}

async fn start_handle_loop(
    addr: impl Into<url::Url>,
    mut receiver: ActorReceiver,
    token: tokio_util::sync::CancellationToken,
) {
    let addr = addr.into();

    let notify = Arc::new(tokio::sync::Notify::new());
    let mut join_set = tokio::task::JoinSet::<()>::new();
    let callbacks = Arc::new(DashMap::new() as WsRequestsCallbacks);

    let mut ws_sender: Option<Arc<Mutex<WebSocketSender>>> = None;
    let mut ws_rx: Option<mpsc::Receiver<String>> = None;
    let mut ws_token: Option<tokio_util::sync::CancellationToken> = None;

    let (pending_tx, mut pending_rx) = mpsc::channel::<ActorMessage>(CONCURRENCY);
    let pending_tx = Arc::new(pending_tx);
    let mut pending_messages = VecDeque::<ActorMessage>::new();

    async fn recv_next_actor_msg(
        pending_messages: &mut VecDeque<ActorMessage>,
        receiver: &mut ActorReceiver,
    ) -> Option<ActorMessage> {
        if let Some(message) = pending_messages.pop_back() {
            Some(message)
        } else {
            receiver.recv().await
        }
    }

    async fn recv_next_ws_msg(rx: Option<&mut mpsc::Receiver<String>>) -> Option<String> {
        if let Some(rx) = rx {
            rx.recv().await
        } else {
            None
        }
    }

    loop {
        if ws_sender.is_none() {
            match tokio_tungstenite::connect_async(addr.clone()).await {
                Ok((ws_stream, _)) => {
                    let (sender, receiver) = ws_stream.split();
                    let sender = Arc::new(Mutex::new(sender));
                    let (tx, rx) = mpsc::channel::<String>(64);

                    ws_sender = Some(sender);
                    ws_rx = Some(rx);

                    let token = token.child_token();
                    ws_token = Some(token.clone());

                    tokio::spawn(start_handle_websocket_messages(receiver, tx, token));
                }
                Err(err) => {
                    error!("Failed to connect: {}. Retrying in 1 second...", err);
                    tokio::time::sleep(Duration::from_secs(1)).await;

                    continue;
                }
            }
        }

        tokio::select! {
            Some(actor_msg) = recv_next_actor_msg(&mut pending_messages, &mut receiver) => {
                debug!("{:?}", actor_msg.request);

                if let Some(ws_sender) = ws_sender.as_ref() {
                    join_set.spawn({
                        let pending_tx = pending_tx.clone();

                        let ws_sender = ws_sender.clone();
                        let callbacks = callbacks.clone();
                        let notify = notify.clone();

                        async move {
                            match handle_actor_request(ws_sender, callbacks, &actor_msg.request).await {
                                Ok(response) => {
                                    if actor_msg.sender.send(response).is_err() {
                                        debug!("failed to send response from actor")
                                    }
                                },
                                Err(err) => {
                                    match err {
                                        error::ActorError::Socket(tungstenite::Error::ConnectionClosed)
                                        | error::ActorError::Socket(tungstenite::Error::AlreadyClosed)
                                        | error::ActorError::Socket(tungstenite::Error::Io(_))
                                        | error::ActorError::Socket(tungstenite::Error::Tls(_))
                                        | error::ActorError::Socket(tungstenite::Error::Protocol(_)) => {
                                            let _ = pending_tx.send(actor_msg).await;

                                            notify.notify_waiters();
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    });
                }
            }
            Some(msg) = recv_next_ws_msg(ws_rx.as_mut()) => {
                join_set.spawn({
                    let callbacks = callbacks.clone();

                    async move {
                        let Ok(deserialzied) = serde_json::from_str::<response::any::Any>(&msg) else {
                            return;
                        };
                        debug!("Response: {:?}", deserialzied);

                        let _ = callbacks
                            .remove(&deserialzied.id)
                            .map(|(_, sender)| sender.send(deserialzied.body));
                    }
                });
            }
            Some(pending_msg) = pending_rx.recv() => {
                pending_messages.push_back(pending_msg);
            }
            _ = notify.notified() => {
                debug!("socket disconnect notify received");

                ws_sender = None;
                ws_rx = None;
                if let Some(token) = ws_token.as_ref() {
                    token.cancel()
                }
            }
            _ = token.cancelled() => {
                debug!("start_handle_loop cancelled");

                join_set.join_all().await;

                break;
            }
        }
    }
}

async fn handle_actor_request(
    ws_sender: Arc<Mutex<WebSocketSender>>,
    callbacks: Arc<WsRequestsCallbacks>,
    request: &request::any::Any,
) -> Result<response::any::Kind, error::ActorError> {
    let (tx, rx) = oneshot::channel();
    let request_id = request.id;

    callbacks.insert(request_id, tx);

    let clos = || async {
        let json_request = serde_json::to_string(&request)?;
        debug!("Raw request: {}", json_request);

        ws_sender
            .lock()
            .await
            .send(tungstenite::Message::Text(json_request))
            .await?;

        let response = rx.await?;

        Result::<response::any::Kind, error::ActorError>::Ok(response)
    };

    let res = clos().await;

    callbacks.remove(&request_id);

    res
}

async fn start_handle_websocket_messages(
    mut ws_receiver: WebSocketReceiver,
    sender: mpsc::Sender<String>,
    token: tokio_util::sync::CancellationToken,
) {
    let mut join_set = tokio::task::JoinSet::<()>::new();

    loop {
        tokio::select! {
            Some(msg) = ws_receiver.next() => {
                match msg {
                    Ok(tungstenite::Message::Text(raw_response)) => {
                        while join_set.len() >= CONCURRENCY {
                            join_set.join_next().await;
                        }

                        debug!("Raw response: {}", raw_response);
                        let _ = sender.send(raw_response).await;
                    },
                    Ok(v) => {
                        debug!("unknown msg received: {:?}", v);
                    },
                    Err(err) => {
                        debug!("socket receiver error: {}", err);
                    },
                }
            },
            _ = token.cancelled() => {
                debug!("start_ws_receiver_handle cancelled");

                join_set.join_all().await;

                break;
            }
        }
    }
}
