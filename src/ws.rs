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
    Socket {
        socket: WebSocket,
        owner: User,
    },
    Message {
        message: FullMessage,
        is_update: bool,
    },
}

type SendMessageFuture<'a> =
    Pin<Box<dyn Future<Output = Result<(), (axum::Error, Uuid)>> + Send + 'a>>;

pub async fn socket_owner_actor(mut rx: Receiver<WebsocketActorMessage>) {
    let mut sockets: Vec<(WebSocket, User)> = Vec::new();

    while let Some(msg) = rx.recv().await {
        match msg {
            WebsocketActorMessage::Socket { socket, owner } => sockets.push((socket, owner)),
            WebsocketActorMessage::Message { message, is_update } => {
                let send_futures: Vec<SendMessageFuture> = sockets
                    .iter_mut()
                    .filter(|(_, owner)| !owner.id.eq(&message.author))
                    .filter_map(|(socket, user)| -> Option<SendMessageFuture> {
                        let mut message_enc = MessageEncoder(user.encryption_key());

                        let ws_msg = message
                            .encode_message_for(&mut message_enc, user, is_update)
                            .ok()?;
                        let fut =
                            async move { socket.send(ws_msg).await.map_err(|e| (e, user.id)) };

                        Some(Box::pin(fut) as SendMessageFuture)
                    })
                    .collect();

                futures::future::join_all(send_futures)
                    .await
                    .into_iter()
                    .filter_map(Result::err)
                    .for_each(|(e, id)| {
                        warn!("error sending ws message: {e:?}");
                        // remove any dead sockets
                        sockets.retain(|(_, user)| !user.id.eq(&id));
                    });
            }
        }
    }
}

struct MessageEncoder(Vec<u8>);

impl MessageEncoder {
    fn prepare_encryption(&self) -> ([u8; 16], Encryptor<aes::Aes128>) {
        let iv = rand::random::<[u8; 16]>();
        let encryptor = Encryptor::<aes::Aes128>::new((&self.0[..]).into(), iv.as_ref().into());

        (iv, encryptor)
    }
}

#[allow(clippy::cast_possible_truncation)]
fn encode_and_encrypt_str(encryptor: Encryptor<aes::Aes128>, content: &str, dst: &mut BytesMut) {
    let ct = encryptor.encrypt_padded_vec_mut::<Pkcs7>(content.as_bytes());

    dst.put_u32(ct.len() as u32);
    dst.extend_from_slice(&ct);
}

impl Encoder<&FullMessage> for MessageEncoder {
    type Error = io::Error;

    fn encode(&mut self, item: &FullMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let (iv, encryptor) = self.prepare_encryption();
        dst.put(&iv[..]);

        dst.put_u8(0);

        // todo clean up this shit
        encode_and_encrypt_str(Encryptor::clone(&encryptor), &item.id.to_string(), dst);
        encode_and_encrypt_str(Encryptor::clone(&encryptor), &item.content, dst);
        encode_and_encrypt_str(
            Encryptor::clone(&encryptor),
            &item.created_at.to_string(),
            dst,
        );
        encode_and_encrypt_str(encryptor, &item.author.to_string(), dst);

        Ok(())
    }
}

struct DeleteMessage(Uuid);

impl Encoder<&DeleteMessage> for MessageEncoder {
    type Error = io::Error;

    fn encode(&mut self, item: &DeleteMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let (iv, encryptor) = self.prepare_encryption();

        dst.put(&iv[..]);

        dst.put_u8(1);
        encode_and_encrypt_str(Encryptor::clone(&encryptor), &item.0.to_string(), dst);

        Ok(())
    }
}

impl FullMessage {
    fn encode_message_for(
        &self,
        encoder: &mut MessageEncoder,
        user: &User,
        is_update: bool,
    ) -> anyhow::Result<Message> {
        if user.admin {
            return if is_update {
                Ok(Message::Text(String::new()))
            } else {
                Ok(Message::Text(serde_json::to_string(&self)?))
            };
        }

        let mut body = BytesMut::new();

        if is_update && !self.published {
            encoder.encode(&DeleteMessage(self.id), &mut body)?;
        } else {
            encoder.encode(self, &mut body)?;
        }

        Ok(Message::Binary(body.freeze().to_vec()))
    }
}
