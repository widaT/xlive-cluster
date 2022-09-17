use std::collections::HashMap;

use crate::message::MediaPacket;
use crate::{AppName, Event, StreamKey};
use tokio::sync::{broadcast, mpsc, oneshot};

pub type Responder<P> = oneshot::Sender<P>;

pub enum JoinResp {
    Local(Handle, Watcher),
    Origin(Handle, Watcher),
}

pub enum ChannelMessage {
    Create((AppName, StreamKey, Responder<Handle>)),
    Release(AppName),
    Join((AppName, Responder<JoinResp>)),
    RegisterTrigger(Event, Trigger),
    Snapshot(Responder<HashMap<AppName, usize>>),
}

pub type ManagerHandle = mpsc::UnboundedSender<ChannelMessage>;
pub type ChannelReceiver = mpsc::UnboundedReceiver<ChannelMessage>;

pub type Trigger = mpsc::UnboundedSender<(String, Watcher)>;
pub type TriggerHandle = mpsc::UnboundedReceiver<(String, Watcher)>;

pub fn trigger_channel() -> (Trigger, TriggerHandle) {
    mpsc::unbounded_channel()
}

pub enum Message {
    Packet(MediaPacket),
    PacketFromOrigin(MediaPacket),
    InitData(
        Responder<(
            Option<MediaPacket>,
            Option<MediaPacket>,
            Option<MediaPacket>,
            Option<Vec<MediaPacket>>,
        )>,
    ),
    Disconnect,
}

pub type Handle = mpsc::UnboundedSender<Message>;
pub type IncomingBroadcast = mpsc::UnboundedReceiver<Message>;
pub type OutgoingBroadcast = broadcast::Sender<MediaPacket>;
pub type Watcher = broadcast::Receiver<MediaPacket>;
