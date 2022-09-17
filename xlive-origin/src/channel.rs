use anyhow::Result;
use bytes::Bytes;
use core::message::{MediaKind, MediaPacket};
use core::register::{Register, RegisterKind};
use core::transport::{IncomingBroadcast, Message, OutgoingBroadcast};
use std::time::{Duration, SystemTime};
use tokio::net::UdpSocket;
pub struct Channel {
    name: String,
    incoming: IncomingBroadcast,
    outgoing: OutgoingBroadcast,
    metadata: Option<MediaPacket>,
    video_seq_header: Option<MediaPacket>,
    audio_seq_header: Option<MediaPacket>,
    gop: Option<Vec<MediaPacket>>,
    closing: bool,
    full_gop: bool,
    udp_socket: Option<UdpSocket>,
}

impl Channel {
    pub fn new(
        name: String,
        incoming: IncomingBroadcast,
        outgoing: OutgoingBroadcast,
        full_gop: bool,
        udp_socket: Option<UdpSocket>,
    ) -> Self {
        Self {
            name,
            incoming,
            outgoing,
            metadata: None,
            video_seq_header: None,
            audio_seq_header: None,
            gop: None,
            closing: false,
            full_gop,
            udp_socket,
        }
    }

    pub async fn run(mut self) {
        let has_udp_client = self.udp_socket.is_some();
        let buf: Bytes = Register {
            kind: RegisterKind::Set,
            channel_name: self.name.clone(),
        }
        .try_into()
        .unwrap();

        if has_udp_client {
            _ = self.udp_socket.as_mut().unwrap().send(&buf).await;
        }
        let mut time = SystemTime::now();

        while !self.closing {
            if let Some(message) = self.incoming.recv().await {
                self.handle_message(message).await;
            }
            if time.elapsed().unwrap() > Duration::from_secs(1) {
                _ = self.udp_socket.as_mut().unwrap().send(&buf).await;
                time = SystemTime::now();
            }
        }

        //channel will be drop
        let buf: Bytes = Register {
            kind: RegisterKind::Delete,
            channel_name: self.name.clone(),
        }
        .try_into()
        .unwrap();

        if has_udp_client {
            _ = self.udp_socket.as_mut().unwrap().send(&buf).await;
        }
    }

    async fn handle_message(&mut self, message: Message) {
        match message {
            Message::Packet(packet) => {
                if let Err(e) = self.set_cache(&packet) {
                    log::error!("Failed to set channel cache {}", e);
                }
                self.broadcast_packet(packet);
            }
            Message::InitData(responder) => {
                let response = (
                    self.metadata.clone(),
                    self.video_seq_header.clone(),
                    self.audio_seq_header.clone(),
                    self.gop.clone(),
                );
                if responder.send(response).is_err() {
                    log::error!("Failed to send init data");
                }
            }
            Message::PacketFromOrigin(_) => unreachable!(),
            Message::Disconnect => {
                self.closing = true;
            }
        }
    }

    fn broadcast_packet(&self, packet: MediaPacket) {
        if self.outgoing.receiver_count() != 0 && self.outgoing.send(packet).is_err() {
            log::error!("Failed to broadcast packet");
        }
    }

    fn set_cache(&mut self, packet: &MediaPacket) -> Result<()> {
        match packet.kind {
            MediaKind::Metadata => {
                self.metadata = Some(packet.clone());
            }
            MediaKind::Video => {
                if packet.is_seq_header && packet.is_key_frame {
                    self.video_seq_header = Some(packet.clone());
                } else if !packet.is_seq_header && packet.is_key_frame {
                    let pck = vec![packet.clone()];
                    self.gop = Some(pck);
                } else if self.full_gop {
                    if let Some(ref mut v) = self.gop {
                        v.push(packet.clone());
                    }
                }
            }
            MediaKind::Audio => {
                if packet.is_seq_header {
                    self.audio_seq_header = Some(packet.clone());
                }
            }
        }
        Ok(())
    }
}

impl Drop for Channel {
    fn drop(&mut self) {
        log::info!("channel {} closed", self.name);
    }
}
