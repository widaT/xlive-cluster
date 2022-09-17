use crate::channel::Channel;
use anyhow::{bail, Result};
use core::transport::JoinResp;
use core::transport::{
    ChannelMessage, ChannelReceiver, Handle, ManagerHandle, OutgoingBroadcast, Trigger,
};
use core::{AppName, Event};
use std::{collections::HashMap, sync::Arc};
use tokio::net::UdpSocket;
use tokio::sync::{broadcast, mpsc, RwLock};
pub struct Manager {
    handle: ManagerHandle,
    incoming: ChannelReceiver,
    channels: Arc<RwLock<HashMap<AppName, (Handle, OutgoingBroadcast)>>>,
    triggers: Arc<RwLock<HashMap<Event, Vec<Trigger>>>>,
    full_gop: bool,
    register_addr: Option<String>,
}

impl Manager {
    pub fn new(full_gop: bool, register_addr: Option<String>) -> Self {
        let (handle, incoming) = mpsc::unbounded_channel();
        let channels = Arc::new(RwLock::new(HashMap::new()));
        let triggers = Arc::new(RwLock::new(HashMap::new()));

        Self {
            handle,
            incoming,
            channels,
            triggers,
            full_gop,
            register_addr,
        }
    }

    pub fn handle(&self) -> ManagerHandle {
        self.handle.clone()
    }

    async fn process_message(&mut self, message: ChannelMessage) -> Result<()> {
        match message {
            ChannelMessage::Create((name, _, responder)) => {
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

                let mut udp_socket: Option<UdpSocket> = None;
                if self.register_addr.is_some() {
                    let udp = UdpSocket::bind("0.0.0.0:9878").await.unwrap();
                    udp.connect(self.register_addr.as_ref().unwrap())
                        .await
                        .unwrap();
                    udp_socket = Some(udp);
                }

                tokio::spawn(async move {
                    Channel::new(name_copy, incoming, outgoing, full_gop, udp_socket)
                        .run()
                        .await;
                });

                if responder.send(handle).is_err() {
                    bail!("Failed to send response");
                }
            }
            ChannelMessage::Join((name, responder)) => {
                let sessions = self.channels.read().await;
                if let Some((handle, watcher)) = sessions.get(&name) {
                    if responder
                        .send(JoinResp::Local(handle.clone(), watcher.subscribe()))
                        .is_err()
                    {
                        bail!("Failed to send response");
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
