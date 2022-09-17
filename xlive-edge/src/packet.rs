use anyhow::Result;
use bytes::Bytes;
use core::message::{MediaKind, MediaPacket};
use rml_rtmp::sessions::StreamMetadata;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::str::FromStr;

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub struct Timestamp {
    value: u64,
}

impl From<u32> for Timestamp {
    fn from(val: u32) -> Self {
        Self { value: val.into() }
    }
}

impl From<Timestamp> for u32 {
    fn from(val: Timestamp) -> Self {
        val.value as u32
    }
}

impl From<u64> for Timestamp {
    fn from(val: u64) -> Self {
        Self { value: val }
    }
}

impl From<Timestamp> for u64 {
    fn from(val: Timestamp) -> Self {
        val.value
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PacketType {
    Meta,
    Video,
    Audio,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Packet {
    pub kind: PacketType,
    pub timestamp: Option<Timestamp>,
    pub payload: Bytes,
}

impl Packet {
    pub fn new<T, B>(kind: PacketType, timestamp: Option<T>, payload: B) -> Self
    where
        T: Into<Timestamp>,
        B: Into<Bytes>,
    {
        let timestamp = timestamp.map(|v| v.into());
        Self {
            kind,
            timestamp,
            payload: payload.into(),
        }
    }

    pub fn new_video<T, B>(timestamp: T, payload: B) -> Self
    where
        T: Into<Timestamp>,
        B: Into<Bytes>,
    {
        Self::new(PacketType::Video, Some(timestamp), payload)
    }

    pub fn new_audio<T, B>(timestamp: T, payload: B) -> Self
    where
        T: Into<Timestamp>,
        B: Into<Bytes>,
    {
        Self::new(PacketType::Audio, Some(timestamp), payload)
    }

    pub fn pack(&self) -> Result<Bytes> {
        let data = bincode::serialize(&self)?;
        Ok(Bytes::from(data))
    }

    pub fn unpack(bytes: &[u8]) -> Result<Self> {
        Ok(bincode::deserialize(bytes)?)
    }
}

impl From<Packet> for MediaPacket {
    fn from(value: Packet) -> Self {
        let timestamp = match value.timestamp {
            Some(a) => a.into(),
            None => 0u32,
        };

        let mut is_seq_header = false;
        let mut is_key_frame = false;
        let kind = match value.kind {
            PacketType::Meta => MediaKind::Metadata,
            PacketType::Video => {
                is_seq_header = is_video_sequence_header(&value.payload);
                is_key_frame = is_video_keyframe(&value.payload);
                MediaKind::Video
            }
            PacketType::Audio => {
                is_seq_header = is_audio_sequence_header(&value.payload);
                MediaKind::Audio
            }
        };

        MediaPacket {
            kind,
            is_seq_header,
            is_key_frame,
            timestamp,
            payload: value.payload,
        }
    }
}

impl From<MediaPacket> for Packet {
    fn from(value: MediaPacket) -> Self {
        let timestamp = Some(Timestamp::from(value.timestamp));

        let kind = match value.kind {
            MediaKind::Metadata => PacketType::Meta,
            MediaKind::Video => PacketType::Video,
            MediaKind::Audio => PacketType::Audio,
        };

        Self {
            kind,
            timestamp,
            payload: value.payload,
        }
    }
}

impl AsRef<[u8]> for Packet {
    fn as_ref(&self) -> &[u8] {
        &self.payload
    }
}

impl TryFrom<Packet> for Bytes {
    type Error = anyhow::Error;

    fn try_from(val: Packet) -> Result<Self, Self::Error> {
        val.pack()
    }
}

impl TryFrom<&[u8]> for Packet {
    type Error = anyhow::Error;

    fn try_from(val: &[u8]) -> Result<Self, Self::Error> {
        Packet::unpack(&val)
    }
}

type StringMap = HashMap<String, String>;
type StrMap<'a> = HashMap<&'a str, String>;

#[derive(Clone, Serialize, Deserialize)]
pub struct Metadata(StringMap);

impl Metadata {
    pub fn get<V, K>(&self, key: K) -> Option<V>
    where
        K: AsRef<str>,
        V: FromStr,
    {
        self.0.get(key.as_ref()).map(|v| v.parse().ok()).flatten()
    }
}

impl From<StringMap> for Metadata {
    fn from(val: HashMap<String, String>) -> Self {
        Self(val)
    }
}

impl<'a> From<StrMap<'a>> for Metadata {
    fn from(val: StrMap<'a>) -> Self {
        let new_map = val
            .into_iter()
            .fold(StringMap::new(), |mut acc, (key, value)| {
                acc.insert(key.to_owned(), value);
                acc
            });
        Self::from(new_map)
    }
}

impl TryFrom<Metadata> for Bytes {
    type Error = anyhow::Error;

    fn try_from(val: Metadata) -> Result<Self, Self::Error> {
        let data = bincode::serialize(&val)?;
        Ok(Bytes::from(data))
    }
}

impl TryFrom<&[u8]> for Metadata {
    type Error = anyhow::Error;

    fn try_from(val: &[u8]) -> Result<Self, Self::Error> {
        Ok(bincode::deserialize(val)?)
    }
}

impl TryFrom<Metadata> for Packet {
    type Error = anyhow::Error;

    fn try_from(val: Metadata) -> Result<Self, Self::Error> {
        Ok(Self {
            kind: PacketType::Meta,
            timestamp: None,
            payload: Bytes::try_from(val)?,
        })
    }
}

impl TryFrom<Packet> for Metadata {
    type Error = anyhow::Error;

    fn try_from(val: Packet) -> Result<Self, Self::Error> {
        let payload = &*val.payload;
        Self::try_from(payload)
    }
}

pub fn from_metadata(val: StreamMetadata) -> Metadata {
    let mut map = HashMap::with_capacity(11);
    if let Some(v) = val.audio_bitrate_kbps {
        map.insert("audio.bitrate", v.to_string());
    }

    if let Some(v) = val.audio_channels {
        map.insert("audio.channels", v.to_string());
    }

    if let Some(v) = val.audio_codec {
        map.insert("audio.codec", v);
    }

    if let Some(v) = val.audio_is_stereo {
        map.insert("audio.stereo", v.to_string());
    }

    if let Some(v) = val.audio_sample_rate {
        map.insert("audio.sampling_rate", v.to_string());
    }

    if let Some(v) = val.video_bitrate_kbps {
        map.insert("video.bitrate", v.to_string());
    }

    if let Some(v) = val.video_codec {
        map.insert("video.codec", v);
    }

    if let Some(v) = val.video_frame_rate {
        map.insert("video.frame_rate", v.to_string());
    }

    if let Some(v) = val.video_height {
        map.insert("video.height", v.to_string());
    }

    if let Some(v) = val.video_width {
        map.insert("video.width", v.to_string());
    }

    if let Some(v) = val.encoder {
        map.insert("encoder", v);
    }

    Metadata::from(map)
}

pub(crate) fn into_metadata(val: Metadata) -> StreamMetadata {
    StreamMetadata {
        video_width: val.get("video.width"),
        video_height: val.get("video.height"),
        video_codec: val.get("video.codec"),
        video_frame_rate: val.get("video.frame_rate"),
        video_bitrate_kbps: val.get("video.bitrate"),
        audio_codec: val.get("audio.codec"),
        audio_bitrate_kbps: val.get("audio.bitrate"),
        audio_sample_rate: val.get("audio.sampling_rate"),
        audio_channels: val.get("audio.channels"),
        audio_is_stereo: val.get("audio.stereo"),
        encoder: val.get("encoder"),
    }
}

fn is_video_sequence_header(data: &Bytes) -> bool {
    // This is assuming h264 h265
    return data.len() >= 2 && data[1] == 0x00;
}

fn is_audio_sequence_header(data: &Bytes) -> bool {
    // This is assuming aac
    return data.len() >= 2 && data[0] == 0xaf && data[1] == 0x00;
}

fn is_video_keyframe(data: &Bytes) -> bool {
    // assumings h264 h265
    return data.len() >= 2 && (data[0] == 0x17 || data[0] == 0x1c);
}
