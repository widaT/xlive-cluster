use anyhow::{bail, Result};
use bytes::Bytes;
use core::message::{Kind, MediaPacket, MessageInitPayload, MessageInitPayloadKind, ProtoMessage};
use core::transport::JoinResp;
use core::{AppName, ChannelMessage, Handle, ManagerHandle, Message};
use futures::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
#[derive(Debug)]
enum State {
    Init,
    Publisher(AppName, Handle),
    Player(AppName),
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
        if let State::Publisher(app_name, session) = &mut self.state {
            _ = session.send(Message::Disconnect);
            _ = self
                .manager_handle
                .send(ChannelMessage::Release(app_name.to_owned()));
        }
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
        while let Some(Ok(data)) = self.frame.next().await {
            let message = ProtoMessage::try_from(data.freeze())?;
            match &self.state {
                State::Init => match message.kind {
                    Kind::Init => {
                        let init_message = MessageInitPayload::try_from(message.payload)?;
                        match init_message.kind {
                            MessageInitPayloadKind::Publisher => {
                                log::info!("got new publisher");
                                let (request, response) = oneshot::channel();
                                if self
                                    .manager_handle
                                    .send(ChannelMessage::Create((
                                        init_message.app_name.clone(),
                                        "".to_owned(),
                                        request,
                                    )))
                                    .is_err()
                                {
                                    self.disconnected("send channel message error").await?;
                                    bail!("send channel message error");
                                }
                                let session_sender = match response.await {
                                    Ok(a) => a,
                                    Err(_) => {
                                        self.disconnected("session_sender send error").await?;
                                        bail!("session_sender send error");
                                    }
                                };
                                self.state = State::Publisher(init_message.app_name, session_sender)
                            }
                            MessageInitPayloadKind::Player => {
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
                                    bail!("ChannelJoinFailed");
                                }

                                self.state = State::Player(init_message.app_name);
                                //respone join ok
                                log::info!("send proto message ok");
                                self.frame.send(ProtoMessage::new_proto_ok().into()).await?;

                                if let Ok(JoinResp::Local(session_sender, mut session_receiver)) =
                                    response.await
                                {
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
                                    log::info!("send proto init media packet finish");

                                    loop {
                                        use tokio::sync::broadcast::error::RecvError;
                                        match session_receiver.recv().await {
                                            Ok(data) => {
                                                let proto_message: ProtoMessage = data.into();
                                                self.frame.send(proto_message.into()).await?;
                                            }
                                            Err(RecvError::Closed) => {
                                                self.disconnected("session_receiver is closed")
                                                    .await?;
                                                break;
                                            }
                                            Err(_) => {}
                                        }
                                    }
                                    log::info!("session_receiver finish");
                                } else {
                                    self.disconnected("app_name not found").await?;
                                }
                            }
                        }
                    }
                    _ => bail!("first message kind mustbe init"),
                },
                State::Publisher(_, handle) => match message.kind {
                    Kind::Errors | Kind::Init | Kind::Ok => {
                        bail!("unreachable")
                    }
                    Kind::Media => {
                        let media_packet: MediaPacket = message.payload.try_into()?;
                        handle
                            .send(Message::Packet(media_packet))
                            .map_err(|_| anyhow::anyhow!("handle send message err "))?;
                    }
                },
                State::Player(_) => unreachable!(),
            }
        }

        self.disconnected("connection close").await?;
        Ok(())
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        let (app_name, role) = match &self.state {
            State::Init => return,
            State::Publisher(app_name, _) => (app_name, "publisher"),
            State::Player(app_name) => (app_name, "player"),
        };
        log::info!(
            "connecton {:?},app name({:?}),state:{:?} disconnected",
            self.id,
            app_name,
            role
        );
    }
}
