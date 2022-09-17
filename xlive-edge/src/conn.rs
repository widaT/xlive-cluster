use crate::packet::{Packet, PacketType};
use crate::rtmp::{Event, Protocol};
use anyhow::Result;
use core::message::MediaPacket;
use core::transport::JoinResp;
use core::{ChannelMessage, Handle, ManagerHandle, Message, Watcher};
use futures::SinkExt;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::{
    sync::{mpsc, oneshot},
    time::timeout,
};
use tokio_stream::StreamExt;
use tokio_util::codec::{BytesCodec, Framed};

type ReturnQueue<P> = (mpsc::UnboundedSender<P>, mpsc::UnboundedReceiver<P>);
const TIME_OUT: std::time::Duration = Duration::from_secs(5);

type NeedInitData = bool; // first connection to orgin get init data from origin,but others from local

enum State {
    Initializing,
    Publishing(Handle),
    Playing(Handle, Watcher, NeedInitData),
    Disconnecting,
}

pub struct Connection {
    id: u64,
    bytes_stream: Framed<TcpStream, BytesCodec>,
    manager_handle: ManagerHandle,
    return_queue: ReturnQueue<Packet>,
    proto: Protocol,
    app_name: Option<String>,
    state: State,
}

impl Connection {
    pub fn new(id: u64, stream: TcpStream, manager_handle: ManagerHandle) -> Self {
        Self {
            id,
            bytes_stream: Framed::new(stream, BytesCodec::new()),
            manager_handle,
            return_queue: mpsc::unbounded_channel(),
            proto: Protocol::new(),
            app_name: None,
            state: State::Initializing,
        }
    }

    pub async fn run(mut self) -> Result<()> {
        loop {
            while let Ok(packet) = self.return_queue.1.try_recv() {
                if self.handle_return_packet(packet).await.is_err() {
                    self.disconnect()?
                }
            }

            match &mut self.state {
                State::Initializing | State::Publishing(_) => {
                    let val = self.bytes_stream.try_next();
                    match timeout(TIME_OUT, val).await? {
                        Ok(Some(data)) => {
                            for event in self.proto.handle_bytes(&data)? {
                                self.handle_event(event).await?;
                            }
                        }
                        _ => self.disconnect()?,
                    }
                }
                State::Playing(_, watcher, _) => {
                    use tokio::sync::broadcast::error::RecvError;
                    match watcher.recv().await {
                        Ok(packet) => self.send_back(packet)?,
                        Err(RecvError::Closed) => self.disconnect()?,
                        Err(_) => (),
                    }
                }
                State::Disconnecting => {
                    log::debug!("Disconnecting...");
                    return Ok(());
                }
            }
        }
    }

    async fn handle_return_packet(&mut self, packet: Packet) -> Result<()> {
        let bytes = match packet.kind {
            PacketType::Meta => self.proto.pack_metadata(packet)?,
            PacketType::Video => self.proto.pack_video(packet)?,
            PacketType::Audio => self.proto.pack_audio(packet)?,
        };
        let res = timeout(TIME_OUT, self.bytes_stream.send(bytes.into())).await?;
        Ok(res?)
    }

    async fn handle_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::ReturnData(data) => {
                self.bytes_stream
                    .send(data)
                    .await
                    .expect("Failed to return data");
            }
            Event::SendPacket(packet) => {
                if let State::Publishing(session) = &mut self.state {
                    session
                        .send(Message::Packet(packet.into()))
                        .map_err(|_| anyhow::anyhow!("ChannelSendFailed"))?;
                }
            }
            Event::AcquireChannel {
                app_name,
                stream_key,
            } => {
                self.app_name = Some(app_name.clone());
                let (request, response) = oneshot::channel();
                self.manager_handle
                    .send(ChannelMessage::Create((app_name, stream_key, request)))
                    .map_err(|_| anyhow::anyhow!("ChannelCreationFailed"))?;
                let session_sender = response
                    .await
                    .map_err(|_| anyhow::anyhow!("ChannelCreationFailed"))?;
                self.state = State::Publishing(session_sender);
            }
            Event::JoinChannel { app_name, .. } => {
                let (request, response) = oneshot::channel();
                self.manager_handle
                    .send(ChannelMessage::Join((app_name, request)))
                    .map_err(|_| anyhow::anyhow!("ChannelJoinFailed"))?;

                match response.await {
                    Ok(resp) => match resp {
                        JoinResp::Local(session_sender, session_receiver) => {
                            self.state = State::Playing(session_sender, session_receiver, false);
                        }
                        JoinResp::Origin(session_sender, session_receiver) => {
                            self.state = State::Playing(session_sender, session_receiver, true);
                        }
                    },
                    Err(_) => self.disconnect()?,
                }
            }
            Event::SendInitData { .. } => {
                if let State::Playing(session, _, from_cluster_fisrt) = &mut self.state {
                    if *from_cluster_fisrt {
                        return Ok(()); //第一个请求
                    }
                    let (request, response) = oneshot::channel();
                    session
                        .send(Message::InitData(request))
                        .map_err(|_| anyhow::anyhow!("ChannelSendFailed "))?;
                    //这边可能出现一致性错误,可能掉帧
                    if let Ok((meta, video, audio, gop)) = response.await {
                        meta.map(|m| self.send_back(m));
                        video.map(|v| self.send_back(v));
                        audio.map(|a| self.send_back(a));
                        gop.map(|gop| {
                            for g in gop {
                                match self.send_back(g) {
                                    Ok(_) => {}
                                    Err(e) => {
                                        log::error!("{}", e);
                                        _ = self.disconnect();
                                    }
                                }
                            }
                        });
                    }
                }
            }
            Event::ReleaseChannel | Event::LeaveChannel => self.disconnect()?,
        }
        Ok(())
    }

    fn send_back(&mut self, packet: MediaPacket) -> Result<()> {
        self.return_queue
            .0
            .send(packet.into())
            .map_err(|_| anyhow::anyhow!("ReturnPacketFailed"))
    }

    fn disconnect(&mut self) -> Result<()> {
        if let State::Publishing(session) = &mut self.state {
            let app_name = self.app_name.clone().unwrap();
            session
                .send(Message::Disconnect)
                .map_err(|_| anyhow::anyhow!("ChannelSendFailed"))?;

            self.manager_handle
                .send(ChannelMessage::Release(app_name))
                .map_err(|_| anyhow::anyhow!("ChannelReleaseFailed"))?;
        }
        self.state = State::Disconnecting;
        Ok(())
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        log::info!("Client {} disconnected", self.id);
    }
}
