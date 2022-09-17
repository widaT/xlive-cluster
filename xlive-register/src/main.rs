use anyhow::Result;
use chrono::Local;
use core::register::Register;
use core::register::RegisterKind;
use std::{collections::HashMap, io::Write};
use tokio::{
    net::UdpSocket,
    sync::{mpsc::unbounded_channel, oneshot},
};
use xlive_register::http_service::Service;
use xlive_register::register::{self, IncomingMessage};
#[tokio::main]
async fn main() -> Result<()> {
    let env = env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info");
    env_logger::Builder::from_env(env)
        .format(|buf, record| {
            writeln!(
                buf,
                "{} {} [{}] {}",
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.module_path().unwrap_or("<unnamed>"),
                &record.args()
            )
        })
        .init();

    let (sender, revciver) = unbounded_channel();

    let mut handles = vec![];

    handles.push(tokio::spawn(async move {
        register::Server {
            incoming: revciver,
            channel_map: HashMap::new(),
            servers: HashMap::new(),
        }
        .run()
        .await
    }));

    let sender_cp = sender.clone();
    handles.push(tokio::spawn(async move {
        let socket = UdpSocket::bind("0.0.0.0:9336").await?;
        println!("Listening on: {}", socket.local_addr()?);
        let mut buf = vec![0u8; 1000];
        while let Ok((size, addr)) = socket.recv_from(&mut buf).await {
            let buffer = &buf[..size];
            if let Ok(msg) = Register::try_from(buffer) {
                match msg.kind {
                    RegisterKind::Set | RegisterKind::Delete => {
                        _ = sender.send(IncomingMessage::Register(msg, addr, None));
                    }
                    RegisterKind::Get => {
                        let (s, r) = oneshot::channel();
                        _ = sender.send(IncomingMessage::Register(msg, addr, Some(s)));
                        if let Ok(buf) = r.await {
                            socket.send_to(&buf, &addr).await?;
                        }
                    }
                }
            }
        }
        Ok::<(), anyhow::Error>(())
    }));

    handles.push(tokio::spawn(async move {
        Service::new(sender_cp).run().await;
        Ok::<(), anyhow::Error>(())
    }));

    for handle in handles {
        if let Err(e) = handle.await {
            log::error!("{}", e);
        }
    }
    Ok(())
}
