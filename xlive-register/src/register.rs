use anyhow::Result;
use bytes::Bytes;
use core::register::{Register, RegisterKind, RegisterResp, RegisterRespKind};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{collections::HashMap, net::SocketAddr};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::oneshot;

pub enum OneshotMsg {
    GetServers(Vec<String>),
    GetAppsMap(HashMap<String, (SocketAddr, u64)>),
}
pub enum OneshotMsgKind {
    GetServers,
    GetAppsMap,
}
pub enum IncomingMessage {
    Register(Register, SocketAddr, Option<oneshot::Sender<Bytes>>),
    Oneshot(OneshotMsgKind, oneshot::Sender<OneshotMsg>),
}

pub struct Server {
    pub incoming: UnboundedReceiver<IncomingMessage>,
    pub channel_map: HashMap<String, (SocketAddr, u64)>,
    pub servers: HashMap<SocketAddr, u64>,
}

impl Server {
    pub async fn run(self) -> Result<()> {
        let Server {
            mut incoming,
            mut channel_map,
            mut servers,
        } = self;

        loop {
            while let Some(data) = incoming.recv().await {
                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                match data {
                    IncomingMessage::Register(msg, addr, outgoing) => {
                        servers.insert(addr, timestamp);
                        match msg.kind {
                            RegisterKind::Set => {
                                channel_map.insert(msg.channel_name, (addr, timestamp));
                            }
                            RegisterKind::Get => {
                                let mut resp = RegisterResp {
                                    kind: RegisterRespKind::NOFOUND,
                                    payload: "".to_owned(),
                                };
                                if let Some((ref socket_addr, ref last_timestamp)) =
                                    channel_map.get(&msg.channel_name)
                                {
                                    log::info!(
                                        "get {} found socket_addr:{} last_timestamp:{}",
                                        msg.channel_name,
                                        socket_addr,
                                        last_timestamp
                                    );
                                    if timestamp - last_timestamp < 10 {
                                        resp = RegisterResp {
                                            kind: RegisterRespKind::OK,
                                            payload: socket_addr.to_string(),
                                        };
                                    } else {
                                        channel_map.remove(&msg.channel_name);
                                        log::info!(
                                            "remove {} alter 10 sec no update",
                                            msg.channel_name,
                                        );
                                    }
                                }
                                let buf: Bytes = resp.try_into().unwrap();
                                if outgoing.is_some() {
                                    _ = outgoing.unwrap().send(buf);
                                }
                            }
                            RegisterKind::Delete => {
                                channel_map.remove(&msg.channel_name);
                            }
                        }
                    }
                    IncomingMessage::Oneshot(kind, sender) => match kind {
                        OneshotMsgKind::GetServers => {
                            let servers_list: Vec<String> = servers
                                .iter()
                                .filter(|(_k, &v)| timestamp - v < 10)
                                .map(|(k, _v)| k.to_string())
                                .collect();

                            _ = sender.send(OneshotMsg::GetServers(servers_list));
                        }
                        OneshotMsgKind::GetAppsMap => {
                            let new_map = channel_map.clone();
                            _ = sender.send(OneshotMsg::GetAppsMap(new_map));
                        }
                    },
                }
            }
        }
    }
}
