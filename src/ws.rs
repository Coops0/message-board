use crate::messages::FullMessage;
use crate::user::User;
use crate::{fallback, AppState};
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
use uuid::Uuid;

#[allow(clippy::unused_async)]
pub async fn ws_route(
    maybe_ws: Option<WebSocketUpgrade>,
    owner: User,
    State(AppState { tx, .. }): State<AppState>,
) -> Response {
    let Some(ws) = maybe_ws else {
        return fallback().await;
    };

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
                        let mut message_enc = MessageEncoder::new(user.encryption_key());

                        let ws_msg = message
                            .encode_message_for(&mut message_enc, user, is_update)
                            .ok()??;

                        let fut =
                            async move { socket.send(ws_msg).await.map_err(|e| (e, user.id)) };

                        Some(Box::pin(fut) as SendMessageFuture)
                    })
                    .collect();

                futures::future::join_all(send_futures)
                    .await
                    .into_iter()
                    .filter_map(Result::err)
                    .for_each(|(_e, id)| {
                        // remove any dead sockets
                        sockets.retain(|(_, user)| !user.id.eq(&id));
                    });
            }
        }
    }
}

struct MessageEncoder {
    encryptor: Encryptor<aes::Aes128>,
    iv: [u8; 16],
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
        let ct = self
            .encryptor
            .clone()
            .encrypt_padded_vec_mut::<Pkcs7>(content.to_string().as_bytes());

        dst.put_u32(ct.len() as u32);
        dst.extend_from_slice(&ct);
    }
    
    fn noise(&self, dst: &mut BytesMut)  {
        dst.extend_from_slice(&rand::random::<[u8; 8]>());
    }
}

impl Encoder<&FullMessage> for MessageEncoder {
    type Error = io::Error;

    fn encode(&mut self, item: &FullMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        self.init(dst);
        dst.put_u8(0);

        self.noise(dst);
        
        self.put_encrypted(item.id, dst);
        self.put_encrypted(&item.content, dst);
        self.put_encrypted(item.created_at, dst);
        self.put_encrypted(item.author, dst);

        self.noise(dst);

        Ok(())
    }
}

struct DeleteMessage(Uuid);
impl Encoder<&DeleteMessage> for MessageEncoder {
    type Error = io::Error;

    fn encode(&mut self, item: &DeleteMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        self.init(dst);
        dst.put_u8(1);

        self.noise(dst);
        
        self.put_encrypted(item.0, dst);
        
        self.noise(dst);

        Ok(())
    }
}

impl FullMessage {
    fn encode_message_for(
        &self,
        encoder: &mut MessageEncoder,
        user: &User,
        is_update: bool,
    ) -> anyhow::Result<Option<Message>> {
        if user.admin {
            return if is_update {
                Ok(None)
            } else {
                Ok(Some(Message::Text(serde_json::to_string(&self)?)))
            };
        }

        let mut body = BytesMut::new();

        if is_update && !self.published {
            encoder.encode(&DeleteMessage(self.id), &mut body)?;
        } else {
            encoder.encode(self, &mut body)?;
        }

        Ok(Some(Message::Binary(body.freeze().to_vec())))
    }
}
