use anyhow::{bail, Result};
use bytes::Bytes;
use core::message::{Kind, MediaPacket, MessageInitPayload, MessageInitPayloadKind, ProtoMessage};
use core::transport::JoinResp;
use core::{ChannelMessage, ManagerHandle, Message};
use futures::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
#[derive(Debug)]
enum State {
    Init,
    Player(String),
}

pub struct Connection {
    id: u32,
    state: State,
    frame: Framed<TcpStream, LengthDelimitedCodec>,
    manager_handle: ManagerHandle,
}

impl Connection {
    pub fn new(stream: TcpStream, id: u32, manager_handle: ManagerHandle) -> Self {
        let frame = Framed::new(stream, LengthDelimitedCodec::new());
        Self {
            frame,
            state: State::Init,
            id,
            manager_handle,
        }
    }

    async fn disconnected(&mut self, msg: &'static str) -> Result<()> {
        let proto_message = ProtoMessage::new_proto_error(Bytes::from(msg));
        self.frame.send(proto_message.into()).await?;
        Ok(())
    }

    async fn send(&mut self, packet: MediaPacket) -> Result<()> {
        let proto_message: ProtoMessage = packet.into();
        self.frame.send(proto_message.into()).await?;
        Ok(())
    }

    pub async fn run(&mut self) -> Result<()> {
        let data = if let Some(Ok(data)) = self.frame.next().await {
            data
        } else {
            bail!("get data error");
        };

        let message = ProtoMessage::try_from(data.freeze())?;
        if let State::Init = &self.state {
            if let Kind::Init = message.kind {
                let init_message = MessageInitPayload::try_from(message.payload)?;
                match init_message.kind {
                    MessageInitPayloadKind::Publisher => {
                        self.disconnected("init message verify failed").await?;
                        bail!("init message verify failed");
                    }
                    MessageInitPayloadKind::Player => {
                        //协议要兼容直接连源站
                        log::info!("got new player");
                        let (request, response) = oneshot::channel();
                        if self
                            .manager_handle
                            .send(ChannelMessage::Join((
                                init_message.app_name.clone(),
                                request,
                            )))
                            .is_err()
                        {
                            self.disconnected("ChannelJoinFailed").await?;
                            return Ok(());
                        }

                        self.state = State::Player(init_message.app_name);
                        //respone join ok
                        log::info!("send proto message ok");
                        self.frame.send(ProtoMessage::new_proto_ok().into()).await?;

                        if let Ok(join_resp) = response.await {
                            let (session_sender, mut session_receiver, need_init_data) =
                                match join_resp {
                                    JoinResp::Local(session_sender, session_receiver) => {
                                        (session_sender, session_receiver, true)
                                    }
                                    JoinResp::Origin(session_sender, session_receiver) => {
                                        (session_sender, session_receiver, false)
                                    }
                                };

                            if need_init_data {
                                let (request, response) = oneshot::channel();
                                if session_sender.send(Message::InitData(request)).is_err() {
                                    self.disconnected("session_sender send failed").await?;
                                    return Ok(());
                                };
                                if let Ok((meta, video, audio, gop)) = response.await {
                                    if let Some(m) = meta {
                                        self.send(m).await?;
                                    }
                                    if let Some(v) = video {
                                        self.send(v).await?;
                                    }
                                    if let Some(a) = audio {
                                        self.send(a).await?;
                                    }
                                    if let Some(a) = gop {
                                        for p in a {
                                            self.send(p).await?;
                                        }
                                    }
                                }
                            }
                            loop {
                                use tokio::sync::broadcast::error::RecvError;
                                match session_receiver.recv().await {
                                    Ok(data) => {
                                        let proto_message: ProtoMessage = data.into();
                                        self.frame.send(proto_message.into()).await?;
                                    }
                                    Err(RecvError::Closed) => {
                                        self.disconnected("session_receiver is closed").await?;
                                        break;
                                    }
                                    Err(_) => {}
                                }
                            }
                        } else {
                            self.disconnected("app_name not found").await?;
                        }
                    }
                }
            } else {
                bail!("first message kind must be init")
            }
        }
        self.disconnected("close").await?;
        Ok(())
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        println!(
            "connecton {:?},state:{:?} disconnected",
            self.id, self.state
        );
    }
}
