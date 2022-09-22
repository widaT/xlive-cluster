use anyhow::{bail, Result};
use core::message::{MediaKind, MediaPacket, ProtoMessage};
use core::transport::{IncomingBroadcast, Message, OutgoingBroadcast};
use futures::SinkExt;
use tokio::net::TcpStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
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
    frame: Option<Framed<TcpStream, LengthDelimitedCodec>>,
}

impl Channel {
    pub fn new(
        name: String,
        incoming: IncomingBroadcast,
        outgoing: OutgoingBroadcast,
        full_gop: bool,
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
            frame: None,
        }
    }

    pub async fn run(mut self, from_cluster: bool, origin_addr: String) -> Result<()> {
        if !from_cluster {
            let stream = TcpStream::connect(origin_addr).await?;
            self.frame = Some(Framed::new(stream, LengthDelimitedCodec::new()));
            self.frame
                .as_mut()
                .unwrap()
                .send(ProtoMessage::new_proto_init(true, &self.name).into())
                .await?;
        }

        while !self.closing {
            if let Some(message) = self.incoming.recv().await {
                self.handle_message(message).await?;
            }
        }
        Ok(())
    }

    async fn handle_message(&mut self, message: Message) -> Result<()> {
        match message {
            Message::Packet(packet) => {
                if let Err(e) = self.set_cache(&packet) {
                    bail!("Failed to set channel cache {}", e);
                }

                let media_packet = packet.clone();
                match self
                    .frame
                    .as_mut()
                    .unwrap()
                    .send(ProtoMessage::from(media_packet).into())
                    .await
                {
                    Ok(_) => {}
                    Err(_) => {
                        bail!("send proto message to cluster err");
                    }
                }
                self.broadcast_packet(packet)?;
            }

            Message::PacketFromOrigin(packet) => {
                self.set_cache(&packet)?;
                self.broadcast_packet(packet)?;
            }

            Message::InitData(responder) => {
                let response = (
                    self.metadata.clone(),
                    self.video_seq_header.clone(),
                    self.audio_seq_header.clone(),
                    self.gop.clone(),
                );
                if responder.send(response).is_err() {
                    bail!("Failed to send init datar");
                }
            }
            Message::Disconnect => {
                self.closing = true;
            }
        }
        Ok(())
    }

    fn broadcast_packet(&self, packet: MediaPacket) -> Result<()> {
        if self.outgoing.receiver_count() != 0 && self.outgoing.send(packet).is_err() {
            bail!("Failed to broadcast packet");
        }
        Ok(())
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
