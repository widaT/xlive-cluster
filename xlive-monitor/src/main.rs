use anyhow::Result;
use chrono::Local;
use xlive_monitor::monitor::Monitor;
use std::io::Write;
use tokio::sync::mpsc::unbounded_channel;
use xlive_monitor::IncomingMessage;
use xlive_monitor::{http_service::Service, splider::Task};
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

    let (sender, recivicer) = unbounded_channel::<IncomingMessage>();

    // let sender_cp = sender.clone();

    let mut handles =vec![];

    //@todo read ips form config file
    let ips =vec![("origin","127.0.0.1:3302")];

    for (name,ip) in ips {
        let sender_cp = sender.clone();
        handles.push(tokio::spawn(async move {
            Task::new(name, ip, sender_cp).run().await?;
            Ok::<(),anyhow::Error>(())
        }));
    }
   

    handles.push(tokio::spawn(async move {
        Monitor::new(recivicer).run().await?;
        Ok::<(),anyhow::Error>(())
    }));

    let sender_cp = sender.clone();

    handles.push(tokio::spawn(async move {
            Service::new(sender_cp).run().await?;
            Ok::<(),anyhow::Error>(())
    }));

    for h in handles {
        if let Err(e) = h.await {
            log::error!("{:?}",e);
        }
    }
    Ok(())
}
