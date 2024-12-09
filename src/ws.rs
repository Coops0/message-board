use crate::messages::FullMessage;
use crate::user::User;
use crate::AppState;
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::Response;
use cbc::cipher::block_padding::Pkcs7;
use cbc::cipher::{BlockEncryptMut, KeyIvInit};
use cbc::Encryptor;
use std::future::Future;
use std::io;
use std::pin::Pin;
use tokio::sync::mpsc::Receiver;
use tokio_util::bytes::{BufMut, BytesMut};
use tokio_util::codec::Encoder;
use tracing::warn;
use uuid::Uuid;

#[allow(clippy::unused_async)]
pub async fn ws_route(
    ws: WebSocketUpgrade,
    owner: User,
    State(AppState { tx, .. }): State<AppState>,
) -> Response {
    ws.on_upgrade(move |socket| async move {
        let _ = tx
            .send(WebsocketActorMessage::Socket { socket, owner })
            .await;
    })
}

pub enum WebsocketActorMessage {
    Socket { socket: WebSocket, owner: User },
    Message(FullMessage),
}

type SendMessageFuture<'a> = Pin<Box<dyn Future<Output = Result<(), axum::Error>> + Send + 'a>>;

pub async fn socket_owner_actor(mut rx: Receiver<WebsocketActorMessage>) {
    let mut sockets: Vec<(WebSocket, User)> = Vec::new();

    while let Some(msg) = rx.recv().await {
        match msg {
            WebsocketActorMessage::Socket { socket, owner } => sockets.push((socket, owner)),
            WebsocketActorMessage::Message(message) => {
                let send_futures: Vec<SendMessageFuture> = sockets
                    .iter_mut()
                    .filter(|(_, owner)| !owner.id.eq(&message.author))
                    .filter_map(|(socket, user)| -> Option<SendMessageFuture> {
                        let mut message_enc = FullMessageEncoder(user.id);

                        let ws_msg = message.encode_message_for(&mut message_enc, user).ok()?;

                        Some(Box::pin(socket.send(ws_msg)) as SendMessageFuture)
                    })
                    .collect();

                futures::future::join_all(send_futures)
                    .await
                    .into_iter()
                    .filter_map(Result::err)
                    .for_each(|e| warn!("error sending ws message: {e:?}"));
            }
        }
    }
}

struct FullMessageEncoder(Uuid);

type Aes128CbcEnc = Encryptor<aes::Aes128>;

#[allow(clippy::cast_possible_truncation)]
fn encode_and_encrypt_str(encryptor: Aes128CbcEnc, content: &str, dst: &mut BytesMut) {
    let ct = encryptor.encrypt_padded_vec_mut::<Pkcs7>(content.as_bytes());

    dst.put_u32(ct.len() as u32);
    dst.extend_from_slice(&ct);
}

impl Encoder<&FullMessage> for FullMessageEncoder {
    type Error = io::Error;

    fn encode(&mut self, item: &FullMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let uuid_string = self.0.to_string();

        let key = &uuid_string.as_bytes()[0..16];
        let iv = rand::random::<[u8; 16]>();

        let encryptor = Aes128CbcEnc::new(key.into(), iv.as_ref().into());

        dst.put(&iv[..]);
        encode_and_encrypt_str(Encryptor::clone(&encryptor), &item.content, dst);
        encode_and_encrypt_str(encryptor, &item.created_at.to_string(), dst);

        Ok(())
    }
}

impl FullMessage {
    fn encode_message_for(
        &self,
        encoder: &mut FullMessageEncoder,
        user: &User,
    ) -> anyhow::Result<Message> {
        if user.admin {
            return Ok(Message::Text(serde_json::to_string(&self)?));
        }

        let mut body = BytesMut::new();
        encoder.encode(self, &mut body)?;

        Ok(Message::Binary(body.freeze().to_vec()))
    }
}
