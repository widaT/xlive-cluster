use bytes::Bytes;
use serde::{Deserialize, Serialize};

use std::convert::TryFrom;

#[repr(u8)]
#[derive(Debug)]
pub enum Kind {
    Errors = 1,
    Init,
    Media,
    Ok, //join to only for palyer
}

impl TryFrom<u8> for Kind {
    type Error = anyhow::Error;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            1 => Self::Errors,
            2 => Self::Init,
            3 => Self::Media,
            4 => Self::Ok,
            _ => return Err(anyhow::anyhow!("unkown message kind")),
        })
    }
}

#[derive(Debug)]
pub struct ProtoMessage {
    pub kind: Kind,
    pub payload: Bytes,
}

impl ProtoMessage {
    pub fn new_proto_error(payload: Bytes) -> Self {
        Self {
            kind: Kind::Errors,
            payload,
        }
    }

    pub fn new_proto_init(is_publisher: bool, app_name: &str) -> Self {
        let kind = if is_publisher {
            MessageInitPayloadKind::Publisher
        } else {
            MessageInitPayloadKind::Player
        };
        let payload: Bytes = MessageInitPayload {
            kind,
            app_name: app_name.to_owned(),
        }
        .try_into()
        .unwrap();
        Self {
            kind: Kind::Init,
            payload,
        }
    }

    pub fn new_proto_ok() -> Self {
        Self {
            kind: Kind::Ok,
            payload: Bytes::from(""), //body is empty
        }
    }
}

impl TryFrom<&[u8]> for ProtoMessage {
    type Error = anyhow::Error;
    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        if value[0] < 5 && value[0] > 0 {
            Ok(Self {
                kind: Kind::try_from(value[0])?,
                payload: Bytes::from(value[1..].to_vec()),
            })
        } else {
            Err(anyhow::anyhow!("unkown message kind"))
        }
    }
}

impl TryFrom<Bytes> for ProtoMessage {
    type Error = anyhow::Error;
    fn try_from(value: Bytes) -> Result<Self, Self::Error> {
        if value[0] < 5 && value[0] > 0 {
            Ok(Self {
                kind: Kind::try_from(value[0])?,
                payload: value.slice(1..),
            })
        } else {
            Err(anyhow::anyhow!("unkown message kind"))
        }
    }
}

impl From<ProtoMessage> for Bytes {
    fn from(message: ProtoMessage) -> Self {
        let mut buf = vec![];
        buf.push(message.kind as u8);
        buf.extend(message.payload);
        buf.into()
    }
}

impl From<MediaPacket> for ProtoMessage {
    fn from(packet: MediaPacket) -> Self {
        Self {
            kind: Kind::Media,
            payload: packet.into(),
        }
    }
}

#[repr(u8)]
#[derive(Debug, Serialize, Deserialize)]
pub enum MessageInitPayloadKind {
    Publisher = 1,
    Player = 2,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MessageInitPayload {
    pub kind: MessageInitPayloadKind,
    pub app_name: String,
}

impl TryFrom<Bytes> for MessageInitPayload {
    type Error = anyhow::Error;
    fn try_from(bytes: Bytes) -> Result<Self, Self::Error> {
        let value: MessageInitPayload = bincode::deserialize(&bytes)?;
        Ok(value)
    }
}

impl TryFrom<MessageInitPayload> for Bytes {
    type Error = anyhow::Error;
    fn try_from(v: MessageInitPayload) -> Result<Self, Self::Error> {
        let buf: Vec<u8> = bincode::serialize(&v)?;
        Ok(Bytes::from(buf))
    }
}

#[repr(u8)]
#[derive(Clone, Debug)]
pub enum MediaKind {
    Metadata = 1,
    Video,
    Audio,
}

impl TryFrom<u8> for MediaKind {
    type Error = anyhow::Error;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            1 => Self::Metadata,
            2 => Self::Video,
            3 => Self::Audio,
            _ => return Err(anyhow::anyhow!("unkown media kind")),
        })
    }
}

#[derive(Clone, Debug)]
pub struct MediaPacket {
    pub kind: MediaKind,
    pub is_seq_header: bool,
    pub is_key_frame: bool,
    pub timestamp: u32,
    pub payload: Bytes,
}

impl TryFrom<Bytes> for MediaPacket {
    type Error = anyhow::Error;
    fn try_from(value: Bytes) -> Result<Self, Self::Error> {
        let first = value[0];
        let kind = (first & 0b1100_0000) >> 6;
        let seq_header = (first & 0b0010_0000) >> 5;
        let key_frame = (first & 0b0001_0000) >> 4;
        let timestamp = (value[1] as u32) << 24
            | (value[2] as u32) << 16
            | (value[3] as u32) << 8
            | value[4] as u32;
        if kind < 4 && kind > 0 {
            Ok(Self {
                kind: MediaKind::try_from(kind)?,
                is_seq_header: seq_header == 1,
                is_key_frame: key_frame == 1,
                timestamp,
                payload: value.slice(5..),
            })
        } else {
            Err(anyhow::anyhow!("unkown media packet kind {:#b}", value[0]))
        }
    }
}

impl From<MediaPacket> for Bytes {
    fn from(m: MediaPacket) -> Self {
        let mut buf = vec![];
        let seq_header = if m.is_seq_header { 1u8 } else { 0 };
        let key_frame = if m.is_key_frame { 1u8 } else { 0 };
        let first = (m.kind as u8) << 6 | seq_header << 5 | key_frame << 4;
        buf.push(first);
        buf.extend(m.timestamp.to_be_bytes());
        buf.extend(m.payload);
        buf.into()
    }
}
