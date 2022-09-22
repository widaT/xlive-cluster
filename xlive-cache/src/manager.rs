use crate::channel::Channel;
use anyhow::{bail, Result};
use bytes::Bytes;
use core::message::{Kind, MediaPacket, ProtoMessage};
use core::register::{Register, RegisterKind, RegisterResp, RegisterRespKind};
use core::transport::{
    ChannelMessage, ChannelReceiver, Handle, ManagerHandle, OutgoingBroadcast, Trigger,
};
use core::transport::{JoinResp, Responder};
use core::Upstream;
use core::{AppName, Event, Message};
use futures::{SinkExt, StreamExt};
use std::{collections::HashMap, sync::Arc};
use tokio::net::{TcpStream, UdpSocket};
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio_util::codec::{Framed, LengthDelimitedCodec};
pub struct Manager {
    handle: ManagerHandle,
    incoming: ChannelReceiver,
    channels: Arc<RwLock<HashMap<AppName, (Handle, OutgoingBroadcast)>>>,
    triggers: Arc<RwLock<HashMap<Event, Vec<Trigger>>>>,
    full_gop: bool,
    upstream: Upstream,
}

impl Manager {
    pub fn new(full_gop: bool, upstream: Upstream) -> Self {
        let (handle, incoming) = mpsc::unbounded_channel();
        let channels = Arc::new(RwLock::new(HashMap::new()));
        let triggers = Arc::new(RwLock::new(HashMap::new()));

        Self {
            handle,
            incoming,
            channels,
            triggers,
            full_gop,
            upstream,
        }
    }

    pub fn handle(&self) -> ManagerHandle {
        self.handle.clone()
    }

    async fn process_message(&mut self, message: ChannelMessage) -> Result<()> {
        match message {
            ChannelMessage::Create(_) => unreachable!(),
            ChannelMessage::Join((name, responder)) => {
                let sessions = self.channels.read().await;
                if let Some((handle, watcher)) = sessions.get(&name) {
                    if responder
                        .send(JoinResp::Local(handle.clone(), watcher.subscribe()))
                        .is_err()
                    {
                        bail!("Failed to send response");
                    }
                } else {
                    //本地没找到，去源站拉，并且作为一个 publisher，cache 缓存seq_header 和gop
                    drop(sessions);
                    let addr = match &self.upstream {
                        Upstream::Register(addr) => {
                            let buf: Bytes = Register {
                                kind: RegisterKind::Get,
                                channel_name: name.clone(),
                            }
                            .try_into()
                            .unwrap();

                            let udp = UdpSocket::bind("0.0.0.0:0").await.unwrap();
                            udp.connect(addr).await.unwrap();
                            _ = udp.send(&buf).await;
                            let mut buf = vec![0u8; 100];
                            let n = udp.recv(&mut buf).await?;
                            let register_resp: RegisterResp = (&buf[..n]).try_into()?;
                            match register_resp.kind {
                                RegisterRespKind::OK => {
                                    log::info!(
                                        "got origin add from register {:?}",
                                        register_resp.payload
                                    );
                                    register_resp.payload
                                }
                                RegisterRespKind::NOFOUND => bail!("publisher in not found"),
                            }
                        }
                        Upstream::Addr(a) => a.clone(),
                    };

                    log::info!("TcpStream::connect {}", &addr);
                    let stream = TcpStream::connect(&addr).await?;
                    let mut frame = Framed::new(stream, LengthDelimitedCodec::new());
                    frame
                        .send(ProtoMessage::new_proto_init(false, &name).into())
                        .await?;

                    if let Some(Ok(data)) = frame.next().await {
                        let proto_msg = ProtoMessage::try_from(data.freeze())?;
                        match proto_msg.kind {
                            Kind::Errors => {
                                log::error!("join chanle err");
                                bail!("join chanle err");
                            }
                            Kind::Init | Kind::Media => unreachable!(),
                            Kind::Ok => {}
                        }

                        let (handle, incoming) = mpsc::unbounded_channel();
                        let (outgoing, _watcher) = broadcast::channel(64);
                        let mut sessions = self.channels.write().await;
                        sessions.insert(name.clone(), (handle.clone(), outgoing.clone()));

                        let triggers = self.triggers.read().await;
                        if let Some(event_triggers) = triggers.get("create_session") {
                            for trigger in event_triggers {
                                trigger.send((name.clone(), outgoing.subscribe()))?;
                            }
                        }

                        let full_gop = self.full_gop;
                        let name_copy = name.clone();

                        let watcher = outgoing.subscribe();
                        let handle_cp = handle.clone();
                        tokio::spawn(async move {
                            _ = Channel::new(name_copy, incoming, outgoing, full_gop)
                                .run()
                                .await;
                        });
                        let manager_handle = self.handle.clone();
                        let name_copy = name.clone();
                        tokio::spawn(async move {
                            loop {
                                match frame.next().await {
                                    Some(Ok(data)) => {
                                        let proto_msg = ProtoMessage::try_from(data.freeze())?;
                                        match &proto_msg.kind {
                                            Kind::Errors => {
                                                _ = handle_cp.send(Message::Disconnect);
                                                _ = manager_handle
                                                    .send(ChannelMessage::Release(name_copy));
                                                log::error!("xlive cache origin connection closed");
                                                bail!("ProtoMessage chanle err");
                                            }
                                            Kind::Init | Kind::Ok => unreachable!(),
                                            Kind::Media => {
                                                let media_packet =
                                                    MediaPacket::try_from(proto_msg.payload)?;
                                                match handle_cp
                                                    .send(Message::PacketFromOrigin(media_packet))
                                                {
                                                    Ok(_) => {}
                                                    Err(_e) => {
                                                        break;
                                                    }
                                                }
                                            }
                                        };
                                    }
                                    Some(Err(e)) => {
                                        log::error!("{:?}", e);
                                        _ = handle_cp.send(Message::Disconnect);
                                        break;
                                    }
                                    None => {}
                                }
                            }
                            Ok::<(), anyhow::Error>(())
                        });

                        if responder
                            .send(JoinResp::Origin(handle.clone(), watcher))
                            .is_err()
                        {
                            bail!("Failed to send response");
                        }
                    }
                }
            }
            ChannelMessage::Release(name) => {
                let mut sessions = self.channels.write().await;
                sessions.remove(&name);
            }
            ChannelMessage::RegisterTrigger(event, trigger) => {
                log::debug!("Registering trigger for {}", event);
                let mut triggers = self.triggers.write().await;
                triggers.entry(event).or_insert_with(Vec::new).push(trigger);
            }
            ChannelMessage::Snapshot(responder) => {
                let sessions = self.channels.read().await;
                let mut info = HashMap::new();
                for (k, v) in sessions.iter() {
                    info.insert(k.to_owned(), v.1.receiver_count());
                }
                _ = responder.send(info);
            }
        }

        Ok(())
    }

    pub async fn run(mut self) {
        while let Some(message) = self.incoming.recv().await {
            if let Err(err) = self.process_message(message).await {
                log::error!("{}", err);
            };
        }
    }
}
