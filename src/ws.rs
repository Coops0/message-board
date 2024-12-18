use crate::{fallback, messages::FullMessage, user::User, util::OptionalExtractor, AppState};
use axum::{
    extract::{
        ws::{Message, WebSocket}, State, WebSocketUpgrade
    }, response::Response
};
use cbc::{
    cipher::{block_padding::Pkcs7, BlockEncryptMut, KeyIvInit}, Encryptor
};
use futures::FutureExt;
use serde_json::json;
use std::{future::Future, io, pin::Pin, time::Duration};
use tokio::{sync::mpsc::Receiver, time::timeout};
use tokio_util::{
    bytes::{BufMut, BytesMut}, codec::Encoder
};
use uuid::Uuid;

#[allow(clippy::unused_async)]
pub async fn ws_route(
    OptionalExtractor(maybe_ws): OptionalExtractor<WebSocketUpgrade>,
    owner: User,
    State(AppState { tx, .. }): State<AppState>
) -> Response {
    let Some(ws) = maybe_ws else {
        return fallback().await;
    };

    ws.on_upgrade(move |socket| async move {
        let _ = tx.send(WebsocketActorMessage::Socket { socket, owner }).await;
    })
}

pub enum WebsocketActorMessage {
    Socket { socket: WebSocket, owner: User },
    Message { message: FullMessage, is_update: bool },
    RequestCount { id: Uuid }
}

type SendMessageFuture<'a, E = axum::Error> =
    Pin<Box<dyn Future<Output = Result<(), E>> + Send + 'a>>;

type BroadcastSendMessageFuture<'a> = SendMessageFuture<'a, (axum::Error, Uuid)>;

pub async fn socket_owner_actor(mut rx: Receiver<WebsocketActorMessage>) {
    let mut sockets: Vec<(WebSocket, User)> = Vec::new();

    loop {
        let message_or_timeout = timeout(Duration::from_secs(1), rx.recv()).await;
        let Ok(msg) = message_or_timeout else {
            prune_dead_sockets(&mut sockets).await;
            continue;
        };

        let Some(msg) = msg else { break };

        match msg {
            WebsocketActorMessage::Socket { socket, owner } => sockets.push((socket, owner)),
            WebsocketActorMessage::Message { message, is_update } => {
                broadcast(&mut sockets, &message, is_update).await;
            }
            WebsocketActorMessage::RequestCount { id } => {
                let len = sockets.len();
                let Some((socket, _)) = sockets.iter_mut().find(|(_, user)| user.id.eq(&id)) else {
                    continue;
                };

                let _ = socket.send(Message::Text(json!({"count": len}).to_string())).await;
            }
        }
    }
}

async fn broadcast(sockets: &mut Vec<(WebSocket, User)>, message: &FullMessage, is_update: bool) {
    let send_futures: Vec<BroadcastSendMessageFuture> = sockets
        .iter_mut()
        .filter(|(_, owner)| !owner.id.eq(&message.author))
        .filter_map(|(socket, user)| -> Option<BroadcastSendMessageFuture> {
            let mut message_enc = MessageEncoder::new(user.encryption_key());

            let ws_msg = message.encode_message_for(&mut message_enc, user, is_update)?;

            let fut = async move { socket.send(ws_msg).await.map_err(|e| (e, user.id)) };
            Some(Box::pin(fut) as BroadcastSendMessageFuture)
        })
        .collect();

    futures::future::join_all(send_futures).await.into_iter().filter_map(Result::err).for_each(
        |(_e, id)| {
            // remove any dead sockets
            sockets.retain(|(_, user)| !user.id.eq(&id));
        }
    );
}

async fn prune_dead_sockets(sockets: &mut Vec<(WebSocket, User)>) {
    let before_len = sockets.len();

    sockets.retain_mut(|(socket, _)| match socket.recv().now_or_never() {
        // No immediate future response - PASS
        // Immediate future response WITH content - PASS
        None | Some(Some(_)) => true,
        // Immediate future response WITH NONE, no content - FAIL
        _ => false
    });

    // new count, update admins
    let len = sockets.len();
    if before_len == len {
        return;
    }

    let send_futures = sockets
        .iter_mut()
        .filter(|(_, user)| user.admin)
        .map(|(socket, _)| {
            Box::pin(
                async move { socket.send(Message::Text(json!({"count": len}).to_string())).await }
            ) as SendMessageFuture
        })
        .collect::<Vec<_>>();

    let _ = futures::future::join_all(send_futures).await;
}

struct MessageEncoder {
    encryptor: Encryptor<aes::Aes128>,
    iv: [u8; 16]
}

impl MessageEncoder {
    fn new<K: Into<Vec<u8>>>(key: K) -> Self {
        let key = key.into();

        let iv = rand::random::<[u8; 16]>();
        let encryptor = Encryptor::<aes::Aes128>::new((&key[..]).into(), iv.as_ref().into());

        Self { encryptor, iv }
    }

    fn init(&self, dst: &mut BytesMut) {
        dst.put(&self.iv[..]);
    }

    #[allow(clippy::cast_possible_truncation, clippy::needless_pass_by_value)]
    fn put_encrypted<S: ToString>(&self, content: S, dst: &mut BytesMut) {
        let ct =
            self.encryptor.clone().encrypt_padded_vec_mut::<Pkcs7>(content.to_string().as_bytes());

        dst.put_u32(ct.len() as u32);
        dst.extend_from_slice(&ct);
    }

    fn noise(dst: &mut BytesMut) {
        dst.extend_from_slice(&rand::random::<[u8; 8]>());
    }
}

impl Encoder<&FullMessage> for MessageEncoder {
    type Error = io::Error;

    fn encode(&mut self, item: &FullMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        self.init(dst);
        dst.put_u8(0);

        Self::noise(dst);

        self.put_encrypted(item.id, dst);
        self.put_encrypted(&item.content, dst);
        self.put_encrypted(item.created_at, dst);
        self.put_encrypted(item.author, dst);

        Self::noise(dst);

        Ok(())
    }
}

struct DeleteMessage(Uuid);
impl Encoder<&DeleteMessage> for MessageEncoder {
    type Error = io::Error;

    fn encode(&mut self, item: &DeleteMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        self.init(dst);
        dst.put_u8(1);

        Self::noise(dst);

        self.put_encrypted(item.0, dst);

        Self::noise(dst);

        Ok(())
    }
}

impl FullMessage {
    fn encode_message_for(
        &self,
        encoder: &mut MessageEncoder,
        user: &User,
        is_update: bool
    ) -> Option<Message> {
        if user.admin {
            return if is_update {
                None
            } else {
                Some(Message::Text(serde_json::to_string(&self).ok()?))
            };
        }

        let mut body = BytesMut::new();

        if is_update && !self.published {
            encoder.encode(&DeleteMessage(self.id), &mut body)
        } else {
            encoder.encode(self, &mut body)
        }
        .ok()?;

        Some(Message::Binary(body.freeze().to_vec()))
    }
}
